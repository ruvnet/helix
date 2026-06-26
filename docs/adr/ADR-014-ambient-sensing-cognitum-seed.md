# ADR-014: Ambient Passive Sensing via the Cognitum Seed

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001, ADR-002, ADR-003, ADR-007, ADR-009, ADR-010, ADR-013

---

## Context

### The signal-collection gap

Every ingestion source described in Helix's Tier A requirement list asks something of the user: connect an account, wear a device, charge a band, log a meal. The richest overnight signals — resting heart rate, breathing regularity, restlessness, presence — are precisely the ones that wearable compliance makes unreliable. A Whoop strap that is not worn because a user forgot to charge it produces no data. A symptom log written the next morning is subject to recall bias. The gap is structural, not behavioral.

The Cognitum Seed is Helix's architectural answer to that gap: an always-on edge AI device placed in the bedroom (or living space) that requires nothing of the user after initial setup and produces a continuous, passive signal stream from ambient sensing.

### mmWave radar for vital-sign sensing: what the physics actually supports

Millimeter-wave (mmWave) radar exploits the Doppler effect and frequency-modulated continuous-wave (FMCW) techniques to detect sub-millimeter surface displacements caused by chest-wall motion during breathing and cardiac activity. Two distinct device classes matter here and must not be conflated:

**60 GHz FMCW sensors (e.g., HLK-LD6002 / HLK-LD6004 class)** — these are the vital-signs-capable tier. The Hi-Link HLK-LD6002, which represents the LD6004 class, integrates a 57–64 GHz radio-frequency transceiver with 2T2R antenna, ARM Cortex-M3 signal processor, and FMCW modulation that resolves chest-wall displacements at centimeter accuracy. Published specifications show: maximum detection distance for respiration and heart rate of approximately 1.5 m; detection angle ±60° azimuth / ±60° pitch; outputs include real-time respiration rate (breaths per minute), estimated heart rate (bpm), and range (cm) [B]. **These sensors can extract respiration rate and approximate heart rate for a single occupant within 1–1.5 m under cooperative, stationary conditions.**

**24 GHz tracking sensors (e.g., HLK-LD2450 class)** — these are a motion/presence/trajectory tier, not a vital-signs tier. The HLK-LD2450 operates at 24.0–24.25 GHz with 250 MHz sweep bandwidth and tracks up to three simultaneous targets using FMCW, reporting X/Y coordinates (mm precision), speed, and distance at 10 Hz over UART [A]. Range: 6–8 m; azimuth ±60°, pitch ±35°. **This sensor does not output respiration rate or heart rate.** Its role is presence confirmation, occupancy counting, movement trajectory, and restlessness quantification — valuable proxy signals for sleep quality and disruption, but not for cardiovascular or respiratory measurements directly.

The correct architecture uses these two classes **complementarily**: a 60 GHz sensor for vital-signs extraction at close range, and a 24 GHz sensor for room-level presence, motion, and occupancy — so that the vital-signs sensor is activated and calibrated only when a confirmed occupant is stationary within range.

### Clinical accuracy: where the research stands [B]

Multiple 2024–2025 studies establish mmWave radar as a credible screening tool for overnight respiratory monitoring:

- A systematic review and network meta-analysis of 20 studies (n = 1,540 participants) found radar-based sleep-disordered-breathing detection achieves an area under the ROC curve (AUC) of approximately 0.91 at an optimal apnea-hypopnea index (AHI) cutoff of ≥22, with sensitivity of 81.6% and specificity of 88.2% [B, PMC12385411].
- A 2025 Doppler radar-based device demonstrated sensitivity 92.7%, specificity 84.6%, Kappa 0.731 against polysomnography — competitive with some commercial home sleep apnea test devices [B].
- Contactless respiration rate monitors validated clinically show mean absolute error of approximately 0.39 breaths per minute under controlled conditions [B, PMC9975830].
- **However**: false-positive apnea event detection is significant in consumer-grade implementations. One cohort study reported 23.4–52.8 false-positive events per participant per night when sleep staging was not applied [B, PMC9570824]. Real-world, non-laboratory environments introduce additional confounders: partner movement, pets, vibration, and temperature fluctuations.

The Google Nest Hub (2nd generation) provides the most visible consumer reference point: it uses Google's Soli 60 GHz radar chip to track respiratory motion and movement from the bedside, validating sleep stages against polysomnography in a cohort of 33 healthy sleepers [B]. Google explicitly positions this as "not intended for medical or diagnostic use" and it carries no FDA clearance for sleep apnea diagnosis [A]. This is the exact screening-not-diagnosis framing Helix must adopt.

**Polysomnography (PSG) remains the diagnostic gold standard for obstructive sleep apnea (OSA) and other sleep-disordered-breathing conditions [A].** PSG simultaneously records EEG, EOG, EMG, airflow, effort, oximetry, and ECG in a controlled clinical setting. No consumer radar device approximates this. Home sleep apnea test (HSAT) devices with FDA clearance for diagnostic use (e.g., WatchPAT, Nox T3) use multi-channel physiological recording, not radar alone. Radar is a **screening tier** that identifies patterns warranting clinical follow-up — it does not replace PSG.

### The Cognitum Seed device platform

The Cognitum Seed is the Helix edge AI device platform. Its architecture includes: on-device vector store (RuVector embedded), WASM runtime for local model inference, MCP integration for secure communication, Ed25519 identity for cryptographic device attestation, OTA firmware update capability, and fleet management for multi-device households. This is a general-purpose edge platform, not a single-purpose radar module; the mmWave sensing tier is one application of the platform.

The critical design constraint: **raw sensor data (the I/Q signal stream from the radar) never leaves the device.** Only normalized, derived signals — respiration rate, estimated heart rate, motion level, presence flag, and anomaly indicator — egress to the user's RuVector vault. This is an architectural privacy guarantee (ADR-001), not a configuration option.

### Regulatory and clinical safety framing

Ambient radar sensing for health purposes intersects with FDA SaMD (Software as a Medical Device) guidance. Under the FDA's Digital Health Center of Excellence framework, a device or software that claims to diagnose, treat, or prevent a specific disease is subject to premarket review. Screening tools that produce signals warranting clinical evaluation — without making diagnostic claims — occupy a lower-risk tier [B, ADR-010]. Helix must maintain the wellness/screening positioning throughout the ambient sensing layer.

---

## Decision

### Decision 1: Deploy a dual-class mmWave sensing tier on the Cognitum Seed

The Seed integrates two radar sensor types with distinct roles:

**60 GHz FMCW sensor (vital-signs tier):** A sensor of the HLK-LD6002/LD6004 class or equivalent, mounted at the bedside within 1–1.5 m of the sleeping position. This sensor performs: (a) respiration rate extraction (breaths per minute), (b) heart-rate-band estimation (approximate, not clinical-grade), (c) breathing regularity scoring, and (d) anomaly flagging for prolonged respiratory pauses above configurable thresholds. Outputs are produced on-device after FMCW signal processing and DSP filtering. Raw I/Q data is never stored or transmitted.

**24 GHz FMCW tracking sensor (presence/motion tier):** A sensor of the HLK-LD2450 class, with wider room coverage (6–8 m range, ±60° azimuth). This sensor performs: (a) occupancy confirmation (the vital-signs sensor activates only when a stationary occupant is confirmed), (b) movement/restlessness quantification (target velocity and XY position at 10 Hz), (c) sleep-macro-state proxy (quiescent vs. active), and (d) multiple-occupant detection (necessary for false-positive management when a partner is present).

Together, the two sensors produce a coherent overnight signal. Neither alone is sufficient: the 60 GHz sensor without presence confirmation produces spurious readings when the bed is empty; the 24 GHz sensor without the 60 GHz sensor cannot distinguish sleep apnea pauses from peaceful stillness.

### Decision 2: On-device first-pass extraction and anomaly detection

Signal processing runs entirely on the Seed's WASM runtime:

1. **Raw FMCW I/Q stream** — remains on-device. Never stored, never transmitted.
2. **Signal conditioning** — clutter removal, range-bin selection, bandpass filtering (0.1–0.5 Hz for respiration; 0.8–2.5 Hz for heart rate estimation).
3. **Feature extraction** — respiration rate (FFT or VMD-based), peak-interval estimates, breathing regularity coefficient, motion level from 24 GHz data.
4. **Anomaly detection** — local on-device model (RuVector WASM) flags: respiratory pauses ≥ 10 s, irregular breathing patterns, resting heart-rate drift outside personal baseline ± 2σ.
5. **Normalized derived signals** — only these egress: {timestamp, respiration_rate_bpm, hr_estimate_bpm, motion_level [0–1], occupancy_flag, anomaly_type, anomaly_confidence, calibration_quality}.
6. **Vault write** — derived signals are encrypted and written to the user's local RuVector vault (ADR-001, ADR-003) via the Seed's MCP client.

This pipeline is the practical implementation of "raw sensor data never leaves the device."

### Decision 3: Normalized vitals-only egress policy

The egress schema is formally typed and enforced at the Seed firmware level. Fields permitted in egress payloads:

```
{
  "ts": ISO-8601 timestamp,
  "rr_bpm": f32 | null,          // respiration rate, null when occupancy not confirmed
  "hr_est_bpm": f32 | null,      // estimated heart rate, null when low SNR
  "motion_level": f32,           // 0.0–1.0 restlessness index from 24 GHz
  "occupancy": bool,             // confirmed stationary occupant in vital-signs range
  "anomaly": {
    "type": enum(RESP_PAUSE | IRREGULAR_RHYTHM | HR_DRIFT | NONE),
    "confidence": f32,           // 0.0–1.0
    "duration_s": u32 | null
  },
  "cal_quality": enum(GOOD | FAIR | POOR | UNCALIBRATED),
  "device_id": Ed25519_pubkey    // device attestation, not user PII
}
```

No raw arrays, no I/Q samples, no room audio, no video. The schema is version-controlled and any field addition requires a firmware review gate.

### Decision 4: Screening-signals-not-diagnoses policy — hard constraint

Every ambient sensing output surface in the Helix application must conform to this framing hierarchy:

- **What the Seed can assert**: "Your breathing regularity was below your baseline for 4 of the last 7 nights." "Respiration pauses of 12+ seconds were detected 3 times overnight on Tuesday."
- **What the Seed cannot assert**: "You have sleep apnea." "You have obstructive sleep apnea, moderate severity." "Your AHI is 22."
- **Correct escalation output (to Escalation Guardian, ADR-009)**: "Pattern detected: repeated respiratory pauses. Consider discussing a sleep study with your doctor."
- **Correct UI copy example**: "Something worth discussing with a clinician: we detected a pattern of interrupted breathing that appears in your sleep data 3 nights this week. This is a screening signal, not a diagnosis. A sleep study (polysomnography) is the way to know for certain."

This policy is encoded as a prompt constraint in the Ambient Sensing agent (part of the Ruflo swarm, ADR-002) and enforced by the Verifier/Critic agent (ADR-008) before any radar-derived finding reaches the user.

### Decision 5: Feed path to the Escalation Guardian

Anomaly signals from the Seed feed the Escalation Guardian (ADR-009) through two paths:

**Immediate path (within session):** An anomaly with confidence ≥ 0.75 and type RESP_PAUSE or IRREGULAR_RHYTHM generates an Escalation Guardian trigger. The guardian evaluates the signal against user history and, if the pattern is sustained (≥ 3 nights in 7), routes to "see a clinician" framing rather than optimization copy. Optimization copy (sleep hygiene tips, etc.) is suppressed when an escalation trigger is active.

**Longitudinal path (via RuVector):** All normalized signals are time-series indexed in RuVector (ADR-003). The Trend/Numeric agent (ADR-007) computes: 7-day resting-HR baseline, 30-day breathing-regularity trend, respiratory-pause event frequency. These derived trend values are first-class inputs to the health score (ADR-016) and the 3D twin (ADR-015).

### Decision 6: Calibration and false-positive management strategy

False-positive management is a first-order product-quality concern. The strategy:

1. **Personal baseline calibration period (7 nights):** The Seed withholds escalation output until it has established a personal baseline for respiration rate, motion level, and HR estimate. Signals during the calibration window are stored but not acted upon for escalation.
2. **Multiple-occupant disambiguation:** The 24 GHz sensor's multi-target capability (up to 3 targets) is used to flag sessions where a second occupant is present. In multi-occupant sessions, the vital-signs extraction SNR is logged and confidence scores are downgraded.
3. **Calibration quality gating:** If cal_quality is POOR or UNCALIBRATED (sensor obstruction, placement error, high motion artifact), that session's anomaly outputs are suppressed from escalation routing and flagged for user review.
4. **Adaptive thresholds:** Respiratory pause thresholds and HR-drift sigma bounds are personalized against the individual's baseline, not against population norms, reducing false positives for people with naturally slow breathing rates.
5. **User correction feedback:** Users can mark flagged nights as "false alarm" (e.g., noise from outside, sick with a cold, shared the bed differently). These corrections feed back into the Seed's local calibration model via the SONA learning loop (ADR-002).
6. **Explicit confidence surfacing:** Every ambient signal shown in the UI carries a confidence indicator. Low-confidence readings are visually distinguished from high-confidence ones and explicitly labeled "low signal quality this night."

### Decision 7: Sensor placement, privacy, and consent

Placement guidance accompanies device setup:
- Optimal: bedside table or nightstand, 0.5–1.5 m lateral distance from the sleeping body at mattress level for the 60 GHz sensor.
- The 24 GHz sensor can be ceiling- or shelf-mounted for broader room coverage.
- The Seed's radar illuminates only through the air; it does not see through walls or produce images. Privacy communication must explain this clearly in non-technical language.

Consent layer:
- Ambient sensing is opt-in at setup, not enabled by default.
- The user explicitly confirms: (1) they understand radar sensing is active in the room, (2) any other occupants in the room have been informed, (3) they understand outputs are screening signals and not diagnoses.
- Household consent policy (multi-occupant): Helix records that secondary occupants may be detected as "motion targets" without biometric attribution. The system does not attempt to identify or profile secondary occupants.

---

## Alternatives Considered

### Alternative A: Wearable devices only (no ambient sensing tier)

Many competing products (Oura ring, Whoop, Eight Sleep) rely entirely on contact wearables for overnight physiological monitoring. Wearables achieve high signal fidelity for HRV, skin temperature, and SpO₂ when worn. They can also measure true multi-lead cardiac signals (Apple Watch ECG).

**Why rejected:** Wearable compliance is the structural vulnerability. Studies consistently show non-wear rates of 20–40% for health wearables after 90 days. The elderly — who have the most to gain from sleep monitoring — are the least consistent wearers. Charging friction is the single largest barrier. Helix's design target of "no wearable to charge" is a genuine product differentiator for the population outside the biohacker segment. Wearable connectors (ADR-012) remain in Helix as an additional input source; they are not the sole or primary overnight signal.

### Alternative B: Camera-based photoplethysmography (rPPG) for contactless vital signs

Remote PPG extracts pulse signals from subtle color changes in facial skin using a standard RGB camera and a bright light source. Research systems have demonstrated heart rate estimation within ±5 bpm accuracy under controlled lighting and stillness conditions.

**Why rejected:** Camera sensing in a bedroom at night requires near-infrared illumination, introduces a fundamentally different privacy profile than radar (images vs. Doppler signals), and requires line-of-sight to exposed skin. Users are acutely sensitive to cameras in bedrooms; a radar-based approach is categorically more acceptable. Furthermore, rPPG accuracy degrades sharply with movement, ambient light variation, and skin-tone variation. Radar is the correct modality for passive overnight sensing.

### Alternative C: Microphone-based breathing and snoring detection

Several consumer apps (SnoreLab, Pillow) use the phone microphone to detect snoring and breathing sounds. Some research proposes sonar-based breathing detection via phone speakers.

**Why rejected:** Microphone-based sensing captures ambient audio in the bedroom — a much more sensitive privacy surface than radar. Audio may capture speech, conversations, and other sensitive content. The acoustic signal is highly susceptible to non-breathing noise sources (partner, pets, street noise). Radar is more specific to physiological motion without the ambient audio capture. Microphone data is explicitly excluded from the Seed's egress schema.

---

## Consequences

### Positive

- **Continuous adherence-free signal**: Helix gains overnight physiological data for users who will never reliably wear a device — the elderly, children being monitored (with parental consent), anyone who finds wearables uncomfortable.
- **Privacy-superior to cloud alternatives**: Raw sensor data stays on-device by architecture. No competitor offering cloud-based vital monitoring can make this claim.
- **Category differentiation**: No upload-based product (ChatGPT Health, Apple Health, Oura app) has access to passive ambient overnight signals. This is a structural data advantage.
- **Screening safety value**: Detecting a pattern of respiratory pauses and routing it to clinical consultation could meaningfully improve outcomes for the estimated 936 million adults worldwide with obstructive sleep apnea (many undiagnosed).
- **Longitudinal context enrichment**: Even when wearables are connected (ADR-012), the Seed's data fills compliance gaps and provides a second independent signal for cross-validation.

### Negative

- **Signal processing complexity**: FMCW radar signal processing, VMD/FFT filtering, and multi-sensor fusion are non-trivial engineering tasks requiring embedded DSP expertise. Calibration across different bedroom configurations adds QA surface.
- **False-positive risk**: As documented above, false-positive rates in non-laboratory conditions are significant. An over-eager escalation system erodes user trust rapidly. The calibration and confidence-gating strategy mitigates but does not eliminate this risk.
- **Multi-occupant ambiguity**: When two people share a bed, attributing signals to individual occupants is an unsolved problem for near-range mmWave radar. The 24 GHz sensor helps with presence counting, but the 60 GHz vital-signs extraction degrades in multi-target scenarios.
- **Wellness/SaMD boundary management**: Screening copy must be reviewed by clinical governance and legal counsel for every release. A careless feature addition that implies diagnosis crosses into regulated SaMD territory.
- **Hardware cost and supply chain**: Physical hardware adds cost, supply-chain risk, and a different GTM motion (shipping a device) vs. a pure software product.

### Mitigations

- Calibration period gating (7 nights) prevents premature escalation.
- Confidence scores on every output; POOR/UNCALIBRATED sessions suppressed from escalation routing.
- Clinical governance review of all ambient-sensing copy, separate from general product copy.
- Multi-occupant flagging in 24 GHz data reduces false vital-sign attribution.
- Tiered rollout: Phase 2 (per product roadmap, §8) — ambient sensing ships after Phase 1 proves the core graph+analyst value, so early users have the strongest possible core product before hardware complexity is added.

---

## Open Questions

1. **Sensor selection finalization**: The HLK-LD6002/LD6004 specs document 1.5 m effective vital-signs range. Many bedside configurations place the sensor 1–2 m from the body. Characterize fall-off in RR and HR accuracy between 1.0 and 2.0 m in representative bedroom configurations before finalizing sensor model.

2. **Multi-occupant vital-sign separation**: Is spatial separation of respiratory signals possible at 24–60 GHz for two occupants < 0.5 m apart? Review current literature on multi-target vital-sign MIMO approaches. If not tractable, define a graceful degradation policy (single-occupant-only mode with partner detection flag).

3. **HR estimation accuracy ceiling**: 60 GHz radar HR estimation is approximate; consumer devices typically achieve ±5–10 bpm under good conditions. Verify whether this is adequate for the clinical use cases Helix plans (baseline drift detection vs. arrhythmia screening). Arrhythmia screening from radar alone is almost certainly not supportable; confirm with medical advisory board.

4. **Regulatory framing for SDB screening**: Engage FDA regulatory counsel to characterize whether the screening output ("patterns consistent with respiratory pauses, recommend sleep study") constitutes a SaMD-regulated claim under the current Digital Health guidance. Jurisdictional review needed for EU MDR as well.

5. **OTA firmware update cadence**: The Seed's WASM runtime enables OTA updates to the signal-processing and anomaly-detection models. A poorly calibrated OTA update could produce a sudden change in false-positive rate that damages user trust. Define a staged rollout policy and rollback mechanism before shipping.

6. **Calibration for participants with clinical OSA already diagnosed**: Users with existing OSA (CPAP users) will have fundamentally different breathing signatures. The adaptive-threshold calibration must handle this population without generating nightly escalations for normal CPAP-assisted breathing.

---

## References

1. Shenzhen Hi-Link Electronic Co., Ltd., "HLK-LD6002 60G Millimeter Wave Respiratory and Heartbeat Detection Radar Module," product page. https://www.hlktech.net/index.php?id=1180 [B — manufacturer spec]

2. Shenzhen Hi-Link Electronic Co., Ltd., "HLK-LD2450 24GHz Human Motion Tracking Trajectory Radar Module," product page and instruction manual. https://www.hlktech.net/index.php?id=1157 [A — manufacturer spec, independently confirmed by ESPHome integration docs]

3. ComponentIndex, "LD2450 Multi-Target Tracking Sensor: Wiring, Code & Pinout." https://componentindex.net/components/ld2450/ [A — verified spec summary]

4. ESPHome, "LD2450 Sensor Component Documentation." https://esphome.io/components/sensor/ld2450/ [A — open-source integration, independently validates spec]

5. PMC12385411 — "Radar-Based Detection of Obstructive Sleep Apnea: A Systematic Review and Network Meta-Analysis of Diagnostic Accuracy Across Frequency Bands," MDPI Diagnostics, 2025. https://pmc.ncbi.nlm.nih.gov/articles/PMC12385411/ [B — systematic review, 1,540 participants]

6. PMC9570824 — "Automated Detection of Sleep Apnea–Hypopnea Events Based on 60 GHz FMCW Radar Using Convolutional Recurrent Neural Networks," PMC, 2022 (prospective cohort). https://www.ncbi.nlm.nih.gov/pmc/articles/PMC9570824/ [B — primary study with false-positive rate data]

7. PMC11966815 — "Diagnostic performance of a Doppler radar-based sleep apnoea testing device," 2025. https://www.ncbi.nlm.nih.gov/pmc/articles/PMC11966815/ [B — sensitivity/specificity data]

8. PMC9975830 — "Clinical validation of a contactless respiration rate monitor," 2023. https://www.ncbi.nlm.nih.gov/pmc/articles/PMC9975830/ [B — RR MAE = 0.39 breaths/min]

9. Google Research, "Contactless Sleep Sensing in Nest Hub." https://research.google/blog/contactless-sleep-sensing-in-nest-hub/ [A — reference for consumer radar sleep sensing framing and PSG validation methodology]

10. MobiHealthNews, "Google's next-gen Nest Hub debuts with contactless sleep monitoring and analysis features." https://www.mobihealthnews.com/news/googles-next-gen-nest-hub-debuts-contactless-sleep-monitoring-and-analysis-features [B — reporting on FDA-non-cleared status]

11. Seeed Studio Wiki, "60GHz mmWave Static Breathing and Heartbeat Radar (MR60BHA1)." https://wiki.seeedstudio.com/Radar_MR60BHA1/ [B — comparable sensor characterization]

12. Wiley / Respirology — Pinilla et al., "Diagnostic Modalities in Sleep Disordered Breathing: Current and Emerging Technology," 2025. https://onlinelibrary.wiley.com/doi/10.1111/resp.70012 [B — clinical context for HSAT and emerging radar diagnostics]

13. npj Digital Medicine, "FDA-cleared home sleep apnea testing devices," 2024. https://www.nature.com/articles/s41746-024-01112-w [B — FDA clearance landscape for comparison]
