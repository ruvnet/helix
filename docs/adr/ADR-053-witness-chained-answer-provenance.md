# ADR-053: Witness-Chained Answer Provenance

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-005 (ProvRecord/`provenance_hash` — extended here), ADR-001 (vault/key material), ADR-008 (verifier verdicts, "prep for my appointment" export), ADR-052 (proof panel — the UI consumer), ADR-056 (Cognitum Seed KB — shares the same witness-chain substrate)

---

## Context

ADR-005 gives every measurement a `provenance_hash` — a SHA-256 fingerprint over key
fields, so a user can confirm a value was not silently mutated after ingestion. That is
a **per-record fingerprint**, not a **chain**: it proves one value wasn't altered, not
that the whole derivation trail behind a displayed claim — from raw import, through
retrieval, numeric computation, and Verifier pass — is tamper-evident end to end. That
stronger property matters most for the "prep for my appointment" export (ADR-008): a
document a clinician or the user's future self may want to independently verify was not
edited after the fact, without trusting a Helix server (there isn't one, ADR-001/047).

**Grounding (mandatory check, via the ruvnet brain):** the RVF ecosystem already ships a
real, working primitive for exactly this.
- The `rulake-witness` plugin computes a **SHAKE-256(32)** hash over stable bundle
  fields and refuses on mismatch (`WITNESS_MATCH` / `WITNESS_MISMATCH_REFUSED`),
  independently recomputable offline with no server. **[A — working code + CLI, `rulake/plugins/rulake-witness`]**
- RuVector's ADR-155 (ruLake, **Status: Accepted**, implemented) lists "Witness chains
  (SHAKE-256, Ed25519, ML-DSA-65 PQ)" among the RVF crates' already-shipped
  capabilities. **[A — accepted/implemented ADR in-repo]**
- QuDAG confirms **ML-DSA-65 (Dilithium3)** as a real, implemented post-quantum
  signature primitive in the same ecosystem. **[A]**
- The Cognitum Seed's own mesh sync already carries "vectors and witness chains via
  delta sync" between paired devices, and a cross-device provenance patent describes a
  hash-chain + immutable provenance-tag design for this exact purpose. **[B — patent
  application, design intent, novelty/approval scored, not a granted patent or a
  confirmed shipped feature of that specific patent claim]**

**What is not grounded:** Helix itself has not wired any of this into its own
ProvRecord or answer pipeline. This ADR proposes applying a real, existing,
cross-project primitive to a new (Helix-specific) target — it is not describing an
existing Helix capability.

## Decision

**Extend ADR-005's provenance schema with a chained, signed witness, reusing the
existing RVF witness-chain construction rather than inventing a new one.**

1. **Extend, don't replace.** Add a `witness_chain` field alongside ADR-005's
   `provenance_hash`: a SHAKE-256(32) hash over the ordered set of (ProvRecord hashes +
   query manifest + Verifier verdict set) for a given answer, chained to the prior
   witness — so tampering with *history*, not just one record, is detectable. Reuse
   the `rulake` `compute_witness` construction; do not reinvent hashing.
2. **Sign it.** Ed25519 (Helix's existing device-identity primitive, ADR-001/014) for
   standard integrity; offer ML-DSA-65 as an opt-in post-quantum tier for long-lived
   "prep for my appointment" exports (ADR-008) users may want verifiable years later.
3. **Surface via the Proof Panel (ADR-052), not a new UI.** A "Verify this trail"
   action inside the existing panel's Verification-state section, showing match/
   mismatch only — never raw hashes by default, mirroring ADR-005 Open Question 1's
   "hidden by default, view-source on demand" answer.
4. **Export-time verification.** The "prep for my appointment" export embeds the
   witness chain so a clinician or the user can independently recompute and confirm it
   later using the same public construction — no Helix server required, consistent
   with ADR-001/047's local-first posture.
5. **Scope.** Applies only to the "clinically meaningful" answer set already defined
   by ADR-008's consensus-required threshold — not every navigational response, to
   avoid needless signing overhead.

## Alternatives Considered

- **Fold this into ADR-052.** Considered, rejected as a merge: ADR-052 is a UX/
  consumption decision (what the user sees); this ADR extends the underlying data
  model ADR-005 defined (what is cryptographically true). Keeping them separate lets
  the crypto primitive (e.g., a future PQ-algorithm migration) be revisited without
  touching the UX ADR.
- **Roll a bespoke hash-chain instead of reusing rulake's construction.** Rejected:
  duplicates an already-implemented, accepted primitive and forfeits interoperability
  with the rest of the RVF ecosystem (the Cognitum Seed mesh already speaks this
  format — see ADR-056).
- **Sign every record at ingestion instead of chaining at answer-time.** Rejected for
  MVP: multiplies signing operations without adding chain-of-custody value across
  derivations; answer-level chaining captures the unit a user might actually dispute.

## Consequences

### Positive
- Tamper-evident provenance for the highest-stakes exports, built on a real,
  already-accepted primitive rather than a bespoke one.
- Strengthens the "prep for my appointment" audit trail with independent, offline
  verifiability — no trust in a Helix server required.
- PQ-signature option is future-proofed against algorithm migration.

### Negative
- Adds chain-maintenance and key-management complexity (Ed25519/ML-DSA on-device).
- Overclaiming risk: marketing must not call this "blockchain-verified" — the nova-
  medicina stub (ADR-055) is a cautionary example of a named pattern outrunning its
  actual implementation.
- ML-DSA-65 signatures are larger than Ed25519, adding export-size overhead.

### Mitigations
| Risk | Mitigation |
|---|---|
| Key management surface | Key material lives in the existing on-device vault (ADR-001); no new surface |
| Overclaiming in copy | One reviewed phrase only — "cryptographically tamper-evident, locally verifiable" — gated with ADR-010 copy review |
| Export size | ML-DSA offered opt-in for long-lived exports only, not the default |

## Open Questions

1. Periodic self-check vs. purely on-demand verification at export/view time.
2. Chain-amendment convention for corrected records (ADR-004 de-duplication, ADR-006
   conflicting-data abstention) — a correction needs a documented amendment, not a
   silent chain rewrite.
3. Whether the patent-pending cross-device provenance design should be treated as a
   dependency, or whether Helix should implement directly against the open
   `rulake-witness` construction to avoid patent entanglement — flag for legal review.

## References

- `rulake/plugins/rulake-witness/README.md`, `.../commands/rulake-verify.md` — SHAKE-256(32) witness, working CLI **[A]**
- RuVector ADR-155 (ruLake, Accepted) — "Witness chains (SHAKE-256, Ed25519, ML-DSA-65 PQ)" **[A]**
- QuDAG — ML-DSA-65 (Dilithium3) post-quantum signatures **[A]**
- `cognitum-platform-docs/patents/appliance/115-witness-provenance-crossdevice.md` — Patent Application I-005, cross-device witness provenance **[B — design intent, unimplemented/unwon patent, not confirmed shipped]**
- Helix ADR-005, ADR-001, ADR-008, ADR-052, ADR-056

---

> Architectural/product guidance, not legal or medical advice. Cryptographic
> tamper-evidence strengthens auditability; it is not a claim of diagnostic or clinical
> validity, and does not alter ADR-010's wellness/SaMD boundary.
