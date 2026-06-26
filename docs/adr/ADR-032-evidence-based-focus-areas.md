# ADR-032: Evidence-Based "Focus Areas" & Vitals Panel (non-diagnostic)

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Related**: ADR-010 (SaMD boundary), ADR-009 (escalation), ADR-006 (evidence tiering), ADR-007 (numerics), ADR-005 (grounding)

---

## Context

The dashboard wants a **vitals panel** and a short list of "**key medical issues to
focus on**". The line that must not be crossed: surfacing *what is out of range or
moving* is wellness/decision-support; asserting *what condition it is* is diagnosis
(FDA SaMD boundary, ADR-010). Choosing focus areas must be **rule-based and
transparent**, not an opaque model guess.

## Decision

Pick focus areas by **deterministic, explainable rules** over the user's own data,
framed as "worth attention", never "you have X".

1. **Vitals panel = the user's own latest values** with provenance, each shown
   against its reference range and trend (no invented vitals).
2. **Focus-area selection rules** (deterministic, ADR-007), each surfacing *why*:
   - a value **out of reference range** (especially newly so / a range crossing),
   - a **worsening trajectory** (sustained slope in the adverse direction),
   - a **red-flag** (handed straight to the Escalation Guardian, ADR-009),
   - a **stale critical marker** ("your last X is N months old" → retest, ADR-006).
   Risk *surfaces* (e.g. cardiometabolic clustering) may be shown as **context with
   their evidence tier**, never as a computed diagnosis.
3. **Ranked, capped, honest.** A few focus items, ranked by severity/recency, each
   citing the driving records (ADR-005) and labeled non-diagnostic (ADR-010).
4. **No advice beyond the data.** Items say "worth discussing with your clinician",
   never a treatment directive.

## Consequences

**Positive.** Transparent, auditable "what to focus on" from the user's own data;
stays firmly on the wellness side of SaMD; ties into escalation + retest prompts.
**Negative.** Rule curation is ongoing clinical-governance work; risk of over- or
under-flagging. **Mitigations.** Governance-reviewed rules (not Darwin-mutable for
safety thresholds); severity ranking; cite-everything.

## References
- FDA general-wellness vs. CDS/SaMD boundary (ADR-010). **[A]**
- ADR-009 (escalation), ADR-006 (tiering/abstention), ADR-007 (deterministic rules). **[A]**

> Architectural/product guidance, not legal or medical advice. Surfaces focus areas; never diagnoses.
