//! # helix-ingest — the piece that finally *uses* the vault.
//!
//! Parse real health-data files through the already-tested `helix-connect`
//! importers → seal every [`helix_provenance::ProvRecord`] into the encrypted
//! `helix-vault` store → RE-OPEN the store fresh and decrypt them back (proving
//! the round-trip) → PROVE encryption-at-rest by asserting sealed payload markers
//! are absent from the raw file → emit a local, gitignored `dossier.json` in the
//! exact shape the web UI consumes.
//!
//! This is also the schedulable unit for future "ongoing" pulls: one run = parse,
//! seal, verify, emit.
//!
//! The passphrase is supplied by the caller ([`RunArgs::passphrase`]); the binary
//! sources it ONLY from `HELIX_VAULT_PASSPHRASE` or an interactive `rpassword`
//! prompt — never a CLI flag, never logged.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use helix_provenance::ProvRecord;

pub mod dossier;
pub mod parse;
pub mod serve;
pub mod vault;

pub use parse::{APPLE_SOURCE, FHIR_SOURCE};

/// Inputs to one ingest run. Both sources are optional but at least one is required.
pub struct RunArgs<'a> {
    pub fhir: Option<&'a Path>,
    pub apple: Option<&'a Path>,
    pub vault_dir: &'a Path,
    pub out: &'a Path,
    pub passphrase: &'a str,
    /// "Now" (epoch millis) written into the dossier for the UI's trend math.
    pub now_ms: i64,
}

/// What a run produced — counts and metadata only. Deliberately carries **no**
/// record values, so a caller can print it without leaking PHI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub record_count: usize,
    pub queued_for_review: usize,
    pub by_source: BTreeMap<String, usize>,
    pub markers_checked: usize,
    pub vault_records_path: PathBuf,
    pub out_path: PathBuf,
    pub encryption_at_rest_proven: bool,
}

/// Execute the full ingest pipeline. Returns a PHI-free [`RunReport`].
pub fn run(args: RunArgs) -> Result<RunReport> {
    if args.fhir.is_none() && args.apple.is_none() {
        bail!("no source given: pass --fhir <path.json> and/or --apple <export.xml>");
    }
    if args.passphrase.is_empty() {
        bail!("empty passphrase");
    }

    // 1. Parse file(s) → records via the tested importers.
    let mut records = Vec::new();
    let mut queued_for_review = 0usize;
    if let Some(path) = args.fhir {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading FHIR file {}", path.display()))?;
        let parsed = parse::parse_fhir_bundle(&text, FHIR_SOURCE)?;
        queued_for_review += parsed.queued_for_review;
        records.extend(parsed.records);
    }
    if let Some(path) = args.apple {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading Apple export {}", path.display()))?;
        let parsed = parse::parse_apple_export(&text, APPLE_SOURCE);
        queued_for_review += parsed.queued_for_review;
        records.extend(parsed.records);
    }
    if records.is_empty() {
        bail!("parsed 0 usable records ({queued_for_review} held for review); nothing to seal");
    }

    // Plaintext markers for the at-rest proof: the concept labels and units that
    // live ONLY inside the sealed payload (NOT the LOINC code, which is in the id).
    let markers = payload_markers(&records);

    // 2. Derive the key from the passphrase and seal every record.
    let key = vault::prepare_key(args.vault_dir, args.passphrase)?;
    vault::seal_records(args.vault_dir, &key, &records)?;

    // 3. RE-OPEN fresh and decrypt back — proves the round-trip across a close.
    let reopened = vault::reopen_records(args.vault_dir, &key)?;
    if reopened.len() < records.len() {
        bail!(
            "round-trip lost records: sealed {}, recovered {}",
            records.len(),
            reopened.len()
        );
    }

    // 4. PROVE ciphertext-at-rest against the raw file bytes.
    let markers_checked = vault::prove_ciphertext_at_rest(args.vault_dir, &markers)?;

    // 5. Emit the decrypted records as dossier.json in the UI schema.
    let value = dossier::build(&reopened, args.now_ms)?;
    dossier::write(&value, args.out)?;

    Ok(RunReport {
        record_count: reopened.len(),
        queued_for_review,
        by_source: dossier::source_counts(&reopened),
        markers_checked,
        vault_records_path: vault::records_db_path(args.vault_dir),
        out_path: args.out.to_path_buf(),
        encryption_at_rest_proven: markers_checked > 0,
    })
}

/// Distinct concept labels + units from the records — the sealed-payload strings
/// used to prove nothing leaked. Deduplicated; empties/too-short dropped downstream.
fn payload_markers(records: &[ProvRecord]) -> Vec<String> {
    use std::collections::BTreeSet;
    let mut set = BTreeSet::new();
    for r in records {
        set.insert(r.concept.clone());
        if !r.unit.is_empty() {
            set.insert(r.unit.clone());
        }
    }
    set.into_iter().collect()
}
