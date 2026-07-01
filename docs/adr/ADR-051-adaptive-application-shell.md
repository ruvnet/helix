# ADR-051: Adaptive Application Shell (Tauri v2 Desktop + PWA Mobile over One WASM Core)

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001 (local-first vault), ADR-013 (on-device inference), ADR-026 (on-device LLM analyst), ADR-047 (single-tenant local-first topology), ADR-049 (scheduled pulls), ADR-050 (design system)

---

## Context

Helix already has one real, shared analytic core: the `helix-wasm` crate in this
workspace compiles the full pipeline — provenance (ADR-005), numeric (ADR-007),
evidence (ADR-006), escalation (ADR-009), ontology (ADR-004), score (ADR-016), sensing
(ADR-014/020), genome (ADR-021), OCR (ADR-022), bioage (ADR-034), and more — to WASM
for the web/mobile UI. A web UI (`ui/`) and an installable mobile PWA (`mobile/`,
already shipping a `manifest.webmanifest` and a service worker) already consume it. What
does **not** yet exist is a formal desktop shell, and no ADR has decided the
shell strategy that keeps "one codebase, adapts desktop↔mobile" true as the product
grows rather than forking into parallel native codebases that re-implement the same
pipeline logic already expressed once in Rust.

## Decision

**A single shared Rust→WASM core (`helix-wasm`) is wrapped by two thin shells: Tauri v2
for desktop, the existing installable PWA for mobile. Neither shell re-implements
business logic; both are chrome around the same core.**

1. **One core, two shells.** `helix-wasm` remains the only place pipeline logic lives.
   Desktop and mobile shells differ only in windowing/chrome/notification plumbing,
   not in analysis, scoring, or grounding logic.
2. **Desktop: Tauri v2**, chosen over Electron because (a) Tauri ships a system webview
   instead of bundling Chromium — materially smaller binaries and lower idle memory,
   which matters for an always-on, local-first health app that should not compete for
   the user's machine; (b) Tauri's backend is Rust, matching every existing Helix
   crate — no bundled Node.js server process is needed, keeping the single-tenant
   local-first topology (ADR-047) intact; (c) Tauri v2's mobile targets are evaluated
   but not adopted here — Helix's mobile path is the PWA (below), kept independent for
   app-store/sideload flexibility.
3. **Mobile: the existing installable PWA**, offline-first via its service worker and
   the same WASM core running in the mobile browser engine. No native app-store
   dependency is required for MVP — consistent with the low-friction, "nothing to
   charge" ethos already established for ambient sensing (ADR-014).
4. **Offline-first everywhere.** Both shells operate against the local encrypted vault
   (ADR-001) and on-device inference (ADR-013, ADR-026) with no required network round
   trip for core analysis. Scheduled pulls (ADR-049) run independently of whether a
   shell is open.
5. **Shared UI layer.** Both shells render the same component library and tokens
   (ADR-050); platform-specific chrome (window controls vs. mobile navigation) is a
   thin adapter, never a fork of business logic or restyle of components.
6. **Update path respects local-first.** Desktop updates via Tauri's signed updater;
   mobile via service-worker cache-busting. Neither requires a cloud dependency to
   apply a logic update, only to *distribute* one.

## Alternatives Considered

- **Electron.** Rejected: bundles a full Chromium + Node runtime alongside the
  already-existing Rust core — a second heavyweight runtime, larger binaries, higher
  idle RAM, poor fit for a local-first always-on app.
- **Parallel native codebases (Swift/Kotlin).** Rejected for MVP: doubles engineering
  surface tracking logic already expressed once in `helix-wasm`. Revisit only if a
  specific feature (e.g., deep OS integration, or ADR-054's AR question) demands it —
  mirroring ADR-015's own deferral of native rendering to Phase 3+.
- **React Native + native modules.** Rejected: introduces a second non-Rust runtime
  bridging back into the WASM core, adding a translation layer with no clear benefit
  over a PWA that already runs the same WASM directly in the mobile browser.

## Consequences

**Positive.** One core, two thin shells; smaller/lighter desktop footprint than
Electron; fully consistent with the existing all-Rust workspace; offline-first by
construction, not by retrofit.

**Negative.** System-webview rendering varies slightly by OS (WebView2 / WebKit /
WebKitGTK), adding a cross-platform QA matrix; the PWA path inherits iOS Safari's PWA
limitations (background sync, push-notification restrictions), which may lag native
for Escalation Guardian (ADR-009) delivery latency.

**Mitigations.** Pin minimum webview versions and test across the matrix; use the Web
Notifications API where available on mobile, falling back to in-app on-open checks on
iOS Safari PWA until/unless a native wrapper is separately justified.

## Open Questions

1. Should the Tauri desktop shell call `helix-wasm` through the WASM runtime (asset/UI
   consistency) or link the native Rust crates directly (performance)? Needs a
   benchmark before Phase 1 ships.
2. How much does iOS PWA push-notification limitation degrade Escalation Guardian
   (ADR-009) delivery latency in practice, and is that acceptable for red-flag copy?
3. App-store distribution vs. sideload-only for a wellness-positioned app (ties to
   ADR-010's app-store-metadata review requirement).

## References

- Tauri v2 documentation — architecture, webview model, updater **[A — vendor docs, standard reference]**
- `crates/helix-wasm/Cargo.toml` (this repo) — confirms the existing shared core and
  its 17 dependent Helix crates **[A — verified in-repo]**
- `ui/`, `mobile/manifest.webmanifest`, `mobile/sw.js` (this repo) — confirms the web
  UI and installable PWA already exist and are the basis this ADR formalizes **[A — verified in-repo]**
- Helix ADR-001, ADR-013, ADR-026, ADR-047, ADR-049, ADR-050

---

> Architectural/product guidance, not legal or medical advice. Shell choice does not
> alter any grounding, evidence-tiering, or clinical-safety decision made elsewhere.
