//! # helix-evidence — ADR-006: Evidence Tiering & Explicit Abstention Policy
//!
//! Functional-medicine guidance ranges from rock-solid to speculative lore.
//! Conflating those tiers is how health products mislead. This crate makes the
//! tier explicit on every recommendation and — just as importantly — makes
//! **abstention a first-class, rewarded outcome**: when the data is missing,
//! stale, or low-confidence, Helix returns a [`GapNotice`] ("your last lipid
//! panel is 14 months old") instead of a guess.
//!
//! Pure logic, clock supplied by the caller (deterministic, testable).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_provenance::{EpochMillis, ProvRecord};

const MS_PER_DAY: i64 = 86_400_000;

/// Strength-of-support label, mapped onto the standard evidence hierarchy of
/// evidence-based medicine (Oxford CEBM): Tier 1 is the user's own measured
/// data; Tier 4 is heuristic "biohacker lore" that must never be dressed up as
/// established fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTier {
    /// Tier 1 — directly from the user's own measurements.
    YourData = 1,
    /// Tier 2 — population reference ranges, clinical guidelines.
    ReferenceStandard = 2,
    /// Tier 3 — peer-reviewed literature (with effect-size/population caveats).
    PeerReviewed = 3,
    /// Tier 4 — heuristic / emerging; explicitly flagged low-evidence.
    Heuristic = 4,
}

impl EvidenceTier {
    /// Short human label for the UI chip.
    pub fn label(self) -> &'static str {
        match self {
            EvidenceTier::YourData => "Your data",
            EvidenceTier::ReferenceStandard => "Reference standard",
            EvidenceTier::PeerReviewed => "Peer-reviewed",
            EvidenceTier::Heuristic => "Heuristic / emerging",
        }
    }

    /// Tier 4 must always be visibly flagged as low-evidence.
    pub fn is_low_evidence(self) -> bool {
        matches!(self, EvidenceTier::Heuristic)
    }
}

/// A recommendation carrying its evidence tier. Constructing one *requires* a
/// tier — there is no untiered recommendation in Helix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TieredRecommendation {
    pub text: String,
    pub tier: EvidenceTier,
}

impl TieredRecommendation {
    pub fn new(text: impl Into<String>, tier: EvidenceTier) -> Self {
        Self {
            text: text.into(),
            tier,
        }
    }
}

/// Why Helix declined to answer about a concept. Abstaining is a feature, not a
/// failure — each reason maps to a specific, honest [`GapNotice`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AbstentionReason {
    /// No record at all for this concept.
    NoData,
    /// The latest record is older than the staleness window.
    Stale { age_days: i64, window_days: i64 },
    /// The latest record's confidence is below the floor.
    LowConfidence { got: f64, floor: f64 },
}

/// What the user sees when Helix abstains: an honest, non-apologetic note that
/// always offers a way forward (never a dead end).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GapNotice {
    pub reason: AbstentionReason,
    pub message: String,
    pub suggested_action: String,
}

/// Outcome of [`assess`]: either Helix may answer, or it must abstain with a
/// gap notice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum AnswerVerdict {
    Answer,
    Abstain(GapNotice),
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum EvidenceError {
    #[error("staleness window must be positive, got {0} days")]
    BadWindow(i64),
    #[error("confidence floor must be in 0.0..=1.0, got {0}")]
    BadFloor(f64),
}

/// Default staleness windows (days) for common data kinds. These are policy, not
/// physics — versioned and reviewable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataKind {
    LipidPanel,
    Ferritin,
    Hormone,
    BloodPressure,
    AmbientVital,
    WearableDaily,
}

impl DataKind {
    pub const fn default_window_days(self) -> i64 {
        match self {
            DataKind::LipidPanel => 365,
            DataKind::Ferritin => 365,
            DataKind::Hormone => 365,
            DataKind::BloodPressure => 90,
            DataKind::AmbientVital => 1,
            DataKind::WearableDaily => 7,
        }
    }
}

/// The abstention gate. Given the current time, the most recent record for a
/// concept (if any), a staleness window and a confidence floor, decide whether
/// Helix may answer or must abstain.
///
/// Triggers, in order: missing → stale → low-confidence. This is the
/// enforcement of ADR-006's "permit and reward abstention".
pub fn assess(
    now: EpochMillis,
    latest: Option<&ProvRecord>,
    window_days: i64,
    confidence_floor: f64,
) -> Result<AnswerVerdict, EvidenceError> {
    if window_days <= 0 {
        return Err(EvidenceError::BadWindow(window_days));
    }
    if !(0.0..=1.0).contains(&confidence_floor) {
        return Err(EvidenceError::BadFloor(confidence_floor));
    }

    let Some(rec) = latest else {
        return Ok(AnswerVerdict::Abstain(GapNotice {
            reason: AbstentionReason::NoData,
            message: "I don't have any record for that yet.".to_string(),
            suggested_action: "Add it on your next panel and I'll track it.".to_string(),
        }));
    };

    let age_days = (now - rec.measured_at).max(0) / MS_PER_DAY;
    if age_days > window_days {
        return Ok(AnswerVerdict::Abstain(GapNotice {
            reason: AbstentionReason::Stale {
                age_days,
                window_days,
            },
            message: format!(
                "Your last {} is {} days old (I treat anything over {} as stale).",
                rec.concept, age_days, window_days
            ),
            suggested_action: format!("Consider retesting {} to refresh the picture.", rec.concept),
        }));
    }

    let conf = rec.confidence.get();
    if conf < confidence_floor {
        return Ok(AnswerVerdict::Abstain(GapNotice {
            reason: AbstentionReason::LowConfidence {
                got: conf,
                floor: confidence_floor,
            },
            message: format!(
                "My most recent {} reading is low-confidence ({:.2}).",
                rec.concept, conf
            ),
            suggested_action: "A confirmatory measurement would let me answer this.".to_string(),
        }));
    }

    Ok(AnswerVerdict::Answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_provenance::{Confidence, MeasurementMethod, RecordId, ReferenceRange};

    const DAY: i64 = 86_400_000;

    fn rec(measured_at: EpochMillis, confidence: f64) -> ProvRecord {
        ProvRecord {
            id: RecordId::from("r1"),
            source: "Quest".into(),
            measured_at,
            method: MeasurementMethod::LabFeed,
            code: Some("2276-4".into()),
            concept: "Ferritin".into(),
            value: 28.0,
            unit: "ng/mL".into(),
            reference_range: Some(ReferenceRange::new(Some(30.0), Some(400.0))),
            confidence: Confidence::new(confidence),
        }
    }

    #[test]
    fn answers_when_fresh_and_confident() {
        let now = 100 * DAY;
        let r = rec(99 * DAY, 1.0);
        assert_eq!(
            assess(now, Some(&r), 365, 0.5).unwrap(),
            AnswerVerdict::Answer
        );
    }

    #[test]
    fn abstains_on_no_data() {
        let v = assess(0, None, 365, 0.5).unwrap();
        match v {
            AnswerVerdict::Abstain(g) => assert_eq!(g.reason, AbstentionReason::NoData),
            _ => panic!("expected abstain"),
        }
    }

    #[test]
    fn abstains_on_stale() {
        let now = 500 * DAY;
        let r = rec(100 * DAY, 1.0); // 400 days old
        match assess(now, Some(&r), 365, 0.5).unwrap() {
            AnswerVerdict::Abstain(g) => {
                assert_eq!(
                    g.reason,
                    AbstentionReason::Stale {
                        age_days: 400,
                        window_days: 365
                    }
                );
            }
            _ => panic!("expected stale abstain"),
        }
    }

    #[test]
    fn abstains_on_low_confidence() {
        let now = 10 * DAY;
        let r = rec(9 * DAY, 0.3);
        match assess(now, Some(&r), 365, 0.6).unwrap() {
            AnswerVerdict::Abstain(g) => {
                assert!(matches!(g.reason, AbstentionReason::LowConfidence { .. }))
            }
            _ => panic!("expected low-confidence abstain"),
        }
    }

    #[test]
    fn bad_inputs_rejected() {
        assert_eq!(assess(0, None, 0, 0.5), Err(EvidenceError::BadWindow(0)));
        assert!(matches!(
            assess(0, None, 30, 1.5),
            Err(EvidenceError::BadFloor(_))
        ));
    }

    #[test]
    fn tier_ordering_and_flags() {
        assert!(EvidenceTier::YourData < EvidenceTier::Heuristic);
        assert!(EvidenceTier::Heuristic.is_low_evidence());
        assert!(!EvidenceTier::YourData.is_low_evidence());
    }

    #[test]
    fn default_windows_are_sane() {
        assert_eq!(DataKind::LipidPanel.default_window_days(), 365);
        assert_eq!(DataKind::AmbientVital.default_window_days(), 1);
    }
}
