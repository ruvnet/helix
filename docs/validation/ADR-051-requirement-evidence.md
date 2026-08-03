# ADR-051 requirement-to-evidence trace

Status: partial foundation evidence only  
Reviewed: 2026-08-03  
Pinned bases: Helix `87ff0e151a3e8e1b52ac9e07cc03673668ada756`; rUv Neural
`caaa14144a70829293737b0ca717ebc818fcc523`

This trace deliberately does not assert completion of ADR-051 or AT-01 through
AT-12. It records what can be reproduced from the H0, N1, N2, N3, H1, H2, and
H3 changes and names the evidence still required from data validation and
release governance.

One earlier result in this document was withdrawn rather than amended: the
static safety and prohibited-claim scripts were reported as passing when they
had in fact executed no checks at all. The correction is recorded under
"Reproduced targeted results" and the affected AT-10 and AT-12 rows say so
explicitly, because a retracted gate is more important to a later reader than a
tidy table.

## Requirement status

| Requirement | Current evidence | Status / missing evidence |
|---|---|---|
| R1 bounded, stage-aware import | Contract carries source, acquisition, and stage-source metadata. N2 adds a bounded streaming EDF/EDF+ parser, explicit source/allocation limits, terminal-error behavior, TAL parsing, unit tests, and a fuzz target/corpus. | **Partial.** BrainVision remains a compatibility re-export rather than the bounded adapter described by the ADR. No expert-hypnogram alignment or hash-addressed 197-recording Sleep-EDF result manifest exists, and the fuzz target has not been run with `cargo-fuzz` on this machine. |
| R2 reproducible qEEG and aperiodic DSP | Closed feature registry and signed algorithm manifest exist upstream. N3 adds masked Welch PSD, trapezoidal integrated band power in explicit `uV2`, shared-mask coherence, an interpolated theta peak, a FOOOF-compatible knee fit with typed failures, rail-aware quality gates, the annotation-first `ruv-neural-neurosleep` profile, and executable criterion benchmarks for both the DSP path and a synthetic scored night. | **Partial.** Implementation and self-consistency tests exist (band power recovers a known sine's power, the theta peak lands within 0.1 Hz, the knee model recovers its own parameters). **SciPy/FOOOF numeric parity is still unproven** — no pinned reference fixtures exist in either repository, so AT-02 and AT-03 are explicitly not claimed and shadow parity over >=50 nights remains a prerequisite. |
| R3 fully bound payload and trusted origin | RFC 8785 payload hash, external persistent signer, separate trust fixture, exact domain separation, strict serde contract, upstream tamper tests, independent serialized tamper matrix, and H1 independent verification with enrolled study/subject signer binding, time-aware revocation at ingestion, consent checks, replay conflict handling, and atomic dispositions. | **Partial.** The changed upstream contract is still an unpublished sibling path labelled `0.1.0`; it needs a version bump, release, and exact released dependency. Persistent revocation propagation/quarantine after an accepted record is stored remains absent. |
| R4 method-compatible longitudinal analysis | Helix H0 enforces generic ADR-007 minima and adds `electrophysiology` provenance. H1 binds enrolled compatibility identifiers to canonical acquisition and algorithm digests, groups fingerprints, and applies typed 7/14/20/10-per-side nightly gates. The coordinated contract now uses integrated `uV2` for absolute band power and recomputes the compatibility fingerprint. | **Partial.** This is gate assessment, not the deterministic baseline/slope/correlation/change-point analyst or a validated normalization bridge. The corrected contract and fixture still require a version bump, upstream release, and exact downstream release pin before alpha. |
| R5 research-only interpretation and one-way boundary | ADRs define prohibited flows; H1 has a closed scalar/output model with no disease probability, recommendation, alert, score, or stimulation field. Static boundary scripts reject NeuroSleep dependencies and identifiers in current actuation and generic consumer paths. The redesigned H2/H3 prototype adds default-off flags, a visualization-only WASM crate, exact client-reconstructed copy and field domains, fixed method/source provenance, text-node rendering, and injection tests. Its public request accepts only flags and identity-free native verified-night views; bundle, trust, policy, consent, signer, and vault-key fields are rejected. | **Partial.** The former caller-supplied trust-root defect is closed. The verified-night JSON itself is not authenticated at the WASM edge, however, so arbitrary JavaScript can forge a view; this is a trusted-host visualization adapter, not independent verified WASM ingestion. An opaque native handle or signed receipt with a pinned verifier, hosted-model spy, and end-to-end negative-flow proof remain absent. |
| R6 local privacy, consent, and study authorization | Raw waveforms are absent from the N1 derived contract. H1 enforces study/subject/purpose consent and signer binding, opaque HMAC keys, replay/idempotency policy, and atomic sealing into an in-memory vault store; leakage and injected-seal-failure tests cover that implementation. | **Partial.** The storage path is not persistent, and withdrawal, deletion, export ledger, recovery, and post-ingest revocation quarantine are not implemented. The in-memory evidence therefore does not satisfy the ADR's persistent sealed time-series release gate. |
| R7 native/WASM/performance parity | Upstream locked workspace and existing WASM build are CI-covered. A physically separate `helix-neurosleep-wasm` prototype emits an identity-free view model and defaults every research capability off; its `wasm32-unknown-unknown` release build passes. | **Partial.** The WASM surface deliberately does not ingest or verify bundles, and its input lacks authenticated native provenance. No browser execution test or native/WASM byte-parity fixture exists, and there is no 8-hour/24-hour latency/RSS or bundle-admission benchmark. |

## Acceptance-test trace

| Test | Status | Reproducible evidence or next test location |
|---|---|---|
| AT-01 | Partial | N3 adds synthetic power, theta-peak, knee-recovery, quality-gate, and masked-coherence tests in `ruv-neural-signal`, plus staging/aggregation tests in `ruv-neural-neurosleep` (508 workspace tests pass). These are self-consistency tests against analytically known synthetic inputs, not reference parity. |
| AT-02 | Pending | Add pinned SciPy and FOOOF 1.1.1 fixtures and accepted/failed-fit tests. The knee model is written to the FOOOF 1.1.1 form `offset - log10(knee + f^exponent)` and validates its own configuration, but no reference output is pinned. The audited machine did not have `fooof`, `mne`, or `pyedflib` installed. |
| AT-03 | Pending | Coherence now uses one shared validity mask and drops a segment from both channels when either is invalid, with tests for mask-driven segment dropping and DC-detrending. Reference parity and one-channel abstention evidence are still absent. |
| AT-04 | Partial | N2 includes parser bounds/unit tests and a fuzz target/corpus. Add an executed fuzz report and a hash-addressed deterministic result or bounded rejection for every Sleep-EDF recording. No Sleep-EDF files were present during this audit. |
| AT-05 | Partial | Upstream unit/fingerprint tamper tests plus `ruv-neural-core/tests/neurosleep_tamper_matrix.rs`, which mutates every serialized leaf and injects an unknown field into every serialized object. The refreshed fixture uses integrated-power units and a fingerprint recomputed from the canonical acquisition/algorithm projection. Cross-release and WASM matrices remain pending. |
| AT-06 | Partial | Upstream wrong-key and separate trust-profile tests pass. H1 tests unregistered, registered, and revoked-at-ingest keys. Deterministic quarantine of already-stored dependent records after later revocation is still absent. |
| AT-07 | Partial | H1 tests digest idempotency, nonce/recording conflicts, and injected atomic seal failure with no partial visibility. The replay ledger is in-memory rather than persistent, so restart/crash durability remains unproven. |
| AT-08 | Partial | H1 tests enrolled canonical acquisition/algorithm bindings, compatibility grouping, and mixed-fingerprint abstention. No validated bridge exists, as intended; persistence and integrated analyst use remain pending. |
| AT-09 | Partial | Generic Helix thresholds are 5/20/10-per-side with typed abstention. H1 implements NeuroSleep 7/14/20/10-per-side valid-night gates, but the actual longitudinal computations are not implemented. |
| AT-10 | Partial | **Prior evidence for this row was void** — the safety and forbidden-claim scripts failed open (see the correction below) and were passing without executing a single check. They now fail closed, cover `helix-neurosleep-wasm` (which holds the user-facing measurement label and caveat copy), and are verified by negative control. Default-off, exact-copy/domain, fixed-provenance, text-node, injection, prohibited-token, and strict-WASM-request tests pass, and the research build links no attestation code at all. Still not full acceptance: the WASM view lacks authenticated native provenance, and external-model-spy, trusted-host-handle, and end-to-end negative-flow evidence remain absent. |
| AT-11 | Partial | Executable criterion benchmarks now exist: `ruv-neural-signal --bench neurosleep` (13 cases across masked Welch, band power, theta peak, knee fit, masked coherence, and quality gating, including partially-invalid-mask variants) and `ruv-neural-neurosleep --bench full_night` (4 cases over a synthetic 420-epoch scored night, including signing). Both run over deterministic synthetic signals with no external fixtures. Still missing: a recorded latency/peak-RSS report at 8-hour and 24-hour scale, a hard bundle-size test, and native/WASM byte-parity output. |
| AT-12 | Partial | Helix CI exists; N1 adds rUv Rust CI and a tracked lockfile. H1 adds authorization, replay, atomicity, domain, and plaintext-leakage tests; N2 adds a parser fuzz target/corpus; the dedicated UI injection/default-off tests pass. N3 extends rUv CI to gate clippy on the signal and neurosleep crates, check formatting per NeuroSleep module, run the fail-closed safety boundary, and run `cargo deny check`. Helix CI gains matching `cargo deny` and NeuroSleep safety-gate jobs. The rUv workflow still scopes formatting to changed modules because the pre-existing workspace does not pass `cargo fmt --all -- --check`. Cargo-fuzz execution, Sleep-EDF evidence, browser WASM and cross-repository parity, trust-safe key custody, and integrated completion remain pending. |

## Foundation commands

Run after all repository writers have stopped. Commands listed here are target
completion gates; a listed command is not evidence that it currently passes.

```bash
# rUv Neural N1 and independent integrity/boundary evidence
cargo test -p ruv-neural-core --locked
cargo test -p ruv-neural-io --all-features --locked
cargo clippy -p ruv-neural-io --all-targets --all-features --locked --no-deps -- -D warnings
bash validation/neurosleep/check_safety_boundary.sh
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo build -p ruv-neural-wasm --target wasm32-unknown-unknown --release --locked
cargo audit --locked

# Helix H0 and present-day boundary evidence
cargo test -p helix-numeric -p helix-pipeline -p helix-focus -p helix-timeline
cargo test -p helix-neurosleep
cargo clippy -p helix-neurosleep --all-targets -- -D warnings
bash validation/neurosleep/check_safety_boundary.sh
bash validation/neurosleep/check_forbidden_claims.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build -p helix-wasm --target wasm32-unknown-unknown --release
cargo audit
```

`cargo-deny` is now installed and passing in both repositories, and each repo
carries a `deny.toml` plus a CI job that runs it. `cargo-fuzz`, `cargo-nextest`,
`hyperfine`, `semgrep`, and `ripgrep` remain absent on the audit host.

The missing `ripgrep` was not benign: it is what made all three static safety
scripts fail open (see the correction below). Any future gate must be assumed
broken until a negative control shows it rejecting a real violation — a script
that prints `clean` on a host missing its own search tool is indistinguishable
from one that passed.

The rUv workspace-wide formatting command still reports extensive pre-existing
differences, so CI checks each NeuroSleep module directly instead. Absence of
tools or a scoped CI check is not final acceptance evidence.

## Reproduced targeted results

The following targeted checks passed in the shared working tree after the
contract-unit and fingerprint fixture refresh:

- `cargo test -p ruv-neural-core --locked`: 41 unit tests, 2 independent
  tamper-matrix tests, and documentation tests passed.
- `cargo test -p ruv-neural-io --all-features --locked`: 11 parser tests and
  documentation tests passed, including signed event onsets, negative amplifier
  gain, TAL lexical grammar, chronology, terminal records, and epoch-end bounds.
- Scoped `ruv-neural-io` Clippy with warnings denied and no dependency linting
  passed. A warning remains in the in-progress N3 `aperiodic.rs` dependency and
  is not N2 acceptance evidence.
- `cargo test -p helix-neurosleep`: 12 trust, policy, compatibility,
  longitudinal-gate, replay, atomicity, flag-default, and leakage tests passed.
- Scoped `helix-neurosleep` Clippy with warnings denied passed.
- **Correction — the earlier "both safety scripts passed" result was void.** All
  three static scripts (the Helix boundary and forbidden-claim scans, and the
  upstream boundary scan) invoked `rg` inside an `if` condition. `rg` is not
  installed on the audit host, so every check exited 127, every `if` evaluated
  false, and each script printed `clean` while testing nothing. This silently
  voided the AT-10 prohibited-claim evidence and the upstream check that no
  raw-waveform-shaped field reaches the derived contract. All three now use
  `grep`, treat a tool error (exit >= 2) as a failure, and refuse to pass
  vacuously when an expected path is absent. Each was re-verified with a
  negative control: an injected `microglial activation detected` string and an
  injected `pub raw_eeg: Vec<f64>` contract field both produce exit 1.
- `cargo deny check` now passes in both repositories (`advisories ok, bans ok,
  licenses ok, sources ok`) against a new `deny.toml` in each that allows only
  permissive licenses and only the crates.io registry. Both dependency graphs
  are free of copyleft licences. The one unmaintained advisory upstream
  (`bincode` 1.x, RUSTSEC-2025-0141) is ignored with a written rationale
  because it is reachable only through `ruv-neural-memory` -> `ruv-neural-cli`,
  outside the NeuroSleep trust path.
- The research build's isolation is now machine-verifiable rather than asserted:
  `cargo tree -p helix-neurosleep --no-default-features -e normal` resolves to
  `serde` and `serde_json` only. No `ruv-neural-core`, `helix-vault`, `sha2`, or
  `hmac` is linked, so the research artifact contains no attestation or
  key-custody code at all.
- Native tests and scoped strict Clippy passed for the redesigned
  `helix-neurosleep-wasm` crate (3 tests), its optimized `wasm32-unknown-unknown`
  build passed, and `npm --prefix ui test` passed all 8 UI tests. These tests do
  not execute the artifact in a browser or authenticate that caller-provided
  verified-night JSON was emitted by the native host.

These targeted results do not supersede the pending evidence in the table and
do not constitute AT-01 through AT-12 completion.

## Coordination and release constraints

- Ruflo state shows a hierarchical, specialized four-agent swarm initialized in
  Helix. MetaHarness 0.1.14 was invoked through pinned `npx` for its read-only
  `analyze`, `score`, and `genome` operations; disposable root files written by
  those operations were removed afterward. There is no minted MetaHarness
  manifest, signed harness witness, retained root npm package, or publishable
  harness artifact. Analysis-tool execution must not be reported as minting or
  runtime integration.
- Open rUv Neural PR #2 already assigns ADR numbers 0015 through 0023. The N1
  companion currently uses requested number 0015; resolve that conflict before
  merging both histories.
- Existing rUv Neural `0.1.0` crates and Helix `0.1.2` crates occupy those
  crates.io versions. Any changed package needs a coordinated version bump,
  package inspection, dry run, dependency-order publication, and exact
  downstream contract pin.
- No author-data reproduction, human validation, regulatory review, ethics
  approval, device profile, or trust-profile choice was available. None may be
  inferred from passing foundation tests.
