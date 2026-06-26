//! # helix-score — ADR-016: Transparent, Decomposable, Non-Diagnostic Health Score
//!
//! People want a single "how am I doing?" number — but a black-box score is both
//! untrustworthy and potentially misleading (the industry failure mode: not one
//! major wearable discloses how its composite is computed). Helix's 0–100 score
//! is the opposite:
//!
//! - **Fully decomposable** — the top-line number is a weighted roll-up of
//!   subsystem sub-scores (cardiometabolic, sleep, inflammation, fitness, …).
//! - **Traceable** — every sub-score lists *which of the user's measurements
//!   drove it* and which way each is trending. Never a black box.
//! - **Versioned** — the methodology carries a semver string; any change is
//!   visible and comparable (ADR-016).
//! - **Non-diagnostic** — explicitly a wellness-orientation aid, not a medical
//!   risk diagnosis (ADR-010), and it always shows its inputs and confidence.
//!
//! Pure, deterministic scoring logic; inputs are computed upstream (ADR-007).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Methodology version — bumped whenever weights or sub-score math change, so a
/// score is always interpretable against a known, comparable formula.
pub const METHODOLOGY_VERSION: &str = "score-v1.0.0";

/// Trend direction for a contributing measurement (mirrors helix-numeric).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trend {
    Improving,
    Stable,
    Worsening,
}

/// A single measurement's contribution to a sub-score, fully attributed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Driver {
    /// Canonical concept (ADR-004), e.g. "ApoB", "Deep sleep".
    pub concept: String,
    /// Normalized 0..=100 contribution of this single measurement.
    pub points: f64,
    pub trend: Trend,
    /// Source record id for click-through provenance (ADR-005).
    pub source_record: String,
}

/// The named subsystems the top-line score decomposes into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subsystem {
    Cardiometabolic,
    Sleep,
    Inflammation,
    Fitness,
}

impl Subsystem {
    pub fn label(self) -> &'static str {
        match self {
            Subsystem::Cardiometabolic => "Cardiometabolic",
            Subsystem::Sleep => "Sleep",
            Subsystem::Inflammation => "Inflammation",
            Subsystem::Fitness => "Fitness",
        }
    }
}

/// A computed sub-score for one subsystem, with its drivers and confidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubScore {
    pub subsystem: Subsystem,
    /// 0..=100.
    pub value: f64,
    /// Relative weight of this subsystem in the composite (>= 0; the composite
    /// normalizes by the total, so weights need not sum to 1).
    pub weight: f64,
    /// Confidence 0..=1 (e.g. lowered when drivers are sparse/stale).
    pub confidence: f64,
    pub drivers: Vec<Driver>,
    pub trend: Trend,
}

/// The composite 0–100 score plus its full decomposition. The number never
/// stands alone — it always carries the sub-scores that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthScore {
    pub value: f64,
    pub methodology_version: String,
    /// Overall confidence = weighted mean of sub-score confidences.
    pub confidence: f64,
    pub subscores: Vec<SubScore>,
    /// Always present, always the same wording: this is not a diagnosis.
    pub disclaimer: String,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ScoreError {
    #[error("no subscores provided")]
    Empty,
    #[error("subscore value {0} out of range 0..=100")]
    ValueOutOfRange(f64),
    #[error("weight {0} must be >= 0")]
    WeightOutOfRange(f64),
    #[error("confidence {0} out of range 0..=1")]
    ConfidenceOutOfRange(f64),
    #[error("subsystem weights sum to {0}, must be > 0")]
    ZeroWeight(f64),
    #[error("a value was not finite")]
    NonFinite,
}

fn check_subscore(s: &SubScore) -> Result<(), ScoreError> {
    if !s.value.is_finite() || !s.weight.is_finite() || !s.confidence.is_finite() {
        return Err(ScoreError::NonFinite);
    }
    if !(0.0..=100.0).contains(&s.value) {
        return Err(ScoreError::ValueOutOfRange(s.value));
    }
    if s.weight < 0.0 {
        return Err(ScoreError::WeightOutOfRange(s.weight));
    }
    if !(0.0..=1.0).contains(&s.confidence) {
        return Err(ScoreError::ConfidenceOutOfRange(s.confidence));
    }
    Ok(())
}

/// Roll subsystem sub-scores into the composite. The composite is a
/// weight-normalized average of sub-score values; overall confidence is the
/// same weighting applied to per-subsystem confidence. Deterministic and
/// auditable — given the sub-scores, anyone can recompute the top-line number.
pub fn compose(subscores: Vec<SubScore>) -> Result<HealthScore, ScoreError> {
    if subscores.is_empty() {
        return Err(ScoreError::Empty);
    }
    for s in &subscores {
        check_subscore(s)?;
    }
    let total_w: f64 = subscores.iter().map(|s| s.weight).sum();
    if total_w <= 0.0 {
        return Err(ScoreError::ZeroWeight(total_w));
    }
    let value = subscores.iter().map(|s| s.value * s.weight).sum::<f64>() / total_w;
    let confidence = subscores
        .iter()
        .map(|s| s.confidence * s.weight)
        .sum::<f64>()
        / total_w;

    Ok(HealthScore {
        value: round1(value),
        methodology_version: METHODOLOGY_VERSION.to_string(),
        confidence: round2(confidence),
        subscores,
        disclaimer:
            "A wellness orientation aid, not a medical risk diagnosis. Always shows its inputs."
                .to_string(),
    })
}

/// Overall trend of the composite: improving if the weighted majority of
/// subsystems are improving, etc. Surfaced so "which way am I heading?" is
/// answerable at a glance (the trend-first framing of ADR-016).
pub fn overall_trend(score: &HealthScore) -> Trend {
    let mut w = [0.0f64; 3]; // [improving, stable, worsening]
    for s in &score.subscores {
        let idx = match s.trend {
            Trend::Improving => 0,
            Trend::Stable => 1,
            Trend::Worsening => 2,
        };
        w[idx] += s.weight;
    }
    if w[0] > w[1] && w[0] > w[2] {
        Trend::Improving
    } else if w[2] > w[0] && w[2] > w[1] {
        Trend::Worsening
    } else {
        Trend::Stable
    }
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}
fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(subsystem: Subsystem, value: f64, weight: f64, conf: f64, trend: Trend) -> SubScore {
        SubScore {
            subsystem,
            value,
            weight,
            confidence: conf,
            drivers: vec![Driver {
                concept: "example".into(),
                points: value,
                trend,
                source_record: "rec-x".into(),
            }],
            trend,
        }
    }

    #[test]
    fn composes_weighted_average_and_is_decomposable() {
        let subs = vec![
            sub(Subsystem::Cardiometabolic, 80.0, 0.4, 0.9, Trend::Stable),
            sub(Subsystem::Sleep, 90.0, 0.3, 0.8, Trend::Improving),
            sub(Subsystem::Inflammation, 70.0, 0.2, 1.0, Trend::Stable),
            sub(Subsystem::Fitness, 60.0, 0.1, 0.6, Trend::Worsening),
        ];
        let score = compose(subs).unwrap();
        // (80*.4 + 90*.3 + 70*.2 + 60*.1) / 1.0 = 32+27+14+6 = 79
        assert!((score.value - 79.0).abs() < 1e-6);
        assert_eq!(score.subscores.len(), 4); // fully decomposed, never a single opaque number
        assert!(score.disclaimer.contains("not a medical risk diagnosis"));
        assert_eq!(score.methodology_version, "score-v1.0.0");
        // every subscore traces to at least one driver record
        assert!(score.subscores.iter().all(|s| !s.drivers.is_empty()));
    }

    #[test]
    fn weights_need_not_sum_to_one() {
        // un-normalized weights still produce the correct weighted average.
        let subs = vec![
            sub(Subsystem::Sleep, 100.0, 2.0, 1.0, Trend::Improving),
            sub(Subsystem::Fitness, 50.0, 2.0, 1.0, Trend::Stable),
        ];
        assert!((compose(subs).unwrap().value - 75.0).abs() < 1e-6);
    }

    #[test]
    fn overall_trend_follows_weight() {
        let subs = vec![
            sub(Subsystem::Cardiometabolic, 80.0, 0.6, 1.0, Trend::Worsening),
            sub(Subsystem::Sleep, 90.0, 0.4, 1.0, Trend::Improving),
        ];
        let score = compose(subs).unwrap();
        assert_eq!(overall_trend(&score), Trend::Worsening);
    }

    #[test]
    fn confidence_is_weighted_mean() {
        let subs = vec![
            sub(Subsystem::Sleep, 80.0, 1.0, 1.0, Trend::Stable),
            sub(Subsystem::Fitness, 80.0, 1.0, 0.5, Trend::Stable),
        ];
        assert!((compose(subs).unwrap().confidence - 0.75).abs() < 1e-9);
    }

    #[test]
    fn rejects_out_of_range_and_empty() {
        assert_eq!(compose(vec![]), Err(ScoreError::Empty));
        let bad = sub(Subsystem::Sleep, 150.0, 0.5, 1.0, Trend::Stable);
        assert_eq!(compose(vec![bad]), Err(ScoreError::ValueOutOfRange(150.0)));
        let badw = sub(Subsystem::Sleep, 50.0, -1.0, 1.0, Trend::Stable);
        assert_eq!(compose(vec![badw]), Err(ScoreError::WeightOutOfRange(-1.0)));
    }

    #[test]
    fn nan_rejected() {
        let bad = sub(Subsystem::Sleep, f64::NAN, 0.5, 1.0, Trend::Stable);
        assert_eq!(compose(vec![bad]), Err(ScoreError::NonFinite));
    }

    #[test]
    fn serializes_with_full_decomposition() {
        let subs = vec![sub(Subsystem::Sleep, 88.0, 1.0, 0.9, Trend::Improving)];
        let json = serde_json::to_string(&compose(subs).unwrap()).unwrap();
        assert!(json.contains("subscores"));
        assert!(json.contains("methodology_version"));
        assert!(json.contains("drivers"));
    }
}
