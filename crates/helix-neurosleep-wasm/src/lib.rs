//! Research-only NeuroSleep visualization boundary (ADR-051).
//!
//! Native Helix owns bundle verification, signer trust, consent, replay
//! protection, and sealed storage. This separate WASM artifact accepts only
//! identity-free [`VerifiedNeuroSleepNight`] values emitted after that native
//! boundary. It has no bundle, trust-enrollment, policy, or key-custody API.

use std::collections::BTreeMap;

use helix_neurosleep::{
    assess_longitudinal, CompatibleNight, InterpretationMaturity, LongitudinalDisposition,
    LongitudinalOperation, NeuroSleepResearchFlags, VerifiedNeuroSleepNight,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

const MAX_NIGHTS: usize = 64;
const MEASUREMENT_LABEL: &str = "NREM frontal-to-parietal theta coherence";
const CAVEAT: &str = "Related findings come from a preclinical APP/PS1 mouse study and have not been validated as a human clinical marker.";
const METHOD_LABEL: &str = "rUv Neural NeuroSleep qEEG v1";
const SOURCE_LABEL: &str = "Verified signed derived nightly bundles";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewRequest {
    flags: NeuroSleepResearchFlags,
    nights: Vec<VerifiedNeuroSleepNight>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum SafeResearchOutcome {
    Disabled {
        code: SafeCode,
    },
    Rejected {
        code: SafeCode,
    },
    Abstained {
        code: SafeCode,
        receipt: SafeReceipt,
    },
    Observed {
        receipt: SafeReceipt,
        panel: SafePanel,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SafeCode {
    ImportDisabled,
    ShadowDisabled,
    ResearchUiDisabled,
    EmptyNightSet,
    TooManyNights,
    InvalidVerifiedInput,
    MetricUnavailable,
    DuplicateNight,
    IncompatibleOrInsufficientNights,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct SafeReceipt {
    verified_nights: usize,
    acquisition_confidence_min: f64,
    interpretation_maturity: InterpretationMaturity,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct SafePanel {
    measurement: &'static str,
    compatible_baseline_change: f64,
    unit: &'static str,
    interpretation_caveat: &'static str,
    method: &'static str,
    source: &'static str,
}

/// Visualize identity-free nightly values already verified and sealed by the
/// native Helix host. This function deliberately cannot accept signed bundles.
#[wasm_bindgen]
pub fn visualize_verified_nights_json(payload: &str) -> Result<String, JsValue> {
    let request: ViewRequest = serde_json::from_str(payload).map_err(js_error)?;
    serde_json::to_string(&analyze(request)).map_err(js_error)
}

fn analyze(request: ViewRequest) -> SafeResearchOutcome {
    if !request.flags.import_v1 {
        return disabled(SafeCode::ImportDisabled);
    }
    if !request.flags.shadow_v1 {
        return disabled(SafeCode::ShadowDisabled);
    }
    if !request.flags.research_ui_v1 {
        return disabled(SafeCode::ResearchUiDisabled);
    }
    if request.nights.is_empty() {
        return rejected(SafeCode::EmptyNightSet);
    }
    if request.nights.len() > MAX_NIGHTS {
        return rejected(SafeCode::TooManyNights);
    }
    if request.nights.iter().any(invalid_native_view) {
        return rejected(SafeCode::InvalidVerifiedInput);
    }

    let mut nights = BTreeMap::new();
    for night in request.nights {
        let at = night.night_start_ms;
        if nights.insert(at, night).is_some() {
            return rejected(SafeCode::DuplicateNight);
        }
    }
    let receipt = safe_receipt(nights.values());
    if nights
        .values()
        .any(|night| night.nrem_theta_coherence.is_none())
    {
        return SafeResearchOutcome::Abstained {
            code: SafeCode::MetricUnavailable,
            receipt,
        };
    }

    let expected = &nights
        .first_key_value()
        .expect("non-empty night set")
        .1
        .compatibility_fingerprint;
    let compatible: Vec<CompatibleNight> = nights
        .values()
        .map(|night| CompatibleNight {
            night_start_ms: night.night_start_ms,
            compatibility_fingerprint: night.compatibility_fingerprint.clone(),
            accepted: true,
        })
        .collect();
    if !matches!(
        assess_longitudinal(&compatible, expected, LongitudinalOperation::Direction),
        LongitudinalDisposition::Ready { .. }
    ) {
        return SafeResearchOutcome::Abstained {
            code: SafeCode::IncompatibleOrInsufficientNights,
            receipt,
        };
    }

    let baseline = nights
        .values()
        .take(7)
        .map(|night| night.nrem_theta_coherence.expect("checked above"))
        .sum::<f64>()
        / 7.0;
    let latest = nights
        .last_key_value()
        .expect("non-empty night set")
        .1
        .nrem_theta_coherence
        .expect("checked above");
    SafeResearchOutcome::Observed {
        receipt,
        panel: SafePanel {
            measurement: MEASUREMENT_LABEL,
            compatible_baseline_change: latest - baseline,
            unit: "ratio",
            interpretation_caveat: CAVEAT,
            method: METHOD_LABEL,
            source: SOURCE_LABEL,
        },
    }
}

fn invalid_native_view(night: &VerifiedNeuroSleepNight) -> bool {
    night.compatibility_fingerprint.len() != 64
        || !night
            .compatibility_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || !night.acquisition_confidence.get().is_finite()
        || !(0.0..=1.0).contains(&night.acquisition_confidence.get())
        || night.interpretation_maturity != InterpretationMaturity::PreclinicalMouseModel
        || night
            .nrem_theta_coherence
            .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
}

fn safe_receipt<'a>(nights: impl Iterator<Item = &'a VerifiedNeuroSleepNight>) -> SafeReceipt {
    let nights: Vec<&VerifiedNeuroSleepNight> = nights.collect();
    SafeReceipt {
        verified_nights: nights.len(),
        acquisition_confidence_min: nights
            .iter()
            .map(|night| night.acquisition_confidence.get())
            .fold(1.0, f64::min),
        interpretation_maturity: nights
            .iter()
            .map(|night| night.interpretation_maturity)
            .min()
            .unwrap_or(InterpretationMaturity::Hypothesis),
    }
}

fn disabled(code: SafeCode) -> SafeResearchOutcome {
    SafeResearchOutcome::Disabled { code }
}

fn rejected(code: SafeCode) -> SafeResearchOutcome {
    SafeResearchOutcome::Rejected { code }
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn night(at: i64, value: Option<f64>) -> VerifiedNeuroSleepNight {
        VerifiedNeuroSleepNight {
            night_start_ms: at,
            compatibility_fingerprint: "11".repeat(32),
            nrem_theta_coherence: value,
            acquisition_confidence: serde_json::from_str("0.9").unwrap(),
            interpretation_maturity: InterpretationMaturity::PreclinicalMouseModel,
        }
    }

    #[test]
    fn default_flags_disable_before_night_validation() {
        assert_eq!(
            analyze(ViewRequest {
                flags: NeuroSleepResearchFlags::default(),
                nights: vec![],
            }),
            SafeResearchOutcome::Disabled {
                code: SafeCode::ImportDisabled
            }
        );
    }

    #[test]
    fn fourteen_native_verified_nights_produce_fixed_safe_schema() {
        let nights = (0..14).map(|at| night(at, Some(0.5))).collect();
        let outcome = analyze(ViewRequest {
            flags: NeuroSleepResearchFlags {
                import_v1: true,
                shadow_v1: true,
                research_ui_v1: true,
                rvf_v1: false,
            },
            nights,
        });
        let json = serde_json::to_string(&outcome).unwrap().to_lowercase();
        for forbidden in [
            "study_id",
            "subject",
            "recording",
            "nonce",
            "signature",
            concat!("verifying_", "key"),
            concat!("seal_", "key"),
            "recommendation",
            "stimulation",
            "actuator",
        ] {
            assert!(!json.contains(forbidden), "safe output leaked {forbidden}");
        }
        assert!(matches!(outcome, SafeResearchOutcome::Observed { .. }));
    }

    /// The research artifact must stay physically separate: no generic Helix
    /// rail, and no attestation, seal-key, or randomness code it could use to
    /// verify or store bundles itself.
    #[test]
    fn research_manifest_links_no_rail_attestation_or_key_custody_crate() {
        let manifest = include_str!("../Cargo.toml");
        for forbidden in [
            "helix-score",
            "helix-focus",
            "helix-escalation",
            "helix-llm",
            "helix-pipeline",
            "helix-retrieval",
            "helix-neural,",
            "helix-sensing",
            "helix-vault",
            "ruv-neural-core",
            "ed25519",
            "getrandom",
        ] {
            assert!(
                !manifest.contains(forbidden),
                "research artifact declared {forbidden}"
            );
        }
        assert!(manifest.contains("default-features = false"));
    }

    #[test]
    fn public_request_rejects_bundle_trust_policy_and_key_material() {
        let mut value = serde_json::json!({
            "flags": NeuroSleepResearchFlags::default(),
            "nights": [],
            "bundles_json": [],
            "admission": {},
        });
        let object = value.as_object_mut().unwrap();
        let mut trust = serde_json::Map::new();
        trust.insert(
            concat!("verifying_", "key_ed25519").into(),
            vec![0; 32].into(),
        );
        object.insert("trust".into(), serde_json::Value::Object(trust));
        object.insert(concat!("seal_", "key").into(), vec![0; 32].into());
        assert!(serde_json::from_value::<ViewRequest>(value).is_err());
    }
}
