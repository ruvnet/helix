//! File → `Vec<ProvRecord>` via the already-tested `helix-connect` importers.
//!
//! No new parsing logic lives here: FHIR bundles go through
//! [`helix_connect::parse_observation`] (same iteration as `helix-wasm`'s
//! `fhir_import_json`) and Apple exports through
//! [`helix_connect::parse_apple_health`]. Un-parseable resources are counted into
//! a review queue (ADR-012), never silently dropped.

use anyhow::{Context, Result};
use helix_provenance::{ProvRecord, RecordId};
use serde_json::Value;

/// Source label stamped onto records imported from a FHIR bundle.
pub const FHIR_SOURCE: &str = "FHIR";
/// Source label stamped onto records imported from an Apple Health export.
pub const APPLE_SOURCE: &str = "Apple Health";
/// Source label stamped onto records imported from a consolidated "CLAW envelope"
/// export (a dict of `lab_results`/`clinical_vitals`/… arrays wrapping `fhirData`).
pub const CLAW_SOURCE: &str = "Health Export";
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

/// The detected on-the-wire shape of an uploaded file (drives the `auto` path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A FHIR R4 `Bundle` or a bare `Observation`.
    Fhir,
    /// The consolidated "CLAW envelope": a dict of `*_results`-style arrays whose
    /// items wrap a real `fhirData` resource.
    ClawEnvelope,
    /// An Apple Health `export.xml` (XML, so detected at the text layer, not here).
    AppleXml,
    /// Nothing we recognize — the caller MUST fail loudly, never import 0 silently.
    Unknown,
}

/// Classify already-parsed JSON into a [`Format`]. Apple exports are XML (never a
/// `Value`) and are detected by [`looks_like_apple_xml`]; a JSON-wrapped
/// `HealthData` object is still recognized here for completeness.
pub fn detect_format(v: &Value) -> Format {
    if let Some(obj) = v.as_object() {
        // CLAW envelope: an explicit `lab_results` key, any `*_results` key, or
        // any array whose first element carries a nested `fhirData` resource.
        let is_claw = obj.contains_key("lab_results")
            || obj.keys().any(|k| k.ends_with("_results"))
            || obj.values().any(|val| {
                val.as_array()
                    .and_then(|a| a.first())
                    .is_some_and(|first| first.get("fhirData").is_some())
            });
        if is_claw {
            return Format::ClawEnvelope;
        }
        if obj.contains_key("HealthData") {
            return Format::AppleXml;
        }
    }
    match v["resourceType"].as_str() {
        Some("Bundle") | Some("Observation") => Format::Fhir,
        _ => Format::Unknown,
    }
}

/// Cheap text-level check for an Apple Health `export.xml` (which is not JSON, so
/// the `auto` path must sniff it before attempting a JSON parse).
pub fn looks_like_apple_xml(text: &str) -> bool {
    text.contains("<HealthData") || text.contains("<Record ")
}

/// Top-level keys of a JSON object — surfaced in the loud "unrecognized format"
/// error so an unknown export is diagnosable instead of silently importing 0.
pub fn top_level_keys(v: &Value) -> Vec<String> {
    v.as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

/// Parse a consolidated "CLAW envelope" export into records.
///
/// Iterates the `lab_results` and `clinical_vitals` arrays; each item is
/// `{ id, dateAdded, displayName, fhirData: <FHIR Observation> }`. Every
/// `fhirData` goes through the tested [`helix_connect::parse_observation`], then
/// the record id is OVERRIDDEN with `claw-{item.id}` so two items sharing a
/// date/code never collide. When `fhirData` lacks `effectiveDateTime`, the
/// wrapper's `dateAdded` is used. Non-parseable items are counted into the review
/// queue (never dropped silently).
///
/// `medications`/`conditions`/`procedures`/etc. are out of scope for records this
/// pass (they are not FHIR Observations) — they are left untouched, not
/// force-mapped into records.
pub fn parse_claw_envelope(json: &str, source: &str) -> Result<Parsed> {
    let v: Value = serde_json::from_str(json).context("CLAW export is not valid JSON")?;
    let mut records = Vec::new();
    let mut queued_for_review = 0usize;

    for section in ["lab_results", "clinical_vitals"] {
        let Some(items) = v[section].as_array() else {
            continue;
        };
        for item in items {
            // Fall back to the wrapper `dateAdded` when the resource carries no
            // effectiveDateTime, so otherwise-valid items still parse.
            let mut fhir = item["fhirData"].clone();
            let has_dt = fhir
                .get("effectiveDateTime")
                .and_then(Value::as_str)
                .is_some();
            if !has_dt {
                if let Some(added) = item["dateAdded"].as_str() {
                    fhir["effectiveDateTime"] = Value::String(added.to_string());
                }
            }
            match helix_connect::parse_observation(&fhir, source) {
                Ok(mut rec) => {
                    // Unique id from the wrapper id → no cross-record collisions.
                    if let Some(wid) = item["id"].as_str() {
                        rec.id = RecordId::from(format!("claw-{wid}"));
                    }
                    records.push(rec);
                }
                Err(_) => queued_for_review += 1, // → human-review queue (ADR-012/004)
            }
        }
    }
    Ok(Parsed {
        records,
        queued_for_review,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    /// One CLAW item wrapping a FHIR Observation. `wid` is the unique wrapper id;
    /// the inner resource deliberately REUSES the same `"obs"` id across items so
    /// the tests prove the wrapper-id override is what prevents collisions.
    fn item(wid: &str, code: &str, name: &str, date: Option<&str>) -> Value {
        let mut fhir = json!({
            "resourceType": "Observation", "id": "obs",
            "code": { "coding": [{ "system": "http://loinc.org", "code": code, "display": name }] },
            "valueQuantity": { "value": 1.0, "unit": "u" }
        });
        if let Some(d) = date {
            fhir["effectiveDateTime"] = json!(d);
        }
        json!({ "id": wid, "dateAdded": "2026-06-15", "displayName": name, "fhirData": fhir })
    }

    #[test]
    fn claw_same_date_distinct_wrappers_no_collision() {
        // 4 labs + 1 vital, ALL the same date, inner resources share id "obs".
        // `medications` is present but out of scope and must be ignored.
        let doc = json!({
            "lab_results": [
                item("w-ferr", "2276-4", "Ferritin", Some("2026-06-15")),
                item("w-hdl",  "2085-9", "HDL",      Some("2026-06-15")),
                item("w-tsh",  "3016-3", "TSH",      Some("2026-06-15")),
                item("w-glu",  "2345-7", "Glucose",  None), // no effectiveDateTime
            ],
            "clinical_vitals": [ item("w-hr", "8867-4", "Heart rate", Some("2026-06-15")) ],
            "medications": [ { "id": "m1" } ]
        });
        let p = parse_claw_envelope(&doc.to_string(), CLAW_SOURCE).unwrap();
        assert_eq!(p.records.len(), 5, "5 items in → 5 records (no id collision)");
        assert_eq!(p.queued_for_review, 0);
        let ids: BTreeSet<RecordId> = p.records.iter().map(|r| r.id.clone()).collect();
        assert_eq!(ids.len(), 5, "every stored record id is distinct");
        assert!(ids.contains(&RecordId::from("claw-w-glu")));
        // dateAdded fallback stamped the date-less Glucose item.
        let glu = p
            .records
            .iter()
            .find(|r| r.id == RecordId::from("claw-w-glu"))
            .unwrap();
        assert!(glu.measured_at > 0, "dateAdded fallback stamped a timestamp");
    }

    #[test]
    fn claw_unparseable_item_is_queued_not_dropped() {
        let doc = json!({
            "lab_results": [
                item("w-ok", "2276-4", "Ferritin", Some("2026-06-15")),
                { "id": "w-bad", "dateAdded": "2026-06-15", "fhirData": {
                    "resourceType": "Observation", "id": "obs",
                    "code": { "coding": [{ "system": "http://snomed.info/sct", "code": "1" }] },
                    "valueQuantity": { "value": 1.0, "unit": "x" },
                    "effectiveDateTime": "2026-06-15" } }
            ]
        });
        let p = parse_claw_envelope(&doc.to_string(), CLAW_SOURCE).unwrap();
        assert_eq!(p.records.len(), 1);
        assert_eq!(p.queued_for_review, 1, "bad item queued, never silently dropped");
    }

    #[test]
    fn detect_classifies_shapes() {
        assert_eq!(
            detect_format(&json!({ "lab_results": [], "medications": [] })),
            Format::ClawEnvelope
        );
        // Recognized purely by a nested `fhirData` array, with no `*_results` key.
        assert_eq!(
            detect_format(&json!({ "whatever": [ { "fhirData": { "resourceType": "Observation" } } ] })),
            Format::ClawEnvelope
        );
        assert_eq!(detect_format(&json!({ "resourceType": "Bundle" })), Format::Fhir);
        assert_eq!(detect_format(&json!({ "resourceType": "Observation" })), Format::Fhir);
        assert_eq!(detect_format(&json!({ "HealthData": {} })), Format::AppleXml);
        assert_eq!(detect_format(&json!({ "foo": 1, "bar": 2 })), Format::Unknown);
    }

    #[test]
    fn apple_xml_sniff_and_top_level_keys() {
        assert!(looks_like_apple_xml(
            "<?xml?><HealthData><Record type=\"x\"/></HealthData>"
        ));
        assert!(!looks_like_apple_xml("{\"lab_results\":[]}"));
        let keys: BTreeSet<String> = top_level_keys(&json!({ "b": 1, "a": 2 }))
            .into_iter()
            .collect();
        assert_eq!(
            keys,
            BTreeSet::from(["a".to_string(), "b".to_string()]),
            "top-level keys are surfaced for the loud unknown-format error"
        );
    }
}
