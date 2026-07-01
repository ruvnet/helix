//! Integration tests for helix-ingest, on SYNTHETIC data only (never real PHI).
//!
//! They prove the four load-bearing properties:
//!   1. parse → seal → RE-OPEN fresh → get: records round-trip through the vault.
//!   2. ciphertext-at-rest: sealed-payload markers (concept label + unit) are
//!      ABSENT from the raw redb file, while the plaintext key is present
//!      (positive control) — so the "absent" assertions are meaningful.
//!   3. dossier.json is emitted and re-parses into the UI's record shape
//!      (== serialized ProvRecord), with no misleading `meta` banner block.
//!   4. a wrong passphrase on an existing vault is rejected (no key mixing).

use std::fs;
use std::path::Path;

use helix_ingest::{run, vault, RunArgs};
use helix_provenance::ProvRecord;

const PASS: &str = "correct-horse-battery-staple";
const NOW_MS: i64 = 1_780_000_000_000;

// --- synthetic fixtures -----------------------------------------------------

/// 3 parseable Observations (Ferritin, HDL, TSH) + 1 unparseable (no LOINC).
const FHIR_BUNDLE: &str = r#"{
  "resourceType": "Bundle",
  "entry": [
    { "resource": { "resourceType": "Observation", "id": "ferritin1",
      "code": { "coding": [{ "system": "http://loinc.org", "code": "2276-4", "display": "Ferritin" }] },
      "valueQuantity": { "value": 28.0, "unit": "ng/mL" }, "effectiveDateTime": "2026-06-19T10:00:00Z",
      "referenceRange": [{ "low": { "value": 30.0 }, "high": { "value": 400.0 } }] } },
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

/// 3 known HealthKit records (HR, RHR, BodyMass) + 1 unmapped (skipped).
const APPLE_XML: &str = r#"<?xml version="1.0"?>
<HealthData>
 <Record type="HKQuantityTypeIdentifierHeartRate" unit="count/min" startDate="2026-06-01 10:00:00 -0700" value="62"/>
 <Record type="HKQuantityTypeIdentifierRestingHeartRate" unit="count/min" startDate="2026-06-01 06:00:00 -0700" value="54"/>
 <Record type="HKQuantityTypeIdentifierBodyMass" unit="kg" startDate="2026-06-02 08:00:00 +0000" value="72.5"/>
 <Record type="HKQuantityTypeIdentifierSomethingUnknown" unit="x" startDate="2026-06-02" value="1"/>
</HealthData>"#;

/// Write both fixtures into `dir` and return their paths.
fn write_fixtures(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let fhir = dir.join("bundle.json");
    let apple = dir.join("export.xml");
    fs::write(&fhir, FHIR_BUNDLE).unwrap();
    fs::write(&apple, APPLE_XML).unwrap();
    (fhir, apple)
}

fn run_full(base: &Path) -> helix_ingest::RunReport {
    let (fhir, apple) = write_fixtures(base);
    let vault_dir = base.join("vault");
    let out = base.join("out").join("dossier.json");
    run(RunArgs {
        fhir: Some(&fhir),
        apple: Some(&apple),
        vault_dir: &vault_dir,
        out: &out,
        passphrase: PASS,
        now_ms: NOW_MS,
    })
    .expect("ingest run should succeed")
}

// --- 1. round-trip ----------------------------------------------------------

#[test]
fn parse_seal_reopen_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let report = run_full(tmp.path());

    // 3 FHIR + 3 Apple = 6 sealed; the 4th FHIR entry was held for review.
    assert_eq!(report.record_count, 6, "expected 6 sealed records");
    assert_eq!(report.queued_for_review, 1, "1 unparseable FHIR resource");
    assert_eq!(report.by_source.get("FHIR"), Some(&3));
    assert_eq!(report.by_source.get("Apple Health"), Some(&3));

    // Independently RE-OPEN the vault fresh and decrypt — the values survive.
    let vault_dir = tmp.path().join("vault");
    let key = vault::prepare_key(&vault_dir, PASS).unwrap();
    let recovered: Vec<ProvRecord> = vault::reopen_records(&vault_dir, &key).unwrap();
    assert_eq!(recovered.len(), 6);

    let ferritin = recovered
        .iter()
        .find(|r| r.concept == "Ferritin")
        .expect("ferritin should round-trip");
    assert_eq!(ferritin.value, 28.0);
    assert_eq!(ferritin.unit, "ng/mL");
    assert_eq!(ferritin.code.as_deref(), Some("2276-4"));

    let hr = recovered
        .iter()
        .find(|r| r.concept == "Heart rate")
        .expect("heart rate should round-trip");
    assert_eq!(hr.value, 62.0);
}

// --- 2. ciphertext-at-rest --------------------------------------------------

#[test]
fn ciphertext_at_rest_payload_absent_keys_present() {
    let tmp = tempfile::tempdir().unwrap();
    let _ = run_full(tmp.path());

    let db_path = vault::records_db_path(&tmp.path().join("vault"));
    let raw = fs::read(&db_path).expect("raw vault file readable");

    // Sealed PAYLOAD strings must NOT appear in the clear.
    for marker in [
        "Ferritin",
        "HDL Cholesterol",
        "Heart rate",
        "Resting heart rate",
        "ng/mL",
        "count/min",
    ] {
        assert!(
            !contains(&raw, marker.as_bytes()),
            "plaintext payload marker {marker:?} leaked into the raw vault file"
        );
    }

    // Positive control: the redb KEY (record id) IS plaintext, proving the file
    // is populated with our data and the "absent" checks above are meaningful.
    // (This is the honest at-rest metadata leak: ids embed source + LOINC code.)
    assert!(
        contains(&raw, b"fhir-FHIR-ferritin1"),
        "expected the plaintext record-id key in the raw file"
    );
}

// --- 3. dossier emitted + re-parseable into the UI record shape -------------

#[test]
fn dossier_emitted_and_reparses_into_ui_record_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let report = run_full(tmp.path());

    let text = fs::read_to_string(&report.out_path).expect("dossier written");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("dossier is valid JSON");

    // UI requires `now` + `records`.
    assert_eq!(doc["now"].as_i64(), Some(NOW_MS));
    let records = doc["records"].as_array().expect("records is an array");
    assert_eq!(records.len(), 6);

    // Every element must deserialize back into a ProvRecord (i.e. the emitted
    // shape IS the UI record shape == serialized ProvRecord).
    let parsed: Vec<ProvRecord> =
        serde_json::from_value(doc["records"].clone()).expect("records parse as Vec<ProvRecord>");
    assert_eq!(parsed.len(), 6);

    // Spot-check the exact UI field shape on one record.
    let r0 = &records[0];
    for field in [
        "id",
        "source",
        "measured_at",
        "method",
        "code",
        "concept",
        "value",
        "unit",
        "reference_range",
        "confidence",
    ] {
        assert!(r0.get(field).is_some(), "record missing UI field {field:?}");
    }
    // method is snake_case; confidence is a bare float (not an object).
    assert!(r0["method"].is_string());
    assert!(r0["confidence"].is_number());

    // No `meta` block — that would trigger the UI's "SAMPLE DATA" banner on real
    // PHI. Provenance lives under the non-UI `_helix_ingest` key instead.
    assert!(doc.get("meta").is_none(), "dossier must not carry a meta banner");
    assert_eq!(doc["_helix_ingest"]["record_count"].as_u64(), Some(6));
    assert_eq!(doc["_helix_ingest"]["encryption_at_rest"], "proven");
}

// --- 4. wrong passphrase rejected -------------------------------------------

#[test]
fn wrong_passphrase_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let (fhir, apple) = write_fixtures(tmp.path());
    let vault_dir = tmp.path().join("vault");
    let out = tmp.path().join("dossier.json");

    // First run creates the vault under the correct passphrase.
    run(RunArgs {
        fhir: Some(&fhir),
        apple: Some(&apple),
        vault_dir: &vault_dir,
        out: &out,
        passphrase: PASS,
        now_ms: NOW_MS,
    })
    .expect("first run succeeds");

    // Second run on the SAME vault with a wrong passphrase must fail cleanly,
    // before any records are (mis)sealed under the wrong key.
    let err = run(RunArgs {
        fhir: Some(&fhir),
        apple: Some(&apple),
        vault_dir: &vault_dir,
        out: &out,
        passphrase: "wrong-passphrase",
        now_ms: NOW_MS,
    })
    .expect_err("wrong passphrase must be rejected");
    let msg = format!("{err:#}").to_lowercase();
    assert!(
        msg.contains("wrong passphrase") || msg.contains("verifier"),
        "unexpected error: {err:#}"
    );
}

// --- 5. at least one source required ----------------------------------------

#[test]
fn requires_at_least_one_source() {
    let tmp = tempfile::tempdir().unwrap();
    let vault_dir = tmp.path().join("vault");
    let out = tmp.path().join("dossier.json");
    let err = run(RunArgs {
        fhir: None,
        apple: None,
        vault_dir: &vault_dir,
        out: &out,
        passphrase: PASS,
        now_ms: NOW_MS,
    })
    .expect_err("no source must error");
    assert!(format!("{err:#}").contains("no source"));
}

/// True iff `needle` occurs contiguously in `haystack`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && needle.len() <= haystack.len()
        && haystack.windows(needle.len()).any(|w| w == needle)
}
