# ADR-045: Encrypted Credential Vault (Zero-Knowledge)

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001 (local-first vault), ADR-013 (on-device inference), ADR-037 (persistent vault), ADR-046 (agentic-browser acquisition), ADR-047 (single-tenant topology), ADR-049 (scheduled pulls)

---

## Context

Helix's "organic flow" of data (ADR-049) depends on automated pulls, and several of those
pulls authenticate as the user: OAuth tokens for API sources, and — for the sources with no
clean API — account username/password pairs driven through an agentic browser (ADR-046).
Those secrets have to live somewhere between pull runs.

The default engineering shortcut for "somewhere" is a plaintext `.env` file, a JSON config,
or the OS keychain in cleartext. For health-account credentials this is unacceptable. **[C]**
A single set of these logins is a master key to the user's entire health footprint — labs,
pharmacy history, weight and body-composition trends, food logs, and genome portals. A
plaintext secret is exposed by any one of: a lost or stolen laptop, a dotfile synced to a
cloud drive, a Time Machine backup, a misconfigured file permission, or — the classic —
accidentally committing it to a repository (ADR-047 keeps the private plane out of every
repo precisely to prevent this).

This is the *same failure class* that ADR-001 rejects for the health data itself. The data
is protected by an encrypted, user-owned vault; the credentials that unlock the accounts
feeding that vault deserve exactly the same protection. Today the data vault exists
(`helix-vault`, ADR-001/037); the missing companion is a vault for the secrets used to
fill it.

---

## Decision

**All account credentials live in an encrypted, passphrase-locked credential vault built on
the `helix-vault` crate — never a plaintext `.env`, config file, or cleartext keychain
entry.**

### Cryptographic construction

- **Master-key derivation**: **Argon2id** (memory-hard KDF, parameters aligned with ADR-001:
  m=65536 KiB, t=3, p=4) derives a 256-bit master key from the user's passphrase plus
  device entropy. **[A]**
- **Sealing**: each credential record is sealed with **XChaCha20-Poly1305** AEAD — the same
  primitive `helix-vault` already exposes via `seal` / `open` (192-bit nonce, misuse-resistant
  on mobile; authenticated, no separate MAC step). **[A — helix-vault/src/lib.rs]**
- **Persistence**: secrets are stored in a redb-backed `PersistentVaultStore`, gated behind
  the crate's **non-default `persist` feature** so the standard and wasm builds never compile
  filesystem/redb code (ADR-001 wasm-safety, ADR-037). **[A — helix-vault Cargo.toml]**

### Zero-knowledge property

Helix-the-company and Helix-the-app can **never** read a stored credential. Secrets are
decrypted **only in-memory, at pull time**, for the duration of a single connector run
(ADR-049), and the working copy is zeroized when the run completes. There is no server-side
copy, no telemetry of credential values, and no code path that exports plaintext. The
credential vault inherits ADR-001's key-custody model verbatim: **no server-side escrow, no
"forgot my password" recovery path** — key loss equals credential loss, not company-mediated
reset.

### Unlock model for unattended pulls

Because scheduled pulls (ADR-049) may run without the user present, the derived master key
MAY be cached for the session in the OS secure enclave (Secure Enclave / StrongBox), so the
vault is unlocked once per session rather than once per pull. This is user-configurable and
off by default; the raw passphrase is never cached.

---

## Consequences

### Positive
- **Security parity with the data.** The credentials that unlock a user's health accounts
  receive the same "can't, not just won't" protection ADR-001 gives the data itself.
- **No central credential store to breach.** Combined with ADR-047's single-tenant topology,
  there is no company-side pile of user logins for an attacker to target.
- **Repo-leak safe.** Because the vault is encrypted and lives in the per-user private plane
  (ADR-047), a leaked config or an accidental `git add` cannot expose usable credentials.

### Negative
- **Passphrase loss = credentials unrecoverable.** The user must re-enter each account. We
  recommend a user-held recovery key or Shamir recovery share, mirroring ADR-001, and prompt
  for one at setup.
- **One more unlock step.** A fully unattended scheduled pull (ADR-049) requires either the
  session-cached enclave key (above) or a user unlock; this is a deliberate friction/security
  tradeoff surfaced in the UX.

### Mitigations
| Risk | Mitigation |
|---|---|
| User loses passphrase | Mandatory recovery-key / Shamir-share prompt at setup (ADR-001) |
| Credential leaked in memory | Decrypt only at pull time; zeroize working copy immediately after |
| Unattended pull can't unlock | Optional session-scoped enclave-cached derived key (user opt-in) |
| Stale/rotated password causes silent auth failure | Re-auth flow triggered by connector `AuthExpired` (ADR-012), user re-enters credential |

---

## References

- `helix-vault` crate (XChaCha20-Poly1305 `seal`/`open`, `PersistentVaultStore` behind
  `persist`) — `crates/helix-vault/` **[A]**
- ADR-001 key-derivation hierarchy (Argon2id → KEK/DEK, no server-side escrow) **[A]**
- Argon2 RFC 9106 (Argon2id, memory-hard KDF) **[A]**
