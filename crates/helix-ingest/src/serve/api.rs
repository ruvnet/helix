//! The JSON API handlers and the in-memory session state.
//!
//! The unlock passphrase is turned into a [`SealKey`] and held ONLY in
//! [`ServeState::key`] for the process lifetime — never written to disk, never
//! logged, never echoed back in a response. Every response carries counts and
//! metadata only; decrypted record values never enter a log line. All ingest
//! paths seal into the encrypted vault first, THEN emit the decrypted dossier to
//! the gitignored `<ui>/private/dossier.json` the UI reads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use base64::Engine as _;
use helix_provenance::ProvRecord;
use helix_vault::SealKey;
use serde::Deserialize;
use serde_json::{json, Value};

use super::connectors::{self, Connector};
use super::hae;
use super::Http;
use crate::{dossier, parse, vault};

/// Live session state, guarded by a `Mutex` at the server layer.
pub struct ServeState {
    pub vault_dir: PathBuf,
    pub ui_dir: PathBuf,
    /// The derived sealing key, present only while unlocked. Zeroizes on drop.
    key: Option<SealKey>,
    connectors: Vec<Connector>,
}

impl ServeState {
    /// Build state for a vault dir and the UI root to serve. Loads the persisted
    /// connector registry (cadence config) if present.
    pub fn new(vault_dir: PathBuf, ui_dir: PathBuf) -> Result<Self> {
        let connectors = connectors::load(&vault_dir);
        Ok(Self {
            vault_dir,
            ui_dir,
            key: None,
            connectors,
        })
    }

    pub fn ui_dir(&self) -> &Path {
        &self.ui_dir
    }
}

/// Epoch millis now (the only wall-clock read; used to stamp the dossier).
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn to_val<T: serde::Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap_or(Value::Null)
}

// --- GET /api/status --------------------------------------------------------

pub fn status(s: &ServeState) -> Http {
    let vault_exists = vault::is_initialized(&s.vault_dir);
    let unlocked = s.key.is_some();
    let (record_count, by_source): (usize, BTreeMap<String, usize>) = match &s.key {
        Some(k) => match vault::reopen_records(&s.vault_dir, k) {
            Ok(recs) => (recs.len(), dossier::source_counts(&recs)),
            Err(_) => (0, BTreeMap::new()),
        },
        None => (vault::count_records(&s.vault_dir).unwrap_or(0), BTreeMap::new()),
    };
    Http::json(json!({
        "vault_exists": vault_exists,
        "unlocked": unlocked,
        "record_count": record_count,
        "by_source": to_val(&by_source),
        "connectors": to_val(&s.connectors),
        "mode": "sealed",
    }))
}

// --- GET /api/connectors ----------------------------------------------------

pub fn connectors(s: &ServeState) -> Http {
    Http::json(json!({
        "connectors": to_val(&s.connectors),
        "live": connectors::LIVE_ID,
        "supported_health_metrics": to_val(&hae::supported_metrics()),
    }))
}

// --- POST /api/vault/unlock -------------------------------------------------

#[derive(Deserialize)]
struct UnlockReq {
    passphrase: String,
    #[serde(default)]
    first_time: Option<bool>,
}

pub fn unlock(s: &mut ServeState, body: &[u8]) -> Http {
    let Ok(req) = serde_json::from_slice::<UnlockReq>(body) else {
        return Http::json_status(400, json!({ "ok": false, "error": "invalid request body" }));
    };
    if req.passphrase.is_empty() {
        return Http::json_status(400, json!({ "ok": false, "error": "empty passphrase" }));
    }
    // First-time iff the caller says so OR the vault does not exist yet.
    let creating = req.first_time.unwrap_or(false) || !vault::is_initialized(&s.vault_dir);
    let key = match vault::prepare_key(&s.vault_dir, &req.passphrase) {
        Ok(k) => k,
        Err(_) => {
            return Http::json_status(500, json!({ "ok": false, "error": "key derivation failed" }))
        }
    };
    match vault::unlock(&s.vault_dir, &key, creating) {
        Ok(()) => {
            s.key = Some(key); // held in memory only
            Http::json(json!({ "ok": true, "first_time": creating }))
        }
        // Wrong passphrase on an existing vault — key is NOT retained.
        Err(_) => Http::json_status(401, json!({ "ok": false, "error": "wrong passphrase" })),
    }
}

// --- POST /api/import -------------------------------------------------------

#[derive(Deserialize)]
struct ImportReq {
    kind: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    data_base64: Option<String>,
}

pub fn import(s: &mut ServeState, body: &[u8]) -> Http {
    if s.key.is_none() {
        return Http::json_status(401, json!({ "error": "vault locked; unlock first" }));
    }
    let Ok(req) = serde_json::from_slice::<ImportReq>(body) else {
        return Http::json_status(400, json!({ "error": "invalid request body" }));
    };
    let text = match decode_payload(&req) {
        Ok(t) => t,
        Err(msg) => return Http::json_status(400, json!({ "error": msg })),
    };
    let records = match req.kind.as_str() {
        "fhir" => match parse::parse_fhir_bundle(&text, parse::FHIR_SOURCE) {
            Ok(p) => p.records,
            Err(e) => return Http::json_status(400, json!({ "error": format!("{e}") })),
        },
        "apple" => parse::parse_apple_export(&text, parse::APPLE_SOURCE).records,
        "claw" => match parse::parse_claw_envelope(&text, parse::CLAW_SOURCE) {
            Ok(p) => p.records,
            Err(e) => return Http::json_status(400, json!({ "error": format!("{e}") })),
        },
        "auto" => match import_auto(&text) {
            Ok(recs) => recs,
            Err(resp) => return resp,
        },
        other => {
            return Http::json_status(
                400,
                json!({ "error": format!(
                    "unknown kind {other:?}; expected \"auto\", \"fhir\", \"apple\", or \"claw\""
                ) }),
            )
        }
    };
    if records.is_empty() {
        return Http::json_status(422, json!({ "error": "parsed 0 usable records" }));
    }
    let imported = records.len();
    let key = s.key.as_ref().unwrap();
    match seal_and_emit(&s.vault_dir, &s.ui_dir, key, &records) {
        Ok((_total, by_source)) => Http::json(json!({
            "imported": imported,
            "by_source": to_val(&by_source),
            "sealed": true,
        })),
        Err(_) => Http::json_status(500, json!({ "error": "seal/emit failed" })),
    }
}

/// Auto-detect an uploaded file and route it to the right importer. Apple exports
/// are XML (sniffed first at the text layer); everything else is parsed as JSON
/// and classified by [`parse::detect_format`]. An unrecognized shape returns a
/// LOUD error carrying the top-level keys — NEVER a silent 0-record success.
fn import_auto(text: &str) -> Result<Vec<ProvRecord>, Http> {
    if parse::looks_like_apple_xml(text) {
        return Ok(parse::parse_apple_export(text, parse::APPLE_SOURCE).records);
    }
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        // Neither Apple XML nor valid JSON — nothing to key off of.
        return Err(Http::json_status(
            422,
            json!({ "error": "unrecognized format", "top_level_keys": [] }),
        ));
    };
    let map_err = |e: anyhow::Error| Http::json_status(400, json!({ "error": format!("{e}") }));
    match parse::detect_format(&v) {
        parse::Format::ClawEnvelope => parse::parse_claw_envelope(text, parse::CLAW_SOURCE)
            .map(|p| p.records)
            .map_err(map_err),
        parse::Format::Fhir => parse::parse_fhir_bundle(text, parse::FHIR_SOURCE)
            .map(|p| p.records)
            .map_err(map_err),
        parse::Format::AppleXml => Ok(parse::parse_apple_export(text, parse::APPLE_SOURCE).records),
        parse::Format::Unknown => Err(Http::json_status(
            422,
            json!({ "error": "unrecognized format", "top_level_keys": parse::top_level_keys(&v) }),
        )),
    }
}

// --- POST /health/ingest (the one LIVE ongoing connector) -------------------

pub fn health_ingest(s: &mut ServeState, body: &[u8]) -> Http {
    if s.key.is_none() {
        return Http::json_status(401, json!({ "error": "vault locked; unlock first" }));
    }
    let Ok(payload) = serde_json::from_slice::<Value>(body) else {
        return Http::json_status(400, json!({ "error": "invalid Health Auto Export JSON" }));
    };
    let parsed = hae::parse(&payload);
    if parsed.records.is_empty() {
        return Http::json_status(
            422,
            json!({
                "imported": 0,
                "mapped": parsed.mapped,
                "skipped": parsed.skipped,
                "error": "no supported metrics in payload",
            }),
        );
    }
    let imported = parsed.records.len();
    let key = s.key.as_ref().unwrap();
    let emitted = seal_and_emit(&s.vault_dir, &s.ui_dir, key, &parsed.records);
    match emitted {
        Ok((_total, by_source)) => {
            // Live connector: record the successful pull watermark and persist it.
            connectors::mark_pull(&mut s.connectors, connectors::LIVE_ID, now_ms());
            connectors::save(&s.vault_dir, &s.connectors);
            Http::json(json!({
                "imported": imported,
                "mapped": parsed.mapped,
                "skipped": parsed.skipped,
                "by_source": to_val(&by_source),
                "sealed": true,
            }))
        }
        Err(_) => Http::json_status(500, json!({ "error": "seal/emit failed" })),
    }
}

// --- shared ingest tail -----------------------------------------------------

/// Seal `records` into the vault, RE-OPEN the whole corpus, and write the
/// decrypted dossier to `<ui>/private/dossier.json`. Returns `(total, by_source)`.
fn seal_and_emit(
    vault_dir: &Path,
    ui_dir: &Path,
    key: &SealKey,
    records: &[ProvRecord],
) -> Result<(usize, BTreeMap<String, usize>)> {
    vault::seal_records(vault_dir, key, records)?;
    let all = vault::reopen_records(vault_dir, key)?;
    let value = dossier::build(&all, now_ms())?;
    let out = ui_dir.join("private").join("dossier.json");
    dossier::write(&value, &out)?;
    Ok((all.len(), dossier::source_counts(&all)))
}

/// Extract the uploaded file text: `data_base64` (raw base64 or a `data:` URL)
/// takes precedence, else `content` verbatim.
fn decode_payload(req: &ImportReq) -> Result<String, String> {
    if let Some(b64) = &req.data_base64 {
        // Strip a `data:<mime>;base64,` prefix if the browser sent a data URL.
        let raw = b64.rsplit_once(',').map(|(_, d)| d).unwrap_or(b64.as_str());
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw.trim())
            .map_err(|_| "data_base64 is not valid base64".to_string())?;
        return String::from_utf8(bytes).map_err(|_| "decoded file is not UTF-8".to_string());
    }
    if let Some(text) = &req.content {
        return Ok(text.clone());
    }
    Err("provide `data_base64` or `content`".to_string())
}
