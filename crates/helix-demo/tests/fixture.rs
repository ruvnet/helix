//! Proves the *committed* artifact the UI actually loads — `ui/demo-dossier.json`
//! — deserializes into real [`ProvRecord`]s and flows through the real pipeline.
//! This is the end-to-end guard: if someone hand-edits the JSON into an invalid
//! shape, or lets it drift from the generator, this test goes red.

use std::path::PathBuf;

use helix_escalation::builtin_registry_v1;
use helix_pipeline::{analyze, AnalyzeRequest, AnswerOutcome};
use helix_provenance::ProvRecord;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("ui")
        .join("demo-dossier.json")
}

fn load() -> serde_json::Value {
    let raw = std::fs::read_to_string(fixture_path()).expect(
        "ui/demo-dossier.json must exist — run `cargo run -p helix-demo --example generate`",
    );
    serde_json::from_str(&raw).expect("fixture is valid JSON")
}

#[test]
fn committed_fixture_matches_generator() {
    let on_disk = std::fs::read_to_string(fixture_path()).unwrap();
    let generated = format!("{}\n", helix_demo::dossier_json_pretty());
    assert_eq!(
        on_disk, generated,
        "ui/demo-dossier.json is stale — regenerate with `cargo run -p helix-demo --example generate`"
    );
}

#[test]
fn fixture_records_deserialize_and_ground_through_the_real_pipeline() {
    let v = load();
    assert!(v["meta"]["disclaimer"]
        .as_str()
        .unwrap()
        .contains("SAMPLE / SYNTHETIC DATA"));

    let records: Vec<ProvRecord> =
        serde_json::from_value(v["records"].clone()).expect("records deserialize into ProvRecord");
    assert!(records.len() > 1000, "a rich, complete dossier");

    let now = v["now"].as_i64().unwrap();
    let ferr: Vec<ProvRecord> = records
        .iter()
        .filter(|r| r.code.as_deref() == Some("2276-4"))
        .cloned()
        .collect();

    let req = AnalyzeRequest {
        concept_code: "2276-4",
        records: &ferr,
        now,
        staleness_window_days: 365,
        confidence_floor: 0.5,
        reference_low: Some(30.0),
        reference_high: Some(400.0),
        flat_band_per_day: 0.02,
        flat_band_frac: 0.0,
    };
    let out = analyze(&req, &builtin_registry_v1()).unwrap();
    let AnswerOutcome::Answered(ans) = out else {
        panic!("the fixture must yield a grounded answer");
    };
    assert!(!ans.claims.is_empty());
    assert!(ans.claims[0].text().contains("Ferritin"));
}
