# Helix UI — Design Directions & Chosen Visual Language

Concept exploration for Helix's world-class interface. These mockups implement the
decisions in [ADR-050 (Design System & Visual Language)](../../adr/ADR-050-design-system-visual-language.md),
[ADR-052 (Proof / Reasoning-Trace UX)](../../adr/ADR-052-proof-reasoning-trace-ux.md),
and [ADR-054 (Real-Time WebGL Digital Twin)](../../adr/ADR-054-realtime-webgl-digital-twin.md).

Each file is a **single self-contained HTML** (inline CSS/JS; CDNs only for fonts +
three.js). Open any of them directly in a browser — no build step.

> **Synthetic data only.** Every mockup renders the same fictional persona **"Alex Rivera, 45"**
> (composite score 74/100). There is **no real PHI** in this directory — it is safe for the
> public repo. Real health data never leaves the user's device (ADR-001, ADR-047).

## Three explorations

| File | Direction | Type pairing | Character |
|---|---|---|---|
| [`direction-a-clinical-aurora.html`](direction-a-clinical-aurora.html) | **Clinical Aurora** | Fraunces + Schibsted Grotesk | Refined luxury-medical — obsidian stage lit by a bioluminescent teal→green helix aurora, frosted glass, generous negative space. Premium and trustworthy. |
| [`direction-b-bio-instrument.html`](direction-b-bio-instrument.html) | **Bio-Instrument** | JetBrains Mono + Chakra Petch | Industrial diagnostic console — near-black, telemetry grid, oscilloscope vitals, amber/lime signal accents. The most distinctive/memorable; reads technical. |
| [`direction-c-living-twin.html`](direction-c-living-twin.html) | **Living Twin** | Bricolage Grotesque + Hanken Grotesk | Warm-dark organic wellness — a breathing helix, tactile rounded cards, encouraging tone. The most human. |

All three share: a real **three.js double-helix twin** (amber strand = your *data*, green/lime =
the *intelligence*) with an SVG/Canvas fallback and `prefers-reduced-motion` support; an
animated **composite Health Score ring** decomposed into six sub-scores; **nudge cards** whose
core feature is an expandable **proof trail** (own-data source → observed value → evidence tier
→ cited reference → non-diagnostic action); and a **90-day holistic event map**.

## Chosen direction — the Hybrid

**[`direction-final-hybrid.html`](direction-final-hybrid.html)** is the canonical reference the
production UI is built from.

- **Frame / shell = Clinical Aurora** — the premium, credible obsidian stage, aurora mesh, and
  glowing three.js twin. Establishes trust for a health-intelligence product.
- **Coaching / nudge surfaces = Living Twin warmth** — the daily "what to do today" moments use
  warmer, softer, encouraging cards so the everyday companion feels human, not clinical.
- **Bio-Instrument density = optional "deep-dive" toggle** — the dense telemetry view is
  available for power users but is *not* the default; the everyday view stays calm.

**Why this hybrid:** Helix must be *both* a trustworthy functional-medicine specialist *and* a
warm daily companion "anybody" can use. Aurora carries the credibility; Twin carries the daily
encouragement; the Instrument view serves the quantified-self power user without imposing its
density on everyone. In every view, the **proof trail stays the centerpiece** — the
anti-hallucination promise (ADR-005/006/052) made visible.

## Status

Concept mockups, **not yet wired to live data**. Next step: integrate the hybrid into the real
app shell (`ui/`, ADR-051) driven by the `helix-wasm` exports and the synthetic demo dossier,
then the same shell adapts to Tauri (desktop) and PWA (mobile).
