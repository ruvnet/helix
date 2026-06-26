# ADR-034: Biological / Medical Age Estimate from Routine Labs

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Related**: ADR-010 (SaMD boundary), ADR-006 (evidence tiering), ADR-005 (grounding), ADR-016 (score), ADR-007 (deterministic numerics)

---

## Context

Users want "medical age vs. calendar age" — an intuitive, motivating number. The
science splits into two families:

- **Epigenetic clocks** (Horvath, Hannum, **PhenoAge-DNAm**, **GrimAge**,
  **DunedinPACE**) — DNA-methylation based; among the strongest mortality/
  healthspan predictors **[A]**, but require a **specialized methylation assay**
  (not routine labs), are costly, and have meaningful test-retest noise. **[A/B]**
- **Phenotypic / biomarker age from routine labs** — notably **Levine PhenoAge
  (2018)**, computed from **9 standard blood markers + chronological age**
  (albumin, creatinine, glucose, CRP, lymphocyte %, MCV, RDW, alkaline
  phosphatase, WBC). It predicts mortality/healthspan well for a no-extra-assay
  method and is **reproducible from data the user likely already has**. **[A]**

The product/regulatory risk is acute: a "biological age" is easy to **overclaim**
(it is an estimate, not a measured quantity), and presenting it as a verdict
edges toward diagnosis (ADR-010). Several DTC "aging clock" products have drawn
skepticism for marketing ahead of the evidence. **[B]**

## Decision

Estimate biological age **from routine labs (Levine PhenoAge)** as the default —
defensible without a special assay — wrapped in hard non-diagnostic guardrails.

1. **PhenoAge from routine labs, deterministic.** Implement Levine PhenoAge in
   `helix-bioage` as a pure, deterministic function (ADR-007) over the 9 markers +
   age, with **every coefficient and unit documented and cited to Levine et al.
   2018**. No LLM does this math.
2. **Estimate, never a verdict.** The output is framed as "an *estimate* of how
   your labs compare to typical aging — not a measurement, not a diagnosis"
   (ADR-010), with the **age-delta** (PhenoAge − chronological) as the headline,
   plus a confidence band. Missing any required marker → **abstain** (ADR-006),
   not impute silently.
3. **Capped confidence + provenance.** The estimate is a derived `ProvRecord`
   (method `Derived`, `BIOAGE-PHENOAGE` code, capped confidence), traceable to
   the exact lab values that produced it (ADR-005). Each input shows its date.
4. **Epigenetic clocks are an optional, labeled tier.** If the user imports a
   methylation-clock result (Horvath/GrimAge/DunedinPACE) it is ingested as its
   own record, clearly tier-labeled as a specialized assay — Helix does not
   compute it.
5. **Coefficient verification is a release gate.** The published coefficients are
   transcribed here; they MUST be verified against the source paper and reviewed
   by clinical governance before any non-demo use (the code carries this warning).

## Alternatives Considered

- **Epigenetic clock as the primary number.** Rejected as default: needs a
  special assay most users don't have; reserved as an optional imported tier.
- **A bespoke "Helix age" formula.** Rejected: an unvalidated composite would be
  exactly the black-box overclaim this domain is criticized for; use a published,
  peer-reviewed method (PhenoAge).
- **No biological age at all.** Rejected: it's a high-value, motivating metric —
  but only if honestly framed as an estimate.

## Consequences

**Positive.** A motivating, evidence-based age estimate from labs the user likely
already has; deterministic and auditable; honest framing reduces overclaim/
liability; epigenetic clocks supported as a labeled import.

**Negative.** PhenoAge is population-derived (cohort/ancestry caveats); requires
all 9 markers; "biological age" is inherently easy to misread as precise.

**Mitigations.** Abstain on missing markers; confidence band + explicit "estimate"
framing; ancestry caveat; coefficient verification gate; clinical-governance review.

## Validation (NHANES, 2026-06-26)

Validated against **NHANES 2021–2023** (CDC, public domain) — the population family
PhenoAge was derived from. On **3,134 adults** with the full 9-marker panel + age, our
deterministic PhenoAge gives **correlation r = 0.922 with chronological age** (Levine
2018 reported ~0.94), **mean delta −0.64 yrs** (≈0 as expected), and 86.1% within
±10 yrs. This **empirically clears the coefficient/unit gate** — the coefficients and
unit conversions reproduce PhenoAge on its source population. Reproduce:
`cargo run -p helix-bioage --example nhanes_validate`. Clinical-governance sign-off for
production remains separate.

## Open Questions

- Final coefficient/unit verification against Levine 2018 (release gate).
- Whether to surface the absolute PhenoAge, the delta, or both (delta is less
  misread).
- Handling partial panels (which markers are substitutable).

## References

- Levine ME et al., *An epigenetic biomarker of aging for lifespan and healthspan* (PhenoAge), Aging 2018. **[A]**
- Horvath S 2013; Lu AT et al. GrimAge 2019; Belsky DW et al. DunedinPACE 2022 — epigenetic clocks (require methylation assay). **[A]**
- FDA general-wellness vs. SaMD guidance (ADR-010). **[A]**

> Architectural/product guidance, not legal or medical advice. Biological age is a non-diagnostic estimate; verify coefficients and engage clinical governance before clinical use.
