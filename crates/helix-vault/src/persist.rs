//! Disk-backed, encrypted-at-rest vault store (ADR-001).
//!
//! Gated behind the non-default `persist` feature so the default build and any
//! wasm target never pull in `redb` or filesystem code. Like [`crate::VaultStore`],
//! this store holds **only** [`SealedRecord`] ciphertext — the on-disk bytes are
//! `nonce || ciphertext`, never plaintext [`crate::RecordId`]-tagged fields. A
//! full copy of the file (breach, backup, bankruptcy sale) yields nothing
//! readable without the user's [`crate::SealKey`].

use std::path::Path;

use redb::{ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::{RecordId, SealedRecord, VaultError, NONCE_LEN};

/// redb table: record id (string) -> sealed wire bytes (`nonce || ciphertext`).
const RECORDS: TableDefinition<&str, &[u8]> = TableDefinition::new("sealed_records");

/// Map any `redb` error into the crate's error type. We keep the message as a
/// `String` so `VaultError` stays `Clone + Eq` (redb errors are neither).
pub(crate) fn store_err<E: std::fmt::Display>(e: E) -> VaultError {
    VaultError::Storage(e.to_string())
}

/// Serialize a sealed record to its on-disk wire form: `nonce || ciphertext`.
/// The nonce is not secret; it must travel with the ciphertext to decrypt.
pub(crate) fn to_wire(sealed: &SealedRecord) -> Vec<u8> {
    let mut buf = Vec::with_capacity(NONCE_LEN + sealed.ciphertext.len());
    buf.extend_from_slice(&sealed.nonce);
    buf.extend_from_slice(&sealed.ciphertext);
    buf
}

/// Reconstruct a sealed record from its wire form. Fails if the stored bytes are
/// too short to contain a full nonce (corruption / truncation).
pub(crate) fn from_wire(bytes: &[u8]) -> Result<SealedRecord, VaultError> {
    if bytes.len() < NONCE_LEN {
        return Err(VaultError::Storage(
            "corrupt sealed record: shorter than a nonce".to_string(),
        ));
    }
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&bytes[..NONCE_LEN]);
    Ok(SealedRecord {
        nonce,
        ciphertext: bytes[NONCE_LEN..].to_vec(),
    })
}

/// A disk-backed [`crate::VaultStore`]: durable across process exits, encrypted
/// at rest, and — by construction — incapable of returning plaintext (it has no
/// `SealKey`). Reads and writes go through short redb transactions.
pub struct PersistentVaultStore {
    db: redb::Database,
}

impl PersistentVaultStore {
    /// Open the vault file at `path`, creating it if absent. The records table is
    /// materialized eagerly so [`get`](Self::get) / [`ids`](Self::ids) work on a
    /// freshly created (empty) vault.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, VaultError> {
        let db = redb::Database::create(path).map_err(store_err)?;
        let txn = db.begin_write().map_err(store_err)?;
        {
            // Opening the table for write creates it if it does not exist yet.
            txn.open_table(RECORDS).map_err(store_err)?;
        }
        txn.commit().map_err(store_err)?;
        Ok(Self { db })
    }

    /// Persist a sealed record under `id`, overwriting any existing entry.
    pub fn put(&self, id: &RecordId, sealed: &SealedRecord) -> Result<(), VaultError> {
        let wire = to_wire(sealed);
        let txn = self.db.begin_write().map_err(store_err)?;
        {
            let mut table = txn.open_table(RECORDS).map_err(store_err)?;
            table
                .insert(id.0.as_str(), wire.as_slice())
                .map_err(store_err)?;
        }
        txn.commit().map_err(store_err)?;
        Ok(())
    }

    /// Fetch the *sealed* bytes for `id`, or `None` if absent. Like the in-memory
    /// store there is deliberately no `get_plaintext`.
    pub fn get(&self, id: &RecordId) -> Result<Option<SealedRecord>, VaultError> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let table = txn.open_table(RECORDS).map_err(store_err)?;
        match table.get(id.0.as_str()).map_err(store_err)? {
            Some(guard) => Ok(Some(from_wire(guard.value())?)),
            None => Ok(None),
        }
    }

    /// List every stored record id.
    pub fn ids(&self) -> Result<Vec<RecordId>, VaultError> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let table = txn.open_table(RECORDS).map_err(store_err)?;
        let mut out = Vec::new();
        for entry in table.iter().map_err(store_err)? {
            let (key, _value) = entry.map_err(store_err)?;
            out.push(RecordId(key.value().to_string()));
        }
        Ok(out)
    }

    /// Number of stored records.
    pub fn len(&self) -> Result<usize, VaultError> {
        let txn = self.db.begin_read().map_err(store_err)?;
        let table = txn.open_table(RECORDS).map_err(store_err)?;
        Ok(table.len().map_err(store_err)? as usize)
    }

    /// Whether the vault holds no records.
    pub fn is_empty(&self) -> Result<bool, VaultError> {
        Ok(self.len()? == 0)
    }
}
