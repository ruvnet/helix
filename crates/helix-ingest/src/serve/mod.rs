//! `helix-ingest serve` — a localhost companion server for guided onboarding.
//!
//! # Security invariants (ADR-057 Sealed mode) — non-negotiable
//! * **Loopback only.** The bind host is hard-coded to `127.0.0.1`; only the port
//!   is user-settable. [`serve`] additionally re-asserts the bound address is a
//!   loopback IP at runtime and refuses to run otherwise. See the `binds_loopback`
//!   test.
//! * **No outbound calls.** This module opens a listening socket and nothing else
//!   — there is no HTTP client here and no dependency that makes one.
//! * **Cross-origin refused (CSRF / DNS-rebinding).** Every request is guarded
//!   before routing: the `Host` must be a loopback authority (a rebound request
//!   carries the attacker's hostname, so this defeats DNS rebinding) and any
//!   `Origin` present must be a loopback origin (defeats a cross-site `fetch`).
//!   See [`header_guard`]. The app's own same-origin requests always pass.
//! * **Passphrase in memory only.** Held in [`api::ServeState`], never logged.
//! * **PHI stays out of logs.** Only counts/paths/status are printed.
//! * **Dossier is gitignored.** Written to `<ui>/private/dossier.json`.
//!
//! The server is synchronous (`tiny_http`) with a small fixed worker pool sharing
//! one `Arc<Server>` and an `Arc<Mutex<ServeState>>`.

pub mod api;
pub mod connectors;
pub mod hae;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{anyhow, bail, Result};
use serde_json::json;
use tiny_http::{Header, Method, Request, Response, Server};

use api::ServeState;

/// The loopback host. There is deliberately NO flag to change this (ADR-057).
const LOOPBACK_HOST: &str = "127.0.0.1";
/// Worker threads serving the single shared listener (browsers open a few
/// parallel connections for assets; more than one worker avoids head-of-line).
const WORKERS: usize = 4;

/// Configuration for [`serve`].
pub struct ServeConfig {
    /// Port to bind on `127.0.0.1`.
    pub port: u16,
    /// Vault directory (encrypted redb store + salt), created on first unlock.
    pub vault_dir: PathBuf,
    /// UI root served statically; `/` maps to `hybrid.html` under it.
    pub ui_dir: PathBuf,
}

/// A fully-formed HTTP response body (status + content-type + bytes). Kept
/// separate from socket I/O so the whole router is unit-testable via [`dispatch`].
pub struct Http {
    pub status: u16,
    pub ctype: &'static str,
    pub body: Vec<u8>,
}

impl Http {
    pub(crate) fn json(v: serde_json::Value) -> Http {
        Self::json_status(200, v)
    }
    pub(crate) fn json_status(status: u16, v: serde_json::Value) -> Http {
        Http {
            status,
            ctype: "application/json",
            body: serde_json::to_vec(&v).unwrap_or_default(),
        }
    }
    pub(crate) fn text(status: u16, msg: &str) -> Http {
        Http {
            status,
            ctype: "text/plain; charset=utf-8",
            body: msg.as_bytes().to_vec(),
        }
    }
}

/// Bind the loopback listener on `port`. Host is fixed to `127.0.0.1`.
fn bind(port: u16) -> Result<Server> {
    let addr = format!("{LOOPBACK_HOST}:{port}");
    Server::http(addr.as_str()).map_err(|e| anyhow!("failed to bind {addr}: {e}"))
}

/// Start the companion server and block serving requests (loopback only).
pub fn serve(cfg: ServeConfig) -> Result<()> {
    let server = bind(cfg.port)?;

    // Defense in depth: refuse to run if the bound address is somehow non-loopback.
    let bound = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| anyhow!("server bound to a non-IP address"))?;
    if !bound.ip().is_loopback() {
        bail!("refusing to serve on non-loopback address {bound}");
    }

    println!("helix-ingest serve: http://{bound}  (loopback only — no outbound, Sealed mode)");
    println!("  vault: {}", cfg.vault_dir.display());
    println!("  ui:    {}  (open http://{bound}/ )", cfg.ui_dir.display());

    let state = Arc::new(Mutex::new(ServeState::new(cfg.vault_dir, cfg.ui_dir)?));
    let server = Arc::new(server);

    let mut handles = Vec::with_capacity(WORKERS);
    for _ in 0..WORKERS {
        let server = Arc::clone(&server);
        let state = Arc::clone(&state);
        handles.push(thread::spawn(move || worker_loop(&server, &state)));
    }
    for h in handles {
        let _ = h.join();
    }
    Ok(())
}

/// One worker: receive → read body → dispatch → respond, forever.
fn worker_loop(server: &Server, state: &Mutex<ServeState>) {
    loop {
        let mut req = match server.recv() {
            Ok(r) => r,
            Err(_) => break,
        };
        // CSRF / DNS-rebinding guard (ADR-057 Sealed mode): validate Host/Origin on
        // EVERY request before routing. The app's own same-origin requests pass; a
        // malicious page (cross-site fetch, or a host that rebinds to 127.0.0.1)
        // does not. Headers are copied out before the body is read.
        let host = req
            .headers()
            .iter()
            .find(|h| h.field.equiv("Host"))
            .map(|h| h.value.as_str().to_string());
        let origin = req
            .headers()
            .iter()
            .find(|h| h.field.equiv("Origin"))
            .map(|h| h.value.as_str().to_string());
        if let Some(rej) = header_guard(host.as_deref(), origin.as_deref()) {
            respond(req, rej);
            continue;
        }
        let method = method_str(req.method());
        let url = req.url().to_string();
        let body = read_body(&mut req);
        let resp = dispatch(state, method, &url, &body);
        respond(req, resp);
    }
}

fn method_str(m: &Method) -> &'static str {
    match m {
        Method::Get => "GET",
        Method::Post => "POST",
        Method::Head => "HEAD",
        _ => "OTHER",
    }
}

fn read_body(req: &mut Request) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = req.as_reader().read_to_end(&mut buf);
    buf
}

fn respond(req: Request, resp: Http) {
    let header = Header::from_bytes(&b"Content-Type"[..], resp.ctype.as_bytes())
        .expect("static content-type header is valid");
    let out = Response::from_data(resp.body)
        .with_status_code(resp.status)
        .with_header(header);
    let _ = req.respond(out);
}

/// Reject cross-origin and DNS-rebinding requests before any route runs (CSRF
/// hardening for the ADR-057 Sealed-mode companion). The `Host` must be a loopback
/// authority — a rebound attacker request carries its own hostname, so this defeats
/// DNS rebinding — and any `Origin` present must be a loopback http(s) origin, so a
/// cross-site `fetch()` is refused. The app's own requests satisfy both. Returns
/// `Some(403)` to reject, `None` to allow.
fn header_guard(host: Option<&str>, origin: Option<&str>) -> Option<Http> {
    match host {
        Some(h) if is_loopback_authority(h) => {}
        _ => {
            return Some(Http::json_status(
                403,
                json!({ "error": "forbidden: request Host is not loopback" }),
            ))
        }
    }
    if let Some(o) = origin {
        if !is_allowed_origin(o) {
            return Some(Http::json_status(
                403,
                json!({ "error": "forbidden: cross-origin request refused" }),
            ));
        }
    }
    None
}

/// True iff `authority` (`host` or `host:port`, including `[::1]:port`) is loopback.
fn is_loopback_authority(authority: &str) -> bool {
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or("") // IPv6 literal: [::1]:port
    } else {
        authority.split(':').next().unwrap_or("")
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// True iff `origin` is an `http`/`https` origin whose authority is loopback.
/// `"null"`, `file://`, and remote origins all return false.
fn is_allowed_origin(origin: &str) -> bool {
    match origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    {
        Some(authority) => is_loopback_authority(authority),
        None => false,
    }
}

/// Route a request to a handler and produce the response. Pure w.r.t. sockets:
/// takes the method, url and body; returns an [`Http`]. This is the seam the
/// integration tests drive directly.
pub fn dispatch(state: &Mutex<ServeState>, method: &str, url: &str, body: &[u8]) -> Http {
    let path = url.split('?').next().unwrap_or("/");
    match (method, path) {
        ("GET", "/api/status") => api::status(&state.lock().unwrap()),
        ("GET", "/api/connectors") => api::connectors(&state.lock().unwrap()),
        ("POST", "/api/vault/unlock") => api::unlock(&mut state.lock().unwrap(), body),
        ("POST", "/api/import") => api::import(&mut state.lock().unwrap(), body),
        ("POST", "/health/ingest") => api::health_ingest(&mut state.lock().unwrap(), body),
        ("GET", _) | ("HEAD", _) => static_file(&state.lock().unwrap(), path),
        _ => Http::json_status(404, json!({ "error": "not found" })),
    }
}

/// Serve a file from the UI root. `/` → `hybrid.html`. Path traversal is blocked
/// by rejecting `..` and confirming the canonical path stays under the UI root.
fn static_file(state: &ServeState, path: &str) -> Http {
    let rel = if path == "/" {
        "hybrid.html"
    } else {
        path.trim_start_matches('/')
    };
    if rel.is_empty() || rel.contains("..") {
        return Http::text(403, "forbidden");
    }
    let full = state.ui_dir().join(rel);
    let (Ok(canon), Ok(root)) = (full.canonicalize(), state.ui_dir().canonicalize()) else {
        return Http::text(404, "not found");
    };
    if !canon.starts_with(&root) || !canon.is_file() {
        return Http::text(404, "not found");
    }
    match std::fs::read(&canon) {
        Ok(bytes) => Http {
            status: 200,
            ctype: ctype_for(rel),
            body: bytes,
        },
        Err(_) => Http::text(404, "not found"),
    }
}

/// Content-type by file extension (small allowlist; default octet-stream).
fn ctype_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" | "map" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bind host is loopback: binding an ephemeral port yields a loopback IP.
    /// This is the ADR-057 Sealed-mode invariant, proven on the real listener.
    #[test]
    fn binds_loopback() {
        let server = bind(0).expect("bind ephemeral loopback port");
        let addr = server.server_addr().to_ip().expect("bound to an IP");
        assert!(
            addr.ip().is_loopback(),
            "server MUST bind a loopback address, got {addr}"
        );
    }

    #[test]
    fn root_maps_to_hybrid_and_traversal_blocked() {
        let tmp = tempfile::tempdir().unwrap();
        let ui = tmp.path().join("ui");
        std::fs::create_dir_all(&ui).unwrap();
        std::fs::write(ui.join("hybrid.html"), b"<!doctype html>hi").unwrap();
        let state = ServeState::new(tmp.path().join("vault"), ui).unwrap();

        let ok = static_file(&state, "/");
        assert_eq!(ok.status, 200);
        assert_eq!(ok.ctype, "text/html; charset=utf-8");

        let bad = static_file(&state, "/../../etc/passwd");
        assert_eq!(bad.status, 403);
    }

    #[test]
    fn header_guard_blocks_cross_origin_and_rebinding() {
        // Loopback Host, no Origin → allowed (curl, direct navigation).
        assert!(header_guard(Some("127.0.0.1:8799"), None).is_none());
        assert!(header_guard(Some("localhost:8799"), None).is_none());
        assert!(header_guard(Some("[::1]:8799"), None).is_none());
        // Same-origin app requests → allowed.
        assert!(header_guard(Some("127.0.0.1:8799"), Some("http://127.0.0.1:8799")).is_none());
        assert!(header_guard(Some("localhost:8799"), Some("http://localhost:8799")).is_none());
        // DNS rebinding: attacker hostname in Host → blocked.
        assert!(header_guard(Some("evil.example.com:8799"), None).is_some());
        // Missing Host → blocked.
        assert!(header_guard(None, None).is_some());
        // Cross-site fetch: foreign Origin → blocked even with a loopback Host.
        assert!(header_guard(Some("127.0.0.1:8799"), Some("https://evil.example.com")).is_some());
        // Opaque Origin (sandboxed iframe / file://) → blocked.
        assert!(header_guard(Some("127.0.0.1:8799"), Some("null")).is_some());
    }
}
