# ADR-054: Real-Time WebGL Digital Twin — Live Binding & Adaptive LOD

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Supersedes**: ADR-015 (Visual Health-Intelligence Layer — 3D Anatomical Digital Twin)
**Related**: ADR-005 (grounding), ADR-006 (evidence tiers), ADR-007 (deterministic numerics), ADR-009 (escalation), ADR-014/ADR-020 (ambient sensing sources), ADR-033 (dashboard diffs), ADR-050 (design system), ADR-051 (adaptive shell)

---

## Context

ADR-015 made the foundational decision for the 3D digital twin: the React Three
Fiber/WebGL stack, the schematic-not-counterfeit anatomical model, the four hard
visualization-safety constraints (V-1 grounded-only, V-2 schematic not counterfeit, V-3
non-alarming, V-4 accessible), the tap-to-reveal information architecture, and a
phased rollout (WebGL now, native evaluated later). **That decision stands unchanged.**
What the "world-class UI + proactive daily specialist" push adds is the concrete
real-time engineering shape ADR-015 named as future work but did not fully specify:

1. **Live data binding.** ADR-015 defines `SystemColorState` as a function of the
   user's data, computed by the Trend/Numeric agent (ADR-007) — but described it as a
   render input, not a subscribable stream. A proactive product needs the twin to
   reflect a new grounded value (an ambient-sensing reading landing overnight,
   ADR-014/020; a dashboard "what changed" diff, ADR-033) without requiring the user
   to manually refresh.
2. **Adaptive LOD for mobile.** ADR-015 Decision 1 names three static mesh tiers
   (full/medium/low) chosen once at install. Real device variance (older phones
   running the ADR-051 PWA, thermal throttling during long sessions) needs runtime
   adaptation, not a single install-time choice.

This ADR supersedes ADR-015 **only insofar as it replaces "static render, static LOD"
with "live-bound render, adaptive LOD."** ADR-015's stack choice, safety constraints,
information architecture, and full alternatives analysis are retained by reference and
not re-litigated here; ADR-015 remains intact as the origin decision.

## Decision

**The twin becomes a live-bound, adaptively-rendered surface: React Three Fiber/WebGL
unchanged, running inside the ADR-051 shell, with `SystemColorState` treated as a
subscribable stream and mesh LOD selected and adjusted at runtime.**

1. **Stack unchanged.** React Three Fiber/WebGL (ADR-015 Decision 1) remains the
   production stack, now running inside the ADR-051 adaptive shell (Tauri desktop
   webview + PWA mobile) so the same scene graph and GLTF assets run unmodified on
   both.
2. **Four hard constraints inherited unchanged.** V-1 through V-4 (ADR-015 Decision 5)
   are not reopened by this ADR; this ADR specifies *how* they are delivered at
   runtime, not whether they hold.
3. **Live metric→region binding.** The Trend/Numeric agent's (ADR-007)
   `SystemColorState` output is a subscribable stream: the twin re-renders a region's
   color/trend-arrow the moment a new grounded value crosses the versioned
   data-binding contract (ADR-015 Decision 2) — including ambient-sensing updates
   (ADR-014/020) and dashboard diffs (ADR-033) — so the twin has the same freshness as
   the rest of the app, not a lagging snapshot.
4. **Adaptive LOD policy.** At scene load, detect device class (GPU tier via WebGL
   renderer string / a short frame-time probe) and select automatically from the
   existing full/medium/low tiers (ADR-015 Decision 1). If frame time drops below a
   30 fps floor for 3+ consecutive seconds (e.g., mobile thermal throttling), downgrade
   one LOD tier at runtime rather than freezing.
5. **Data-binding contract remains the single source of truth.** No new binding logic
   is introduced; this ADR makes the existing versioned contract (ADR-015 Decision 2)
   drive a live, adaptive render loop instead of a one-time render call.

## Alternatives Considered

*ADR-015's full alternatives analysis (text-only interface, 2D-chart-only dashboard,
AI-generated "personalized" imagery) stands and is incorporated by reference, not
repeated here.*

- **Static, install-time-only LOD selection (no runtime adaptation).** Rejected: real
  mobile hardware variance and thermal throttling during extended sessions make a
  single install-time choice brittle; runtime adaptation is a small addition given the
  LOD tiers already exist.
- **Polling refresh instead of a subscribable stream.** Rejected: polling adds latency
  between a grounded update landing (e.g., after an ADR-049 scheduled pull) and the
  twin reflecting it — undermining the "organic flow" freshness goal ADR-049 exists to
  serve.

## Consequences

### Positive
- The twin becomes a live reflection of the whole grounded pipeline, not a snapshot —
  reinforcing the "body as navigation metaphor" (ADR-015) with real freshness.
- Adaptive LOD extends usable device range without a second engineering track.

### Negative
- A live-streaming render loop adds state-management complexity (subscription
  lifecycle, avoiding re-render storms on rapid or bursty updates).
- Runtime LOD downgrade adds a QA matrix dimension (device × thermal state).
- Still inherits ADR-015's anatomical/explanatory-library licensing and clinical-
  governance costs, unchanged.

### Mitigations
| Risk | Mitigation |
|---|---|
| Re-render storms | Debounce/coalesce region updates landing within a short window rather than per-metric re-render |
| LOD downgrade thrash | Tune the 30 fps/3-second threshold against the target device matrix before release |
| Licensing/governance cost | Reuse ADR-015's existing licensed-asset and clinical-review plan unchanged |

## Open Questions

1. Exact debounce window for coalescing rapid region updates.
2. Is WebGL-renderer-string GPU-tier detection reliable across Tauri's system
   webviews (WebView2/WebKit/WebKitGTK, ADR-051), or is a frame-time-only fallback
   needed?
3. ADR-015's Open Question 3 (AR overlay) remains open; this ADR does not resolve it.

## References

- ADR-015 (superseded decision; retained by reference for stack choice, safety
  constraints, information architecture, full alternatives, and citation list) **[A]**
- Helix ADR-007, ADR-014, ADR-020, ADR-033, ADR-050, ADR-051

---

> Architectural/product guidance, not legal or medical advice. This ADR does not
> change any visualization-safety constraint (V-1–V-4) or the wellness/SaMD boundary
> (ADR-010) established by ADR-015.
