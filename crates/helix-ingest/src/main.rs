//! `helix-ingest` CLI (ADR-029/001).
//!
//! Two modes:
//! ```text
//! # one-shot ingest (unchanged): file → sealed vault → gitignored dossier
//! helix-ingest --fhir <path.json> --apple <export.xml> --vault <dir> --out <dossier.json>
//!
//! # NEW: local companion server (loopback only) powering guided onboarding
//! helix-ingest serve [--port 8799] [--vault <dir>] [--ui <dir>]
//! ```
//!
//! For the ingest mode, both sources are optional but at least one is required.
//! The passphrase comes ONLY from `HELIX_VAULT_PASSPHRASE` or an interactive
//! prompt — never a flag, never logged. The summary prints counts/metadata only;
//! record values never touch a log. The `serve` mode binds `127.0.0.1` ONLY.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};
use helix_ingest::{run, serve, vault, RunArgs};

/// Parse health-data files through the tested importers, seal them into the
/// encrypted vault, prove the round-trip + encryption-at-rest, and emit a local
/// (gitignored) dossier.json for the UI — or run the local companion server.
#[derive(Parser, Debug)]
#[command(name = "helix-ingest", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    // --- one-shot ingest flags (used when no subcommand is given) ---
    /// FHIR R4 bundle (or bare Observation) JSON to import.
    #[arg(long, value_name = "path.json")]
    fhir: Option<PathBuf>,

    /// Apple Health `export.xml` to import.
    #[arg(long, value_name = "export.xml")]
    apple: Option<PathBuf>,

    /// Vault directory (holds the encrypted redb store + salt). Created if absent.
    #[arg(long, value_name = "dir")]
    vault: Option<PathBuf>,

    /// Where to write the decrypted dossier.json. Default is under the gitignored
    /// `./private/` path so PHI never lands in a tracked file.
    #[arg(long, value_name = "dossier.json", default_value = "./private/dossier.json")]
    out: PathBuf,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the localhost companion server (127.0.0.1 only) for guided onboarding.
    Serve(ServeArgs),
}

#[derive(Args, Debug)]
struct ServeArgs {
    /// Port to bind on 127.0.0.1 (the host is fixed to loopback; ADR-057).
    #[arg(long, default_value_t = 8799)]
    port: u16,

    /// Vault directory (created on first unlock).
    #[arg(long, value_name = "dir", default_value = "./private/vault")]
    vault: PathBuf,

    /// UI root to serve; `/` loads `hybrid.html` from here.
    #[arg(long, value_name = "dir", default_value = "./ui")]
    ui: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Serve(a)) => serve::serve(serve::ServeConfig {
            port: a.port,
            vault_dir: a.vault,
            ui_dir: a.ui,
        }),
        None => run_ingest(cli),
    }
}

/// The original one-shot ingest flow (unchanged behavior).
fn run_ingest(cli: Cli) -> Result<()> {
    let Some(vault_dir) = cli.vault else {
        bail!("provide --vault <dir> (or use `helix-ingest serve`)");
    };
    if cli.fhir.is_none() && cli.apple.is_none() {
        bail!("provide at least one of --fhir <path.json> or --apple <export.xml>");
    }

    // Decide whether we are initializing a fresh vault BEFORE touching anything,
    // so a first-time passphrase can be confirmed.
    let creating = !vault::is_initialized(&vault_dir);
    let passphrase = read_passphrase(creating)?;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let report = run(RunArgs {
        fhir: cli.fhir.as_deref(),
        apple: cli.apple.as_deref(),
        vault_dir: &vault_dir,
        out: &cli.out,
        passphrase: &passphrase,
        now_ms,
    })?;

    print_summary(&report);
    Ok(())
}

/// Read the passphrase from the environment or an interactive prompt. NEVER from
/// a CLI flag; never echoed. On first-time vault creation, confirm it twice.
fn read_passphrase(creating: bool) -> Result<String> {
    if let Ok(p) = std::env::var("HELIX_VAULT_PASSPHRASE") {
        if p.is_empty() {
            bail!("HELIX_VAULT_PASSPHRASE is set but empty");
        }
        return Ok(p);
    }
    let p = rpassword::prompt_password("Vault passphrase: ")?;
    if p.is_empty() {
        bail!("empty passphrase");
    }
    if creating {
        let confirm = rpassword::prompt_password("Confirm passphrase: ")?;
        if confirm != p {
            bail!("passphrases do not match");
        }
    }
    Ok(p)
}

/// Print counts/metadata only — no record values, ever.
fn print_summary(report: &helix_ingest::RunReport) {
    println!("helix-ingest: sealed and verified {} record(s)", report.record_count);
    if report.queued_for_review > 0 {
        println!("  held for review: {}", report.queued_for_review);
    }
    println!("  by source:");
    for (source, count) in &report.by_source {
        println!("    {source}: {count}");
    }
    println!("  vault: {}", report.vault_records_path.display());
    println!("  dossier: {}", report.out_path.display());
    println!(
        "  round-trip: OK (re-opened fresh, decrypted {} back)",
        report.record_count
    );
    println!(
        "  ciphertext-at-rest markers checked (all absent from raw file): {}",
        report.markers_checked
    );
    println!(
        "encryption-at-rest: {}",
        if report.encryption_at_rest_proven {
            "PROVEN"
        } else {
            "NOT PROVEN"
        }
    );
}
