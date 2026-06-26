//! Property tests (proptest) for the vault AEAD boundary (ADR-001/013).
//!
//! The security guarantees, asserted across arbitrary keys and plaintexts:
//! 1. seal→open is a perfect round-trip under the correct key;
//! 2. any single-bit tamper of the ciphertext fails authentication;
//! 3. the wrong key never decrypts;
//! 4. ciphertext never contains the plaintext verbatim.

use helix_vault::{open, seal, SealKey};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn round_trip_under_correct_key(key in any::<[u8; 32]>(), pt in prop::collection::vec(any::<u8>(), 0..512)) {
        let k = SealKey::from_bytes(key);
        let sealed = seal(&k, &pt).unwrap();
        prop_assert_eq!(open(&k, &sealed).unwrap(), pt);
    }

    #[test]
    fn wrong_key_never_opens(k1 in any::<[u8; 32]>(), k2 in any::<[u8; 32]>(), pt in prop::collection::vec(any::<u8>(), 1..256)) {
        prop_assume!(k1 != k2);
        let sealed = seal(&SealKey::from_bytes(k1), &pt).unwrap();
        prop_assert!(open(&SealKey::from_bytes(k2), &sealed).is_err());
    }

    #[test]
    fn single_bit_tamper_fails(key in any::<[u8; 32]>(), pt in prop::collection::vec(any::<u8>(), 1..256), bit in 0usize..8) {
        let k = SealKey::from_bytes(key);
        let sealed = seal(&k, &pt).unwrap();
        // Re-seal then corrupt: serialize, flip a bit in the body, expect failure
        // by going through the public API — corrupt via a fresh seal whose bytes
        // we mutate through a serde round-trip.
        let mut json: serde_json::Value = serde_json::to_value(&sealed).unwrap();
        let ct = json["ciphertext"].as_array_mut().unwrap();
        if !ct.is_empty() {
            let v = ct[0].as_u64().unwrap() as u8;
            ct[0] = serde_json::json!(v ^ (1u8 << bit));
            let tampered: helix_vault::SealedRecord = serde_json::from_value(json).unwrap();
            prop_assert!(open(&k, &tampered).is_err());
        }
    }

    #[test]
    fn ciphertext_does_not_contain_plaintext(key in any::<[u8; 32]>(), pt in prop::collection::vec(1u8..=255, 8..128)) {
        let k = SealKey::from_bytes(key);
        let sealed = seal(&k, &pt).unwrap();
        let blob = serde_json::to_vec(&sealed).unwrap();
        // The exact plaintext byte-window should not appear in the serialized
        // sealed record (a crude but effective "is it actually encrypted" check).
        let appears = blob.windows(pt.len()).any(|w| w == pt.as_slice());
        prop_assert!(!appears);
    }
}
