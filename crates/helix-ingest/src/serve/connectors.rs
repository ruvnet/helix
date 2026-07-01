//! The connector registry — honest status metadata, no fake "connected".
//!
//! Exactly one connector is **live** today: Apple Health, via the local push
//! endpoint `POST /health/ingest` (a Health Auto Export JSON payload). Every other
//! source is a real, named entry marked `coming_soon` — the UI can show the full
//! roadmap without ever implying a source is wired when it is not.
//!
//! Cadence defaults come straight from ADR-049 (scheduled per-source pull
//! cadences). A scheduler is NOT built here (ADR-049's launchd/cron loop is future
//! work); this module only persists the cadence config + last-pull watermark so
//! the choice survives a restart. Persisted state can only overlay `cadence` and
//! `last_pull` — `status`/`name`/`mechanism` always come from the defaults below,
//! so a tampered `connectors.json` can never forge a source to `live`.

use std::path::Path;

use serde::{Deserialize, Serialize};

const CONNECTORS_FILE: &str = "connectors.json";

/// Honest wiring state. `live` = data actually flows today; `coming_soon` = a real
/// planned source that is not yet connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorStatus {
    Live,
    ComingSoon,
}

/// One source in the registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    /// Stable machine id, e.g. `"apple_health"`.
    pub id: String,
    /// Human label, e.g. `"Apple Health"`.
    pub name: String,
    pub status: ConnectorStatus,
    /// ADR-049 default pull cadence (`daily` / `biweekly` / `monthly` / `one_time`).
    pub cadence: String,
    /// How data arrives (ADR-049 "Mechanism" column) — informational.
    pub mechanism: String,
    /// Epoch-millis of the last successful pull, or `None` if never pulled.
    pub last_pull: Option<i64>,
}

/// Id of the one live connector (Apple Health local push).
pub const LIVE_ID: &str = "apple_health";

/// The canonical registry (ADR-049 cadence defaults). Source of truth for
/// `status`/`name`/`mechanism`; persisted config may only overlay cadence/last_pull.
pub fn defaults() -> Vec<Connector> {
    let c = |id: &str, name: &str, status, cadence: &str, mech: &str| Connector {
        id: id.to_string(),
        name: name.to_string(),
        status,
        cadence: cadence.to_string(),
        mechanism: mech.to_string(),
        last_pull: None,
    };
    use ConnectorStatus::{ComingSoon, Live};
    vec![
        c(
            LIVE_ID,
            "Apple Health",
            Live,
            "daily",
            "Local push → on-device ingest (POST /health/ingest)",
        ),
        c(
            "renpho",
            "RENPHO",
            ComingSoon,
            "daily",
            "Vaulted-credential pull (ADR-045/046)",
        ),
        c(
            "quest_fhir",
            "Quest / FHIR API",
            ComingSoon,
            "monthly",
            "FHIR API or PDF/OCR (ADR-012)",
        ),
        c(
            "walgreens",
            "Walgreens",
            ComingSoon,
            "biweekly",
            "Agentic-browser scrape (ADR-046)",
        ),
        c(
            "lose_it",
            "Lose It",
            ComingSoon,
            "daily",
            "API / vaulted-credential pull",
        ),
    ]
}

/// Load the registry: defaults, with `cadence` + `last_pull` overlaid from
/// `<vault>/connectors.json` when present. Never trusts persisted status/name.
pub fn load(vault_dir: &Path) -> Vec<Connector> {
    let mut reg = defaults();
    let path = vault_dir.join(CONNECTORS_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return reg;
    };
    let Ok(saved): Result<Vec<Connector>, _> = serde_json::from_str(&text) else {
        return reg;
    };
    for entry in &mut reg {
        if let Some(s) = saved.iter().find(|s| s.id == entry.id) {
            entry.cadence = s.cadence.clone();
            entry.last_pull = s.last_pull;
        }
    }
    reg
}

/// Persist the registry to `<vault>/connectors.json` (best-effort, owner-only).
pub fn save(vault_dir: &Path, reg: &[Connector]) {
    if std::fs::create_dir_all(vault_dir).is_err() {
        return;
    }
    let path = vault_dir.join(CONNECTORS_FILE);
    if let Ok(text) = serde_json::to_string_pretty(reg) {
        if std::fs::write(&path, text).is_ok() {
            restrict(&path);
        }
    }
}

/// Stamp `last_pull` for the connector with `id` (no-op if unknown).
pub fn mark_pull(reg: &mut [Connector], id: &str, now_ms: i64) {
    if let Some(c) = reg.iter_mut().find(|c| c.id == id) {
        c.last_pull = Some(now_ms);
    }
}

fn restrict(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}
