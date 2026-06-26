# ADR-029: Live Connector Clients — FHIR/SMART + Wearables (Rust, sandbox-first)

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-012 (connector degradation), ADR-004 (ontology), ADR-005 (provenance), ADR-001 (vault)

---

## Context

ADR-012 defined the connector *contract* (graceful degradation: live API → export
→ PDF/OCR). The live API tier itself — actual FHIR/SMART-on-FHIR and wearable
OAuth clients — was deferred because the **real endpoints require partner
agreements, app registration, and per-provider onboarding** that can't be
completed in a workspace. But the *buildable* parts are substantial and worth
shipping now: the request/response **shapes**, the **FHIR→ProvRecord parsing**,
the OAuth token model, retry/rate-limit handling, and the **degradation ladder**
itself — all testable against a **sandbox/mock** without any partner credential.

## Decision

Add `helix-connect`: Rust connector clients written against a transport trait, so
the parsing, normalization, and degradation logic are fully built and tested now,
and a real HTTP transport (with partner auth) drops in later.

1. **Transport trait.** `HttpTransport` abstracts the network. Production uses a
   real client + per-provider auth; tests use a deterministic sandbox returning
   canned FHIR/wearable payloads. No partner credential is needed to build or test.
2. **FHIR Observation → ProvRecord.** Parse FHIR R4 `Observation` resources into
   provenance records: LOINC code (ADR-004), value + UCUM unit, effective date,
   reference range, source system — feeding the same pipeline as every other
   source (ADR-005). Un-parseable resources go to the review queue (ADR-012).
3. **Degradation ladder, encoded.** A `Connector` tries the live API; on auth
   failure / rate-limit / unavailability it reports the next fallback tier
   (export, then PDF/OCR via `helix-ocr`, ADR-022) rather than failing — ADR-012
   made executable.
4. **OAuth token model.** A typed `OAuthToken` (access/refresh/expiry/scopes); the
   client refreshes when expired. **Tokens are secrets** — never logged, held only
   in the user's vault (ADR-001). (The crate models the flow; the actual
   authorization redirect is a client/app concern.)
5. **Provenance + rate-limit honesty.** Every imported record carries its source
   and timestamp; the client surfaces rate-limit/partial-import state instead of
   silently dropping data.

## Alternatives Considered

- **Wait for partner agreements to build anything.** Rejected: the parsing,
  normalization, and degradation logic are the hard, testable parts and don't need
  a credential — build them now behind the transport trait.
- **One bespoke client per provider, no abstraction.** Rejected: O(n) maintenance;
  the FHIR/SMART standard + a transport trait give one tested core.
- **Trust the API and skip the degradation ladder.** Rejected: ADR-012 exists
  because these APIs are gated and flaky; the fallback is the product's resilience.

## Consequences

**Positive.** The connector core (parse, normalize, degrade, token-refresh) is
fully built and tested with no partner credential; a real transport + auth is a
small, well-scoped addition; FHIR Observations flow into the dossier with full
provenance.

**Negative.** The live network/auth path remains unbuilt (genuinely needs partner
onboarding); FHIR is sprawling — coverage is incremental; wearable APIs vary.

**Mitigations.** Sandbox transport keeps the logic honest and CI-testable;
incremental resource coverage behind the review queue; the trait isolates the
partner-specific auth.

## Open Questions

- Which providers first (driven by partner availability, not code).
- FHIR resource coverage beyond `Observation` (Condition, MedicationRequest…).
- Real HTTP client + async vs. blocking, and where token storage binds to the vault.

## References

- HL7 FHIR R4 `Observation`; SMART on FHIR authorization. **[A]**
- Helix ADR-012 (connector degradation; Quest/Labcorp have no consumer APIs), ADR-004, ADR-022 (OCR fallback). **[A]**

> Architectural/product guidance, not legal or medical advice. Live partner integrations require their agreements; this builds the testable core.
