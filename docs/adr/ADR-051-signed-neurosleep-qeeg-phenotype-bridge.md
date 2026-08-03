# ADR-051: Signed NeuroSleep qEEG Phenotype Bridge from rUv Neural to Helix

**Status**: Proposed
**Date**: 2026-08-03
**Decision scope**: `ruvnet/ruv-neural` and `ruvnet/helix`
**Canonical owner**: Helix
**Upstream companion**: `ruv-neural/docs/adr/0015-neurosleep-qeeg-export.md`
**Related**: ADR-001, ADR-003, ADR-005–010, ADR-013, ADR-018, ADR-020, ADR-026,
ADR-036, ADR-050

## Context

Constantino and colleagues reported NREM loss, altered delta/theta power, reduced theta
coherence, a reduced aperiodic exponent, and EEG slowing in APP/PS1 mice. Microglial
depletion restored sleep in that model without clearing amyloid. This is preclinical animal
evidence: it supplies neither a validated human mapping from EEG to microglial state nor
human diagnostic sensitivity, specificity, predictive value, or treatment efficacy.

rUv Neural already owns EEG acquisition abstractions, signal preprocessing, Welch PSD,
band power, coherence, sleep-state types, and evidence generation. Helix owns local health
provenance, deterministic longitudinal analysis, evidence qualification, abstention, and
user-facing claims. The existing loose `NeuralSession` adapter is not suitable: it accepts
unknown metric keys, does not cryptographically verify trusted origin, discards the real
upstream envelope, and labels EEG as ambient sensing.

At the audited bases (`ruv-neural` `caaa14144a70829293737b0ca717ebc818fcc523`, Helix
`87ff0e151a3e8e1b52ac9e07cc03673668ada756`), the generic numeric implementation also
disagreed with ADR-007 by admitting trends, correlations, and change-points below their
documented sample minima. Those numeric invariants are an independent prerequisite.

## Decision

Implement a local-first, research-only, one-way NeuroSleep observation rail. rUv Neural
owns bounded import, preprocessing, sleep-state-aware qEEG, artifact masking, numeric
quality, method manifests, canonical payload hashing, and signing. Helix independently
verifies the exact released upstream contract, trusted signer identity, consent, replay,
quality, and compatibility before storing derived research features and producing
deterministic longitudinal observations.

Ship it in a physically separate offline research build. Runtime flags stage import,
shadow analysis, research UI, and optional indexing inside that build; flags alone are not
the safety or regulatory boundary. NeuroSleep types and values have no path to a hosted
language model, composite health score, Focus Areas, red-flag escalation, general
recommendations, protocol selection, stimulation loop, or actuator.

The allowed output is a measured change from a compatible personal baseline with explicit
measurement support and interpretation maturity. Constantino-linked interpretation remains
`preclinical_mouse_model`. Helix must not infer or estimate Alzheimer disease, mild
cognitive impairment, amyloid burden, microglial activation, neuroinflammation, treatment
response, or clinical risk, and must not recommend a drug, supplement, sleep intervention,
gamma entrainment, or other treatment from this evidence. Deterministic approved templates,
not an LLM, generate clinically adjacent copy.

The MetaHarness/Darwin optimization surfaces, including `helix-evolve`, may test stable
generic parameters but must not mutate NeuroSleep contracts, trust rules, quality gates,
sample-count minima, compatibility rules, translational labels, forbidden-claim rules, or
the observation-to-actuation boundary.

## Contract and trust boundary

Authoritative Rust types live in `ruv-neural-core`; Helix consumes an exact released
version and must not maintain handwritten duplicate wire structs. Version one carries a
signed payload containing study-scoped pseudonymous subject and recording identifiers,
time bounds, nonce, consent scope, source digest and size, acquisition metadata, algorithm
manifest, quality, stage summary, stage-specific qEEG, a compatibility fingerprint, and
literature context.

Every payload field is included in an RFC 8785 canonical JSON SHA-256 digest. Ed25519 signs
`ruv-neural/neurosleep/1\0 || signer_key_id || \0 || payload_sha256`; key identifiers
containing NUL are rejected. A public key embedded in a bundle is never a trust root.
Helix resolves the signer from an enrolled personal-device, laboratory, or study trust
profile and checks revocation at the verification time. Unknown fields, schemas, metrics,
units, species, extractor digests, and untrusted keys are quarantined or rejected, never
coerced.

Ingestion is fail closed and atomic. The payload digest, recording identifier, and nonce
form the idempotency/replay key: identical reimport returns the existing result; reuse with
a different payload is rejected. A one-byte change creates no provenance record, time
series point, or index reference. Legacy `RUVN-*` records remain `legacy_unverified` and
are excluded from NeuroSleep analytics.

`MeasurementMethod::Electrophysiology` is the provenance-v2 method. A compatibility path
may use `Device` plus required `AcquisitionModality::Eeg`; EEG is never ambient sensing.
Acquisition confidence and interpretation maturity are distinct fields.

## Numeric, method, and storage invariants

The longitudinal statistical unit is one valid night, never an epoch. Generic ADR-007
gates are trend >= 5 observations, correlation >= 20 aligned observations, and change
point >= 10 observations on each side. NeuroSleep adds stricter gates: >= 7 compatible
valid nights for baseline, >= 14 before a user-facing direction/slope, >= 20 for
correlation, and >= 10 per change-point side. Insufficiency is a typed abstention and must
never serialize as `flat`, `stable`, or a zero.

Helix trends only identical compatibility fingerprints unless a separately validated bridge
connects them. The fingerprint binds device and modality, channels/reference/montage,
sampling rate and firmware, every DSP and artifact parameter, stage source/model, crate and
source versions, extractor/configuration digests, and schema version. A method change,
missing stage provenance, excessive artifact, low fit quality, or insufficient compatible
nights abstains.

Raw EEG/EOG/EMG and epoch arrays remain encrypted locally and never enter Helix language
model context, browser logs, telemetry, federation, or the signed derived bundle. Helix
stores verified scalar features and the evidence envelope in a sealed study-scoped exact
time series. Raw neural data, identifiers, phenotype labels, and numeric vectors do not
enter an unencrypted RVF or semantic index. A dedicated RuVector namespace is a separate
capability gate requiring encryption, authorization, reference-only retrieval, and
plaintext-leakage tests.

## DSP and sufficiency profile

Version one imports bounded EDF/EDF+ sessions through an `EpochSource`; BrainVision remains
compatible and author data may use a separate adapter. Expert hypnograms are preferred.
Heart-rate/motion proxy staging may label context but cannot support paper-equivalent qEEG.
Automated EEG/EMG staging is a separate experimental feature.

The initial closed metric registry includes sleep-state durations/bouts; absolute and
relative delta/theta power; relevant alpha power; theta center frequency and power;
pairwise full-band/theta coherence; aperiodic exponent/offset and fit error; and artifact
burden, all with fixed units and finite-or-typed-null values. Artifact-affected epochs are
masked/rejected, never cross-channel interpolated; coherence pairs use the same synchronous
mask.

The paper-compatible profile uses 10-second, 2,500-sample epochs after explicit resampling
to 250 Hz and records all known differences from the publication. Unspecified coherence,
relative-power, and FOOOF details are implementation choices until obtained from the
authors. Wake-only aperiodic fits abstain below the frozen quality gates. Engineering
coverage and stage-duration minima are sufficiency gates, not clinical thresholds.

## Security, privacy, and regulatory boundary

Raw neural waveforms are P0 restricted; derived nightly features and method metadata are P1
sensitive. Signer keys, identity linkage, consent, revocation, and recovery material remain
separately protected. Processing is purpose- and study-scoped, deny-by-default, with
withdrawal, retention, access, export, and deletion audit outcomes that contain reason codes
but no neural values.

The first release is observational research software, not an Alzheimer detector or a
treatment surface. Any human disease screening/prediction track requires counsel-reviewed
device classification, a regulated build, quality and cybersecurity lifecycles, and locked
prospective external validation. The existing 40 Hz stimulation loop remains separate;
the cited mouse study used CSF1R-mediated depletion and does not establish gamma entrainment
efficacy. Pexidartinib and other interventions are outside this decision.

## Delivery and rollback

Delivery order is: Helix H0 numeric/provenance safety foundation; upstream N1 contract and
trusted signing fixtures; N2 bounded I/O; N3 DSP/profile and parity; upstream alpha release;
Helix H1 verified ingestion/sealed storage; H2 native/WASM derived-bundle parity; H3 escaped
research UI; then integrated validation. Optional encrypted RuVector work cannot block the
typed time-series path.

Four research-build flags default off: `neurosleep.import.v1`, `neurosleep.shadow.v1`,
`neurosleep.research_ui.v1`, and `neurosleep.rvf.v1`. Rollback disables the affected flag,
stops new acceptance, and suppresses derived analytics without destructive migration;
versioned evidence remains retained under policy and is never silently reinterpreted.

## Acceptance evidence

- **AT-01–03**: synthetic/reference PSD, band power, FOOOF, theta peak, and coherence parity;
  fit/mask failures are typed nulls or abstentions.
- **AT-04**: all 197 Sleep-EDF Expanded recordings deterministically succeed or return a
  bounded rejection without panic, unbounded allocation, or partial write. This validates
  mechanics, not disease accuracy.
- **AT-05–07**: every analytic/method field is bound; untrusted/revoked keys fail; reimport
  is idempotent and nonce/recording conflicts fail.
- **AT-08–09**: fingerprints segment comparisons; <14 compatible valid nights abstains;
  correlation requires 20 nights and change-points 10 per side.
- **AT-10**: dependency and end-to-end negative tests prove no score, Focus Area,
  escalation, recommendation, disease claim, hosted model, protocol, or actuator path.
- **AT-11**: frozen eight-hour and 24-hour fixtures meet declared native latency/memory and
  bundle-size targets.
- **AT-12**: formatting, lint, workspace, WASM, contract, tamper, fuzz, consent,
  authorization, leakage, UI-injection, dependency-audit, and native/WASM byte-parity gates
  pass in both repositories.

Final acceptance is a clean Helix research installation that verifies a trusted bounded
fixture bundle field-for-field, stores only derived research features, abstains through the
compatible-night minimum, then displays a reproducible baseline change with a preclinical
caveat, while rejecting tamper, untrusted origin, replay, incompatible method, insufficient
nights, and every diagnostic or therapeutic claim.

## Consequences

This establishes one numeric authority, local neural-data custody, cryptographically
verifiable provenance, and method-aware longitudinal research output. Costs are coordinated
cross-repository releases, parser/DSP maintenance, conservative early abstention, and no
near-term disease claim. A future human-validation or regulated-product ADR must revisit
intended use, cohorts, confounders, retention jurisdictions, and clinical ownership.

## References

- [Constantino et al. study](https://pubmed.ncbi.nlm.nih.gov/42252510/)
- [Sleep-EDF Expanded](https://physionet.org/content/sleep-edfx/1.0.0/)
- [FDA Clinical Decision Support Software guidance](https://www.fda.gov/regulatory-information/search-fda-guidance-documents/clinical-decision-support-software)
- [FDA pexidartinib prescribing information](https://www.accessdata.fda.gov/drugsatfda_docs/label/2025/211810s013lbl.pdf)
