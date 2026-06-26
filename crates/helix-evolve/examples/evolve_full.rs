//! Complete eval + Darwin evolve from Helix's REAL shipping defaults, over a
//! diverse, realistically-labeled multi-marker eval set.
//! Run: `cargo run -p helix-evolve --example evolve_full`
//!
//! Ground truth is labeled by what a clinician would say about each series —
//! independent of the parameters. We then ask: do the shipping defaults get them
//! all right, and can evolution (safety frozen) do better?

use helix_evolve::{evolve, fitness, Bounds, EvalCase, Expected, Params};
use helix_numeric::TrendDirection::{self, Falling, Flat, Rising};
use helix_pipeline::{analyze, AnalyzeRequest, AnswerOutcome};
use helix_provenance::{Confidence, MeasurementMethod, ProvRecord, RecordId, ReferenceRange};

const DAY: i64 = 86_400_000;
const NOW: i64 = 1000 * DAY;

#[allow(clippy::too_many_arguments)]
fn mk(
    name: &str,
    code: &str,
    concept: &str,
    lo: f64,
    hi: f64,
    pts: &[(i64, f64, f64)], // (days_ago, value, confidence)
    expected: Expected,
) -> EvalCase {
    let records = pts
        .iter()
        .enumerate()
        .map(|(i, &(d, v, c))| ProvRecord {
            id: RecordId::from(format!("{name}-{i}")),
            source: "Quest".into(),
            measured_at: NOW - d * DAY,
            method: MeasurementMethod::LabFeed,
            code: Some(code.into()),
            concept: concept.into(),
            value: v,
            unit: "u".into(),
            reference_range: Some(ReferenceRange::new(Some(lo), Some(hi))),
            confidence: Confidence::new(c),
        })
        .collect();
    EvalCase {
        name: name.into(),
        concept_code: code.into(),
        records,
        now: NOW,
        reference_low: Some(lo),
        reference_high: Some(hi),
        expected,
    }
}

fn ans(d: TrendDirection) -> Expected {
    Expected::Answered(d)
}

fn cases() -> Vec<EvalCase> {
    vec![
        // --- Ferritin (ng/mL, 30–400): large scale ---
        mk(
            "ferritin-decline",
            "2276-4",
            "Ferritin",
            30.0,
            400.0,
            &[(90, 62.0, 1.0), (45, 44.0, 1.0), (0, 27.0, 1.0)],
            ans(Falling),
        ),
        mk(
            "ferritin-recovery",
            "2276-4",
            "Ferritin",
            30.0,
            400.0,
            &[(120, 18.0, 1.0), (60, 34.0, 1.0), (0, 52.0, 1.0)],
            ans(Rising),
        ),
        mk(
            "ferritin-stable",
            "2276-4",
            "Ferritin",
            30.0,
            400.0,
            &[
                (90, 209.0, 1.0),
                (60, 207.0, 1.0),
                (30, 210.0, 1.0),
                (0, 208.0, 1.0),
            ],
            ans(Flat),
        ),
        mk(
            "ferritin-stale",
            "2276-4",
            "Ferritin",
            30.0,
            400.0,
            &[(800, 120.0, 1.0)],
            Expected::Abstained,
        ),
        // --- HbA1c (%, 4.0–5.6): small scale — slow real trends live below 0.01/day ---
        mk(
            "hba1c-creep",
            "4548-4",
            "HbA1c",
            4.0,
            5.6,
            &[(180, 5.3, 1.0), (90, 5.6, 1.0), (0, 5.9, 1.0)],
            ans(Rising),
        ),
        mk(
            "hba1c-stable",
            "4548-4",
            "HbA1c",
            4.0,
            5.6,
            &[(120, 5.4, 1.0), (60, 5.5, 1.0), (0, 5.4, 1.0)],
            ans(Flat),
        ),
        mk(
            "hba1c-lowconf",
            "4548-4",
            "HbA1c",
            4.0,
            5.6,
            &[(5, 6.1, 0.4)],
            Expected::Abstained,
        ),
        // --- TSH (mIU/L, 0.4–4.0): small scale ---
        mk(
            "tsh-drift-up",
            "3016-3",
            "TSH",
            0.4,
            4.0,
            &[(200, 2.0, 1.0), (100, 2.8, 1.0), (0, 3.6, 1.0)],
            ans(Rising),
        ),
        mk(
            "tsh-stable",
            "3016-3",
            "TSH",
            0.4,
            4.0,
            &[(90, 1.8, 1.0), (45, 2.0, 1.0), (0, 1.9, 1.0)],
            ans(Flat),
        ),
        // --- Total cholesterol (mg/dL, 125–200): large scale, naturally noisy ---
        mk(
            "chol-rise",
            "2093-3",
            "Cholesterol",
            125.0,
            200.0,
            &[(120, 168.0, 1.0), (60, 190.0, 1.0), (0, 212.0, 1.0)],
            ans(Rising),
        ),
        mk(
            "chol-noise",
            "2093-3",
            "Cholesterol",
            125.0,
            200.0,
            &[
                (120, 185.0, 1.0),
                (90, 172.0, 1.0),
                (60, 196.0, 1.0),
                (30, 179.0, 1.0),
                (0, 188.0, 1.0),
            ],
            ans(Flat),
        ),
        // --- Vitamin D (ng/mL, 30–100) ---
        mk(
            "vitd-supplement",
            "1989-3",
            "Vitamin D",
            30.0,
            100.0,
            &[(120, 19.0, 1.0), (60, 33.0, 1.0), (0, 46.0, 1.0)],
            ans(Rising),
        ),
        mk(
            "vitd-stable",
            "1989-3",
            "Vitamin D",
            30.0,
            100.0,
            &[(90, 52.0, 1.0), (45, 49.0, 1.0), (0, 51.0, 1.0)],
            ans(Flat),
        ),
        mk(
            "vitd-stale",
            "1989-3",
            "Vitamin D",
            30.0,
            100.0,
            &[(700, 22.0, 1.0)],
            Expected::Abstained,
        ),
        // --- Edge cases ---
        mk(
            "single-fresh",
            "2276-4",
            "Ferritin",
            30.0,
            400.0,
            &[(3, 180.0, 1.0)],
            ans(Flat),
        ),
        mk(
            "two-point-drop",
            "2276-4",
            "Ferritin",
            30.0,
            400.0,
            &[(30, 110.0, 1.0), (0, 42.0, 1.0)],
            ans(Falling),
        ),
        mk(
            "glucose-stale-lowconf",
            "2339-0",
            "Glucose",
            70.0,
            99.0,
            &[(400, 99.0, 0.5)],
            Expected::Abstained,
        ),
    ]
}

fn classify(p: &Params, c: &EvalCase, reg: &helix_escalation::ThresholdRegistry) -> (String, bool) {
    let req = AnalyzeRequest {
        concept_code: &c.concept_code,
        records: &c.records,
        now: c.now,
        staleness_window_days: p.staleness_window_days,
        confidence_floor: p.confidence_floor,
        reference_low: c.reference_low,
        reference_high: c.reference_high,
        flat_band_per_day: p.flat_band_per_day,
    };
    let (got, ok) = match (analyze(&req, reg).unwrap(), &c.expected) {
        (AnswerOutcome::Abstained(_), Expected::Abstained) => ("abstained".into(), true),
        (AnswerOutcome::Abstained(_), Expected::Answered(_)) => ("abstained".into(), false),
        (AnswerOutcome::Answered(_), Expected::Abstained) => ("answered".into(), false),
        (AnswerOutcome::Answered(a), Expected::Answered(w)) => {
            (format!("{:?}", a.trend.direction), a.trend.direction == *w)
        }
    };
    (got, ok)
}

fn main() {
    let reg = helix_escalation::builtin_registry_v1();
    let cases = cases();
    let shipping = Params {
        confidence_floor: 0.5,
        staleness_window_days: 365,
        flat_band_per_day: 0.01,
    };

    println!(
        "Eval set: {} labeled cases · 6 markers · registry FROZEN ({})\n",
        cases.len(),
        reg.version
    );

    // Per-case behavior under the shipping defaults.
    let base_fit = fitness(&shipping, &cases, &reg);
    println!(
        "SHIPPING DEFAULTS {shipping:?}  → fitness {:.2}",
        base_fit.score
    );
    let mut misses = vec![];
    for c in &cases {
        let (got, ok) = classify(&shipping, c, &reg);
        if !ok {
            misses.push(c.name.clone());
            println!("  ✗ {:<22} expected {:?}, got {}", c.name, c.expected, got);
        }
    }
    println!(
        "  {} correct, {} wrong\n",
        cases.len() - misses.len(),
        misses.len()
    );

    // Thorough evolve: many seeds × generations, keep the best (avoids local optima).
    let mut best = evolve(shipping, &Bounds::default(), &cases, &reg, 400, 1);
    for seed in 2..=24 {
        let r = evolve(shipping, &Bounds::default(), &cases, &reg, 400, seed);
        if r.best_fitness.score > best.best_fitness.score {
            best = r;
        }
    }

    println!(
        "EVOLVED {:?}  → fitness {:.2}",
        best.best_params, best.best_fitness.score
    );
    let mut still = vec![];
    for c in &cases {
        let (got, ok) = classify(&best.best_params, c, &reg);
        if !ok {
            still.push(c.name.clone());
            println!("  ✗ {:<22} expected {:?}, got {}", c.name, c.expected, got);
        }
    }
    println!(
        "  {} correct, {} wrong",
        cases.len() - still.len(),
        still.len()
    );
    println!(
        "\nIMPROVEMENT  fitness +{:.2}  ({} → {} wrong)  · over_confident={} (safety preserved)",
        best.improvement(),
        misses.len(),
        still.len(),
        best.best_fitness.over_confident
    );
}
