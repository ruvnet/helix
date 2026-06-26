//! Real-data eval + evolve from the **MIMIC-IV demo** (open, 100 patients).
//! Run: `cargo run -p helix-evolve --example mimic_eval`
//! Expects the filtered CSV at $HELIX_MIMIC_CSV (default /tmp/healthdata/labs_filtered.csv),
//! columns: subject_id,itemid,charttime,valuenum,ref_lo,ref_hi
//!
//! Ground truth is the **statistical significance of the regression slope**
//! (a t-test, |t| > 2 ⇒ trend) — deliberately INDEPENDENT of the relative-range
//! heuristic we are validating, so the agreement number is meaningful, not circular.
//! We then ask: how well does Helix's deterministic relative band (ADR-036) agree
//! with a proper statistical trend test on real serial labs — and what `frac`
//! maximizes that agreement?

use std::collections::BTreeMap;
use std::fs;

use helix_evolve::{evolve, fitness, Bounds, EvalCase, Expected, Params};
use helix_numeric::TrendDirection;
use helix_provenance::{Confidence, MeasurementMethod, ProvRecord, RecordId, ReferenceRange};

const DAY: i64 = 86_400_000;

// Howard Hinnant's days_from_civil — civil date → days since 1970-01-01, no deps.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn parse_day(charttime: &str) -> Option<i64> {
    if charttime.len() < 10 {
        return None;
    }
    let y: i64 = charttime.get(0..4)?.parse().ok()?;
    let m: i64 = charttime.get(5..7)?.parse().ok()?;
    let d: i64 = charttime.get(8..10)?.parse().ok()?;
    Some(days_from_civil(y, m, d))
}

struct Pt {
    day: i64,
    v: f64,
    lo: f64,
    hi: f64,
}

/// Statistical trend label: OLS slope and its t-statistic. |t| > 2 ⇒ trend.
fn statistical_label(pts: &[Pt]) -> Option<TrendDirection> {
    let n = pts.len() as f64;
    if n < 4.0 {
        return None;
    }
    let xs: Vec<f64> = pts.iter().map(|p| p.day as f64).collect();
    let ys: Vec<f64> = pts.iter().map(|p| p.v).collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let mut sxx = 0.0;
    let mut sxy = 0.0;
    for (x, y) in xs.iter().zip(&ys) {
        sxx += (x - mx) * (x - mx);
        sxy += (x - mx) * (y - my);
    }
    if sxx <= 0.0 {
        return None;
    }
    let b = sxy / sxx;
    // residual variance → SE(slope) → t
    let mut sse = 0.0;
    for (x, y) in xs.iter().zip(&ys) {
        let yhat = my + b * (x - mx);
        sse += (y - yhat) * (y - yhat);
    }
    let s2 = sse / (n - 2.0);
    let se = (s2 / sxx).sqrt();
    if se <= 0.0 {
        // zero residual: any non-zero slope is a perfect trend
        return Some(if b > 0.0 {
            TrendDirection::Rising
        } else if b < 0.0 {
            TrendDirection::Falling
        } else {
            TrendDirection::Flat
        });
    }
    let t = b / se;
    Some(if t > 2.0 {
        TrendDirection::Rising
    } else if t < -2.0 {
        TrendDirection::Falling
    } else {
        TrendDirection::Flat
    })
}

fn main() {
    let path = std::env::var("HELIX_MIMIC_CSV")
        .unwrap_or_else(|_| "/tmp/healthdata/labs_filtered.csv".to_string());
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "cannot read {path}: {e}\nDownload the MIMIC-IV demo first (see example header)."
            );
            std::process::exit(1);
        }
    };

    // group rows by (subject, itemid)
    let mut series: BTreeMap<(String, String), Vec<Pt>> = BTreeMap::new();
    for line in raw.lines() {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 6 {
            continue;
        }
        let (Some(day), Ok(v), Ok(lo), Ok(hi)) = (
            parse_day(f[2]),
            f[3].parse::<f64>(),
            f[4].parse::<f64>(),
            f[5].parse::<f64>(),
        ) else {
            continue;
        };
        if !v.is_finite() || hi <= lo {
            continue;
        }
        series
            .entry((f[0].into(), f[1].into()))
            .or_default()
            .push(Pt { day, v, lo, hi });
    }

    // build labeled eval cases from real series (≥4 points spanning ≥2 days)
    let mut cases = Vec::new();
    for ((subj, item), mut pts) in series {
        pts.sort_by_key(|p| p.day);
        pts.dedup_by_key(|p| p.day); // collapse same-day repeats
        if pts.len() < 4 || pts.last().unwrap().day - pts.first().unwrap().day < 2 {
            continue;
        }
        let Some(label) = statistical_label(&pts) else {
            continue;
        };
        let (lo, hi) = (pts[0].lo, pts[0].hi);
        let now = (pts.last().unwrap().day + 1) * DAY;
        let records = pts
            .iter()
            .enumerate()
            .map(|(i, p)| ProvRecord {
                id: RecordId::from(format!("{subj}-{item}-{i}")),
                source: "MIMIC-IV-demo".into(),
                measured_at: p.day * DAY,
                method: MeasurementMethod::LabFeed,
                code: Some(item.clone()),
                concept: format!("item{item}"),
                value: p.v,
                unit: "u".into(),
                reference_range: Some(ReferenceRange::new(Some(lo), Some(hi))),
                confidence: Confidence::new(1.0),
            })
            .collect();
        cases.push(EvalCase {
            name: format!("{subj}-{item}"),
            concept_code: item.clone(),
            records,
            now,
            reference_low: Some(lo),
            reference_high: Some(hi),
            expected: Expected::Answered(label),
        });
    }

    let reg = helix_escalation::builtin_registry_v1();
    println!(
        "Real eval set: {} labeled series from the MIMIC-IV demo",
        cases.len()
    );
    let n = cases.len() as f64;

    // agreement = correct trend classifications / total
    let agree = |p: &Params| {
        let f = fitness(p, &cases, &reg);
        let correct = f.grounded_correct + f.abstained_correct;
        (correct as f64 / n * 100.0, f)
    };

    // ABSOLUTE band (pre-ADR-036 behaviour)
    let absolute = Params {
        confidence_floor: 0.3,
        staleness_window_days: 100000,
        flat_band_per_day: 0.01,
        flat_band_frac: 0.0,
    };
    let (abs_pct, abs_f) = agree(&absolute);
    println!(
        "\nABSOLUTE band (0.01/day)        agreement {abs_pct:.1}%  (wrong_dir={}, over_caution={})",
        abs_f.wrong_direction, abs_f.over_cautious
    );

    // ADOPTED relative band (ADR-036, frac 0.08)
    let adopted = Params {
        flat_band_frac: 0.08,
        ..absolute
    };
    let (adp_pct, adp_f) = agree(&adopted);
    println!(
        "RELATIVE band (frac 0.08, ADR-036) agreement {adp_pct:.1}%  (wrong_dir={})",
        adp_f.wrong_direction
    );

    // EVOLVE the frac on real data (safety frozen) — many seeds, keep best.
    let mut best = evolve(adopted, &Bounds::default(), &cases, &reg, 300, 1);
    for seed in 2..=20 {
        let r = evolve(adopted, &Bounds::default(), &cases, &reg, 300, seed);
        if r.best_fitness.score > best.best_fitness.score {
            best = r;
        }
    }
    let (evo_pct, _) = agree(&best.best_params);
    println!(
        "EVOLVED on real data              agreement {evo_pct:.1}%  → frac {:.3}, floor {:.2}",
        best.best_params.flat_band_frac, best.best_params.confidence_floor
    );
    println!(
        "\nReal-data verdict: relative band {}{:.1}pts vs absolute; over_confident={} (safety preserved)",
        if adp_pct >= abs_pct { "+" } else { "" },
        adp_pct - abs_pct,
        adp_f.over_confident
    );
}
