//! Endpoint/integration tests for `helix-ingest serve`, on SYNTHETIC data only
//! (never real PHI). They drive the router seam [`helix_ingest::serve::dispatch`]
//! directly — no sockets — so they are deterministic, while the loopback-bind
//! invariant is proven separately by the `binds_loopback` unit test in the serve
//! module (it opens a real listener and asserts the bound IP is loopback).
//!
//! Covered:
//!   * vault unlock — first-time create, then wrong-passphrase reject on reopen.
//!   * `POST /api/import` round-trips (FHIR via `content`, Apple via base64 data
//!     URL) and writes a valid, `meta`-free dossier the UI can load.
//!   * `GET /api/status` reflects the sealed corpus (counts + by_source + mode).
//!   * `GET /api/connectors` reports exactly one `live` source, rest `coming_soon`.
//!   * `POST /health/ingest` maps a documented HAE subset, skips the rest honestly,
//!     seals, and stamps the live connector's last_pull.
//!   * locked vault rejects import / health ingest with 401.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use base64::Engine as _;
use helix_ingest::serve::api::ServeState;
use helix_ingest::serve::{dispatch, Http};
use serde_json::{json, Value};

const PASS: &str = "correct-horse-battery-staple";

/// 3 parseable Observations (Ferritin, HDL, TSH) + 1 unparseable (no LOINC).
const FHIR_BUNDLE: &str = r#"{
  "resourceType": "Bundle",
  "entry": [
    { "resource": { "resourceType": "Observation", "id": "ferritin1",
      "code": { "coding": [{ "system": "http://loinc.org", "code": "2276-4", "display": "Ferritin" }] },
      "valueQuantity": { "value": 28.0, "unit": "ng/mL" }, "effectiveDateTime": "2026-06-19T10:00:00Z" } },
    { "resource": { "resourceType": "Observation", "id": "hdl1",
      "code": { "coding": [{ "system": "http://loinc.org", "code": "2085-9", "display": "HDL Cholesterol" }] },
      "valueQuantity": { "value": 58.0, "unit": "mg/dL" }, "effectiveDateTime": "2026-06-10" } },
    { "resource": { "resourceType": "Observation", "id": "tsh1",
      "code": { "coding": [{ "system": "http://loinc.org", "code": "3016-3", "display": "TSH" }] },
      "valueQuantity": { "value": 2.1, "unit": "mIU/L" }, "effectiveDateTime": "2026-06-10" } },
    { "resource": { "resourceType": "Observation", "id": "bad1",
      "code": { "coding": [{ "system": "http://snomed.info/sct", "code": "1" }] },
      "valueQuantity": { "value": 1.0, "unit": "x" }, "effectiveDateTime": "2026-01-01" } }
  ]
}"#;

/// 3 known HealthKit records + 1 unmapped (skipped by the importer).
const APPLE_XML: &str = r#"<?xml version="1.0"?>
<HealthData>
 <Record type="HKQuantityTypeIdentifierHeartRate" unit="count/min" startDate="2026-06-01 10:00:00 -0700" value="62"/>
 <Record type="HKQuantityTypeIdentifierRestingHeartRate" unit="count/min" startDate="2026-06-01 06:00:00 -0700" value="54"/>
 <Record type="HKQuantityTypeIdentifierBodyMass" unit="kg" startDate="2026-06-02 08:00:00 +0000" value="72.5"/>
 <Record type="HKQuantityTypeIdentifierSomethingUnknown" unit="x" startDate="2026-06-02" value="1"/>
</HealthData>"#;

// --- helpers ----------------------------------------------------------------

struct Env {
    _tmp: tempfile::TempDir,
    vault: PathBuf,
    ui: PathBuf,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let ui = tmp.path().join("ui");
    fs::create_dir_all(&ui).unwrap();
    fs::write(ui.join("hybrid.html"), b"<!doctype html><html>helix</html>").unwrap();
    let vault = tmp.path().join("vault");
    Env {
        _tmp: tmp,
        vault,
        ui,
    }
}

fn state(e: &Env) -> Mutex<ServeState> {
    Mutex::new(ServeState::new(e.vault.clone(), e.ui.clone()).unwrap())
}

fn jbody(h: &Http) -> Value {
    serde_json::from_slice(&h.body).expect("response body is JSON")
}

fn unlock(st: &Mutex<ServeState>, pass: &str) -> Http {
    let body = json!({ "passphrase": pass }).to_string();
    dispatch(st, "POST", "/api/vault/unlock", body.as_bytes())
}

// --- 1. unlock: create + wrong-pass reject ----------------------------------

#[test]
fn unlock_creates_then_rejects_wrong_passphrase() {
    let e = env();

    // First unlock on a fresh vault creates it.
    let st = state(&e);
    let r = unlock(&st, PASS);
    assert_eq!(r.status, 200, "create-unlock should succeed");
    let v = jbody(&r);
    assert_eq!(v["ok"], json!(true));
    assert_eq!(v["first_time"], json!(true), "fresh vault is first_time");

    // A brand-new session (fresh state) with the WRONG passphrase is rejected,
    // and correct passphrase then succeeds without first_time.
    let st2 = state(&e);
    let bad = unlock(&st2, "not-the-passphrase");
    assert_eq!(bad.status, 401, "wrong passphrase must be rejected");
    assert_eq!(jbody(&bad)["ok"], json!(false));

    let good = unlock(&st2, PASS);
    assert_eq!(good.status, 200);
    assert_eq!(jbody(&good)["first_time"], json!(false));
}

// --- 2. import (FHIR) round-trips + dossier + status reflects ----------------

#[test]
fn import_fhir_roundtrips_and_status_reflects() {
    let e = env();
    let st = state(&e);
    unlock(&st, PASS);

    let body = json!({ "kind": "fhir", "content": FHIR_BUNDLE }).to_string();
    let r = dispatch(&st, "POST", "/api/import", body.as_bytes());
    assert_eq!(r.status, 200, "import should succeed");
    let v = jbody(&r);
    assert_eq!(v["imported"], json!(3), "3 parseable, 1 held for review");
    assert_eq!(v["sealed"], json!(true));
    assert_eq!(v["by_source"]["FHIR"], json!(3));

    // Dossier written to the gitignored ui/private path, valid and meta-free.
    let dossier = e.ui.join("private").join("dossier.json");
    let doc: Value = serde_json::from_str(&fs::read_to_string(&dossier).unwrap()).unwrap();
    assert_eq!(doc["records"].as_array().unwrap().len(), 3);
    assert!(doc.get("meta").is_none(), "no SAMPLE banner on real PHI");
    assert_eq!(doc["_helix_ingest"]["record_count"], json!(3));

    // Status reflects the sealed corpus.
    let s = dispatch(&st, "GET", "/api/status", b"");
    let sv = jbody(&s);
    assert_eq!(sv["vault_exists"], json!(true));
    assert_eq!(sv["unlocked"], json!(true));
    assert_eq!(sv["record_count"], json!(3));
    assert_eq!(sv["mode"], json!("sealed"));
    assert_eq!(sv["by_source"]["FHIR"], json!(3));
}

// --- 3. import (Apple) via base64 data URL ----------------------------------

#[test]
fn import_apple_via_base64_data_url() {
    let e = env();
    let st = state(&e);
    unlock(&st, PASS);

    let b64 = base64::engine::general_purpose::STANDARD.encode(APPLE_XML);
    let data_url = format!("data:application/xml;base64,{b64}");
    let body = json!({ "kind": "apple", "data_base64": data_url }).to_string();

    let r = dispatch(&st, "POST", "/api/import", body.as_bytes());
    assert_eq!(r.status, 200);
    let v = jbody(&r);
    assert_eq!(v["imported"], json!(3), "3 known HK records, 1 unmapped skipped");
    assert_eq!(v["by_source"]["Apple Health"], json!(3));
}

// --- 4. connector registry: exactly one live -------------------------------

#[test]
fn connectors_report_one_live_rest_coming_soon() {
    let e = env();
    let st = state(&e);
    let r = dispatch(&st, "GET", "/api/connectors", b"");
    assert_eq!(r.status, 200);
    let v = jbody(&r);
    assert_eq!(v["live"], json!("apple_health"));

    let conns = v["connectors"].as_array().unwrap();
    let live: Vec<&str> = conns
        .iter()
        .filter(|c| c["status"] == json!("live"))
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(live, vec!["apple_health"], "exactly one live connector");

    for id in ["renpho", "quest_fhir", "walgreens", "lose_it"] {
        let c = conns.iter().find(|c| c["id"] == json!(id)).unwrap();
        assert_eq!(c["status"], json!("coming_soon"), "{id} must be coming_soon");
        assert!(c["cadence"].is_string(), "{id} carries an ADR-049 cadence");
    }
}

// --- 5. /health/ingest maps a documented HAE subset -------------------------

#[test]
fn health_ingest_maps_subset_and_marks_pull() {
    let e = env();
    let st = state(&e);
    unlock(&st, PASS);

    let hae = json!({
        "data": { "metrics": [
            { "name": "heart_rate", "units": "count/min",
              "data": [{ "date": "2026-06-01 10:00:00 -0700", "Min": 58, "Avg": 62, "Max": 121 }] },
            { "name": "step_count", "units": "count",
              "data": [{ "date": "2026-06-01 00:00:00 -0700", "qty": 8412 }] },
            { "name": "totally_unmapped_metric", "units": "x",
              "data": [{ "date": "2026-06-01", "qty": 1 }] }
        ]}
    })
    .to_string();

    let r = dispatch(&st, "POST", "/health/ingest", hae.as_bytes());
    assert_eq!(r.status, 200);
    let v = jbody(&r);
    assert_eq!(v["imported"], json!(2), "heart_rate + step_count mapped");
    assert_eq!(v["sealed"], json!(true));
    assert_eq!(v["by_source"]["Apple Health"], json!(2));
    let skipped = v["skipped"].as_array().unwrap();
    assert!(
        skipped.iter().any(|x| x == "totally_unmapped_metric"),
        "unmapped metric reported honestly, not faked"
    );

    // The live connector's last_pull watermark is now set (and persisted).
    let c = dispatch(&st, "GET", "/api/connectors", b"");
    let apple = jbody(&c)["connectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == json!("apple_health"))
        .cloned()
        .unwrap();
    assert!(apple["last_pull"].is_number(), "live pull watermark stamped");
}

// --- 6. locked vault rejects writes -----------------------------------------

#[test]
fn locked_vault_rejects_import_and_health_ingest() {
    let e = env();
    let st = state(&e); // never unlocked

    let imp = dispatch(
        &st,
        "POST",
        "/api/import",
        json!({ "kind": "fhir", "content": FHIR_BUNDLE })
            .to_string()
            .as_bytes(),
    );
    assert_eq!(imp.status, 401, "import requires an unlocked vault");

    let hi = dispatch(&st, "POST", "/health/ingest", b"{}");
    assert_eq!(hi.status, 401, "health ingest requires an unlocked vault");
}

/// SYNTHETIC "CLAW envelope" export (never real PHI): 4 `lab_results` + 1
/// `clinical_vitals`, ALL the SAME date, DISTINCT codes/displayNames, UNIQUE
/// wrapper ids, and inner resources that all reuse id `"obs"` — so ONLY the
/// wrapper-id override keeps them from colliding. `w-glu` has no
/// `effectiveDateTime` (proves the `dateAdded` fallback). `medications` is present
/// but out of scope and must be ignored.
const CLAW_ENVELOPE: &str = r#"{
  "lab_results": [
    { "id": "w-ferr", "dateAdded": "2026-06-15", "displayName": "Ferritin",
      "fhirData": { "resourceType": "Observation", "id": "obs",
        "code": { "coding": [{ "system": "http://loinc.org", "code": "2276-4", "display": "Ferritin" }] },
        "valueQuantity": { "value": 28.0, "unit": "ng/mL" }, "effectiveDateTime": "2026-06-15" } },
    { "id": "w-hdl", "dateAdded": "2026-06-15", "displayName": "HDL",
      "fhirData": { "resourceType": "Observation", "id": "obs",
        "code": { "coding": [{ "system": "http://loinc.org", "code": "2085-9", "display": "HDL" }] },
        "valueQuantity": { "value": 58.0, "unit": "mg/dL" }, "effectiveDateTime": "2026-06-15" } },
    { "id": "w-tsh", "dateAdded": "2026-06-15", "displayName": "TSH",
      "fhirData": { "resourceType": "Observation", "id": "obs",
        "code": { "coding": [{ "system": "http://loinc.org", "code": "3016-3", "display": "TSH" }] },
        "valueQuantity": { "value": 2.1, "unit": "mIU/L" }, "effectiveDateTime": "2026-06-15" } },
    { "id": "w-glu", "dateAdded": "2026-06-15", "displayName": "Glucose",
      "fhirData": { "resourceType": "Observation", "id": "obs",
        "code": { "coding": [{ "system": "http://loinc.org", "code": "2345-7", "display": "Glucose" }] },
        "valueQuantity": { "value": 92.0, "unit": "mg/dL" } } }
  ],
  "clinical_vitals": [
    { "id": "w-hr", "dateAdded": "2026-06-15", "displayName": "Heart rate",
      "fhirData": { "resourceType": "Observation", "id": "obs",
        "code": { "coding": [{ "system": "http://loinc.org", "code": "8867-4", "display": "Heart rate" }] },
        "valueQuantity": { "value": 60.0, "unit": "count/min" }, "effectiveDateTime": "2026-06-15" } }
  ],
  "medications": [ { "id": "m1", "displayName": "ignored",
    "fhirData": { "resourceType": "MedicationStatement" } } ]
}"#;

// --- 7. CLAW envelope: N items in → N records (same date, no collision) ------

#[test]
fn import_claw_same_date_no_collision() {
    let e = env();
    let st = state(&e);
    unlock(&st, PASS);

    let body = json!({ "kind": "claw", "content": CLAW_ENVELOPE }).to_string();
    let r = dispatch(&st, "POST", "/api/import", body.as_bytes());
    assert_eq!(r.status, 200, "claw import should succeed");
    let v = jbody(&r);
    assert_eq!(v["imported"], json!(5), "5 same-date items → 5 records (medications ignored)");
    assert_eq!(v["sealed"], json!(true));
    assert_eq!(v["by_source"]["Health Export"], json!(5));

    // Status confirms the sealed corpus really holds 5 DISTINCT records — proof
    // that the wrapper-id override defeats the same-date/same-inner-id collision.
    let s = jbody(&dispatch(&st, "GET", "/api/status", b""));
    assert_eq!(s["record_count"], json!(5), "no id collision on same-date records");
    assert_eq!(s["by_source"]["Health Export"], json!(5));
}

// --- 8. auto-detection routes the CLAW envelope to the CLAW path -------------

#[test]
fn import_auto_routes_claw_envelope() {
    let e = env();
    let st = state(&e);
    unlock(&st, PASS);

    let body = json!({ "kind": "auto", "content": CLAW_ENVELOPE }).to_string();
    let r = dispatch(&st, "POST", "/api/import", body.as_bytes());
    assert_eq!(r.status, 200, "auto should detect + import the CLAW envelope");
    let v = jbody(&r);
    assert_eq!(v["imported"], json!(5), "auto routed to the CLAW adapter");
    assert_eq!(v["by_source"]["Health Export"], json!(5));
}

// --- 9. auto routes a FHIR bundle, and an Unknown blob fails LOUDLY ----------

#[test]
fn import_auto_routes_fhir_and_rejects_unknown() {
    let e = env();
    let st = state(&e);
    unlock(&st, PASS);

    // A plain FHIR bundle auto-routes to the FHIR importer (3 parseable).
    let fhir = json!({ "kind": "auto", "content": FHIR_BUNDLE }).to_string();
    let rf = dispatch(&st, "POST", "/api/import", fhir.as_bytes());
    assert_eq!(rf.status, 200, "auto detects a FHIR bundle");
    assert_eq!(jbody(&rf)["by_source"]["FHIR"], json!(3));

    // An unrecognized blob returns a LOUD error, NOT a silent 0-record success.
    let junk = json!({ "kind": "auto", "content": "{\"foo\":1,\"bar\":2}" }).to_string();
    let ru = dispatch(&st, "POST", "/api/import", junk.as_bytes());
    assert_ne!(ru.status, 200, "unknown format must not return success");
    let uv = jbody(&ru);
    assert_eq!(uv["error"], json!("unrecognized format"));
    let keys: Vec<String> = uv["top_level_keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap().to_string())
        .collect();
    assert!(
        keys.contains(&"foo".to_string()) && keys.contains(&"bar".to_string()),
        "top-level keys surfaced for diagnosis: {keys:?}"
    );
}
