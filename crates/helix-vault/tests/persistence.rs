//! Disk-persistence integration test for the `persist` feature (ADR-001).
//!
//! Proves the two properties that make the vault a *vault*:
//!   1. Durability — records survive dropping the store and reopening from disk.
//!   2. Encrypted at rest — the on-disk file never contains plaintext fields;
//!      only the user's `SealKey` can recover them.
#![cfg(feature = "persist")]

use helix_provenance::{Confidence, MeasurementMethod, ProvRecord, RecordId};
use helix_vault::{open, seal, PersistentVaultStore, SealKey};

/// A fixed key stands in for "the user re-supplies the same key next session".
fn user_key() -> SealKey {
    SealKey::from_bytes([42u8; 32])
}

fn sample_records() -> Vec<(RecordId, ProvRecord)> {
    let mk = |id: &str, concept: &str, source: &str, value: f64, unit: &str| {
        (
            RecordId::from(id),
            ProvRecord {
                id: RecordId::from(id),
                source: source.to_string(),
                measured_at: 1_720_000_000_000,
                method: MeasurementMethod::LabFeed,
                code: None,
                concept: concept.to_string(),
                value,
                unit: unit.to_string(),
                reference_range: None,
                confidence: Confidence::FULL,
            },
        )
    };
    vec![
        mk("rec-1", "Ferritin", "Quest", 28.0, "ng/mL"),
        mk("rec-2", "Vitamin D", "Labcorp", 41.5, "ng/mL"),
        mk("rec-3", "Hemoglobin A1c", "Quest", 5.2, "%"),
    ]
}

#[test]
fn records_survive_reopen_and_disk_is_encrypted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("vault.redb");
    let key = user_key();
    let records = sample_records();

    // --- Session 1: seal & persist, then drop the store entirely. ---
    {
        let store = PersistentVaultStore::open(&path).expect("open new vault");
        for (id, rec) in &records {
            let plaintext = serde_json::to_vec(rec).expect("serialize record");
            let sealed = seal(&key, &plaintext).expect("seal");
            store.put(id, &sealed).expect("put sealed");
        }
    } // store dropped here — nothing kept in memory.

    // --- Session 2: reopen from the SAME path with the SAME key. ---
    {
        let store = PersistentVaultStore::open(&path).expect("reopen vault");

        let mut ids = store.ids().expect("list ids");
        ids.sort();
        assert_eq!(
            ids,
            vec![
                RecordId::from("rec-1"),
                RecordId::from("rec-2"),
                RecordId::from("rec-3")
            ],
            "all ids should survive reopen"
        );

        for (id, original) in &records {
            let sealed = store
                .get(id)
                .expect("get")
                .expect("record present after reopen");
            let plaintext = open(&key, &sealed).expect("open with user key");
            let recovered: ProvRecord =
                serde_json::from_slice(&plaintext).expect("deserialize record");
            assert_eq!(&recovered, original, "decrypted record must equal original");
        }

        // Wrong key cannot read a persisted record.
        let other = SealKey::from_bytes([7u8; 32]);
        let sealed = store.get(&RecordId::from("rec-1")).unwrap().unwrap();
        assert!(open(&other, &sealed).is_err(), "wrong key must fail to open");
    } // drop before reading raw bytes.

    // --- Encrypted-at-rest proof: raw file bytes contain no plaintext fields. ---
    let raw = std::fs::read(&path).expect("read raw vault file");
    let haystack = raw.as_slice();
    for needle in [
        b"Ferritin".as_slice(),
        b"Vitamin D".as_slice(),
        b"Hemoglobin".as_slice(),
        b"Quest".as_slice(),
        b"Labcorp".as_slice(),
        b"ng/mL".as_slice(),
    ] {
        assert!(
            !contains(haystack, needle),
            "plaintext {:?} must NOT appear on disk",
            String::from_utf8_lossy(needle)
        );
    }
}

/// Naive substring search over raw bytes.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
