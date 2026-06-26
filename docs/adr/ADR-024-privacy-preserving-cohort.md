# ADR-024: Privacy-Preserving Cohort Feature Extraction (federation primitive)

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-011 (federation), ADR-001 (vault), ADR-005 (provenance), ADR-023 (vectors), §7.4

---

## Context

ADR-011 wants opt-in, PII-stripped **cohort intelligence** ("people like you who
corrected low vitamin D saw X") via Ruflo federation — but it is currently a
documented N/A because its hardest requirement is unbuilt: *"aggregation/
anonymization must be genuinely robust (re-identification risk is real)."*
Re-identification from "anonymized" health/genetic data is a known hazard (§7.4),
so the **anonymization primitive must exist and be testable before any
federation transport ships**.

RuVector provides the vector substrate (ADR-023) over which cohort similarity
would run; rvDNA (ADR-021) even ships 64-dim genomic profile vectors for cohort
matching. But none of that is safe to contribute upstream until the **local
generalization + differential-privacy step** is in place: the client must
transform its dossier into a feature vector that is *safe to leave the device*,
or contribute nothing.

## Decision

Implement the **local cohort-contribution primitive** (`helix-cohort`) — the
thing ADR-011 federates — with the privacy guarantees enforced *in the crate*,
before any network transport:

1. **Generalize, never raw.** Continuous values are coarsened to non-identifying
   **bands** (e.g. "vitamin D: deficient/insufficient/sufficient"), never raw
   numbers, dates, free text, or record ids. Raw `ProvRecord`s never leave the
   vault (ADR-001).
2. **Cell-suppression (local k-anonymity proxy).** Each generalized feature
   carries an estimated cohort-cell size; if it is **below the k threshold**, or
   the feature is a flagged quasi-identifier that is rare, that feature is
   **suppressed**. Better to contribute less than to contribute something
   identifying.
3. **Differential privacy.** Surviving numeric features get **Laplace noise**
   calibrated to an **ε budget split across features** (sensitivity-normalized).
   The mechanism is explicit and the spent ε is reported.
4. **Refuse-when-unsafe.** If nothing survives generalization + suppression, the
   primitive returns an error and contributes **nothing** — the same
   refuse-when-unknown discipline as ADR-005, applied to privacy.
5. **Opt-in + genomics excluded.** Cohort contribution is opt-in (ADR-011) and
   **genomic-derived features are excluded by default** (ADR-021/GINA).
6. **Injected randomness, deterministic tests.** The DP noise source is a trait,
   so production uses a CSPRNG while the policy (generalization, suppression,
   ε accounting) is pure and exhaustively testable.

## Alternatives Considered

- **Contribute generalized features without DP.** Rejected: generalization alone
  is not robust against linkage/auxiliary-data attacks; DP gives a formal bound.
- **Server-side anonymization only.** Rejected: that means raw-ish data leaves
  the device first — exactly the §7.4 / ADR-001 failure mode. Privacy must be
  enforced *before* egress.
- **k-anonymity only (no DP).** Rejected: k-anonymity is brittle alone
  (homogeneity / background-knowledge attacks); pair cell-suppression with DP.

## Consequences

**Positive.** ADR-011 graduates from N/A to "the privacy primitive is built and
tested"; the network-effect intelligence becomes reachable on a sound footing;
the ε budget makes the privacy cost explicit and auditable.

**Negative.** Generalization loses signal (coarse bands); DP noise reduces
utility; the cohort-cell-size estimate needs a source (local prior / federated
count). Genomics excluded narrows cohort richness (by design).

**Mitigations.** Tune ε per the privacy/utility trade-off with governance review;
report spent ε so it is visible; allow advanced users to widen scope explicitly;
the suppression default errs toward sharing less.

## Open Questions

- Source of cohort-cell-size estimates (local model vs. a privacy-preserving
  federated count).
- ε budget policy and how it accumulates across repeated contributions.
- Which features are quasi-identifiers in the health context (curate the list).

## References

- ADR-011 (federation), §7.4 (re-identification hazard), ADR-001, ADR-021 (genomics exclusion). **[A]**
- Dwork & Roth, *The Algorithmic Foundations of Differential Privacy* — Laplace mechanism. **[A]**
- ruvnet/ruvector — vector substrate for cohort similarity (ADR-023). **[A]**

> Architectural/product guidance, not legal or medical advice. Cohort contribution is opt-in; privacy is enforced before any data leaves the device.
