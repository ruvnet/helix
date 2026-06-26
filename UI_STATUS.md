# Helix UI + WASM Mobile — Status & Loop Ledger

**Loop:** `/loop 5m … until complete and validated` · **Started:** 2026-06-25
**Standalone repo:** https://github.com/ruvnet/helix (pushed via subtree split from ruvector `feat/helix-implementation`)

Goal: a modern web **management UI** for Helix (screenshots, modals, step-by-step guides) and a
**WASM mobile app**, both driven by the real Rust helix-* crates compiled to WebAssembly, validated
in a real browser.

## Definition of "complete and validated" (exit criteria)
1. `helix-wasm` crate compiles the analytic pipeline (core/score/ontology/verifier) to wasm; `wasm-pack build` green.
2. Web management UI loads the wasm and runs the real pipeline: add records → grounded answer / abstain / escalate; decomposable health score; evidence-tier chips.
3. UI has: ≥1 modal, a step-by-step onboarding guide, responsive layout.
4. Mobile app (PWA: manifest + service worker + mobile layout) reuses the same wasm; installable.
5. Validated in the browser (Chrome via /browser): screenshots captured of dashboard, an answer with citations, a modal, the guide, and the mobile view.
6. A short docs page embeds the screenshots + a "how it works" walkthrough.

## Plan (crate/dir → purpose)
| Path | Purpose | Status |
|------|---------|--------|
| `crates/helix-wasm` | wasm-bindgen binding over helix-core/score/ontology/verifier (crypto-free, no getrandom) | ⬜ iter 1 |
| `ui/` | web management console (vanilla TS/JS + the wasm pkg; modern CSS) | ⬜ |
| `ui/pkg/` | wasm-pack output (gitignored build artifact, or committed for static hosting) | ⬜ |
| `mobile/` | PWA mobile app (manifest, service worker, mobile-first layout) reusing the wasm | ⬜ |
| `docs/ui/` | screenshots + walkthrough | ⬜ |

Note: `helix-vault` (crypto/getrandom) is intentionally NOT in the wasm binding — key custody on mobile uses
platform secure storage; the wasm surface is the analytic pipeline only. helix-core does not depend on vault.

## New integration request (2026-06-25): ruvnet/ruv-neural
`ruv-neural` = Rust/WASM closed-loop gamma-entrainment (40 Hz) / EEG research harness, "research-grade,
not a medical device". **Integration plan (faithful to Helix's ADRs):** treat ruv-neural's signed session
evidence as a *data source* (like the Cognitum Seed, ADR-014) — EEG/entrainment metrics → ProvRecords with
provenance + screening-not-diagnosis framing (ADR-006/009/010), surfaced as a "Neuro" subsystem in the
health score (ADR-016) and a data-source card in the UI. New crate `helix-neural` (ingestion adapter +
session→ProvRecord mapping). Do NOT claim clinical/therapeutic effect — research signal only.

## Ledger
- **Iter 1 (2026-06-25):** repo `ruvnet/helix` created (public, 10 topics, description); branch pushed;
  helix subtree pushed to `main`. `helix-wasm` binding built — `wasm-pack build` GREEN, real pipeline runs
  in-browser (analyze_json/compose_score_json), pkg committed at `ui/pkg` (169KB wasm). `.cargo/config.toml`
  fixes the mold-linker-vs-rust-lld clash for wasm. 3 native binding tests green.

## Next picks
1. Web UI: dashboard (health score ring + body systems), Ask panel (grounded answer/abstain/escalate),
   Records modal, onboarding guide. Loads `ui/pkg/helix.js`.
2. Serve locally; /browser screenshot + validate interactions (real wasm pipeline).
3. `helix-neural` crate: ruv-neural session evidence → ProvRecords (the integration request).
4. Mobile PWA: manifest, service worker, mobile layout; /browser mobile-viewport screenshot.
5. docs/ui walkthrough with screenshots; final validation sweep; close loop.
