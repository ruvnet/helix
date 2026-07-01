# ADR-056: Cognitum Seed as Personal Offline Knowledge Substrate

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-014 (ambient sensing — orthogonal), ADR-020 (WiFi-CSI sensing — orthogonal), ADR-001 (vault), ADR-013 (on-device inference), ADR-011 (federation exclusions), ADR-026 (on-device LLM analyst), ADR-053 (witness-chained provenance)

---

## Context

ADR-014 and ADR-020 already decided the Cognitum Seed's role as an ambient *sensing*
device (mmWave + WiFi-CSI vitals). This ADR addresses a **different, orthogonal** role
of the same physical/software platform: the Seed as the user's private, offline
**knowledge substrate** — a place Helix's own dossier data (not sensor readings) can
be mirrored to and queried from, independent of whether ambient sensing is enabled at
all. A user who never opts into sensing (ADR-014/020 are opt-in) could still pair a
Seed purely as an offline KB appliance — for example, for a household member without a
phone, or as a local, never-cloud backup target.

**Grounding check (mandatory, via the ruvnet brain) — confirmed [A], real and working,
not design-only:**
- **REST + MCP surface.** The Seed exposes a real HTTPS REST API (confirmed at
  `https://<device>:8443`, `https://cognitum.local:8443`, self-signed Device CA with
  TOFU TLS pinning implemented in the `cognitum-seed-client` Rust crate — "Typed REST +
  MCP client for Cognitum seed devices") alongside an MCP JSON-RPC tool surface wired
  for direct use from an MCP-aware client. **[A]**
- **RVF store with real export/import.** The Seed persists an on-device RVF vector
  store; `GET /api/v1/store/import/formats` and import endpoints exist, with a
  dimension mismatch returning a real `409` — confirming RVF import/export is a
  checked, working format on-device, not aspirational. **[A]**
- **Ed25519 identity.** `GET /api/v1/identity` returns a real per-device identity
  (used for the TLS cert SAN and mesh pairing); paired Seeds "share vectors and witness
  chains via delta sync" in a mesh — confirming both the identity primitive and that
  witness-chained sync (the mechanism ADR-053 proposes for Helix's own answers) already
  runs device-to-device on this platform. **[A]**
- **Genuinely offline-capable.** The Seed's own operator guide documents fully local
  operation (USB/loopback trust-bypass, no internet required to pair or use the local
  API; a dedicated `/api/v1/network/internet-check` endpoint exists precisely because
  offline is a first-class supported state). **[A]**

**What is not grounded:** Helix has no code today wiring its own dossier into a paired
Seed. This ADR proposes applying a real, working device capability to a new target — it
is not describing an existing Helix feature.

## Decision

**A paired Cognitum Seed is an optional, user-owned offline mirror/KB appliance for the
Helix dossier — a second storage tier, not a replacement for the primary vault, and
fully independent of the ambient-sensing decision.**

1. **Primary vault stays authoritative.** The phone/desktop vault (ADR-001) is always
   the source of truth; a paired Seed is an opt-in, eventually-consistent mirror.
2. **Export path.** The dossier (ProvRecords, evidence-tiered facts, the score
   decomposition — never more raw data than the user already owns) exports to the
   Seed's RVF store via the existing store-import REST endpoint, using the Seed's own
   dimension/format check as a data-integrity gate — the confirmed 409-on-mismatch
   behavior becomes Helix's protection against importing wrong-shaped data.
3. **Query path.** The Seed's MCP surface lets Helix's on-device LLM analyst
   (ADR-026) — or, for advanced users, any MCP-aware client — query the offline KB
   entirely on the local network, no cloud round trip, consistent with ADR-013's
   on-device-first posture.
4. **No new auth model.** Identity and pairing follow the Seed's existing model
   unchanged (Ed25519 device identity, USB/local-network pairing, bearer-token-per-
   Seed) — Helix is a client of the existing `cognitum-seed-client` contract, not the
   author of a parallel one.
5. **Explicitly orthogonal to ADR-014/020.** A user may adopt this KB role with
   ambient sensing fully disabled, and vice versa. UI copy must never conflate the two
   toggles — enabling one must not imply or require the other.
6. **Witness-chain carryover.** Because the Seed's mesh sync already carries witness
   chains device-to-device, a Seed-hosted mirror is a natural, low-additional-effort
   target for ADR-053's tamper-evident export trail — the same primitive, reused.

## Alternatives Considered

- **Sensing-only; don't use the Seed for KB purposes.** Rejected: leaves a real,
  already-available hardware capability unused and forecloses the no-phone-household-
  member use case that is cheaply available on the existing platform.
- **Build a separate, Helix-specific offline-KB appliance.** Rejected: duplicates
  hardware/firmware investment the Cognitum Seed platform already provides.
- **Bundle the KB role with enabling ambient sensing.** Rejected: conflates two
  genuinely separate decisions and would force a sensing opt-in on a user who only
  wants offline storage — undermining the voluntariness of the sensing consent model
  (ADR-014 Decision 7).

## Consequences

### Positive
- Reuses real, already-implemented device capability (REST+MCP, RVF import/export,
  Ed25519 identity, offline operation) for a genuinely useful, orthogonal purpose.
- Strengthens local-first positioning: a Helix household can run entirely without any
  cloud dependency, even for backup.
- Dovetails with ADR-053's witness-chain work at near-zero additional cost.

### Negative
- Adds a second storage tier to reason about (mirror vs. primary vault
  authoritativeness during conflicts).
- Risk of the exact sensing/KB conflation this ADR explicitly rejects, if UI copy is
  careless.
- The Seed's firmware/OTA update cadence (ADR-014 Open Question 5) now also affects KB
  availability, not just sensing accuracy.

### Mitigations
| Risk | Mitigation |
|---|---|
| Authoritativeness conflicts | Primary vault (ADR-001) always wins; Seed mirror documented as best-effort/eventually-consistent only |
| Sensing/KB conflation | Two independent settings toggles, named and described separately in the UI |
| OTA availability risk | Reuse ADR-014's required staged-rollout/rollback policy; it now also covers KB uptime |

## Open Questions

1. Conflict-resolution policy when the Seed mirror and primary vault diverge.
2. Can multiple household members' dossiers coexist on one Seed's RVF store, or does
   each person need a separate Seed?
3. How this interacts with ADR-011's federation exclusions — genomic/PII-sensitive
   records should stay off any mesh-synced Seed by the same default that excludes them
   from federation.

## References

- `cognitum-v0-appliance/crates/cognitum-seed-client/Cargo.toml` — "Typed REST + MCP
  client for Cognitum seed devices" **[A]**
- `cognitum-seed/src/cognitum-agent/src/guide.html` — REST API at `:8443`, Ed25519
  identity (`/api/v1/identity`), RVF store import with 409-on-dimension-mismatch, mesh
  vector + witness-chain delta sync, offline-first operation **[A]**
- `cognitum-v0-appliance/docs/adr/ADR-212-api-management-page.md` — confirms the MCP
  tool surface and its JSON-RPC `tools/list`/`tools/call` contract **[A]**
- Helix ADR-001, ADR-013, ADR-014, ADR-020, ADR-011, ADR-053

---

> Architectural/product guidance, not legal or medical advice. This ADR decides a
> storage/knowledge topology; it does not decide or expand ambient sensing, which
> remains governed exclusively by ADR-014/020.
