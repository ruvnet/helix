# ADR-047: Single-Tenant, Local-First Product Topology

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001 (local-first vault), ADR-011 (federation), ADR-013 (on-device inference), ADR-037 (persistent vault), ADR-045 (credential vault)

---

## Context

Helix begins as one person's build: the owner is using their own health data to prove the
system out. That raises an architectural question that will shape everything downstream —
**how do we separate "the owner's personal prove-out" from "the product other people run"?**

The conventional SaaS answer is multi-tenancy: one company-operated backend, many users
partitioned by tenant ID, all data resident on company infrastructure. For Helix that answer
is not merely disfavored — it is **structurally incompatible** with ADR-001. A local-first,
user-owned, zero-knowledge vault means the company holds no plaintext and no keys; there is
nothing to multi-tenant, because there is no central store of user data by design. Bolting a
multi-tenant backend on top would re-introduce exactly the breach-and-bankruptcy target that
ADR-001 (and the 23andMe event) exists to eliminate.

---

## Decision

**Helix is not multi-tenant. Every user runs their own single-tenant instance on their own
device. The owner's personal instance is simply "user #1" — the same software, no special
tenancy.**

### Two planes

The system cleanly separates into two planes, and they never mix:

- **Shared open engine (public plane).** The code — connectors, normalization, the analyst,
  the trend engine, the vault crates — lives in a public repository and contains **zero user
  data**. This is what everyone runs; improvements benefit every user. The engine is
  data-free by construction.

- **Per-user private plane.** Everything with a person in it: the **data vault** (ADR-001/037),
  the **credential vault** (ADR-045), the user's **RVF assets** (health knowledge graph,
  embeddings), and the **agentic-browser auth-state** (ADR-046). This plane is **encrypted,
  device-resident, and never present in any repository** — public or private.

The owner's PHI therefore lives entirely in the owner's private plane, alongside their own
copy of the public engine. "User #1" is a role, not a privilege: no code path treats the
owner's instance differently from any other user's.

### What this rules out

- No company-operated database of user health data.
- No tenant partitioning, tenant IDs, or per-tenant row-level security — because there is no
  shared store.
- No server-side "admin" that can read across users. Cross-user value is possible **only**
  through ADR-011's opt-in, PII-stripped, differentially-private federation — never through
  direct access.

---

## Consequences

### Positive
- **Radical privacy and trust.** There is no central breach target and no corpus to sell in
  a bankruptcy (ADR-001). The privacy claim is architectural, not a promise.
- **Clean open/closed split.** The engine can be fully open-source and community-improvable
  precisely because it carries no data; contributors never touch anyone's PHI.
- **Owner is a real user.** Dogfooding is genuine — the owner runs the exact instance a
  stranger would, so bugs and UX friction surface on the same path everyone uses.

### Negative
- **No server-side conveniences.** No central analytics, no server-side ML over the whole
  population, no company-mediated password reset, no "log in from any browser." These are
  deliberately forgone; population-level signal comes only via ADR-011 federation.
- **Backup and sync are the user's responsibility.** With no company cloud of record, each
  user must arrange their own encrypted backup and cross-device sync (the ADR-001 sync model:
  user-provided storage, encrypted before it leaves the device). Poor UX here is the top
  friction risk.
- **Support is constrained.** The company cannot inspect a user's data to debug their
  instance; diagnostics must be local-first and privacy-preserving.

### Mitigations
| Risk | Mitigation |
|---|---|
| User loses device with no backup | Guided encrypted-backup setup at onboarding (ADR-001 sync model) |
| Cross-device continuity | User-controlled encrypted sync; validated key transfer before any data moves |
| Debuggability without data access | Local, on-device diagnostics; opt-in, PII-stripped telemetry only |
| Population insight without central store | ADR-011 opt-in federated cohort (differentially private) |

---

## Open Questions

1. Does the public engine need a hard CI guard that fails the build if any file resembling
   private-plane data (vault files, `.rvf` with PHI, credential blobs) is staged? Strongly
   lean yes.
2. For multi-device users, is per-user sync a first-party feature of the engine or a
   documented "bring-your-own-storage" pattern? Affects onboarding scope.
