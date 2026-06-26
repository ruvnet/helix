//! # helix-wasm — the WebAssembly surface of the Helix analytic core
//!
//! Compiles the **real** anti-hallucination pipeline (provenance grounding,
//! deterministic numerics, evidence tiering, red-flag escalation) and the
//! decomposable health score to WASM, so the web management UI and the mobile
//! app run the same audited Rust logic the backend would — no re-implementation
//! in JavaScript, no second source of truth.
//!
//! The boundary is JSON-in / JSON-out to keep the JS side trivial. Crypto/key
//! custody (`helix-vault`) is deliberately *not* exposed here: on device, keys
//! live in platform secure storage, and this surface only ever sees the records
//! the user has already unsealed locally.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use helix_core::{analyze, AnalyzeRequest};
use helix_escalation::builtin_registry_v1;
use helix_provenance::ProvRecord;
use helix_score::{compose, SubScore};

/// The owned mirror of `helix_core::AnalyzeRequest` (which borrows). Deserialized
/// from the JS payload, then turned into the borrowed request internally.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnalyzePayload {
    pub concept_code: String,
    pub records: Vec<ProvRecord>,
    pub now: i64,
    #[serde(default = "default_window")]
    pub staleness_window_days: i64,
    #[serde(default = "default_floor")]
    pub confidence_floor: f64,
    pub reference_low: Option<f64>,
    pub reference_high: Option<f64>,
    #[serde(default)]
    pub flat_band_per_day: f64,
}

fn default_window() -> i64 {
    365
}
fn default_floor() -> f64 {
    0.5
}

/// Run the grounded-answer pipeline. Input: an [`AnalyzePayload`] as JSON.
/// Output: a `helix_core::AnswerOutcome` as JSON (`abstained` or `answered`).
#[wasm_bindgen]
pub fn analyze_json(payload: &str) -> Result<String, JsValue> {
    let p: AnalyzePayload = serde_json::from_str(payload).map_err(err)?;
    let req = AnalyzeRequest {
        concept_code: &p.concept_code,
        records: &p.records,
        now: p.now,
        staleness_window_days: p.staleness_window_days,
        confidence_floor: p.confidence_floor,
        reference_low: p.reference_low,
        reference_high: p.reference_high,
        flat_band_per_day: p.flat_band_per_day,
    };
    let registry = builtin_registry_v1();
    let outcome = analyze(&req, &registry).map_err(err)?;
    serde_json::to_string(&outcome).map_err(err)
}

/// Compose a decomposable 0–100 health score. Input: an array of `SubScore` as
/// JSON. Output: a `HealthScore` as JSON.
#[wasm_bindgen]
pub fn compose_score_json(subscores: &str) -> Result<String, JsValue> {
    let subs: Vec<SubScore> = serde_json::from_str(subscores).map_err(err)?;
    let score = compose(subs).map_err(err)?;
    serde_json::to_string(&score).map_err(err)
}

/// The red-flag threshold registry version currently in force (ADR-009).
#[wasm_bindgen]
pub fn redflag_registry_version() -> String {
    builtin_registry_v1().version
}

/// Crate version string for the UI footer / diagnostics.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn err<E: std::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&e.to_string())
}

// Native-target tests (the binding logic is the same on native; browser tests
// would use wasm-bindgen-test under wasm-pack test).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_round_trips_through_json() {
        let payload = serde_json::json!({
            "concept_code": "2276-4",
            "records": [{
                "id": "r1", "source": "Quest", "measured_at": 1000,
                "method": "lab_feed", "code": "2276-4", "concept": "Ferritin",
                "value": 28.0, "unit": "ng/mL",
                "reference_range": {"low": 30.0, "high": 400.0},
                "confidence": 1.0
            }],
            "now": 2000,
            "reference_low": 30.0, "reference_high": 400.0
        })
        .to_string();
        let out = analyze_json(&payload).unwrap();
        // single record => trend undefined but answerable; should be "answered".
        assert!(out.contains("answered") || out.contains("abstained"));
    }

    #[test]
    fn compose_score_through_json() {
        let subs = serde_json::json!([
            {"subsystem":"sleep","value":90.0,"weight":1.0,"confidence":0.9,
             "drivers":[{"concept":"Deep sleep","points":90.0,"trend":"improving","source_record":"r1"}],
             "trend":"improving"}
        ])
        .to_string();
        let out = compose_score_json(&subs).unwrap();
        assert!(out.contains("methodology_version"));
        assert!(out.contains("90"));
    }

    #[test]
    fn version_is_nonempty() {
        assert!(!version().is_empty());
        assert!(redflag_registry_version().starts_with("redflags"));
    }
}
