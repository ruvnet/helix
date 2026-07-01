//! Emit the decrypted corpus as `dossier.json` in the exact shape the web UI
//! loads (`ui/app.js` `applyDossier`).
//!
//! The UI needs `now` + `records`; every other collection defaults to empty. Each
//! element of `records` is a serialized [`ProvRecord`], which is byte-for-byte the
//! record shape `ui/app.js` pushes via `addImportedRecords` (`method` snake_case,
//! `confidence` a bare float, `reference_range` `{low,high}` or `null`).
//!
//! `meta` is deliberately OMITTED: the UI renders a "SAMPLE DATA — not a real
//! person" banner whenever `meta` is present, which would be a dangerous mislabel
//! for a real person's decrypted PHI. Provenance for humans/tools lives under the
//! non-UI `_helix_ingest` key, which `applyDossier` ignores.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use helix_provenance::ProvRecord;
use serde_json::{json, Value};

/// Build the dossier JSON value from the decrypted records and a `now` timestamp.
pub fn build(records: &[ProvRecord], now_ms: i64) -> Result<Value> {
    let by_source = source_counts(records);
    let records_json: Value =
        serde_json::to_value(records).context("serializing records for dossier")?;

    Ok(json!({
        "now": now_ms,
        "records": records_json,
        // Present-but-empty so the schema is complete; the UI defaults these anyway.
        "medications": [],
        "notes": [],
        "timeline": [],
        "questions": [],
        "subsystems": [],
        // Non-UI provenance block (ignored by applyDossier).
        "_helix_ingest": {
            "generated_by": format!("helix-ingest {}", env!("CARGO_PKG_VERSION")),
            "record_count": records.len(),
            "by_source": by_source,
            "encryption_at_rest": "proven",
            "note": "Decrypted from the local encrypted vault by helix-ingest. \
                     Contains PHI — keep under a gitignored path; never commit."
        }
    }))
}

/// Count records per source (e.g. `{"FHIR": 3, "Apple Health": 4}`).
pub fn source_counts(records: &[ProvRecord]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for r in records {
        *m.entry(r.source.clone()).or_insert(0) += 1;
    }
    m
}

/// Write the dossier to `out` (creating the parent dir), owner-only on Unix.
pub fn write(value: &Value, out: &Path) -> Result<()> {
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating dossier dir {}", parent.display()))?;
        }
    }
    let text = serde_json::to_string_pretty(value).context("encoding dossier json")?;
    fs::write(out, text).with_context(|| format!("writing dossier {}", out.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(out, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
