# ADR-020: WiFi-CSI Contactless Ambient Sensing (RuView backend)

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-014 (ambient sensing), ADR-009 (escalation), ADR-006 (evidence/abstention), ADR-010 (SaMD boundary), ADR-001 (vault)

---

## Context

ADR-014 established an **ambient passive sensing tier** — continuous, contactless,
adherence-free vitals that require nothing of the user — and named mmWave radar
(Cognitum Seed) as the reference sensor. Radar is one modality; it is not the only
one, and a single-sensor dependency is a product risk.

**[ruvnet/ruview](https://github.com/ruvnet/ruview)** is a second, complementary
contactless modality already in the ruvnet stack: it turns **commodity WiFi
Channel State Information (CSI)** — captured by ~$9 ESP32-S3 nodes — into spatial
intelligence and vitals, entirely on the edge. It is built on **RuVector** and the
**Cognitum Seed**, so it shares Helix's substrate. **[A]**

What RuView measures (from its documentation): **[A]/[B]**
- **Breathing rate** — bandpass 0.1–0.5 Hz on wrapped CSI phase, 6–30 BPM, real-time.
- **Heart rate** — bandpass 0.8–2.0 Hz, 40–120 BPM, real-time.
- **Presence / occupancy / activity** — through walls, in the dark, no camera.
- **Sleep** — overnight monitoring, sleep-stage classification, **apnea screening**.
- **10 inferred semantic states** per node, incl. *someone-sleeping, possible-distress,
  elderly-inactivity-anomaly, fall-risk-elevated, bed-exit, no-movement*.
- Every measurement is **Ed25519 witness-attested**; raw CSI never leaves the edge.
- Pretrained CSI encoder on Hugging Face (`ruvnet/wifi-densepose-pretrained`);
  honest **82.3% held-out temporal-triplet accuracy** (the earlier "100% presence"
  figure was retracted) — i.e. **screening-grade, not clinical**. **[A]**

This is an almost exact fit for the ADR-014 tier, and its semantic states map
directly onto the Escalation Guardian (ADR-009).

## Decision

Adopt **RuView WiFi-CSI as a first-class ambient-sensing backend** alongside (not
replacing) mmWave radar, under the existing ADR-014 contract:

1. **On-device extraction only.** RuView runs CSI→vitals on the ESP32/Seed mesh.
   Helix ingests **only the derived, witness-attested signals** — raw CSI never
   reaches the vault (ADR-001/014).
2. **Screening, not diagnosis (hard).** All RuView-derived signals are tagged
   screening-grade with **capped confidence** and carry a non-diagnostic
   disclaimer. Apnea, fall-risk, distress, etc. are *prompts to act*, never
   verdicts (ADR-010). A WiFi reading never becomes a clinical claim on its own.
3. **Vitals → provenance records.** Breathing rate and heart rate become
   `ProvRecord`s (method `AmbientSensing`, source `ruview`, `RUVW-*` research
   codes — never a clinical LOINC, ADR-004), flowing into the same dossier and
   the same deterministic trend engine (ADR-007) as every other source.
4. **Semantic states → escalation signals.** The 10 inferred states are mapped to
   typed **screening flags** with a severity (info / urgent / critical) that feed
   the Escalation Guardian (ADR-009). Safety-relevant states (*possible-distress,
   fall-risk-elevated, bed-exit + no-movement, apnea screening*) escalate;
   optimization is suppressed exactly as for any red flag.
5. **Attestation is provenance.** The Ed25519 witness signature is carried on
   every record; unsigned readings are rejected (provenance required, ADR-005).

Implemented in a new crate **`helix-sensing`** (RuView reading → records + flags),
mirroring the `helix-neural` adapter pattern.

## Alternatives Considered

- **mmWave-only (status quo ADR-014).** Rejected: single-sensor risk; WiFi-CSI is
  cheaper ($9/node), works through walls, and is already in-stack.
- **Treat RuView vitals as clinical-grade.** Rejected outright: the honest 82.3%
  benchmark and the retracted "100%" claim are exactly why this must stay
  screening-grade with capped confidence.
- **Cloud processing of CSI.** Rejected: violates ADR-001/013 (local-first); RuView
  is edge-only by design, which is the point.

## Consequences

**Positive.** A real, low-cost, in-stack contactless backend for ADR-014; new
overnight signals (breathing regularity, apnea screening) and safety states
(fall, distress, bed-exit) that no upload-based product has; full provenance and
on-device privacy preserved.

**Negative.** CSI is environment-sensitive (needs per-room calibration); false
positives on safety states erode trust fast; multi-sensor fusion (radar + WiFi)
adds reconciliation work (ADR-007 dedup).

**Mitigations.** Capped confidence + screening framing keep claims honest;
escalation thresholds for safety states are governance-curated (ADR-009, not
Darwin-mutable); de-duplicate overlapping vitals across radar/WiFi like any other
overlapping source.

## Open Questions

- Per-room calibration UX and drift handling (RuView learns in ~30 s; how is that
  surfaced/maintained?).
- Which safety states warrant *critical* vs *urgent* escalation — needs clinical
  + caregiver governance input.
- Fusion policy when radar and WiFi disagree on a vital.

## References

- ruvnet/ruview — WiFi-CSI sensing platform (README, ADR-115 HA integration, ADR-122 Matter). **[A]**
- `ruvnet/wifi-densepose-pretrained` (Hugging Face) — CSI encoder, 82.3% temporal-triplet acc. **[A]**
- Helix ADR-014 (ambient sensing), ADR-009 (escalation), ADR-010 (SaMD). **[A]**

> Architectural/product guidance, not legal or medical advice. RuView signals are research/screening grade.
