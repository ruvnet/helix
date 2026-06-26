# ADR-030: Federation Transport — Opt-In Cohort Contribution (Rust, privacy-gated)

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-011 (federation), ADR-024 (cohort privacy primitive), ADR-001 (vault), ADR-013 (on-device), §7.4

---

## Context

ADR-024 built `helix-cohort`: the local **privacy primitive** that turns a
dossier into a contribution-safe vector (generalize + cell-suppression + DP) or
contributes nothing. ADR-011 wants opt-in cohort intelligence ("people like you
saw X"). The missing piece is the **transport** that actually moves a
contribution and returns aggregate cohort signals — but it must be built so the
privacy gate (ADR-024) is *unbypassable* and consent is explicit (ADR-011).

The real federation *network* (a running aggregator, peers, trust) is
infrastructure that doesn't exist in a workspace. But the **client side** — the
consent check, the hard gate that only a `CohortVector` (already generalized + DP-
noised) may leave, the contribution envelope, and the aggregate-signal model — is
fully buildable and testable behind a transport trait.

## Decision

Add `helix-fed`: the federation client, with privacy enforced **before** any
egress.

1. **Only a `CohortVector` leaves — by type.** The contribution function accepts
   *only* a `helix_cohort::CohortVector` (the output of the ADR-024 gate). Raw
   records, embeddings, or un-noised features **cannot be passed** — there is no
   API that sends them. The privacy gate is the only door (ADR-001/024).
2. **Explicit opt-in, per contribution.** A `Consent` token (scope + expiry) is
   required; without it, `contribute` refuses. Opt-out is the default (ADR-011).
3. **Genomics excluded at the door too.** ADR-024 already excludes genomic
   features; the envelope re-asserts it (defense in depth).
4. **Transport trait.** `FedTransport` abstracts the network — a local stub /
   in-memory aggregator for tests, a real signed-dispatch transport (Ruflo
   federation, Ed25519) later. No live network needed to build/test.
5. **Aggregates in, never individuals.** The return type is a `CohortSignal`
   (cohort size + an aggregate stat + the ε that was spent) — never another
   person's data. A signal is context for the analyst (ADR-005 still grounds the
   user's *own* answer); cohort signals are labeled Tier-3-ish population context,
   never the user's Tier-1 data.

## Alternatives Considered

- **Send features and anonymize server-side.** Rejected outright — raw-ish data
  leaving the device is the §7.4 / ADR-001 failure mode. Privacy is enforced
  *before* egress, by type.
- **Implicit/always-on contribution.** Rejected: ADR-011 is opt-in; consent is
  per-contribution and expires.
- **Wait for the live aggregator to build the client.** Rejected: the gate,
  consent, envelope, and signal model are the privacy-critical, testable parts —
  build them now behind the transport trait.

## Consequences

**Positive.** The federation client is built so privacy is structurally
unbypassable (only a DP-noised `CohortVector` can be sent); consent is explicit;
genomics excluded twice; a real signed transport is a localized addition.

**Negative.** The live aggregator/peer network and its trust model are unbuilt
(genuinely infrastructure); aggregate-signal utility depends on a real cohort;
ε-budget accounting across contributions needs a persistent ledger.

**Mitigations.** Transport trait + in-memory aggregator keep it CI-testable; the
type system blocks raw egress; ε is reported per contribution; opt-out default.

## Open Questions

- Real transport: Ruflo federation signed dispatch (Ed25519) vs. a dedicated
  aggregator service; trust model for peers.
- Persistent ε-budget ledger across sessions (privacy accounting over time).
- Cohort-signal evidence tier + how it's visually distinguished from the user's
  own data in the UI (must never look like Tier-1).

## References

- Helix ADR-011 (federation), ADR-024 (cohort privacy primitive), ADR-001 / §7.4 (the 23andMe lesson). **[A]**
- Ruflo federation (signed dispatch, Ed25519) — the eventual real transport. **[A]**

> Architectural/product guidance, not legal or medical advice. Contribution is opt-in; only differentially-private, generalized vectors ever leave the device.
