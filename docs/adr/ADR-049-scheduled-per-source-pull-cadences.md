# ADR-049: Scheduled Per-Source Pull Cadences with Auto-Refresh

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-012 (connector abstraction / graceful degradation), ADR-041 (connector clients), ADR-045 (credential vault), ADR-046 (agentic-browser acquisition), ADR-048 (AIMDS guardrails)

---

## Context

The product goal is an **organic flow**: the user should not have to remember to submit data.
They connect a source once, and thereafter it stays fresh on its own. Different sources,
however, change at very different rates and cost very different amounts to pull — heart rate
updates continuously, weight daily, pharmacy fills every few weeks, labs monthly, and the
genome exactly once. A single global sync interval would either hammer slow-changing sources
(wasting rate-limit budget and, for scraped sources, ToS goodwill) or starve fast-changing
ones.

Two ambient problems come with unattended pulls: **credentials expire** (OAuth access tokens
lapse; vaulted-credential browser sessions time out), and **duplicate work** (re-pulling data
already in the vault). Both must be handled without user intervention for the flow to feel
organic.

---

## Decision

**A local scheduler runs each connector on its own cadence, refreshes credentials
automatically, and pulls only new data into the encrypted event-map.**

### Local scheduler

Scheduling is **local-first** — launchd (macOS) or cron, on the user's own device — never a
company-operated job runner, consistent with ADR-047's single-tenant topology. Each connector
(ADR-012/041) is triggered on its own schedule; a run is one fault-isolated Ruflo Ingestion
agent, so one source failing or rate-limiting never blocks the others.

### Per-source cadence (defaults; user-configurable)

| Source | Cadence | Mechanism |
|---|---|---|
| Apple Health | Daily | Local push endpoint → on-device ingest (ADR-012) |
| RENPHO (body composition) | Daily | Vaulted-credential pull (ADR-045/046) |
| Lose It (nutrition) | Daily | API / vaulted-credential pull |
| Walgreens (pharmacy) | Bi-weekly | Agentic-browser scrape (ADR-046) |
| Labs / EMR | Monthly | FHIR API or PDF/OCR (ADR-012) |
| Genome | One-time | User-owned file import (ADR-001) |

### Auto-refresh of credentials

- **OAuth sources**: refresh tokens are used to auto-renew expired access tokens before each
  pull — standard OAuth 2.0 refresh flow, no user prompt on the happy path. **[A]**
- **Vaulted-credential / browser sources**: a saved session (ADR-046 `state-save`) is reused
  until it expires; on expiry the connector reports `AuthExpired` (ADR-012) and re-authenticates
  using credentials from the vault (ADR-045). Only a *hard* auth failure (changed password,
  MFA challenge) surfaces to the user.

### Incremental, sanitized ingest

Each run fetches only records newer than that connector's last sync watermark
(`fetch_since`, ADR-012) and merges **only new data** into the encrypted event-map — no
re-import of existing facts. All pulled content, especially anything scraped, first passes the
AIMDS/AIDefence gate (ADR-048) before it is written.

---

## Consequences

### Positive
- **Hands-off freshness.** Once connected, sources stay current with no manual submission —
  the organic-flow goal is met.
- **Rate-limit- and ToS-friendly.** Matching cadence to how fast a source actually changes
  minimizes API-quota burn and reduces the footprint of ToS-gray scraped sources (ADR-046).
- **Cheap incremental runs.** Watermark-based `fetch_since` keeps each pull small and the
  vault free of duplicates.

### Negative
- **Failure handling is essential.** Unattended runs must survive transient outages: the
  scheduler needs retry with exponential backoff and the ADR-012 circuit breaker so a
  down source is not hammered.
- **Rate limits and single-session caveats.** Some sources cap request rate, and some (e.g.
  the RENPHO API) tolerate only a single active session — a scheduled pull must not collide
  with the user's own app session. Cadence and locking must respect this. **[B — verify per
  source at build time]**
- **Silent staleness risk.** A repeatedly failing connector could quietly stop updating.
  Freshness/last-success state must be visible to the user, and hard auth failures must
  prompt.

### Mitigations
| Risk | Mitigation |
|---|---|
| Transient source outage | Retry + exponential backoff; ADR-012 circuit breaker |
| Rate-limit exhaustion | Per-source cadence + adaptive polling on HTTP 429 (ADR-012) |
| Single-session source collision (RENPHO) | Session lock; avoid pulling while user's app session is active |
| Expired OAuth token | Auto-renew via refresh token before pull (no user prompt) |
| Expired browser session | `AuthExpired` → re-auth from credential vault (ADR-045/046) |
| Silent staleness | Per-source last-success surfaced in UI; hard auth failure prompts user |

---

## Open Questions

1. Should cadence adapt automatically (e.g. back off a source that rarely returns new data,
   speed up one that changes more than expected), or stay user-set? Lean toward a sensible
   default with user override.
2. What is the exact single-session behavior of each vaulted-credential source (RENPHO in
   particular), and how do we detect a concurrent user session to avoid eviction?
