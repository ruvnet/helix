//! Passphrase-unlocked credential vault (ADR-001/013).
//!
//! Per-source login credentials (the keys to a user's *other* health silos) are
//! given the same at-rest guarantee as the corpus: on disk they are nothing but
//! [`crate::SealedRecord`] ciphertext. The difference from [`PersistentVaultStore`]
//! is how the key is obtained — a master passphrase is stretched through
//! **Argon2id** with a per-vault random salt, so no derived key, plaintext field,
//! or passphrase is ever written to disk.
//!
//! Gated behind the non-default `persist` feature (shares the redb backend, and
//! adds `argon2` + `serde_json`), so the default build and any wasm target never
//! compile the KDF or filesystem code.
//!
//! [`PersistentVaultStore`]: crate::PersistentVaultStore

use std::collections::BTreeMap;
use std::path::Path;

use argon2::{Algorithm, Argon2, Params, Version};
use redb::{ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::persist::{from_wire, store_err, to_wire};
use crate::{open, seal, SealKey, VaultError, KEY_LEN};

/// redb table: single-vault metadata (salt, passphrase verifier).
const META: TableDefinition<&str, &[u8]> = TableDefinition::new("credential_vault_meta");
/// redb table: source name -> sealed credential wire bytes (`nonce || ciphertext`).
const CREDS: TableDefinition<&str, &[u8]> = TableDefinition::new("credentials");

const SALT_KEY: &str = "salt";
const VERIFIER_KEY: &str = "verifier";
/// Sealed at create time; re-opened at unlock to detect a wrong passphrase. Not
/// secret — its only job is to make a bad key fail AEAD authentication cleanly.
const VERIFIER_PLAINTEXT: &[u8] = b"helix-credential-vault-verifier-v1";
/// 128-bit salt — comfortably above Argon2's minimum, and unique per vault.
const SALT_LEN: usize = 16;

/// What kind of secret a [`Credential`] holds. Governs how a consumer should use
/// the `fields`, not how they are stored (all kinds are sealed identically).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialKind {
    Password,
    OAuthToken,
    ApiKey,
}

/// A single per-source login credential. `fields` is deliberately open-ended
/// (e.g. `username`/`password`, or `access_token`/`refresh_token`) so different
/// [`CredentialKind`]s can carry their own shape.
///
/// Its [`Debug`] is **redacted**: field *values* are secret (passwords, tokens)
/// and are never printed — mirroring how [`crate::SealKey`] hides key material.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    pub source: String,
    pub kind: CredentialKind,
    pub fields: BTreeMap<String, String>,
    pub updated_at_unix: u64,
}

impl core::fmt::Debug for Credential {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Field NAMES (e.g. "username", "password") are not secret, but their
        // VALUES are — redact every value so a stray `{:?}` never leaks a secret.
        let redacted: BTreeMap<&str, &str> = self
            .fields
            .keys()
            .map(|k| (k.as_str(), "***redacted***"))
            .collect();
        f.debug_struct("Credential")
            .field("source", &self.source)
            .field("kind", &self.kind)
            .field("fields", &redacted)
            .field("updated_at_unix", &self.updated_at_unix)
            .finish()
    }
}

/// Derive a 32-byte [`SealKey`] from a master passphrase + per-vault salt using
/// Argon2id (the side-channel- and GPU-resistant variant). The derived key lives
/// only in memory and zeroizes on drop; it is never persisted.
fn derive_key(passphrase: &str, salt: &[u8]) -> Result<SealKey, VaultError> {
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default());
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| VaultError::Storage(format!("argon2 key derivation failed: {e}")))?;
    Ok(SealKey::from_bytes(key))
}

/// A disk-backed vault of per-source login credentials, unlocked by a master
/// passphrase. Like [`PersistentVaultStore`] it holds **only** ciphertext; unlike
/// it, the [`SealKey`] is derived from a passphrase rather than supplied directly.
///
/// A full copy of the file (breach, backup, bankruptcy sale) yields nothing
/// readable without the passphrase — that is ADR-001 as a type, not a promise.
///
/// [`PersistentVaultStore`]: crate::PersistentVaultStore
pub struct CredentialVault {
    db: redb::Database,
    key: SealKey,
}

impl CredentialVault {
    /// Create a new vault at `path`: generate a random salt, derive the key via
    /// Argon2id, and persist the (plaintext) salt plus a sealed verifier token.
    /// No passphrase, derived key, or credential is written yet.
    pub fn create(path: impl AsRef<Path>, passphrase: &str) -> Result<Self, VaultError> {
        let mut salt = [0u8; SALT_LEN];
        getrandom::getrandom(&mut salt).map_err(|_| VaultError::Rng)?;
        let key = derive_key(passphrase, &salt)?;
        let verifier = to_wire(&seal(&key, VERIFIER_PLAINTEXT)?);

        let db = redb::Database::create(path).map_err(store_err)?;
        let txn = db.begin_write().map_err(store_err)?;
        {
            let mut meta = txn.open_table(META).map_err(store_err)?;
            meta.insert(SALT_KEY, salt.as_slice()).map_err(store_err)?;
            meta.insert(VERIFIER_KEY, verifier.as_slice())
                .map_err(store_err)?;
        }
        {
            // Materialize the credentials table so `get`/`sources` work on an
            // otherwise-empty, freshly created vault.
            txn.open_table(CREDS).map_err(store_err)?;
        }
        txn.commit().map_err(store_err)?;
        Ok(Self { db, key })
    }

    /// Open an existing vault at `path`, re-deriving the key from `passphrase` and
    /// the stored salt. A **wrong passphrase** derives the wrong key, which fails
    /// to open the sealed verifier — returning [`VaultError::OpenFailed`] cleanly,
    /// never a panic.
    pub fn open(path: impl AsRef<Path>, passphrase: &str) -> Result<Self, VaultError> {
        let db = redb::Database::create(path).map_err(store_err)?;

        let (salt, verifier_wire) = {
            let txn = db.begin_read().map_err(store_err)?;
            let meta = txn.open_table(META).map_err(store_err)?;
            let salt = meta
                .get(SALT_KEY)
                .map_err(store_err)?
                .map(|g| g.value().to_vec())
                .ok_or_else(|| VaultError::Storage("vault missing salt (not created?)".into()))?;
            let verifier = meta
                .get(VERIFIER_KEY)
                .map_err(store_err)?
                .map(|g| g.value().to_vec())
                .ok_or_else(|| VaultError::Storage("vault missing verifier token".into()))?;
            (salt, verifier)
        };

        let key = derive_key(passphrase, &salt)?;

        // Verify the passphrase by opening the sealed verifier. Wrong passphrase
        // => wrong key => AEAD authentication fails => clean `Err`.
        let recovered = open(&key, &from_wire(&verifier_wire)?)?;
        if recovered != VERIFIER_PLAINTEXT {
            return Err(VaultError::OpenFailed);
        }

        Ok(Self { db, key })
    }

    /// Seal `cred` under the vault key and store it, keyed by `cred.source`
    /// (overwriting any existing entry for that source).
    pub fn put(&self, cred: &Credential) -> Result<(), VaultError> {
        let plaintext = serde_json::to_vec(cred).map_err(|e| VaultError::Storage(e.to_string()))?;
        let wire = to_wire(&seal(&self.key, &plaintext)?);

        let txn = self.db.begin_write().map_err(store_err)?;
        {
            let mut table = txn.open_table(CREDS).map_err(store_err)?;
            table
                .insert(cred.source.as_str(), wire.as_slice())
                .map_err(store_err)?;
        }
        txn.commit().map_err(store_err)?;
        Ok(())
    }

    /// Fetch and open the credential for `source`, or `None` if absent.
    pub fn get(&self, source: &str) -> Result<Option<Credential>, VaultError> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let table = txn.open_table(CREDS).map_err(store_err)?;
        match table.get(source).map_err(store_err)? {
            Some(guard) => {
                let plaintext = open(&self.key, &from_wire(guard.value())?)?;
                let cred = serde_json::from_slice(&plaintext)
                    .map_err(|e| VaultError::Storage(e.to_string()))?;
                Ok(Some(cred))
            }
            None => Ok(None),
        }
    }

    /// List every stored source name (credential keys), in redb key order.
    pub fn sources(&self) -> Result<Vec<String>, VaultError> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let table = txn.open_table(CREDS).map_err(store_err)?;
        let mut out = Vec::new();
        for entry in table.iter().map_err(store_err)? {
            let (key, _value) = entry.map_err(store_err)?;
            out.push(key.value().to_string());
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_secret_field_values() {
        let mut fields = BTreeMap::new();
        fields.insert("username".to_string(), "alice".to_string());
        fields.insert("password".to_string(), "super-secret-value".to_string());
        let cred = Credential {
            source: "example".to_string(),
            kind: CredentialKind::Password,
            fields,
            updated_at_unix: 0,
        };
        let dbg = format!("{cred:?}");
        assert!(dbg.contains("username")); // field NAMES are fine to show
        assert!(dbg.contains("***redacted***"));
        assert!(!dbg.contains("super-secret-value")); // VALUES never leak
        assert!(!dbg.contains("alice"));
    }
}
