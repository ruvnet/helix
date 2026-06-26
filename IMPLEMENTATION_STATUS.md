# Helix Implementation — Status & Loop Ledger

**Branch:** `feat/helix-implementation` · **Started:** 2026-06-25 · **Driver:** `/loop 5m … until SOTA`
**Stack rule:** Rust only (CLAUDE.md). Mobile/3D client is out of scope for this workspace; we build the
testable, benchmarkable **core** the ADRs specify.

This file is the loop's memory. Each iteration: read it, do the next unchecked item, run
`cargo test --manifest-path helix/Cargo.toml`, update the ledger, keep going.

## Definition of "SOTA" (exit criteria)
A core is SOTA-complete for this loop when **all** hold:
1. Every load-bearing ADR (001–019) has a real, compiling Rust crate or a documented N/A (client-only).
2. `cargo test` green across the helix workspace; meaningful unit + property tests per crate.
3. `cargo clippy -- -D warnings` clean; `cargo fmt --check` clean.
4. Security pass: no `unsafe` without justification, input validation at boundaries, fuzz/property tests on parsers & numerics, `cargo audit` clean.
5. Benchmarks (criterion) exist for hot paths (numeric engine, grounding gate, ontology lookup) with recorded baselines.
6. An integration test wires the grounded-answer pipeline end to end (retrieve → numerics → ground → tier → escalate).

## Crate plan (ADR → crate)
| Crate | ADR | Status |
|-------|-----|--------|
| `helix-provenance` | 005 | ✅ implemented + tests |
| `helix-numeric` | 007 | ✅ implemented + tests |
| `helix-evidence` | 006 | ✅ implemented + tests |
| `helix-escalation` | 009 | ✅ implemented + tests |
| `helix-ontology` | 004 | ✅ implemented + tests |
| `helix-vault` | 001, 013 | ✅ implemented + tests (real AEAD) |
| `helix-verifier` | 008 | ✅ implemented + tests |
| `helix-core` (pipeline) | 002, 005, integration | ✅ implemented + 3 e2e integration tests |
| `helix-score` | 016 | ⬜ |
| `helix-router` | 019 | ⬜ (may wrap ruvector tiny-dancer) |
| sensing / twin / federation / darwin | 014/015/011/017/018 | ⬜ (spec/interface stubs; HW + client deferred) |

## Ledger
- **Iter 1 (2026-06-25):** branch created; helix Cargo workspace; `helix-provenance` (grounding gate,
  ProvRecord schema, type-level "no record → no claim") + `helix-numeric` (mean/delta/%change/OLS
  slope/range-crossings/pearson/CUSUM change-point), all with unit tests. Parent Cargo.toml excludes
  helix so it stays a detached, liftable workspace. ← build/test verification in progress.

- **Iter 2 (2026-06-25):** `helix-evidence` (ADR-006: Tier 1–4 enum mapped to CEBM, abstention gate with
  NoData/Stale/LowConfidence triggers + GapNotice, default staleness windows) and `helix-escalation`
  (ADR-009: versioned RedFlagThreshold registry with cited critical values — K⁺/Hb/glucose/SpO₂ + Seed
  screening REI, evaluate→level+suppress_optimization, unknown-code-never-assumed-safe). 33 tests green,
  clippy+fmt clean. Both depend only on helix-provenance / std.

- **Iter 3 (2026-06-25):** `helix-core` — the keystone grounded-answer pipeline `analyze()` composing all
  four primitives in safe order (abstain → escalate → deterministic numerics → ground → tier). Recommendation
  suppressed when escalation fires. 3 end-to-end integration tests prove the three outcomes: grounded+cited+
  trended answer (falling ferritin w/ range crossing), stale→abstain, critical K⁺→escalate+suppress.
  37 tests total green; clippy+fmt clean. Confirmed: parent workspace independent of helix even after the
  linter dropped the exclude lines (parent uses explicit members; helix has its own [workspace]).

- **Iter 4 (2026-06-25):** `helix-ontology` (ADR-004: CodeSystem enum w/ FHIR URIs, Domain→canonical-system
  map, `normalize()` gate that returns Normalized or Queued(LowConfidence/Ambiguous/NoCandidate) — never
  silently coerces, FHIR Coding round-trip). Added criterion bench `engine` for the numeric hot path
  (slope/range_crossings/change_point/pearson @ n=16/256/4096). **Security pass:** zero `unsafe` across all
  crates; `cargo audit` clean (54 deps, exit 0, no RUSTSEC); release profile lto+codegen-units=1. 44 tests
  green; clippy+fmt clean; bench compiles.

- **Iter 5 (2026-06-25):** `helix-verifier` (ADR-008: independent `verify()` gate — cross-family fusion
  invariant encoded as `ModelFamily` type [rejects verifier==synthesizer], `ClaimChecker` trait abstracts
  the different-family LLM, Informational=1-pass vs Clinical=odd-quorum≥3 majority consensus, Approved/
  DownGraded/Rejected dispositions). Added **CI gate** `.github/workflows/ci.yml` (fmt+clippy-deny+test+
  bench-compile + cargo-audit job) that ships with the standalone repo. 50 tests green; clippy+fmt clean;
  zero unsafe.

- **Iter 6 (2026-06-25):** `helix-vault` (ADR-001/013) — REAL AEAD boundary, not a stub. XChaCha20-Poly1305
  (192-bit random nonce, no reuse footgun), `SealKey` zeroize-on-drop + redacted Debug, `seal`/`open` with
  authentication (wrong key / tampered ciphertext → OpenFailed). The ADR-001 property as a TYPE: `VaultStore`
  holds only ciphertext and has no plaintext accessor (serializing the whole store never exposes plaintext —
  tested), and `UserKeyring` is the sole capability that can open. 58 tests; clippy+fmt clean; `cargo audit`
  clean across 75 deps incl. the crypto stack; zero unsafe in helix code.

## Next iteration picks (ordered)
1. `helix-score` (ADR-016): decomposable 0–100 score (subsystem sub-scores, each tracing to driving records,
   trend dir, confidence) — never black-box; versioned methodology.
2. Capture criterion baselines (run `cargo bench`); record measured ns/op table in ledger.
3. `COVERAGE.md` mapping every ADR (001–019) to crate-or-documented-N/A so SOTA exit criterion #1 is provably
   met; interface notes for the client/hardware ADRs (014/015/011/017/018/019) that can't be realized here.
4. Final SOTA sweep: re-check all 6 exit criteria, then propose closing the loop (CronDelete 4c424726).
