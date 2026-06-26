# ADR-033: Dashboard Updates & Recommendations (evidence-tiered, grounded)

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Related**: ADR-006 (evidence tiering/abstention), ADR-005 (grounding), ADR-026 (LLM narrator), ADR-008 (verifier), ADR-010 (SaMD)

---

## Context

The dashboard's "**updates & recommendations**" feed is where a health product most
easily drifts into hallucinated or over-confident advice. It must inherit Helix's
whole anti-hallucination stack rather than be a free-text suggestion box.

## Decision

Every dashboard update/recommendation is **grounded, evidence-tiered, and
narrator-only**.

1. **Updates are grounded facts.** "What changed since you last looked" is computed
   from the record diff (new values, range crossings, change-points) — each cites
   the records that changed (ADR-005). No update without a backing datum.
2. **Recommendations carry an evidence tier (ADR-006).** Tier 1 your-data · Tier 2
   reference standards/guidelines · Tier 3 peer-reviewed literature · Tier 4
   heuristic/lore (explicitly flagged). Tier-4 is never dressed as established fact.
3. **The LLM narrates, never invents (ADR-026).** Phrasing comes from the on-device
   model under the number-guard; the Verifier (ADR-008) gates clinically meaningful
   items; abstention is allowed and rewarded ("I don't have enough to suggest that").
4. **Action framing, not prescription (ADR-010).** "Consider discussing X with your
   clinician / retest Y in N weeks", never a treatment directive. Red-flags route to
   the Escalation Guardian and suppress optimization (ADR-009).

## Consequences

**Positive.** A recommendations feed that is honest, cited, and safe by construction;
reuses the existing stack. **Negative.** More "I don't have that yet" than a
confident-sounding competitor; tiering must be designed to read as trustworthy.
**Mitigations.** Good UX for tiers + abstention; cite-everything; governance review.

## References
- ADR-006 (evidence tiers + abstention), ADR-005 (grounding), ADR-026 (narrator + number-guard), ADR-008 (verifier), ADR-010 (SaMD). **[A]**

> Architectural/product guidance, not legal or medical advice. Recommendations are grounded, tiered, and non-prescriptive.
