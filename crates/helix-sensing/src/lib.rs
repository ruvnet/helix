//! # helix-sensing — ADR-020: RuView WiFi-CSI contactless ambient sensing
//!
//! Adapter for [ruvnet/ruview](https://github.com/ruvnet/ruview): commodity WiFi
//! Channel State Information (CSI), captured on ~$9 ESP32 nodes, turned into
//! contactless vitals and spatial semantic states — entirely on the edge.
//!
//! This crate ingests a RuView reading (already extracted on-device and Ed25519
//! witness-attested) and produces two things for Helix:
//!
//! 1. **Provenance records** for the vitals (breathing rate, heart rate) — tagged
//!    `AmbientSensing`, `ruview` source, `RUVW-*` research codes (never a clinical
//!    LOINC, ADR-004), with **capped confidence** because the published CSI model
//!    is honestly screening-grade (82.3% held-out accuracy), not clinical.
//! 2. **Screening flags** mapping the inferred semantic states (possible-distress,
//!    fall-risk, apnea screening, …) to Escalation Guardian severities (ADR-009).
//!
//! Hard rules: raw CSI never enters here (only derived signals, ADR-001/014);
//! unsigned readings are rejected (provenance required, ADR-005); nothing is a
//! diagnosis (ADR-010).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_escalation::EscalationLevel;
use helix_provenance::{
    Confidence, EpochMillis, MeasurementMethod, ProvRecord, RecordId, ReferenceRange,
};

/// Framing that must accompany any RuView-derived signal.
pub const SCREENING_DISCLAIMER: &str =
    "Contactless WiFi-sensing signal (RuView) — screening only, not a diagnosis. \
     Worth a conversation with a clinician or caregiver, never a verdict.";

const SOURCE: &str = "ruview";

/// Vitals extracted on-device from CSI. Either may be absent for a given reading.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CsiVitals {
    /// Breaths per minute (RuView band 6–30).
    pub breathing_bpm: Option<f64>,
    /// Heart rate in BPM (RuView band 40–120).
    pub heart_rate_bpm: Option<f64>,
}

/// RuView's inferred semantic states (the "10 semantic states" per node).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticState {
    SomeoneSleeping,
    RoomActive,
    PossibleDistress,
    ElderlyInactivityAnomaly,
    MeetingInProgress,
    BathroomOccupied,
    FallRiskElevated,
    BedExit,
    NoMovement,
    MultiRoomTransition,
    /// Overnight sleep-disordered-breathing screening signal.
    ApneaScreening,
}

impl SemanticState {
    /// Severity this state escalates to (ADR-009). Most states are ambient
    /// context (no escalation); only safety/health-relevant ones escalate, and
    /// even then as screening prompts, never diagnoses.
    pub fn escalation_level(self) -> EscalationLevel {
        use SemanticState::*;
        match self {
            PossibleDistress | FallRiskElevated => EscalationLevel::Critical,
            ElderlyInactivityAnomaly | ApneaScreening => EscalationLevel::Urgent,
            BedExit | NoMovement => EscalationLevel::None, // context, not alarms on their own
            SomeoneSleeping | RoomActive | MeetingInProgress | BathroomOccupied
            | MultiRoomTransition => EscalationLevel::None,
        }
    }
}

/// A single RuView reading from one node, as exported by the edge mesh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuViewReading {
    pub node_id: String,
    pub room: String,
    pub recorded_at: EpochMillis,
    #[serde(default)]
    pub vitals: CsiVitals,
    #[serde(default)]
    pub states: Vec<SemanticState>,
    /// Ed25519 witness signature (carried as provenance; verification is RuView's).
    pub witness_signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SensingError {
    #[error("unsigned RuView reading rejected (provenance/attestation required)")]
    Unsigned,
    #[error("reading has neither vitals nor states")]
    Empty,
    #[error("a vital value was not finite")]
    NonFinite,
}

/// A screening flag derived from a semantic state, ready for the Escalation
/// Guardian (ADR-009).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreeningFlag {
    pub state: SemanticState,
    pub room: String,
    pub level: EscalationLevel,
    pub message: String,
}

fn vital_record(
    reading: &RuViewReading,
    code: &str,
    concept: &str,
    value: f64,
    unit: &str,
    range: Option<ReferenceRange>,
) -> Result<ProvRecord, SensingError> {
    if !value.is_finite() {
        return Err(SensingError::NonFinite);
    }
    Ok(ProvRecord {
        id: RecordId::from(format!(
            "ruvw-{}-{}-{}",
            reading.node_id, code, reading.recorded_at
        )),
        source: SOURCE.to_string(),
        measured_at: reading.recorded_at,
        method: MeasurementMethod::AmbientSensing,
        code: Some(format!("RUVW-{code}")),
        concept: concept.to_string(),
        value,
        unit: unit.to_string(),
        reference_range: range,
        // Screening-grade encoder (82.3% honest accuracy) → capped confidence.
        confidence: Confidence::new(0.5),
    })
}

/// Map a RuView reading's vitals into provenance records. Rejects unsigned or
/// empty readings; only present, finite vitals are emitted.
pub fn reading_to_records(reading: &RuViewReading) -> Result<Vec<ProvRecord>, SensingError> {
    if reading.witness_signature.trim().is_empty() {
        return Err(SensingError::Unsigned);
    }
    if reading.vitals.breathing_bpm.is_none()
        && reading.vitals.heart_rate_bpm.is_none()
        && reading.states.is_empty()
    {
        return Err(SensingError::Empty);
    }
    let mut out = Vec::new();
    if let Some(br) = reading.vitals.breathing_bpm {
        out.push(vital_record(
            reading,
            "BREATHING-RATE",
            "Respiration rate (WiFi-CSI)",
            br,
            "breaths/min",
            Some(ReferenceRange::new(Some(6.0), Some(30.0))),
        )?);
    }
    if let Some(hr) = reading.vitals.heart_rate_bpm {
        out.push(vital_record(
            reading,
            "HEART-RATE",
            "Heart rate (WiFi-CSI)",
            hr,
            "bpm",
            Some(ReferenceRange::new(Some(40.0), Some(120.0))),
        )?);
    }
    Ok(out)
}

/// Map a reading's semantic states to Escalation Guardian screening flags. Only
/// states with a non-`None` severity are returned.
pub fn screening_flags(reading: &RuViewReading) -> Vec<ScreeningFlag> {
    reading
        .states
        .iter()
        .filter_map(|&s| {
            let level = s.escalation_level();
            if level == EscalationLevel::None {
                return None;
            }
            let message = match s {
                SemanticState::PossibleDistress => {
                    "Possible distress detected — check on this person now (screening, not a diagnosis)."
                }
                SemanticState::FallRiskElevated => {
                    "Elevated fall risk pattern — worth checking in (screening, not a diagnosis)."
                }
                SemanticState::ElderlyInactivityAnomaly => {
                    "Unusual inactivity vs. baseline — consider checking in (screening, not a diagnosis)."
                }
                SemanticState::ApneaScreening => {
                    "Irregular overnight breathing pattern — consider a sleep study (screening, not a diagnosis)."
                }
                _ => "Screening signal worth a look (not a diagnosis).",
            };
            Some(ScreeningFlag {
                state: s,
                room: reading.room.clone(),
                level,
                message: message.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading() -> RuViewReading {
        RuViewReading {
            node_id: "esp-bedroom".into(),
            room: "bedroom".into(),
            recorded_at: 1_750_000_000_000,
            vitals: CsiVitals {
                breathing_bpm: Some(14.0),
                heart_rate_bpm: Some(62.0),
            },
            states: vec![
                SemanticState::SomeoneSleeping,
                SemanticState::ApneaScreening,
            ],
            witness_signature: "ed25519:sig".into(),
        }
    }

    #[test]
    fn vitals_become_screening_records() {
        let recs = reading_to_records(&reading()).unwrap();
        assert_eq!(recs.len(), 2);
        for r in &recs {
            assert_eq!(r.source, "ruview");
            assert_eq!(r.method, MeasurementMethod::AmbientSensing);
            assert!(r.code.as_ref().unwrap().starts_with("RUVW-"));
            assert!(r.confidence.get() <= 0.5);
        }
    }

    #[test]
    fn unsigned_rejected() {
        let mut r = reading();
        r.witness_signature = "".into();
        assert_eq!(reading_to_records(&r), Err(SensingError::Unsigned));
    }

    #[test]
    fn non_finite_rejected() {
        let mut r = reading();
        r.vitals.breathing_bpm = Some(f64::INFINITY);
        assert_eq!(reading_to_records(&r), Err(SensingError::NonFinite));
    }

    #[test]
    fn distress_escalates_critical() {
        let mut r = reading();
        r.states = vec![SemanticState::PossibleDistress];
        let flags = screening_flags(&r);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].level, EscalationLevel::Critical);
        assert!(flags[0].message.contains("not a diagnosis"));
    }

    #[test]
    fn apnea_screens_urgent_not_diagnostic() {
        let flags = screening_flags(&reading());
        // SomeoneSleeping → no flag; ApneaScreening → urgent
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].state, SemanticState::ApneaScreening);
        assert_eq!(flags[0].level, EscalationLevel::Urgent);
        assert!(!flags[0].message.to_lowercase().contains("you have"));
    }

    #[test]
    fn ambient_context_states_do_not_escalate() {
        let mut r = reading();
        r.states = vec![
            SemanticState::RoomActive,
            SemanticState::MeetingInProgress,
            SemanticState::BedExit,
            SemanticState::NoMovement,
        ];
        assert!(screening_flags(&r).is_empty());
    }

    #[test]
    fn disclaimer_non_diagnostic() {
        assert!(SCREENING_DISCLAIMER.contains("not a diagnosis"));
    }
}
