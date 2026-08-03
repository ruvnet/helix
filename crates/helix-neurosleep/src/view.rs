//! Derived-only view types shared with the research build.
//!
//! Nothing here may depend on the signed-bundle contract, signer trust, seal
//! keys, or sealed storage. The research WASM artifact compiles this module
//! (and `config` / `longitudinal`) without the `native-ingest` feature, so the
//! separate research build links no key-custody or attestation code at all.

use serde::{Deserialize, Serialize};

/// Fraction of the night that survived coverage and artifact screening.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AcquisitionConfidence(pub(crate) f64);

impl AcquisitionConfidence {
    pub fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationMaturity {
    Hypothesis,
    PreclinicalMouseModel,
    HumanObservational,
    HumanValidated,
}

impl InterpretationMaturity {
    /// Only the native ingest path parses signer-asserted maturity strings; the
    /// research build receives an already-typed value.
    #[cfg(feature = "native-ingest")]
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "hypothesis" => Some(Self::Hypothesis),
            "preclinical_mouse_model" => Some(Self::PreclinicalMouseModel),
            "human_observational" => Some(Self::HumanObservational),
            "human_validated" => Some(Self::HumanValidated),
            _ => None,
        }
    }
}

/// Identity-free view input emitted only after native verification and atomic
/// sealed storage. The research WASM surface may visualize this type, but may
/// not enroll signers, set admission policy, or accept bundle evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedNeuroSleepNight {
    pub night_start_ms: i64,
    pub compatibility_fingerprint: String,
    pub nrem_theta_coherence: Option<f64>,
    pub acquisition_confidence: AcquisitionConfidence,
    pub interpretation_maturity: InterpretationMaturity,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-051 one-way boundary: this rail observes derived nightly values and
    /// must never reach Helix's score, Focus Area, escalation, recommendation,
    /// LLM, neural-session, retrieval, or stimulation rails.
    pub(crate) const FORBIDDEN_RAILS: [&str; 12] = [
        "helix-score",
        "helix-focus",
        "helix-escalation",
        "helix-llm",
        "helix-pipeline",
        "helix-retrieval",
        "helix-neural",
        "helix-sensing",
        "helix-evidence",
        "helix-timeline",
        "helix-demo",
        "helix-wasm",
    ];

    #[test]
    fn manifest_declares_no_generic_helix_rail() {
        let manifest = include_str!("../Cargo.toml");
        for rail in FORBIDDEN_RAILS {
            assert!(
                !manifest.contains(rail),
                "helix-neurosleep declared a dependency on {rail}"
            );
        }
    }

    #[test]
    fn identity_free_view_rejects_identifying_fields() {
        let mut value = serde_json::json!({
            "night_start_ms": 0,
            "compatibility_fingerprint": "11".repeat(32),
            "nrem_theta_coherence": 0.5,
            "acquisition_confidence": 0.9,
            "interpretation_maturity": "preclinical_mouse_model",
        });
        assert!(serde_json::from_value::<VerifiedNeuroSleepNight>(value.clone()).is_ok());
        for identifier in ["study_id", "subject_pseudonym", "recording_id", "nonce"] {
            let mut probe = value.clone();
            probe
                .as_object_mut()
                .unwrap()
                .insert(identifier.into(), "leak".into());
            assert!(
                serde_json::from_value::<VerifiedNeuroSleepNight>(probe).is_err(),
                "derived view accepted {identifier}"
            );
        }
        value.as_object_mut().unwrap().remove("night_start_ms");
        assert!(serde_json::from_value::<VerifiedNeuroSleepNight>(value).is_err());
    }
}
