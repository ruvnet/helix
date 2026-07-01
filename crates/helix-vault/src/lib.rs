//! # helix-vault — ADR-001/013: User-Owned, Local-First Encrypted Vault
//!
//! Health + genomic data is the most sensitive data a person has, and the
//! 23andMe collapse is the cautionary tale: a centralized vendor vault is both a
//! breach target and a bankruptcy asset. Helix's answer is architectural — the
//! canonical corpus is **encrypted with a key only the user holds**, and the
//! company-side store is structurally unable to read it.
//!
//! This crate encodes that boundary in the type system:
//!
//! - [`SealKey`] — a 256-bit key that zeroizes on drop; the user holds it.
//! - [`seal`] / [`open`] — authenticated encryption (XChaCha20-Poly1305, a
//!   misuse-resistant AEAD with a 192-bit random nonce — no nonce-reuse footgun).
//! - [`VaultStore`] — holds **only** [`SealedRecord`] ciphertext. It exposes no
//!   method that returns plaintext. This is the "company cannot monetize or
//!   transfer the raw corpus" property (ADR-001), made un-bypassable.
//! - [`UserKeyring`] — the *only* thing that can turn sealed records back into
//!   plaintext, and it never leaves the user's device.
//!
//! Tamper-evidence comes for free: AEAD authentication means any modification to
//! the ciphertext (or use of the wrong key) fails [`open`] rather than returning
//! garbage.

use std::collections::BTreeMap;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub use helix_provenance::RecordId;

#[cfg(feature = "persist")]
mod persist;
#[cfg(feature = "persist")]
pub use persist::PersistentVaultStore;

#[cfg(feature = "persist")]
mod credentials;
#[cfg(feature = "persist")]
pub use credentials::{Credential, CredentialKind, CredentialVault};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;

/// A 256-bit vault key. Zeroized on drop so it does not linger in memory. The
/// user owns this; the company-side store never does.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SealKey([u8; KEY_LEN]);

impl SealKey {
    /// Construct from raw bytes (e.g. derived from a passphrase via a KDF —
    /// the KDF itself lives a layer up).
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        SealKey(bytes)
    }

    /// Generate a fresh random key from the OS CSPRNG.
    pub fn generate() -> Result<Self, VaultError> {
        let mut k = [0u8; KEY_LEN];
        getrandom::getrandom(&mut k).map_err(|_| VaultError::Rng)?;
        Ok(SealKey(k))
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(Key::from_slice(&self.0))
    }
}

impl core::fmt::Debug for SealKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never print key material.
        f.write_str("SealKey(***redacted***)")
    }
}

/// An opaque, encrypted record. Serializable for at-rest storage / sync, but the
/// plaintext is unrecoverable without the user's [`SealKey`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedRecord {
    nonce: [u8; NONCE_LEN],
    ciphertext: Vec<u8>,
}

impl SealedRecord {
    /// Size of the ciphertext (for storage accounting). Reveals nothing.
    pub fn len(&self) -> usize {
        self.ciphertext.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ciphertext.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VaultError {
    #[error("OS random number generator failed")]
    Rng,
    #[error("encryption failed")]
    SealFailed,
    #[error("decryption/authentication failed (wrong key or tampered ciphertext)")]
    OpenFailed,
    #[error("no record with id {0:?}")]
    NotFound(RecordId),
    /// Disk-backed store failure (open/read/write/corruption). Only present with
    /// the `persist` feature; the message is a `String` so `VaultError` stays
    /// `Clone + Eq`.
    #[cfg(feature = "persist")]
    #[error("vault storage error: {0}")]
    Storage(String),
}

/// Encrypt `plaintext` under `key` with a fresh random nonce.
pub fn seal(key: &SealKey, plaintext: &[u8]) -> Result<SealedRecord, VaultError> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce).map_err(|_| VaultError::Rng)?;
    let ciphertext = key
        .cipher()
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| VaultError::SealFailed)?;
    Ok(SealedRecord { nonce, ciphertext })
}

/// Decrypt and authenticate a [`SealedRecord`] under `key`. Fails if the key is
/// wrong or the ciphertext was modified.
pub fn open(key: &SealKey, sealed: &SealedRecord) -> Result<Vec<u8>, VaultError> {
    key.cipher()
        .decrypt(
            XNonce::from_slice(&sealed.nonce),
            sealed.ciphertext.as_ref(),
        )
        .map_err(|_| VaultError::OpenFailed)
}

/// The company-side store. Holds **only** ciphertext, keyed by record id. By
/// construction it has no `SealKey` and exposes no plaintext accessor — so even
/// a full dump of this store (breach, subpoena, bankruptcy sale) yields nothing
/// readable. That is ADR-001 as a type, not a promise.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VaultStore {
    records: BTreeMap<RecordId, SealedRecord>,
}

impl VaultStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put(&mut self, id: RecordId, sealed: SealedRecord) {
        self.records.insert(id, sealed);
    }

    /// Fetch the *sealed* bytes. There is deliberately no `get_plaintext`.
    pub fn get(&self, id: &RecordId) -> Option<&SealedRecord> {
        self.records.get(id)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// The user's keyring — held on-device only. The single capability that can turn
/// the store's ciphertext back into plaintext (ADR-013 keeps this local).
pub struct UserKeyring {
    key: SealKey,
}

impl UserKeyring {
    pub fn new(key: SealKey) -> Self {
        Self { key }
    }

    /// Seal a record and place it in the store.
    pub fn seal_into(
        &self,
        store: &mut VaultStore,
        id: RecordId,
        plaintext: &[u8],
    ) -> Result<(), VaultError> {
        let sealed = seal(&self.key, plaintext)?;
        store.put(id, sealed);
        Ok(())
    }

    /// The only path from store ciphertext back to plaintext.
    pub fn open_from(&self, store: &VaultStore, id: &RecordId) -> Result<Vec<u8>, VaultError> {
        let sealed = store
            .get(id)
            .ok_or_else(|| VaultError::NotFound(id.clone()))?;
        open(&self.key, sealed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> SealKey {
        SealKey::from_bytes([7u8; KEY_LEN])
    }

    #[test]
    fn seal_open_round_trips() {
        let k = key();
        let pt = b"ferritin 28 ng/mL Quest 2026-06";
        let sealed = seal(&k, pt).unwrap();
        assert_ne!(sealed.ciphertext, pt); // actually encrypted
        assert_eq!(open(&k, &sealed).unwrap(), pt);
    }

    #[test]
    fn wrong_key_fails_to_open() {
        let sealed = seal(&key(), b"secret").unwrap();
        let other = SealKey::from_bytes([9u8; KEY_LEN]);
        assert_eq!(open(&other, &sealed), Err(VaultError::OpenFailed));
    }

    #[test]
    fn tampered_ciphertext_fails_authentication() {
        let mut sealed = seal(&key(), b"secret").unwrap();
        sealed.ciphertext[0] ^= 0xFF; // flip a bit
        assert_eq!(open(&key(), &sealed), Err(VaultError::OpenFailed));
    }

    #[test]
    fn fresh_nonce_each_seal() {
        let k = key();
        let a = seal(&k, b"same").unwrap();
        let b = seal(&k, b"same").unwrap();
        // Same plaintext, different ciphertext (random nonce) — no determinism leak.
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ciphertext, b.ciphertext);
    }

    #[test]
    fn store_holds_only_ciphertext_and_keyring_opens() {
        let keyring = UserKeyring::new(key());
        let mut store = VaultStore::new();
        let id = RecordId::from("rec-1");
        keyring
            .seal_into(&mut store, id.clone(), b"deep sleep 58 min")
            .unwrap();

        // The store alone yields only opaque bytes...
        let sealed = store.get(&id).unwrap();
        assert!(!sealed.is_empty());
        // ...serializing the whole store never exposes plaintext.
        let dump = serde_json::to_string(&store).unwrap();
        assert!(!dump.contains("deep sleep"));

        // Only the keyring recovers it.
        let pt = keyring.open_from(&store, &id).unwrap();
        assert_eq!(pt, b"deep sleep 58 min");
    }

    #[test]
    fn missing_record_errors() {
        let keyring = UserKeyring::new(key());
        let store = VaultStore::new();
        let id = RecordId::from("nope");
        assert_eq!(
            keyring.open_from(&store, &id),
            Err(VaultError::NotFound(id))
        );
    }

    #[test]
    fn generate_key_produces_distinct_keys() {
        let k1 = SealKey::generate().unwrap();
        let k2 = SealKey::generate().unwrap();
        // Encrypt the same thing under both; ciphertext must differ.
        let a = seal(&k1, b"x").unwrap();
        let b = seal(&k2, b"x").unwrap();
        assert!(open(&k2, &a).is_err()); // k1's record won't open under k2
        assert!(open(&k1, &b).is_err());
    }

    #[test]
    fn debug_never_leaks_key() {
        let dbg = format!("{:?}", key());
        assert!(dbg.contains("redacted"));
        assert!(!dbg.contains('7'));
    }
}
