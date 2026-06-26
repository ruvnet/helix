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

- **Iter 2 (2026-06-25):** Web management UI built (`ui/index.html` + `app.css` + `app.js`) — modern dark
  console: dashboard (decomposable score ring + body systems), Ask Helix (grounded/abstain/escalate from the
  real wasm pipeline), Records, Data sources (incl. Cognitum Seed + ruv-neural cards), modals (score info,
  breakdown, system detail, add-record), step-by-step onboarding guide. **Validated in Chrome against live
  WASM:** score=76 from compose_score_json; grounded ferritin answer with 3 citations + deterministic trend
  "−38% vs first reading · crossed reference range"; guide modal renders. `docs/ui/README.md` walkthrough.

- **Iter 3 (2026-06-25):** `helix-neural` — the **ruv-neural integration**. Maps a signed gamma-entrainment
  (40 Hz) / EEG session into provenance-tagged ProvRecords (RUVN-* research codes, never clinical LOINC;
  AmbientSensing method; capped 0.6 confidence; RESEARCH_DISCLAIMER "not a diagnosis / not a therapeutic
  claim"); rejects unsigned/empty/non-finite sessions; `neuro_orientation()` 0–100 for an optional Neuro
  subsystem. Exposed via wasm: `neural_session_to_records_json` + `neural_disclaimer`. 87 tests green;
  clippy+fmt clean; wasm pkg rebuilt.

- **Iter 4 (2026-06-25):** Mobile PWA + screenshots + GitHub Pages + SEO.
  - `mobile/`: installable PWA (manifest, service worker offline cache, icon) reusing the SAME helix-wasm
    pipeline; mobile-first layout, bottom sheet, live grounded-answer screen.
  - **Real screenshots** captured via headless google-chrome (MCP extension was flaky) of the running UI:
    dashboard, grounded answer (cited, −38%, Tier 1), guide modal, mobile. Embedded in README.md + docs/ui.
  - **Published GitHub Pages** at https://ruvnet.github.io/helix/ (landing + /ui/ console + /mobile/ app),
    HTTPS enforced, .nojekyll. Full **SEO**: meta/description/keywords, OG + Twitter cards, canonical, JSON-LD,
    og-image, robots.txt, sitemap.xml, repo homepage set.
  - **Validated the DEPLOYED site**: headless screenshot of https://ruvnet.github.io/helix/ui/ renders the
    WASM UI (score 76, live engine) — wasm loads over HTTPS in production.

## Status: COMPLETE & VALIDATED ✅
All exit criteria met. Live site: https://ruvnet.github.io/helix/ · Repo: https://github.com/ruvnet/helix
1. helix-wasm pipeline → wasm ✅  2. UI runs real pipeline (grounded/abstain/escalate, score) ✅
3. modals + step-by-step guide ✅  4. mobile PWA (manifest+SW) ✅  5. browser-validated (local + live) ✅
6. screenshots in README ✅  + ruv-neural integration ✅ + GitHub Pages + SEO ✅

## Optional remaining polish (not blocking)
- Capture abstain + escalate state screenshots (deep-link hooks exist: #ask etc.) into docs/ui.
- Wire a ruv-neural sample + Neuro subsystem card into the live UI.
- agent-browser npm based validation flow (headless chrome currently used).
