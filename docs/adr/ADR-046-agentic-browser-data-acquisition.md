# ADR-046: Agentic-Browser Data Acquisition for No-API Sources

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-012 (connector abstraction / graceful degradation), ADR-041 (connector clients), ADR-045 (credential vault), ADR-047 (single-tenant topology), ADR-048 (AIMDS guardrails), ADR-049 (scheduled pulls)

---

## Context

The connector matrix (ADR-012) is honest about a hard reality: several of the most valuable
sources have **no clean consumer API**. Walgreens and comparable pharmacy portals expose
medication history only to their own apps; many EMR/patient portals gate structured export;
and consumer devices such as RENPHO surface body-composition history primarily through their
own app or the web, not a documented developer API. **[B — verify per source at build time]**

ADR-012's degradation ladder already anticipates this — Tier 2 (user-initiated export) and
Tier 3 (PDF/OCR) exist for exactly these sources. But both tiers still require the user to
*manually* log in, navigate, and export, which defeats the "organic flow, no manual
submission" goal of ADR-049. We need a way to perform the login-and-export a human would do,
automatically, **on the user's own device**, without shipping their credentials anywhere.

---

## Decision

**Use a local, headless agentic browser to acquire data from no-API sources: it logs in with
vaulted credentials (ADR-045), navigates, triggers the source's own export/scrape, and saves
session state so it need not re-login every run.**

### Mechanism

- **Runtime**: rUv's agentic browser stack — `agent-browser` / `@claude-flow/browser` —
  running **locally and headless** on the user's device. **[A — present in the Helix/ruvnet
  substrate]** It exposes login, navigate, click, fill, extract, and screenshot primitives
  suitable for driving a portal the way a person would.
- **Credentials**: pulled from the encrypted credential vault (ADR-045), decrypted only
  in-memory for the run, and typed into the login form. Credentials **never leave the
  machine** — this is a local browser session, not a cloud scraper.
- **Session persistence**: after a successful login the browser's authenticated session
  state is captured (`state-save` / cookie + storage snapshot, itself sealed via the vault)
  so subsequent runs resume without re-authenticating, and re-login is triggered only on
  expiry (ADR-049's re-auth path).
- **Connector shape**: each agentic-browser source is a Tier-2/Tier-3 `HelixConnector`
  (ADR-012/041). It reports the same `ConnectorHealth`, `AuthExpired`, and `Degraded` states
  as any other connector, so it participates in fault isolation, circuit-breaking, and the
  degradation ladder unchanged.

### Trust boundary

**All scraped page content is treated as UNTRUSTED input.** A logged-in portal page is
attacker-influenceable (injected text in a message center, a malicious PDF, a manipulated
field). Every byte extracted by the agentic browser is routed through the AIMDS/AIDefence
guardrails of ADR-048 **before** it reaches the FM Analyst or any LLM, and before it is
written to the vault with its provenance marker.

---

## Consequences

### Positive
- **Unlocks the no-API sources** (Walgreens, RENPHO, gated portals) that the API-only path
  cannot reach — turning ADR-012's Tier-2/Tier-3 sources into hands-off pulls (ADR-049).
- **Credentials stay on-device.** Because the browser runs locally, this preserves ADR-001's
  local-first guarantee and ADR-047's "no central data/credential target" property.
- **Uniform connector semantics.** Agentic-browser sources plug into the existing registry,
  provenance, and degradation machinery without special-casing.

### Negative
- **ToS-gray for some providers.** Automated login/scrape may conflict with a source's terms
  of service. This requires a **per-source legal check** before shipping that connector;
  some sources will be user-initiated-only or excluded. **[C]**
- **Brittle to site changes.** Selectors and flows break when a portal redesigns. This is a
  first-class `Degraded` condition — the connector falls back down the ADR-012 ladder (to
  user export or PDF/OCR) rather than failing the whole pull.
- **Scraped-content injection risk.** A logged-in page can carry prompt-injection or
  malicious payloads aimed at the analyst LLM. Mitigated by routing all scraped content
  through ADR-048 before it reaches the model or the vault.

### Mitigations
| Risk | Mitigation |
|---|---|
| Provider ToS conflict | Per-source legal review; user-initiated-only or exclude where required |
| Portal redesign breaks scrape | `Degraded` → graceful fall-through to Tier-2/Tier-3 (ADR-012) |
| Injected/malicious page content | Mandatory AIMDS/AIDefence pass on all scraped content (ADR-048) |
| Session-state theft | Saved session state sealed in the credential vault (ADR-045), never plaintext |

---

## Open Questions

1. Which no-API sources are ToS-permissible for automated access vs. user-initiated-only?
   Build the per-source disposition table before Phase-3 rollout.
2. Should the agentic browser run under the same scheduler process as OAuth pulls (ADR-049),
   or in an isolated sandbox to contain a compromised page? Lean toward isolation.
