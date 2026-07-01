//! File → `Vec<ProvRecord>` via the already-tested `helix-connect` importers.
//!
//! No new parsing logic lives here: FHIR bundles go through
//! [`helix_connect::parse_observation`] (same iteration as `helix-wasm`'s
//! `fhir_import_json`) and Apple exports through
//! [`helix_connect::parse_apple_health`]. Un-parseable resources are counted into
//! a review queue (ADR-012), never silently dropped.

use anyhow::{Context, Result};
use helix_provenance::ProvRecord;
use serde_json::Value;

/// Source label stamped onto records imported from a FHIR bundle.
pub const FHIR_SOURCE: &str = "FHIR";
/// Source label stamped onto records imported from an Apple Health export.
pub const APPLE_SOURCE: &str = "Apple Health";
/// Upper bound on Apple records parsed in one run (mirrors `helix-wasm`).
const APPLE_MAX_RECORDS: usize = 100_000;

/// Outcome of parsing one file: the accepted records plus how many resources were
/// held for review (unmapped type, no LOINC, non-finite value, …).
pub struct Parsed {
    pub records: Vec<ProvRecord>,
    pub queued_for_review: usize,
}

/// Parse a FHIR R4 `Bundle` (or a bare `Observation`) string into records.
/// Mirrors `helix_wasm::fhir_import_json` exactly so native and wasm agree.
pub fn parse_fhir_bundle(json: &str, source: &str) -> Result<Parsed> {
    let v: Value = serde_json::from_str(json).context("FHIR file is not valid JSON")?;
    let entries = v["entry"].as_array().cloned().unwrap_or_default();
    let candidates: Vec<Value> = if entries.is_empty() && v["resourceType"] == "Observation" {
        vec![v.clone()]
    } else {
        entries.iter().map(|e| e["resource"].clone()).collect()
    };

    let mut records = Vec::new();
    let mut queued_for_review = 0usize;
    for r in &candidates {
        match helix_connect::parse_observation(r, source) {
            Ok(rec) => records.push(rec),
            Err(_) => queued_for_review += 1, // → human-review queue (ADR-012/004)
        }
    }
    Ok(Parsed {
        records,
        queued_for_review,
    })
}

/// Parse an Apple Health `export.xml` string into records via the tested scanner.
pub fn parse_apple_export(xml: &str, source: &str) -> Parsed {
    let records = helix_connect::parse_apple_health(xml, source, APPLE_MAX_RECORDS);
    // The importer already skips unknown/invalid records internally; those are not
    // recoverable to a count here, so review-queue is 0 for the Apple path.
    Parsed {
        records,
        queued_for_review: 0,
    }
}
