//! End-to-end integration test for the grounded-answer pipeline (ADR-005/006/007/009).
//!
//! Proves the three outcomes the anti-hallucination design promises:
//! 1. a grounded, cited, trended answer for fresh in-range-ish data;
//! 2. an honest abstention when data is stale;
//! 3. a red-flag escalation that suppresses optimization for a critical value.

use helix_escalation::{builtin_registry_v1, EscalationLevel};
use helix_evidence::AbstentionReason;
use helix_numeric::TrendDirection;
use helix_pipeline::{analyze, AnalyzeRequest, AnswerOutcome};
use helix_provenance::{Confidence, MeasurementMethod, ProvRecord, RecordId, ReferenceRange};

const DAY: i64 = 86_400_000;

fn ferritin(id: &str, t_days: i64, value: f64) -> ProvRecord {
    ProvRecord {
        id: RecordId::from(id),
        source: "Quest".into(),
        measured_at: t_days * DAY,
        method: MeasurementMethod::LabFeed,
        code: Some("2276-4".into()),
        concept: "Ferritin".into(),
        value,
        unit: "ng/mL".into(),
        reference_range: Some(ReferenceRange::new(Some(30.0), Some(400.0))),
        confidence: Confidence::FULL,
    }
}

fn potassium(t_days: i64, value: f64) -> ProvRecord {
    ProvRecord {
        id: RecordId::from(format!("k-{t_days}")),
        source: "Labcorp".into(),
        measured_at: t_days * DAY,
        method: MeasurementMethod::LabFeed,
        code: Some("2823-3".into()),
        concept: "Serum potassium".into(),
        value,
        unit: "mmol/L".into(),
        reference_range: Some(ReferenceRange::new(Some(3.5), Some(5.1))),
        confidence: Confidence::FULL,
    }
}

#[test]
fn fresh_falling_ferritin_yields_grounded_cited_trended_answer() {
    let reg = builtin_registry_v1();
    let records = vec![
        ferritin("f1", 100, 45.0),
        ferritin("f2", 130, 33.0),
        ferritin("f3", 160, 28.0),
    ];
    let req = AnalyzeRequest {
        concept_code: "2276-4",
        records: &records,
        now: 161 * DAY,
        staleness_window_days: 365,
        confidence_floor: 0.5,
        reference_low: Some(30.0),
        reference_high: Some(400.0),
        flat_band_per_day: 0.01,
    };

    let out = analyze(&req, &reg).unwrap();
    let AnswerOutcome::Answered(ans) = out else {
        panic!("expected an answer, got {out:?}");
    };

    // No red flag for ferritin → optimization allowed.
    assert_eq!(ans.escalation.level, EscalationLevel::None);
    assert!(ans.recommendation.is_some());

    // Deterministic trend: falling, with a range crossing into Below.
    assert_eq!(ans.trend.direction, TrendDirection::Falling);
    assert_eq!(ans.trend.sample_size, 3);
    assert!(ans.trend.slope_per_day.unwrap() < 0.0);
    assert_eq!(ans.trend.crossings.len(), 1); // 33 -> 28 crosses below 30

    // Grounded: exactly one claim, backed by all three real records.
    assert_eq!(ans.claims.len(), 1);
    assert_eq!(ans.claims[0].evidence().len(), 3);
    assert!(ans.claims[0].text().contains("trending down"));

    // Serializable end to end (UI / audit).
    let json = serde_json::to_string(&ans).unwrap();
    assert!(json.contains("Ferritin"));
}

#[test]
fn stale_data_abstains_with_gap_notice() {
    let reg = builtin_registry_v1();
    let records = vec![ferritin("old", 100, 28.0)];
    let req = AnalyzeRequest {
        concept_code: "2276-4",
        records: &records,
        now: 600 * DAY, // 500 days after the only reading
        staleness_window_days: 365,
        confidence_floor: 0.5,
        reference_low: Some(30.0),
        reference_high: Some(400.0),
        flat_band_per_day: 0.01,
    };

    match analyze(&req, &reg).unwrap() {
        AnswerOutcome::Abstained(notice) => {
            assert!(matches!(notice.reason, AbstentionReason::Stale { .. }));
            assert!(notice.suggested_action.to_lowercase().contains("retest"));
        }
        other => panic!("expected abstention, got {other:?}"),
    }
}

#[test]
fn critical_potassium_escalates_and_suppresses_optimization() {
    let reg = builtin_registry_v1();
    // Fresh, confident, but critically high potassium (>= 6.0).
    let records = vec![potassium(199, 4.2), potassium(200, 6.4)];
    let req = AnalyzeRequest {
        concept_code: "2823-3",
        records: &records,
        now: 200 * DAY,
        staleness_window_days: 365,
        confidence_floor: 0.5,
        reference_low: Some(3.5),
        reference_high: Some(5.1),
        flat_band_per_day: 0.01,
    };

    let AnswerOutcome::Answered(ans) = analyze(&req, &reg).unwrap() else {
        panic!("expected an answer (escalated), not abstention");
    };
    assert_eq!(ans.escalation.level, EscalationLevel::Critical);
    assert!(ans.escalation.suppress_optimization);
    // ADR-009: optimization content is suppressed when a red flag fires.
    assert!(ans.recommendation.is_none());
    assert!(ans.escalation.message.to_lowercase().contains("urgent"));
    // Facts are still grounded and surfaced.
    assert_eq!(ans.claims.len(), 1);
}
