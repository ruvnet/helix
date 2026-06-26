# ADR-016: Composite 0–100 Health Score — Transparent, Decomposable, Non-Diagnostic

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001, ADR-003, ADR-005, ADR-006, ADR-007, ADR-008, ADR-009, ADR-010, ADR-015

---

## Context

### The user's need: one answer to "how am I doing?"

Every Helix user arrives with a version of the same question: "How am I doing, and which way am I heading?" The product's full data picture — hundreds of biomarkers, wearable streams, ambient vitals, medication history, genomics — is comprehensive precisely because health is complex. But complexity without a synthesis is just more noise.

A single glanceable score addresses a real user need: a landmark. Not a definitive medical verdict, but a directional anchor that makes the comprehensive picture navigable. Without it, the dossier is a library without a table of contents. The score is the table of contents.

The design challenge: make the score genuinely useful without being deceptive. That requires solving three problems simultaneously — transparency (what went into this number?), decomposability (which systems are driving it?), and calibration (how confident is the system in this score right now?).

### The industry failure mode: black-box proprietary scores

The consumer wearable industry offers the most relevant negative example. Oura, Whoop, and Garmin all compute composite "readiness," "recovery," and "health" scores that are glanceable and popular. The research verdict is stark: **not a single manufacturer discloses how their scores are calculated, and very few provide peer-reviewed evidence that they actually reflect meaningful physiology** [B, biosourcesoftware.com; de Gruyter, 2025].

A 2025 peer-reviewed analysis of composite wearable scores found that raw HRV and resting heart rate data showed significant associations with validated stress measures, but the proprietary composite algorithms lacked transparency and independent validation. The core finding: "raw HRV and RHR data are useful; the composite scores add questionable value" when those scores are opaque [B, de Gruyter]. Users experience this as confusion: wearing an Oura ring and a Whoop simultaneously produces dramatically different readiness numbers because "each company uses different data timeframes, weights HRV or sleep differently, and feeds it all through proprietary formulas" [B, kygo.app].

The failure modes of opaque composite scores:
1. **False precision**: a score of 73 vs. 74 implies a meaningful difference that the underlying data cannot support.
2. **Inscrutable variation**: the user cannot understand why their score dropped 8 points overnight and has no way to investigate.
3. **Unvalidated weighting**: without disclosed methodology, there is no way to know whether the score is measuring something real or reflecting the developer's prior beliefs about which signals matter.
4. **Gaming susceptibility**: if users don't know the formula, they can't meaningfully modify behavior to improve the score — they optimize for the score, not for the underlying health state.
5. **Lack of versioned accountability**: when the score computation changes (it always does), users have no way to know that a score change is a formula change rather than a health change.

Helix's composite score must solve all five.

### The COMPASS precedent: deterministic, transparent, decomposable

COMPASS (COMPosite Activity Scoring System), a 2025 research framework for deterministic health scoring, establishes the right architectural pattern: a transparent, deterministic workflow of three sequential steps — **thresholding** (is each measurement within its reference range?), **standardization** (normalize each measurement to a common scale), and **aggregation** (combine standardized values into sub-scores and a composite) [B, biorxiv.org/2025]. The key innovation is that the workflow is described in full and reproducible: given the same inputs, the same score is produced every time, and the derivation can be traced step by step.

This is the pattern Helix adopts, enhanced with: personal-baseline components (not just population norms), trend direction weighting (a value moving in the right direction contributes more positively than an identical value with a worsening trend), and explicit confidence quantification.

### The AHRQ composite measure architecture as methodological reference

AHRQ's Patient Safety Indicators (PSI) composite measures (v2024) provide a well-established approach to combining heterogeneous clinical quality indicators into a defensible composite [B, ahrq.gov]. The architecture: component measures are assigned weights (numerator weights + harm weights), normalized to a common scale, and aggregated. Crucially, the weights are published, the computation is documented, and version numbers track changes over time. This is the institutional standard for defensible composite scoring in healthcare.

Helix's score adopts this spirit: published weights, documented computation, versioned methodology, available for inspection by users and, eventually, external researchers.

### Regulatory framing: wellness orientation, not medical risk diagnosis (ADR-010)

The composite health score must remain within the wellness positioning established in ADR-010. The key boundary:

- **Permissible**: "Your overall wellness orientation is 72/100. Your sleep subscore dropped 8 points this week, driven by shorter deep sleep duration and a higher resting heart rate. Here is exactly which data drove that."
- **Not permissible**: "Your cardiovascular risk score is 72/100." "You are at moderate risk of a cardiac event." "Your disease risk score is elevated."

The score measures wellness state — the aggregated status of positive health indicators — not disease risk, mortality risk, or clinical diagnosis likelihood. These are fundamentally different framings. Wellness orientation is a motivational and navigational tool; risk stratification is a clinical function requiring regulatory approval, clinical validation, and often a prescribing clinician in the loop.

Every surface that shows the score must display the framing ("wellness orientation, not a medical diagnosis") in immediate proximity. This is a Verifier/Critic agent (ADR-008) enforcement point, not just a design guideline.

---

## Decision

### Decision 1: 0–100 composite score with full decomposability

Helix produces a single composite score on a 0–100 scale, where higher is better (more wellness indicators in favorable range with positive trends). The score decomposes into six named subsystem sub-scores, each independently interpretable:

| Subscore | What it measures |
|---|---|
| **Cardiometabolic** | Lipids (LDL-C, ApoB, HDL, TG), blood pressure, resting HR, HRV, glucose/insulin markers, hs-CRP |
| **Sleep** | Duration, efficiency, deep sleep proportion, REM proportion, restlessness, respiratory regularity (Seed, ADR-014) |
| **Inflammation** | hs-CRP, ferritin, WBC, ESR, homocysteine; where available: IL-6, TNF-α |
| **Fitness** | VO₂max proxy, step count trend, resting HR trend, HRV trend, workout consistency |
| **Metabolic** | Fasting glucose, HbA1c, fasting insulin, HOMA-IR proxy, thyroid panel (TSH, fT4), vitamin D, B12, ferritin |
| **Recovery** | HRV, resting HR, sleep efficiency, subjective energy score, Seed overnight restlessness, cross-source consistency |

Each sub-score is 0–100. The composite is a weighted average of sub-scores, with weights determined by data availability and evidence quality for each subsystem's contribution to overall wellness (initial weights documented in §Methodology Appendix below).

The UI presentation hierarchy:
- **Score ring** (ADR-015 / §12 product spec): single composite number, trend arrow, 7-day sparkline.
- **Subscore breakdown**: tapping the composite opens the six sub-scores, each with: value, trend direction, confidence indicator, and a one-line plain-language summary.
- **Driving data points**: tapping a sub-score shows the specific measurements that drove it — each with value, reference range, source, date, and direction of change.
- **Score explanation**: a permanent "how is this calculated?" link opens a human-readable (not just technical) description of the methodology, current version number, and the last date the methodology was updated.

This is the "score you can open" design principle from the product spec: "tap the 82 and see exactly which of your readings produced it. Never a black box."

### Decision 2: Deterministic computation in the Trend/Numeric agent (ADR-007 binding)

The composite score and all sub-scores are computed entirely by the deterministic Trend/Numeric agent (ADR-007). No LLM participates in the computation of any numerical score component. The LLM's role is to produce the plain-language explanation of the computed score — not the score itself.

Computation pipeline per subsystem:

```
Step 1 — MEASUREMENT RETRIEVAL:
  Query RuVector for all measurements in the subsystem's data binding contract,
  within the configured recency window (default: most recent value, 90-day lookback).
  Tag each measurement with: value, reference_range, source, timestamp, recency_weight.

Step 2 — THRESHOLDING AND NORMALIZATION:
  For each measurement:
    - in_range: bool = (value >= ref_range.low) AND (value <= ref_range.high)
    - z_score = (value - ref_range.midpoint) / ref_range.width * 0.5
    - clamped_z = clamp(z_score, -3.0, 3.0)
    - normalized_0_100 = 50 + (clamped_z * -16.67)  # inverts so higher=better

Step 3 — TREND WEIGHTING:
  For each measurement with ≥ 2 historical values:
    - slope = linear_regression(timestamps, values).slope
    - normalized_slope = tanh(slope / ref_range.width) * 10  # -10 to +10 points
    - measurement_score += normalized_slope  # trend bonus/penalty

Step 4 — RECENCY WEIGHTING:
  recency_weight = exp(-age_days / 90)  # exponential decay; 90-day half-life
  weighted_score = measurement_score * recency_weight

Step 5 — SUBSCORE AGGREGATION:
  subscore = weighted_mean(weighted_scores, weights=measurement_weights)
  # measurement_weights from published methodology table

Step 6 — CONFIDENCE ESTIMATION:
  data_coverage = count(available_measurements) / count(expected_measurements)
  mean_recency = mean(recency_weights)
  confidence = data_coverage * mean_recency  # 0.0–1.0

Step 7 — COMPOSITE AGGREGATION:
  composite = weighted_mean(subscores, weights=subscore_weights, min_data_threshold=0.3)
  # A sub-score with data_coverage < 0.3 contributes at 50% weight with a "low data" flag
```

The entire computation is in pure Rust (per project architecture rules), deterministic for identical inputs, and unit-tested against a reference dataset.

### Decision 3: Versioned, published, defensible methodology

The score methodology is version-controlled and publicly documented (within the app, not behind a paywall). Each version increment is semantically versioned:

- **MAJOR** (x.0.0): a change to subsystem weights or composite aggregation formula that would cause a score change of ≥ 5 points for a fixed set of reference inputs.
- **MINOR** (x.y.0): addition of a new measurement type to a subsystem's data binding (could improve coverage; slightly modifies score).
- **PATCH** (x.y.z): bug fix, reference-range update for an existing measurement, or recency-decay parameter adjustment (score change < 1 point for reference inputs).

On every version change:
- The new version number is shown on the score screen.
- A changelog entry is stored in the user's vault explaining what changed and why.
- The user is shown a comparison of their score under the old and new methodology for the 30 days before the update, so they can distinguish a methodology change from an actual health change.

This versioning policy directly addresses the opacity problem: if Oura changes its readiness formula, the user has no way to know. Helix users will always know.

The methodology document includes: measurement-to-subscore mapping table, measurement weights within each subscore, subscore-to-composite weights, reference range sources (e.g., AHRQ, clinical society guidelines, published population studies), recency-decay parameters, and confidence-estimation formula. Every parameter has a cited rationale.

### Decision 4: Confidence display — never hide uncertainty

Every score and sub-score surface displays a confidence indicator derived from data coverage and recency. The confidence states:

| Confidence | Coverage + recency condition | UI presentation |
|---|---|---|
| **High** | ≥ 80% of expected measurements present, mean recency ≥ 0.6 | Score displayed normally |
| **Medium** | 50–79% coverage or recency 0.3–0.59 | Score shown with amber confidence ring; "Based on partial data" label |
| **Low** | < 50% coverage or mean recency < 0.3 | Score shown with gray confidence ring; "Limited data — score is approximate" label |
| **Insufficient** | < 30% coverage | Composite score not shown; sub-scores for available systems shown; prompt to add data |

A user with only wearable data and no lab panels may achieve Medium confidence for Sleep and Fitness sub-scores but Insufficient for Cardiometabolic and Inflammation. The UI shows this clearly: two sub-scores with values, four with "add data to unlock" states. The composite is only displayed when at least three of six sub-scores meet Low confidence or better.

Confidence is never hidden. A score that appears to be a precise number when it is based on three-year-old lab results is misleading; Helix treats displaying a confident-looking score on stale data as a defect, not a feature.

### Decision 5: Wellness framing — not medical risk diagnosis

Every score surface carries the following elements:

1. **Persistent framing label**: "Wellness orientation — not a medical diagnosis." This cannot be dismissed, hidden, or configured away by the user or any operator configuration.
2. **First-use explanation**: on first view of the score, an explanation card is shown: "This score reflects how your wellness indicators are trending based on your data. It is not a medical risk score or a diagnostic tool. Use it to track direction, not to replace your doctor."
3. **Escalation routing (ADR-009)**: if any component driving the score meets Escalation Guardian thresholds (ADR-009 — values indicating urgent clinical evaluation), the score surface is replaced with an escalation notification. Optimization framing is suppressed. The Escalation Guardian explicitly overrides the scoring surface for red-flag states.
4. **Recommendations are Tier-1 through Tier-4 labeled (ADR-006)**: any action suggestions surfaced alongside the score carry their evidence tier. "Exercise more" is Tier 2 (guideline-level evidence) not Tier 1 (your data) unless the user's own activity data is driving the suggestion.

The Verifier/Critic agent (ADR-008) audits every text surface associated with the score before it reaches the user to confirm framing compliance. Any text that uses diagnostic language ("your risk of", "you are likely to develop", "this indicates") is flagged and rejected.

### Decision 6: Trend-first framing as the primary motivational surface

The product spec's insight is correct: a score's value as a behavior-change tool comes more from its direction than its absolute value. A person at 68 trending up +6 over 4 weeks is doing better — and should feel better — than a person at 74 trending down. The trend is the primary signal; the absolute score is context.

UI implications:
- The 7-day and 30-day trend arrows are visually equal in prominence to the score number itself.
- The daily briefing (ADR-015) leads with the trend direction, not the absolute score.
- Change-point detection (via the Trend/Numeric agent, ADR-007) surfaces "your sleep sub-score improved significantly in the last 10 days — here's what changed" as a proactive insight.
- Score history is stored as a time series in RuVector (ADR-003) and graphable over any user-selected window.

Trend framing also reduces false-precision anxiety: a user who sees their score as "73 and improving" is less likely to fixate on the difference between 73 and 74 than one who sees only the absolute number.

---

## Alternatives Considered

### Alternative A: Single opaque score (industry standard — Oura/Whoop model)

A single 0–100 score computed by a proprietary algorithm, not disclosed to users. This is the dominant industry approach and is clearly understood by users as a directional indicator.

**Why rejected:** The research is clear that opaque composite scores erode trust over time and add questionable clinical value beyond the raw input signals [B, de Gruyter 2025]. The specific failure modes documented above — false precision, inscrutable variation, unvalidated weighting — are exactly the failure modes Helix is designed to avoid. Helix's core differentiation is grounded, transparent, traceable intelligence (ADR-005). An opaque score directly contradicts this. Furthermore, a health product that says "trust me" about its most prominent output without showing its work is a liability: users who experience a score drop they can't explain may be driven toward the wrong clinical action.

### Alternative B: Multiple separate scores without a composite (per-domain only)

Display six separate sub-scores (cardiometabular: 62, sleep: 78, inflammation: 55, etc.) without a composite. This is more information-rich and avoids the composite aggregation problem.

**Why rejected:** Users empirically want and use a single summary score as a navigational anchor. The glanceable briefing, the daily check-in, the "how am I doing?" question — these are all answered by a composite. Multiple scores without a composite require the user to do the synthesis themselves, which is exactly the problem Helix is solving. The composite also provides a trend signal across all domains simultaneously — when the composite drops, the sub-scores tell you why. That diagnostic value is only available with a composite. The decomposable composite is not a compromise between options A and B; it is the design that captures the benefits of both.

### Alternative C: Clinical risk scores (e.g., Framingham, ASCVD, AHA scoring)

Use established clinical risk prediction tools — Framingham 10-year CVD risk, ASCVD pooled cohort equations, etc. — as the score foundation. These are validated, published, and clinically meaningful.

**Why rejected:** Clinical risk scores are specifically designed to stratify disease probability, not to measure current wellness state or motivate behavior change. Displaying an "ASCVD 10-year risk: 12%" to a user is a clinical act that implies the user has been appropriately risk-stratified — which requires a clinical context, a prescribing clinician, and appropriate framing that is not compatible with a wellness product. This would cross the SaMD boundary (ADR-010) and require regulatory clearance. Clinical risk scores are inputs that an individual component might reference (e.g., the cardiometabolic sub-score may incorporate ASCVD-relevant inputs), but they cannot be the output surface of a wellness product without regulatory approval. This is a clear ADR-010 violation.

---

## Consequences

### Positive

- **Trust through transparency**: a score the user can open and trace to specific measurements is fundamentally more trustworthy than a score they must accept on faith. This is the differentiation from every existing wearable score.
- **Motivational efficacy**: trend-first framing, sub-score granularity, and specific driving data create the right behavioral feedback loop — the user knows exactly what to improve and can verify whether their interventions worked.
- **Versioned accountability**: users know when the methodology changes and what changed. This is a form of honesty that is unusual in consumer health software.
- **Clinical governance defensibility**: a published, versioned, deterministic methodology with cited rationale for every parameter is far more defensible in a regulatory review or a product liability situation than a proprietary black box.
- **Integration with the full Helix pipeline**: the composite score is a first-class input to the Functional-Medicine Analyst, which uses it to frame proactive insights. The 3D twin (ADR-015) shows sub-scores as the organ/system color states. The score is not a separate feature — it is a synthesis layer over the full data graph.

### Negative

- **Methodology maintenance burden**: a versioned, published methodology must actually be maintained. Reference ranges evolve as clinical guidelines update. New measurement types must be integrated. This is an ongoing clinical governance task, not a one-time engineering effort.
- **False-precision user education**: even with confidence indicators and wellness framing, some users will treat the composite as a medical verdict. The first-use explanation, persistent framing, and Escalation Guardian override mitigate but do not eliminate this.
- **Low-data users**: a user who has only imported Apple Health step data will see a Low or Insufficient composite with most sub-scores grayed out. This is the honest response, but it reduces the first-session impact. The onboarding flow must set expectations about data coverage before the score is shown.
- **Scoring weight disputes**: published weights invite criticism and debate. "Why is sleep weighted more than cardiometabolic?" is a question that will arise from users, press, and researchers. The methodology document must include cited rationale for every weight, and the clinical governance board must be prepared to defend and update it.
- **Versioning discipline**: MAJOR version changes that shift scores by ≥ 5 points will produce user confusion ("my score dropped 8 points but I feel the same"). The version comparison view (old score vs. new score on same historical data) is the mitigant, but it requires engineering investment to produce correctly.

### Mitigations

- Clinical governance board reviews methodology annually (or triggered by significant new clinical evidence) and approves any weight change.
- First-use educational flow sets user expectations for low-data state before the score is displayed.
- A "what changed?" notification on every score methodology update, with a human-readable explanation of the change.
- The Verifier/Critic agent (ADR-008) audits all score-adjacent text for diagnostic language before display.
- The Escalation Guardian (ADR-009) permanently overrides the score surface for red-flag states, ensuring the score never becomes a false reassurance mechanism in emergencies.

---

## Methodology Appendix: Initial Weight Table (v0.1.0)

This appendix documents the initial proposed weights. These are subject to revision by the clinical governance board before v1.0 release and will be updated with citations to supporting literature.

**Composite weights (sub-score → composite):**

| Sub-score | Initial weight | Rationale summary |
|---|---|---|
| Cardiometabular | 0.25 | Cardiovascular disease is the leading cause of death; lipids, glucose, BP are among the most evidence-rich wellness predictors |
| Sleep | 0.20 | Sleep quality and duration have strong independent evidence for metabolic, cognitive, and cardiovascular outcomes |
| Inflammation | 0.20 | Chronic low-grade inflammation (hs-CRP, ferritin) is a cross-cutting signal for multiple disease trajectories |
| Metabolic | 0.15 | Overlaps with cardiometabular but captures thyroid, micronutrient, and insulin-sensitivity signals separately |
| Fitness | 0.12 | Activity and aerobic fitness are strong mortality predictors but more directly actionable and less diagnostic than biomarkers |
| Recovery | 0.08 | HRV and recovery metrics are important but partially overlap with Sleep and Fitness signals |

Weights sum to 1.0. Low-data sub-scores (< 0.3 coverage) contribute at 50% weight with full weight going to the average of available sub-scores.

**Reference range sources (initial):**
- Lipids: American Heart Association / ACC guidelines 2023.
- Glucose / HbA1c: ADA Standards of Care 2024.
- Thyroid: ATA 2023 guidelines; population reference ranges.
- CRP / inflammatory markers: Clinical laboratory reference intervals (population-derived).
- Sleep: NSF sleep quality guidelines; published age-stratified norms.
- HRV: Age-stratified population norms (RMSSD); personal baseline-relative approach preferred.
- Activity: WHO Global Physical Activity Guidelines 2020.

All reference ranges are stored as a versioned, LOINC-tagged reference table in the RuVector vault (shared system namespace, not user-specific). Updates to this table trigger a PATCH version increment.

---

## Open Questions

1. **Clinical governance board composition**: who are the medical advisors who review and approve the methodology before v1.0 release and on subsequent updates? Suggest: at least one cardiologist, one internist/functional medicine specialist, one sleep medicine physician, one biostatistician, and one patient advocate.

2. **Personal baseline vs. population norm weighting**: for measurements like resting HR and HRV, a pure population-norm approach disadvantages elite athletes with naturally low HR and high HRV. The current design uses z-scores around population midpoints. Should personal baseline normalization apply for measurements with ≥ 90 days of history? Define the crossover rule.

3. **Lab coverage gap management**: a user who has a 2-year-old lipid panel and no recent test will have a high-recency-weight-decayed cardiometabular score. At what recency decay threshold should the UI prompt "your cardiometabular score is based on data from X months ago — consider a fresh panel"? Define the threshold and the UX intervention.

4. **Score display for users in active clinical treatment**: a user on statins will have different lipid targets than population norms; a user with T2D has different glucose targets. Should the scoring methodology support personalized reference ranges set by a clinician (e.g., imported from a FHIR care plan)? This is architecturally possible but requires a clinical workflow integration that is out of scope for Phase 1.

5. **External validation study**: before the score is presented as a meaningful wellness indicator, it should be validated against an external criterion — for example, whether higher scores correlate with lower self-reported fatigue, higher subjective wellbeing, or prospective health events. Define the validation plan and timeline; this is the difference between a defensible product feature and an unvalidated proprietary claim.

---

## References

1. de Gruyter / Technology in Exercise and Biomechanics, "Readiness, recovery, and strain: an evaluation of composite health scores in consumer wearables," 2025. https://www.degruyterbrill.com/document/doi/10.1515/teb-2025-0001/html [B — peer-reviewed critique of opaque composite scores]

2. BioSource Software, "5-Second Science: Wearable Composite Health Scores Require Validation." https://www.biosourcesoftware.com/post/5-second-science-wearable-composite-health-scores-require-validation [B — documents disclosure gap across major wearable vendors]

3. AHRQ, "Patient Safety Indicators (PSI) Composite Measures, v2024." https://qualityindicators.ahrq.gov/Downloads/Modules/PSI/V2024/PSI_Composite_Measures.pdf [A — authoritative reference for composite measure methodology in healthcare; provides component weight and aggregation architecture]

4. biorxiv.org, "COMPASS: A Web-Based COMPosite Activity Scoring System to Navigate Health and Disease Through Deterministic Digital Biomarkers," 2025. https://www.biorxiv.org/content/10.64898/2025.12.02.687315.full.pdf [B — deterministic transparent scoring framework as architectural precedent]

5. Arxiv, "Should policy makers trust composite indices? A commentary on the pitfalls of inappropriate indices for policy formation," 2020. https://arxiv.org/pdf/2008.13637 [B — general methodology on composite index pitfalls; applicable to health score design]

6. Rachele Pojednic, Substack, "Should you trust your wearable? What your recovery score isn't telling you." https://rachelepojednic.substack.com/p/should-you-trust-your-wearable-what [C — secondary/opinion; useful articulation of user-facing opacity problem]

7. Open Wearables Initiative, "Health Scores for Wearable Data." https://openwearables.io/health-scores [B — industry initiative on open standards for wearable health scores; relevant for methodology transparency approach]

8. kygo.app, "Recovery Score Comparison: WHOOP vs Oura vs Garmin 2026." https://www.kygo.app/post/recovery-scores-compared-whoop-oura-garmin [C — secondary; illustrative of cross-vendor score variation problem]

9. American Heart Association / ACC, "2023 ACC/AHA Guideline on the Management of Blood Cholesterol." [A — reference range source for lipid components of cardiometabular subscore]

10. American Diabetes Association, "Standards of Medical Care in Diabetes 2024," Diabetes Care, 2024. [A — reference range source for metabolic subscore glucose and HbA1c components]

11. World Health Organization, "WHO Global Physical Activity Guidelines 2020." [A — activity target reference for fitness subscore]

12. National Sleep Foundation, "Sleep Quality Recommendations," 2023. [A — sleep duration and efficiency targets for sleep subscore]

13. Helix PHI ADR Product Specification, §6 (ADR-016) and §12 (visual layer), ISO Vision LLC, 2026. [A — primary product requirement source for this ADR]
