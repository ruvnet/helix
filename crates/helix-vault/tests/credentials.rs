//! CredentialVault integration test (ADR-001/013) — `persist` feature only.
//!
//! Per-source login credentials are the keys to a user's other health silos, so
//! they get the same vault treatment as the corpus itself. This test proves the
//! three properties that make it a *vault*:
//!   1. Round-trip — credentials sealed under a master passphrase survive a full
//!      drop + reopen with the SAME passphrase.
//!   2. Wrong passphrase is rejected cleanly (returns `Err`, never a panic).
//!   3. Encrypted at rest — no fake secret value (or the passphrase) ever appears
//!      in the raw on-disk bytes.
//!
//! NOTE: every credential here is fake test data — no real login is involved.
#![cfg(feature = "persist")]

use std::collections::BTreeMap;

use helix_vault::{Credential, CredentialKind, CredentialVault};

const PASSPHRASE: &str = "correct horse";
const WRONG_PASSPHRASE: &str = "battery staple";

// Obvious fakes — never real credentials.
const FAKE_WALGREENS_PASSWORD: &str = "hunter2-not-a-real-password";
const FAKE_RENPHO_PASSWORD: &str = "s3krit-renpho-placeholder";
const FAKE_USERNAME: &str = "fakeuser@example.com";

fn fake_credentials() -> Vec<Credential> {
    let mut walgreens = BTreeMap::new();
    walgreens.insert("username".to_string(), FAKE_USERNAME.to_string());
    walgreens.insert("password".to_string(), FAKE_WALGREENS_PASSWORD.to_string());

    let mut renpho = BTreeMap::new();
    renpho.insert("username".to_string(), "renpho-fake@example.com".to_string());
    renpho.insert("password".to_string(), FAKE_RENPHO_PASSWORD.to_string());

    vec![
        Credential {
            source: "walgreens".to_string(),
            kind: CredentialKind::Password,
            fields: walgreens,
            updated_at_unix: 1_720_000_000,
        },
        Credential {
            source: "renpho".to_string(),
            kind: CredentialKind::Password,
            fields: renpho,
            updated_at_unix: 1_720_000_100,
        },
    ]
}

#[test]
fn credentials_round_trip_reject_wrong_pass_and_encrypt_at_rest() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("credentials.redb");
    let creds = fake_credentials();

    // --- Session 1: create with the master passphrase, store 2 creds, drop. ---
    {
        let vault = CredentialVault::create(&path, PASSPHRASE).expect("create vault");
        for c in &creds {
            vault.put(c).expect("put credential");
        }
        // Debug must never leak secret field VALUES.
        let dbg = format!("{:?}", &creds[0]);
        assert!(
            !dbg.contains(FAKE_WALGREENS_PASSWORD),
            "Debug must redact secret field values, got: {dbg}"
        );
    } // vault dropped — nothing kept in memory.

    // --- Session 2: reopen with the SAME passphrase; both come back equal. ---
    {
        let vault = CredentialVault::open(&path, PASSPHRASE).expect("reopen with right pass");

        let mut sources = vault.sources().expect("list sources");
        sources.sort();
        assert_eq!(sources, vec!["renpho".to_string(), "walgreens".to_string()]);

        for original in &creds {
            let got = vault
                .get(&original.source)
                .expect("get")
                .expect("credential present after reopen");
            assert_eq!(&got, original, "decrypted credential must equal original");
        }

        // Unknown source is a clean `None`, not an error.
        assert!(vault.get("does-not-exist").expect("get missing").is_none());
    }

    // --- Wrong passphrase must fail cleanly (Err, no panic). ---
    {
        let result = CredentialVault::open(&path, WRONG_PASSPHRASE);
        assert!(result.is_err(), "wrong passphrase must return Err");
    }

    // --- Encrypted-at-rest: raw file bytes contain NONE of the fake secrets. ---
    let raw = std::fs::read(&path).expect("read raw vault file");
    for needle in [
        FAKE_WALGREENS_PASSWORD.as_bytes(),
        FAKE_RENPHO_PASSWORD.as_bytes(),
        FAKE_USERNAME.as_bytes(),
        PASSPHRASE.as_bytes(),
    ] {
        assert!(
            !contains(&raw, needle),
            "secret {:?} must NOT appear on disk",
            String::from_utf8_lossy(needle)
        );
    }
}

/// Naive substring search over raw bytes.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}
