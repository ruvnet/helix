//! Regenerate `ui/demo-dossier.json` from the Rust source of truth.
//!
//! Run: `cargo run -p helix-demo --example generate`
//!
//! The UI (`ui/app.js`) and mobile PWA (`mobile/mobile.js`) fetch that JSON and
//! feed its records through the real WASM pipeline — so this file is the one
//! place the synthetic "Alex Rivera" dossier is defined.

use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    // crate dir → repo root → ui/demo-dossier.json
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out = repo_root.join("ui").join("demo-dossier.json");

    let json = helix_demo::dossier_json_pretty();
    std::fs::write(&out, format!("{json}\n"))?;

    let counts = helix_demo::domain_counts();
    println!("wrote {} ({} bytes)", out.display(), json.len());
    println!("domain counts: {counts:#}");
    Ok(())
}
