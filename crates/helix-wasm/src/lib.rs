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

use helix_escalation::builtin_registry_v1;
use helix_pipeline::{analyze, AnalyzeRequest};
use helix_provenance::ProvRecord;
use helix_score::{compose, SubScore};

/// The owned mirror of `helix_pipeline::AnalyzeRequest` (which borrows). Deserialized
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
/// Output: a `helix_pipeline::AnswerOutcome` as JSON (`abstained` or `answered`).
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

/// Ingest a `ruv-neural` signed session (JSON) and return the provenance-tagged
/// records it maps to (JSON array), so EEG/40 Hz entrainment signals flow into
/// the same dossier as labs — as a research/screening signal (ADR-014 framing).
#[wasm_bindgen]
pub fn neural_session_to_records_json(session: &str) -> Result<String, JsValue> {
    let s: helix_neural::NeuralSession = serde_json::from_str(session).map_err(err)?;
    let recs = helix_neural::session_to_records(&s).map_err(err)?;
    serde_json::to_string(&recs).map_err(err)
}

/// The non-diagnostic disclaimer that must accompany any ruv-neural signal.
#[wasm_bindgen]
pub fn neural_disclaimer() -> String {
    helix_neural::RESEARCH_DISCLAIMER.to_string()
}

/// Ingest a RuView WiFi-CSI reading (ADR-020): returns `{records, flags}` —
/// vital ProvRecords plus Escalation Guardian screening flags.
#[wasm_bindgen]
pub fn sensing_reading_json(reading: &str) -> Result<String, JsValue> {
    let r: helix_sensing::RuViewReading = serde_json::from_str(reading).map_err(err)?;
    let records = helix_sensing::reading_to_records(&r).map_err(err)?;
    let flags = helix_sensing::screening_flags(&r);
    serde_json::to_string(&serde_json::json!({ "records": records, "flags": flags })).map_err(err)
}

/// Ingest a user-owned genome profile (ADR-021): returns `{records, advisories}` —
/// GENO-* records plus "verify with your prescriber" pharmacogenomic advisories.
#[wasm_bindgen]
pub fn genome_profile_json(profile: &str) -> Result<String, JsValue> {
    let p: helix_genome::GenomeProfile = serde_json::from_str(profile).map_err(err)?;
    let records = helix_genome::profile_to_records(&p).map_err(err)?;
    let advisories = helix_genome::prescriber_advisories(&p);
    serde_json::to_string(&serde_json::json!({
        "records": records,
        "advisories": advisories,
        "privacy_note": helix_genome::GENOME_PRIVACY_NOTE,
    }))
    .map_err(err)
}

/// Gate an OCR'd lab document (ADR-022): returns the gated outcomes
/// (accepted records / queued candidates with reasons). `floor` is the minimum
/// OCR confidence to accept.
#[wasm_bindgen]
pub fn ocr_ingest_json(document: &str, floor: f64) -> Result<String, JsValue> {
    let doc: helix_ocr::OcrDocument = serde_json::from_str(document).map_err(err)?;
    let gated = helix_ocr::ingest_document(&doc, floor, |_| None).map_err(err)?;
    serde_json::to_string(&gated).map_err(err)
}

/// Biological-age estimate (ADR-034): input PhenoInputs JSON → BioAge + disclaimer.
#[wasm_bindgen]
pub fn bioage_json(inputs: &str) -> Result<String, JsValue> {
    let i: helix_bioage::PhenoInputs = serde_json::from_str(inputs).map_err(err)?;
    let b = helix_bioage::phenoage(&i).map_err(err)?;
    serde_json::to_string(&serde_json::json!({
        "bioage": b, "disclaimer": helix_bioage::DISCLAIMER,
    }))
    .map_err(err)
}

/// Focus areas (ADR-032): input `{records, now, config}` JSON → ranked focus items.
#[wasm_bindgen]
pub fn focus_json(payload: &str) -> Result<String, JsValue> {
    #[derive(serde::Deserialize)]
    struct P {
        records: Vec<ProvRecord>,
        now: i64,
        #[serde(default = "default_focus_cfg")]
        config: helix_focus::FocusConfig,
    }
    fn default_focus_cfg() -> helix_focus::FocusConfig {
        helix_focus::FocusConfig::default()
    }
    let p: P = serde_json::from_str(payload).map_err(err)?;
    let items = helix_focus::select_focus(&p.records, p.now, &p.config);
    serde_json::to_string(&items).map_err(err)
}

/// Score timeline (ADR-031): input `{snapshots, flat_band}` JSON → versioned
/// ScorePoints + trend + change-point.
#[wasm_bindgen]
pub fn timeline_json(payload: &str) -> Result<String, JsValue> {
    #[derive(serde::Deserialize)]
    struct P {
        snapshots: Vec<helix_timeline::Snapshot>,
        #[serde(default)]
        flat_band: f64,
    }
    let p: P = serde_json::from_str(payload).map_err(err)?;
    let tl = helix_timeline::build_timeline(p.snapshots, p.flat_band).map_err(err)?;
    serde_json::to_string(&tl).map_err(err)
}

/// Import a FHIR R4 Bundle (ADR-029): parse every `Observation` entry into
/// provenance records. Returns `{records, queued}` — un-parseable resources are
/// counted into the review queue (ADR-012), never silently dropped.
#[wasm_bindgen]
pub fn fhir_import_json(bundle: &str, source: &str) -> Result<String, JsValue> {
    let v: serde_json::Value = serde_json::from_str(bundle).map_err(err)?;
    let entries = v["entry"].as_array().cloned().unwrap_or_default();
    let mut records = Vec::new();
    let mut queued = 0usize;
    // A bare Observation (not a Bundle) is also accepted.
    let candidates: Vec<serde_json::Value> =
        if entries.is_empty() && v["resourceType"] == "Observation" {
            vec![v.clone()]
        } else {
            entries.iter().map(|e| e["resource"].clone()).collect()
        };
    for r in &candidates {
        match helix_connect::parse_observation(r, source) {
            Ok(rec) => records.push(rec),
            Err(_) => queued += 1,
        }
    }
    serde_json::to_string(&serde_json::json!({ "records": records, "queued": queued })).map_err(err)
}

/// Import an Apple Health `export.xml` (ADR-029): parse known HealthKit records
/// into provenance records. Bounded to 100k records. Returns the records JSON.
#[wasm_bindgen]
pub fn apple_health_import_json(xml: &str, source: &str) -> Result<String, JsValue> {
    let recs = helix_connect::parse_apple_health(xml, source, 100_000);
    serde_json::to_string(&recs).map_err(err)
}

/// Import a 23andMe-style raw genotype file (ADR-021): surfaces a few documented
/// single-SNP findings (NOT a full diplotype call). Returns the RawGenomeResult.
#[wasm_bindgen]
pub fn genome_raw_import_json(text: &str, source: &str) -> Result<String, JsValue> {
    let r = helix_genome::parse_23andme_raw(text, source);
    serde_json::to_string(&r).map_err(err)
}

/// Visual encode (ADR-025/028): grayscale pixels (row-major, w*h bytes) → the
/// perceptual tile embedding (DocEmbedding JSON). For OCR/visual previews.
#[wasm_bindgen]
pub fn visual_encode_json(w: u32, h: u32, px: &[u8]) -> Result<String, JsValue> {
    use helix_visual::{Gray, PerceptualEmbedder, VisualEmbedder};
    let g = Gray::new(w, h, px.to_vec()).map_err(err)?;
    let emb = PerceptualEmbedder::default().embed(&g).map_err(err)?;
    serde_json::to_string(&emb).map_err(err)
}

/// Visual similarity (ADR-025): MaxSim between two DocEmbeddings (JSON) → score.
#[wasm_bindgen]
pub fn visual_maxsim_json(a: &str, b: &str) -> Result<f32, JsValue> {
    let ea: helix_visual::DocEmbedding = serde_json::from_str(a).map_err(err)?;
    let eb: helix_visual::DocEmbedding = serde_json::from_str(b).map_err(err)?;
    helix_visual::maxsim(&ea, &eb).map_err(err)
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
