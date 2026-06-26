# ADR-031: Longitudinal Health-Score Timeline

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Related**: ADR-016 (composite score), ADR-007 (deterministic numerics), ADR-006 (uncertainty/abstention), ADR-015 (visual)

---

## Context

ADR-016 produced a decomposable 0–100 score for "right now". The dashboard wants
that score **plotted over time** — "which way am I heading?". Done naively this
re-introduces the black-box-score failure mode (proprietary wearable scores that
move without explanation, retroactively change as algorithms update, and present
false precision). **[B]**

## Decision

Add a **versioned score time series** computed deterministically (ADR-007) from
the historical subsystem inputs.

1. **Recompute, never back-fill opaquely.** Each historical point is the ADR-016
   composite of the data available *at that time*, tagged with the **methodology
   version** (ADR-016). A methodology change is a visible break in the series, not
   a silent retroactive rewrite.
2. **Trend-first + change-points.** Surface slope/direction and detected
   change-points (`helix-numeric` CUSUM) rather than implying every wiggle is
   signal; show the sample/window behind any trend claim (ADR-006 temporal grounding).
3. **Uncertainty shown, not hidden.** Points with sparse/stale inputs carry a
   wider confidence band; the chart distinguishes "solid" from "weak signal".
4. **Grounded + non-alarming.** Every point opens to the subsystem sub-scores and
   the records that drove it (ADR-016/005); palette is non-alarming (ADR-015 viz
   principles). A wellness-orientation aid, not a risk diagnosis (ADR-010).

## Consequences

**Positive.** Honest "how am I trending" view; reproducible (anyone can recompute
a point from the stored inputs); methodology changes are auditable.
**Negative.** Requires retaining historical subsystem inputs; gaps make early
history sparse. **Mitigations.** Confidence bands on sparse points; gap notices.

## References
- ADR-016 (decomposable score), ADR-007 (deterministic engine), ADR-006 (uncertainty). **[A]**
- Wellness-score opacity critiques (proprietary readiness/recovery scores). **[B]**

> Architectural/product guidance, not legal or medical advice. Trend-first, grounded, non-diagnostic.
