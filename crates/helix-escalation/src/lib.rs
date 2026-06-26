//! # helix-escalation — ADR-009: Red-Flag Escalation & Clinician-in-the-Loop
//!
//! Some values demand "see a professional now," not optimization tips. The
//! [`Escalation Guardian`](evaluate) watches every incoming value against a
//! **versioned, governance-controlled** registry of red-flag thresholds. When a
//! threshold fires, optimization content is *suppressed absolutely* and the user
//! is routed to urgent care.
//!
//! The threshold registry is explicitly **outside Darwin Mode's mutation space**
//! (ADR-018): only the medical advisory board changes these, and every change
//! bumps the registry version. Thresholds below cite their clinical source.
//!
//! Ambient-sensing (Cognitum Seed, ADR-014) inputs are framed as **screening,
//! not diagnosis** — a Seed respiratory-event flag escalates to "consider a
//! sleep study," never "you have apnea."

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Severity of an escalation. Both non-`None` levels suppress optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationLevel {
    /// No red flag — normal analysis may proceed.
    None,
    /// Amber: abnormal and worth a prompt clinical conversation.
    Urgent,
    /// Red: potentially dangerous; route to urgent/emergency care now.
    Critical,
}

/// Whether a threshold is screening-grade (ambient sensing) or a measured
/// clinical value. Screening flags never use diagnostic language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Clinical,
    Screening,
}

/// One red-flag rule for a concept, keyed by its canonical code (LOINC etc.).
/// Bounds are optional so one-sided flags (e.g. SpO₂ only-low) are expressible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedFlagThreshold {
    /// Canonical code (ADR-004), e.g. LOINC "2823-3" for serum potassium.
    pub code: String,
    pub concept: String,
    pub unit: String,
    pub grade: Grade,
    pub low_critical: Option<f64>,
    pub low_urgent: Option<f64>,
    pub high_urgent: Option<f64>,
    pub high_critical: Option<f64>,
    /// Clinical provenance for the numbers (audit + governance).
    pub source: String,
}

impl RedFlagThreshold {
    /// Evaluate a single value against this rule. Critical bounds dominate
    /// urgent bounds; low/high are checked independently.
    pub fn level_for(&self, value: f64) -> EscalationLevel {
        if self.low_critical.is_some_and(|t| value <= t)
            || self.high_critical.is_some_and(|t| value >= t)
        {
            return EscalationLevel::Critical;
        }
        if self.low_urgent.is_some_and(|t| value <= t)
            || self.high_urgent.is_some_and(|t| value >= t)
        {
            return EscalationLevel::Urgent;
        }
        EscalationLevel::None
    }
}

/// The full evaluation outcome for a value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EscalationResult {
    pub level: EscalationLevel,
    /// When true, the analyst MUST suppress optimization/recommendation content
    /// for this turn (ADR-009 hard rule).
    pub suppress_optimization: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EscalationError {
    #[error("no threshold registered for code {0}")]
    UnknownCode(String),
    #[error("value is not finite")]
    NonFinite,
}

/// A versioned set of thresholds. The version string is part of the audit trail
/// and changes only via clinical governance — never Darwin Mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThresholdRegistry {
    pub version: String,
    pub thresholds: Vec<RedFlagThreshold>,
}

impl ThresholdRegistry {
    pub fn get(&self, code: &str) -> Option<&RedFlagThreshold> {
        self.thresholds.iter().find(|t| t.code == code)
    }

    /// Evaluate a measured value for a concept. Unknown codes are an explicit
    /// error (we never silently treat an un-modelled value as safe).
    pub fn evaluate(&self, code: &str, value: f64) -> Result<EscalationResult, EscalationError> {
        if !value.is_finite() {
            return Err(EscalationError::NonFinite);
        }
        let t = self
            .get(code)
            .ok_or_else(|| EscalationError::UnknownCode(code.to_string()))?;
        let level = t.level_for(value);
        let suppress = level != EscalationLevel::None;
        let message = match (level, t.grade) {
            (EscalationLevel::None, _) => format!("{} is within safe bounds.", t.concept),
            (EscalationLevel::Urgent, Grade::Clinical) => format!(
                "{} = {} {} is abnormal — worth discussing with a clinician soon.",
                t.concept, value, t.unit
            ),
            (EscalationLevel::Critical, Grade::Clinical) => format!(
                "{} = {} {} is in a critical range — please seek urgent medical care now.",
                t.concept, value, t.unit
            ),
            (EscalationLevel::Urgent, Grade::Screening) => format!(
                "Screening signal: {} pattern is worth raising with a clinician (not a diagnosis).",
                t.concept
            ),
            (EscalationLevel::Critical, Grade::Screening) => format!(
                "Screening signal: {} pattern is pronounced — consider a clinical evaluation / sleep study (not a diagnosis).",
                t.concept
            ),
        };
        Ok(EscalationResult {
            level,
            suppress_optimization: suppress,
            message,
        })
    }
}

/// The built-in v1 registry. Numbers cite their clinical source; these are the
/// starting point a medical advisory board ratifies and versions (ADR-009).
pub fn builtin_registry_v1() -> ThresholdRegistry {
    ThresholdRegistry {
        version: "redflags-v1.0.0".to_string(),
        thresholds: vec![
            RedFlagThreshold {
                code: "2823-3".into(),
                concept: "Serum potassium".into(),
                unit: "mmol/L".into(),
                grade: Grade::Clinical,
                low_critical: Some(2.5),
                low_urgent: Some(3.0),
                high_urgent: Some(5.5),
                high_critical: Some(6.0),
                source: "Common lab critical-value panels (e.g. AJCP 2014 critical-value survey)"
                    .into(),
            },
            RedFlagThreshold {
                code: "718-7".into(),
                concept: "Hemoglobin".into(),
                unit: "g/dL".into(),
                grade: Grade::Clinical,
                low_critical: Some(6.5),
                low_urgent: Some(8.0),
                high_urgent: Some(18.0),
                high_critical: Some(20.0),
                source: "Transfusion-threshold literature; common critical-value panels".into(),
            },
            RedFlagThreshold {
                code: "2345-7".into(),
                concept: "Serum glucose".into(),
                unit: "mg/dL".into(),
                grade: Grade::Clinical,
                low_critical: Some(40.0),
                low_urgent: Some(54.0),
                high_urgent: Some(300.0),
                high_critical: Some(500.0),
                source: "ADA hypoglycemia Level-2 (<54) / severe; common critical-value panels"
                    .into(),
            },
            RedFlagThreshold {
                code: "59408-5".into(),
                concept: "Oxygen saturation (SpO₂)".into(),
                unit: "%".into(),
                grade: Grade::Clinical,
                low_critical: Some(88.0),
                low_urgent: Some(90.0),
                high_urgent: None,
                high_critical: None,
                source: "Common hypoxemia thresholds (SpO₂ <90% urgent, <88% critical)".into(),
            },
            RedFlagThreshold {
                // Cognitum Seed respiratory-event index — SCREENING ONLY.
                code: "SEED-REI".into(),
                concept: "Ambient respiratory-event index".into(),
                unit: "events/hour".into(),
                grade: Grade::Screening,
                low_critical: None,
                low_urgent: None,
                high_urgent: Some(15.0),
                high_critical: Some(30.0),
                source:
                    "Radar-screening analogue of AHI severity bands (screening, not PSG diagnosis)"
                        .into(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potassium_critical_high_escalates_and_suppresses() {
        let reg = builtin_registry_v1();
        let r = reg.evaluate("2823-3", 6.4).unwrap();
        assert_eq!(r.level, EscalationLevel::Critical);
        assert!(r.suppress_optimization);
        assert!(r.message.contains("urgent"));
    }

    #[test]
    fn potassium_in_range_is_none_no_suppression() {
        let reg = builtin_registry_v1();
        let r = reg.evaluate("2823-3", 4.2).unwrap();
        assert_eq!(r.level, EscalationLevel::None);
        assert!(!r.suppress_optimization);
    }

    #[test]
    fn glucose_urgent_band() {
        let reg = builtin_registry_v1();
        let r = reg.evaluate("2345-7", 50.0).unwrap(); // <54 urgent, >40 not critical
        assert_eq!(r.level, EscalationLevel::Urgent);
        assert!(r.suppress_optimization);
    }

    #[test]
    fn spo2_one_sided_low_only() {
        let reg = builtin_registry_v1();
        assert_eq!(
            reg.evaluate("59408-5", 87.0).unwrap().level,
            EscalationLevel::Critical
        );
        assert_eq!(
            reg.evaluate("59408-5", 98.0).unwrap().level,
            EscalationLevel::None
        );
    }

    #[test]
    fn seed_signal_uses_screening_language_not_diagnosis() {
        let reg = builtin_registry_v1();
        let r = reg.evaluate("SEED-REI", 35.0).unwrap();
        assert_eq!(r.level, EscalationLevel::Critical);
        assert!(r.message.to_lowercase().contains("not a diagnosis"));
        assert!(!r.message.to_lowercase().contains("you have"));
    }

    #[test]
    fn unknown_code_errors_never_assumed_safe() {
        let reg = builtin_registry_v1();
        assert!(matches!(
            reg.evaluate("0000-0", 1.0),
            Err(EscalationError::UnknownCode(_))
        ));
    }

    #[test]
    fn non_finite_rejected() {
        let reg = builtin_registry_v1();
        assert_eq!(
            reg.evaluate("2823-3", f64::NAN),
            Err(EscalationError::NonFinite)
        );
    }

    #[test]
    fn registry_is_versioned() {
        assert_eq!(builtin_registry_v1().version, "redflags-v1.0.0");
    }
}
