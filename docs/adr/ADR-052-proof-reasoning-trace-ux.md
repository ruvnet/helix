# ADR-052: Proof / Reasoning-Trace UX — Making Grounding Legible

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-005 (grounded answering/citation chips), ADR-006 (evidence tiering/abstention), ADR-007 (deterministic numerics), ADR-008 (verifier/critic), ADR-009 (escalation), ADR-010 (SaMD copy), ADR-015 (twin tap-to-reveal panel), ADR-033 (dashboard recs), ADR-050 (design system), ADR-053 (witness-chained provenance), ADR-021 (genome — pharmacogenomic flags, amendment)

---

## Context

ADR-005 through ADR-008 built a rigorous backend pipeline — retrieval with provenance
(005), evidence tiering and abstention (006), deterministic numeric computation (007),
and independent cross-family verification (008) — but its legibility to the user is
currently scattered: ADR-005's inline citation chips, ADR-006's evidence-tier chips,
ADR-015's twin tap-to-reveal panel, and ADR-033's dashboard citations are each
independently specified, in different words, for different surfaces. No single ADR
owns "what does the user actually see when they ask *why*." The world-class UI push
wants one canonical "show your work" surface everywhere a claim appears. This ADR is a
**UX/consumption decision** — it does not add grounding capability, it makes the
capability ADR-005/006/007/008 already built visible and consistent.

## Decision

**Every answer or nudge that makes a factual claim carries a "show your work"
affordance that opens one standard Proof Panel — implemented once, called everywhere.**

1. **Universal affordance.** Any surface producing a grounded claim (chat answer,
   dashboard recommendation ADR-033, twin tap-to-reveal ADR-015, non-emergency
   escalation copy ADR-009) exposes the same collapse/expand gesture. True red-flag
   emergency copy stays terse per ADR-010 Gate 5 and is exempt.
2. **Fixed panel sections, in order:**
   - **Sources** — the ProvRecord(s) consulted (ADR-005): source system, date,
     provenance hash.
   - **Evidence tier** — the tier chip(s) (ADR-006) with the standard tooltip.
   - **Numeric derivation** — the deterministic trend/computation trail (ADR-007),
     stated as arithmetic over stored points, explicitly "not LLM-computed."
   - **Verification state** — a simple badge (Verified / Reformulated /
     Dropped-and-replaced) from the Verifier (ADR-008) — not the full internal
     verdict set, preserving ADR-008's existing answer to "should every verification
     event be shown" (no — see ADR-008 Open Question 4).
   - **Abstention state** — if part of the response abstained, the gap notice and its
     reason (ADR-006) appears inline, not hidden.
3. **One component, every caller.** The panel is built once in the ADR-050 component
   library. ADR-005's citation chip, ADR-006's tier chip, ADR-015's tap-to-reveal
   panel, and ADR-033's dashboard citations all open *this* component rather than each
   maintaining a bespoke variant.
4. **Abstention gets a proof affordance too.** A gap notice (ADR-006) opens a lighter
   version of the same panel showing exactly why (missing / stale / low-confidence /
   conflicting), reinforcing "abstention is honesty, not failure" as a felt, not just
   stated, product property.
5. **Progressive disclosure.** Collapsed by default — a short summary line (echoing
   ADR-008's "cross-checking sources..." language, e.g. "checked · 3 sources") —
   expandable on demand. Depth is available; it is never forced on the user.
6. **Accessible and on-brand.** Built from ADR-050 tokens/typography; provides a
   text-equivalent list (not a graphics-only view) meeting WCAG AA, consistent with
   ADR-015 Constraint V-4 generalized app-wide by ADR-050.
7. **Extensible for pharmacogenomics and literature.** Genomic advisories (ADR-021
   amendment, this pass) and literature-grounded Tier-3 citations (ADR-055) surface
   inside this same panel — the panel is the one legibility surface, not a new one per
   data type.

## Alternatives Considered

- **Leave the current scattered per-feature approach as-is.** Rejected: users learn a
  different "trust gesture" per feature, undermining the consistency the grounding
  architecture exists to build trust through.
- **Expose the full raw Verifier `ClaimVerdict` by default.** Rejected: ADR-008 already
  decided (Open Question 4) that surfacing every verification event is noisy. The
  panel is a curated distillation for the user; the raw verdict set stays in the audit
  log (ADR-002/008).
- **Replace the structured panel with a single confidence number.** Rejected: this
  collapses exactly the tier conflation ADR-006's Context section names as the core
  failure mode Helix exists to avoid.

## Consequences

### Positive
- One consistent, learnable trust gesture across the entire app.
- Makes the anti-hallucination architecture a felt experience, not just a backend
  property — the actual differentiator becomes visible.
- Reduces engineering duplication: one component, many callers.

### Negative
- Retrofitting existing bespoke surfaces (ADR-015 tap-to-reveal, ADR-033 feed) to the
  canonical shape is real rework.
- Risk of the panel becoming an unreviewed dumping ground for every new data type if
  not curated.
- Adds interaction depth some users may never open (mitigated by collapse-by-default).

### Mitigations
| Risk | Mitigation |
|---|---|
| Retrofit cost | Componentize once (ADR-050); migrate call sites incrementally |
| Panel scope creep | New sections require an ADR-052 update, not ad hoc addition |
| Unused depth | Collapsed-by-default with a short trust-signal summary line |
| Privacy of usage analytics | Local-only open-rate counters, no export absent consent (ADR-001/011) |

## Open Questions

1. Exact copy for the collapsed summary line, and its localization.
2. Does the Verification-state badge need a 4th, power-user disclosure level showing
   more of the `ClaimVerdict`?
3. How does ADR-053's witness-chain "verify this trail" affordance nest inside vs.
   beside this panel?

## References

- Helix ADR-005 §Decision (citation format), ADR-006 §Decision (tiers/abstention),
  ADR-007, ADR-008 §Decision (verdict schema, Open Question 4), ADR-015 §Decision 3
  (tap-to-reveal precedent), ADR-033, ADR-050 **[A]**

---

> Architectural/product guidance, not legal or medical advice. This ADR changes how
> existing grounding guarantees are surfaced; it does not change what is required to
> be grounded, tiered, or verified.
