# ADR-035: Darwin-Style Parameter Evolution (safety-frozen)

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Related**: ADR-018 (deferred self-optimization), ADR-007 (deterministic engine), ADR-006 (abstention), ADR-005 (grounding), ADR-009 (escalation), ADR-010 (SaMD)

---

## Context

Helix has many hand-set numeric knobs — the abstention `confidence_floor`, the
`staleness_window_days`, the trend `flat_band_per_day` dead-band, score weights,
the OCR confidence floor. ADR-018 deferred any self-optimization until two things
existed: a **held-out eval set** and a **fitness function**.

The risk in naïvely applying "make it better" optimization to a health tool is
acute: the cheapest way to raise a naïve accuracy metric is to **answer more
often** — which is exactly the over-confident behavior Helix exists to prevent
(recall ≠ grounding). And the red-flag escalation thresholds (ADR-009) and the
SaMD boundary (ADR-010) are **safety-critical** — they must never be moved by an
automated search.

## Decision

Adopt **Darwin Mode's principle — mutate the configuration, keep only measurable
improvements — scoped to non-safety parameters, with a grounding-first fitness**,
implemented deterministically in `helix-evolve`.

1. **Frozen model, frozen safety.** Evolution never touches model weights and
   never touches the escalation registry or SaMD rules — the registry is passed in
   **frozen** and is not part of the search space. Tunable set (`Params`):
   `confidence_floor`, `staleness_window_days`, `flat_band_per_day`, within bounds
   that forbid degenerate values.
2. **Grounding-first fitness.** Each parameter set is scored against a labeled
   eval set by running the real pipeline (ADR-007). **Answering when Helix should
   have abstained (over-confidence) is penalized ~7× more than abstaining when it
   could have answered.** Evolution therefore *cannot* win by making Helix less
   conservative — that scores worst.
3. **Held-out, ground-truthed eval set.** Cases encode behavior that is correct
   *independent of the params* (a clearly-declining fresh series → answer falling;
   a noisy-but-flat series → answer flat; an 800-day-old value → abstain; a
   low-confidence reading → abstain). The optimum is the parameter set that matches
   ground truth, not one that maximizes answers.
4. **Deterministic & air-gapped (ADR-018).** A seeded LCG, no clock, no I/O, no
   network — same seed + same eval set ⇒ identical evolution, so every run is
   replayable and auditable. Hill-climb: propose a mutation, evaluate, keep only if
   fitness strictly improves.
5. **Proposal, not auto-promotion.** Evolved parameters are an output to be
   reviewed and version-pinned by governance, never silently shipped — consistent
   with no-auto-promotion. Safety thresholds remain a separate, human-owned change.

Validation (the bundled demo): from a `flat_band=0.0` baseline (which misreads a
noisy-flat series as a trend, fitness 3.30) evolution finds `flat_band≈0.022`
(fitness 4.00, +0.70) while **over-confidence stays at zero and the confidence
floor rises** — a real accuracy gain with safety strictly preserved.

## Alternatives Considered

- **The MetaHarness Darwin MCP tool directly.** It evolves *agent harnesses*, not a
  Rust library's numeric parameters; the principle transfers but the mechanism
  doesn't. We implement the principle natively and deterministically instead.
- **Optimize a single accuracy score.** Rejected: it rewards answering more — the
  anti-pattern. The fitness must be grounding-first.
- **Let evolution tune everything (incl. safety).** Rejected outright: red-flag and
  SaMD thresholds are governance-owned (ADR-009/010), frozen out of the search.

## Consequences

**Positive.** Turns hand-tuned knobs into evidence-tuned ones; reproducible and
auditable; structurally cannot erode safety; the eval set doubles as a regression
gate. **Negative.** Only as good as the eval set (small, hand-built today); risks
overfitting a tiny set. **Mitigations.** Grow + diversify the eval set; held-out
split; bounds; human sign-off before any parameter ships.

## Open Questions

- Eval-set curation at scale (who labels, how many cases, per-concept sets).
- Whether to expand the search space to score weights / OCR floor (same frozen-safety rule).
- A cross-validation split once the eval set is large enough.

## References

- ADR-018 (self-optimization prerequisites), ADR-006/005 (abstention/grounding), ADR-007 (deterministic engine), ADR-009/010 (frozen safety). **[A]**
- Darwin Mode principle: mutate config → measure on held-out set → keep only wins; model frozen, air-gapped. **[B]**

> Architectural/product guidance, not legal or medical advice. Evolution tunes non-safety parameters only; safety thresholds remain human-governed.
