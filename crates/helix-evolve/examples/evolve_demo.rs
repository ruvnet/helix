//! Demo: evolve Helix's non-safety pipeline params against a labeled eval set.
//! Run: `cargo run -p helix-evolve --example evolve_demo`

use helix_evolve::{evolve, fitness, Bounds, EvalCase, Expected, Params};
use helix_numeric::TrendDirection;
use helix_provenance::{Confidence, MeasurementMethod, ProvRecord, RecordId, ReferenceRange};

const DAY: i64 = 86_400_000;
const NOW: i64 = 1000 * DAY;

fn rec(id: &str, days_ago: i64, value: f64, conf: f64) -> ProvRecord {
    ProvRecord {
        id: RecordId::from(id),
        source: "Quest".into(),
        measured_at: NOW - days_ago * DAY,
        method: MeasurementMethod::LabFeed,
        code: Some("2276-4".into()),
        concept: "Ferritin".into(),
        value,
        unit: "ng/mL".into(),
        reference_range: Some(ReferenceRange::new(Some(30.0), Some(400.0))),
        confidence: Confidence::new(conf),
    }
}
fn case(name: &str, records: Vec<ProvRecord>, expected: Expected) -> EvalCase {
    EvalCase {
        name: name.into(),
        concept_code: "2276-4".into(),
        records,
        now: NOW,
        reference_low: Some(30.0),
        reference_high: Some(400.0),
        expected,
    }
}

fn main() {
    let reg = helix_escalation::builtin_registry_v1();
    let cases = vec![
        case(
            "declining",
            vec![
                rec("a0", 60, 60.0, 1.0),
                rec("a1", 30, 45.0, 1.0),
                rec("a2", 0, 28.0, 1.0),
            ],
            Expected::Answered(TrendDirection::Falling),
        ),
        case(
            "noisy-flat",
            vec![
                rec("b0", 60, 50.0, 1.0),
                rec("b1", 45, 52.0, 1.0),
                rec("b2", 30, 49.0, 1.0),
                rec("b3", 15, 51.0, 1.0),
                rec("b4", 0, 50.6, 1.0),
            ],
            Expected::Answered(TrendDirection::Flat),
        ),
        case(
            "stale",
            vec![rec("c0", 800, 120.0, 1.0)],
            Expected::Abstained,
        ),
        case(
            "low-conf",
            vec![rec("d0", 5, 120.0, 0.45)],
            Expected::Abstained,
        ),
    ];

    let baseline = Params {
        confidence_floor: 0.5,
        staleness_window_days: 365,
        flat_band_per_day: 0.0,
    };
    let base_fit = fitness(&baseline, &cases, &reg);
    let res = evolve(baseline, &Bounds::default(), &cases, &reg, 300, 42);

    println!(
        "Eval set: {} labeled cases (red-flag registry FROZEN: {})",
        cases.len(),
        reg.version
    );
    println!("\nBASELINE  {baseline:?}");
    println!(
        "  fitness {:.2}  grounded={} abstained={} over_confident={} wrong_dir={}",
        base_fit.score,
        base_fit.grounded_correct,
        base_fit.abstained_correct,
        base_fit.over_confident,
        base_fit.wrong_direction
    );
    println!("\nEVOLVED   {:?}", res.best_params);
    println!(
        "  fitness {:.2}  grounded={} abstained={} over_confident={} wrong_dir={}",
        res.best_fitness.score,
        res.best_fitness.grounded_correct,
        res.best_fitness.abstained_correct,
        res.best_fitness.over_confident,
        res.best_fitness.wrong_direction
    );
    println!(
        "\nIMPROVEMENT  +{:.2}  (over_confident stayed {} — safety preserved)",
        res.improvement(),
        res.best_fitness.over_confident
    );
}
