//! Sealing records into the encrypted-at-rest vault, and PROVING it.
//!
//! The corpus store is [`helix_vault::PersistentVaultStore`] (redb +
//! XChaCha20-Poly1305). It holds *only* [`helix_vault::SealedRecord`] ciphertext
//! and has no key of its own, so the sealing key is derived here from the user's
//! passphrase (Argon2id, mirroring `helix-vault`'s credential-vault KDF) with a
//! per-vault random salt persisted alongside the store.
//!
//! On-disk layout under the `--vault` directory:
//!   * `records.redb` — sealed [`ProvRecord`]s, keyed by record id, plus one
//!     sealed verifier record that makes a wrong passphrase fail cleanly instead
//!     of silently sealing under the wrong key.
//!   * `salt`         — the 16-byte Argon2id salt (NOT secret; it must persist so
//!     the same passphrase re-derives the same key on re-open).
//!
//! IMPORTANT at-rest note: redb *keys* are the record ids in plaintext. Our ids
//! embed source + LOINC code + timestamp, so those leak as metadata. The sealed
//! *value* — concept label, numeric value, unit, reference range — does not. The
//! ciphertext-at-rest proof therefore asserts the concept/unit are absent from
//! the raw file (asserting the LOINC code would be wrong: it is in the key).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use helix_provenance::{ProvRecord, RecordId};
use helix_vault::{open, seal, PersistentVaultStore, SealKey};

const RECORDS_DB: &str = "records.redb";
const SALT_FILE: &str = "salt";
const SALT_LEN: usize = 16;
/// Reserved id prefix for internal (non-corpus) records; excluded from the dossier.
const RESERVED_PREFIX: &str = "__helix_ingest";
const VERIFIER_ID: &str = "__helix_ingest_verifier__";
const VERIFIER_PLAINTEXT: &[u8] = b"helix-ingest-verifier-v1";

/// Path of the encrypted redb store inside a vault directory.
pub fn records_db_path(dir: &Path) -> PathBuf {
    dir.join(RECORDS_DB)
}

fn salt_path(dir: &Path) -> PathBuf {
    dir.join(SALT_FILE)
}

/// Whether this vault directory has already been initialized (salt present).
/// Used by the CLI to decide whether to confirm a freshly chosen passphrase.
pub fn is_initialized(dir: &Path) -> bool {
    salt_path(dir).is_file()
}

/// Best-effort tighten to owner-only (0600) on Unix; no-op elsewhere.
fn restrict(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Load the per-vault salt, generating and persisting one on first use.
fn load_or_create_salt(dir: &Path) -> Result<Vec<u8>> {
    fs::create_dir_all(dir).with_context(|| format!("creating vault dir {}", dir.display()))?;
    let path = salt_path(dir);
    if path.is_file() {
        let salt = fs::read(&path).with_context(|| format!("reading salt {}", path.display()))?;
        if salt.len() != SALT_LEN {
            bail!("corrupt vault salt at {} (unexpected length)", path.display());
        }
        Ok(salt)
    } else {
        let mut salt = [0u8; SALT_LEN];
        getrandom::getrandom(&mut salt).map_err(|_| anyhow!("OS RNG failed generating salt"))?;
        fs::write(&path, salt).with_context(|| format!("writing salt {}", path.display()))?;
        restrict(&path);
        Ok(salt.to_vec())
    }
}

/// Derive the 256-bit sealing key from a passphrase + salt via Argon2id. Mirrors
/// `helix-vault`'s private credential-vault KDF (same algorithm/params) so the
/// two agree on key material. The key zeroizes on drop (see [`SealKey`]).
pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<SealKey> {
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default());
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow!("argon2 key derivation failed: {e}"))?;
    Ok(SealKey::from_bytes(key))
}

/// Prepare the key for a vault directory: load/create salt, then derive.
pub fn prepare_key(dir: &Path, passphrase: &str) -> Result<SealKey> {
    let salt = load_or_create_salt(dir)?;
    derive_key(passphrase, &salt)
}

/// Open the store and enforce the passphrase against the sealed verifier.
/// * existing verifier → must open to the known plaintext, else the passphrase is
///   wrong (or the file was tampered) → hard error, nothing is written.
/// * no verifier + `init` → seal and store one now (fresh vault).
fn open_verified(dir: &Path, key: &SealKey, init: bool) -> Result<PersistentVaultStore> {
    let store = PersistentVaultStore::open(records_db_path(dir))
        .with_context(|| format!("opening vault store in {}", dir.display()))?;
    let vid = RecordId::from(VERIFIER_ID);
    match store.get(&vid).context("reading vault verifier")? {
        Some(sealed) => {
            let pt = open(key, &sealed)
                .map_err(|_| anyhow!("wrong passphrase (vault verifier failed to open)"))?;
            if pt != VERIFIER_PLAINTEXT {
                bail!("vault verifier mismatch — refusing to touch this vault");
            }
        }
        None if init => {
            let sealed = seal(key, VERIFIER_PLAINTEXT).context("sealing verifier")?;
            store.put(&vid, &sealed).context("writing verifier")?;
        }
        None => bail!("vault is not initialized (missing verifier)"),
    }
    Ok(store)
}

/// Seal every record and persist it under its own id. Verifies (or, for a fresh
/// vault, establishes) the passphrase first, so a mistyped passphrase on an
/// existing vault can never mix keys and corrupt the corpus.
pub fn seal_records(dir: &Path, key: &SealKey, records: &[ProvRecord]) -> Result<()> {
    let store = open_verified(dir, key, true)?;
    for rec in records {
        let plaintext = serde_json::to_vec(rec).context("serializing record before sealing")?;
        let sealed = seal(key, &plaintext).context("sealing record")?;
        store.put(&rec.id, &sealed).context("storing sealed record")?;
    }
    restrict(&records_db_path(dir)); // best-effort; harmless if the file is open
    Ok(())
}

/// Establish (`init = true`) or verify (`init = false`) the passphrase against the
/// vault verifier WITHOUT sealing any corpus records. Used by the companion
/// server's unlock endpoint: `Ok(())` iff the passphrase is correct (existing
/// vault) or the vault was freshly initialized. A wrong passphrase on an existing
/// vault is a hard error, so the caller must NOT retain the key on `Err`.
pub fn unlock(dir: &Path, key: &SealKey, init: bool) -> Result<()> {
    open_verified(dir, key, init)?;
    Ok(())
}

/// Count corpus records (non-reserved ids) WITHOUT decrypting — usable while the
/// vault is locked (no key held). Returns 0 if the store does not exist yet.
pub fn count_records(dir: &Path) -> Result<usize> {
    let path = records_db_path(dir);
    if !path.is_file() {
        return Ok(0);
    }
    let store = PersistentVaultStore::open(&path)
        .with_context(|| format!("opening vault store in {}", dir.display()))?;
    let mut n = 0usize;
    for id in store.ids().context("listing vault ids")? {
        if !id.0.starts_with(RESERVED_PREFIX) {
            n += 1;
        }
    }
    Ok(n)
}

/// Re-open the store FRESH (proving durability across a close) and decrypt every
/// corpus record back out. Internal/reserved records are skipped.
pub fn reopen_records(dir: &Path, key: &SealKey) -> Result<Vec<ProvRecord>> {
    let store = open_verified(dir, key, false)?;
    let mut out = Vec::new();
    for id in store.ids().context("listing vault ids")? {
        if id.0.starts_with(RESERVED_PREFIX) {
            continue;
        }
        let sealed = store
            .get(&id)
            .context("reading sealed record")?
            .ok_or_else(|| anyhow!("record {:?} vanished between list and read", id))?;
        let plaintext = open(key, &sealed)
            .map_err(|_| anyhow!("failed to decrypt record {:?} (wrong key?)", id))?;
        let rec: ProvRecord =
            serde_json::from_slice(&plaintext).context("deserializing decrypted record")?;
        out.push(rec);
    }
    Ok(out)
}

/// True iff `needle` occurs as a contiguous byte run in `haystack`.
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// PROVE encryption-at-rest: read the raw store bytes and assert that not one of
/// the plaintext `markers` (concept labels / unit strings drawn from the sealed
/// payloads) appears. Any hit means a payload leaked in the clear → hard error.
/// Returns the number of markers checked.
pub fn prove_ciphertext_at_rest(dir: &Path, markers: &[String]) -> Result<usize> {
    let path = records_db_path(dir);
    let bytes = fs::read(&path).with_context(|| format!("reading raw vault {}", path.display()))?;
    let mut checked = 0usize;
    for m in markers {
        let m = m.trim();
        if m.len() < 3 {
            continue; // too short to be a meaningful marker
        }
        if contains_bytes(&bytes, m.as_bytes()) {
            bail!(
                "ENCRYPTION-AT-REST VIOLATION: a plaintext payload marker was found \
                 in the raw vault file. Refusing to certify."
            );
        }
        checked += 1;
    }
    Ok(checked)
}
