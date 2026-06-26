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

## Ledger
- **Iter 1 (2026-06-25):** repo `ruvnet/helix` created (public, 10 topics, description), branch pushed,
  helix subtree pushed to main. Building `helix-wasm` binding + first `wasm-pack build`.

## Next picks
1. Web UI: dashboard (health score ring + body systems), Ask panel (grounded answer/abstain/escalate), Records modal, onboarding guide.
2. wasm-pack build into ui/pkg; serve locally; /browser screenshot + validate interactions.
3. Mobile PWA: manifest, service worker, mobile layout; /browser mobile-viewport screenshot.
4. docs/ui walkthrough with screenshots; final validation sweep; close loop.
