# ADR-010: Wellness Positioning vs. SaMD (Software as a Medical Device) Regulatory Boundary

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-005 (Grounded answering), ADR-006 (Evidence tiering & abstention), ADR-009 (Red-flag escalation), ADR-014 (Cognitum Seed sensing), ADR-016 (0–100 health score)

---

> **Regulatory disclaimer**: This ADR provides architectural and product positioning guidance
> for the engineering and product team. It does not constitute legal or regulatory advice.
> Engage a regulatory counsel with FDA digital health experience and, separately, a clinical
> governance advisor before shipping any features that approach the device boundary or any
> feature that could be characterized as diagnostic or therapeutic. Nothing in this ADR
> should be read as a legal determination of Helix's regulatory status.

---

## Context

### What FDA SaMD regulation means for a personal health intelligence product

The US Food and Drug Administration regulates "medical devices" under the Federal Food,
Drug, and Cosmetic Act (FD&C Act). Software can be a medical device — called a "Software as
a Medical Device" (SaMD) — and the consequences of crossing that threshold are substantial:
pre-market submission requirements (510(k), De Novo, or PMA), quality system regulations
(21 CFR Part 820), labeling rules, registration, listing, and post-market surveillance.

For a startup in the personal health intelligence space, inadvertently becoming a medical
device is an existential risk: FDA enforcement is a real possibility, and the compliance
cost to correctly navigate the SaMD pathway can take years and millions of dollars even
for a clearly beneficial product.

### The general wellness safe harbor (2016 FDA guidance)

**[A]** In July 2016, FDA finalized "General Wellness: Policy for Low Risk Devices" guidance
(updated February 2019), which describes the class of products for which FDA exercises
enforcement discretion. A product qualifies for this safe harbor if it meets both criteria:

1. **Intended only for general wellness use**: maintaining or encouraging a healthy
   lifestyle and not related to the diagnosis, cure, mitigation, prevention, or treatment
   of any disease or condition.
2. **Low risk to safety**: not invasive (does not penetrate skin or mucous membranes),
   not implanted, and does not use technology that may pose a safety risk absent regulatory
   controls.

**[A]** The 21st Century Cures Act (December 2016, codified at FD&C Act §520(o)) further
defined software excluded from the device definition: software whose function is limited to
administrative support, general wellness promotion, electronic patient records (within
certain limits), and CDS functions meeting specific criteria (see below).

### The CDS boundary: when decision support becomes a device

**[A]** FDA's 2022 "Clinical Decision Support Software" Final Guidance clarified which CDS
functions escape the device definition under §520(o)(1)(E) of the FD&C Act. Four criteria
must all be met for CDS to be excluded:

1. Not intended to acquire, process, or analyze a medical image or signal.
2. Displays, analyzes, or prints medical information about a patient that is generally
   accepted in health care as a training or reference source.
3. Supports or provides recommendations to a health care professional (HCP) who is
   otherwise able to independently review the basis for the recommendation.
4. Enables the HCP to independently review the basis for the recommendations so that
   they need not rely primarily on the software to make a clinical decision.

**[A]** If any criterion is not met — particularly if software provides recommendations
directly to consumers rather than HCPs, or if the user cannot independently verify the
reasoning — the CDS function does not benefit from the exclusion and may require device
evaluation.

**[A]** FDA's January 2026 updated "Clinical Decision Support Software" Final Guidance
(superseding the 2022 guidance) broadened what counts as "medical information about a
patient" to encompass "patient-specific data used in or related to clinical care" and
added transparency requirements: apps with AI/ML-derived recommendations must provide
"clear, accessible documentation" about data inputs and logic. The 2026 guidance also
reflects a modestly more deregulatory posture on general wellness wearables and on
non-invasive parameter estimation.

### What "tips" a wellness app into device territory

Based on the 2016 general wellness guidance, 2022 CDS guidance, and 2026 updates, the
following features or claims tend to move a consumer health product toward regulated SaMD
status:

**[A] Diagnostic claims**: Asserting or implying that the software can identify, diagnose,
detect, or rule out a specific disease or condition in the user. Example: "You have sleep
apnea" (vs. "your breathing pattern overnight is irregular — worth discussing with a
clinician").

**[A] Treatment recommendations**: Recommending a specific therapy, medication adjustment,
or clinical intervention rather than providing information for the user or their clinician
to consider.

**[A] Patient-specific clinical decisions without HCP review pathway**: Making a
recommendation that the user is expected to act on without independent verification by a
qualified clinician. The distinction is between "here is a pattern in your data" and "do X."

**[B] High-risk physiological parameter output**: Continuously monitoring high-risk
parameters (e.g., blood glucose, arrhythmia detection) in a clinical management context
— especially when the output is intended to drive a treatment or intervention decision.

**[A] Continuous or near-continuous measurements feeding real-time predictions**: FDA's
2026 guidance notes that apps relying on "continuous or near-continuous measurements,
image analysis, real-time or near-term predictions" generally remain subject to FDA oversight
when positioned for clinical use.

**[B] Black-box AI generating clinical recommendations**: The 2026 guidance's transparency
requirement makes black-box AI pipelines that produce patient-specific recommendations
(without explainability or an independent review pathway) more likely to receive scrutiny.

### The escalation/guardian pattern and its regulatory significance

ADR-009 specifies a "red-flag escalation" pathway where dangerous values route to "see a
clinician now." This pattern sits at the regulatory boundary. The Escalation Guardian is
a safety feature, not a diagnostic engine — it says "this value is outside the range
associated with safety; please seek clinical evaluation," not "you have condition X." This
framing keeps it within the wellness/safety-information safe harbor provided the thresholds
are established by clinical advisors and the copy is non-diagnostic.

Similarly, the Cognitum Seed's detection of irregular breathing patterns (ADR-014) is
explicitly framed as "screening signals that warrant a conversation," not as a diagnosis
of sleep apnea, which requires polysomnography. Maintaining this framing in the product
copy, the UI, and the app store metadata is a compliance imperative.

---

## Decision

Helix ships initially as a **general wellness and decision-support product** with a
deliberate, legally reviewed non-diagnostic positioning. Features that approach or cross
the SaMD boundary are treated as a separate, independently governed regulatory track —
requiring regulatory counsel approval and clinical governance review before shipping.

### The wellness gates: five rules that must hold at every release

These are pass/fail gates, not aspirational guidelines. Any feature failing a gate must
be either redesigned to pass or placed on the regulated track before release:

**Gate 1 — No diagnosis.**
Helix does not diagnose a disease, condition, disorder, or syndrome. Language that
constitutes diagnosis ("You have anemia," "This indicates hypothyroidism," "Your pattern
is consistent with PCOS") is prohibited in the product. Replace with: "Your ferritin is
below the reference range — this is worth discussing with your clinician." Framing the
observation as data (what it is) with a recommendation to consult a professional (what
to do) is safe; asserting a diagnostic conclusion is not.

**Gate 2 — No treatment prescription.**
Helix does not prescribe, recommend stopping, or recommend adjusting a specific medication
or therapy. Information about a medication's known mechanisms, from published sources, is
allowed with attribution and evidence tiering (ADR-006). "Consider discussing X with your
prescriber" is allowed. "Take X" or "stop Y" is not.

**Gate 3 — Transparent, non-black-box reasoning.**
Every recommendation traces to the user's own data (ADR-005) with source, timestamp, and
reference range. The user can follow the logic. This satisfies the transparency requirement
of the 2026 FDA CDS guidance and limits automation-bias risk.

**Gate 4 — Non-invasive, low-risk sensing only.**
Cognitum Seed uses mmWave radar — non-contact, non-invasive, non-ionizing radiation at
low power levels. No sensor penetrates skin. The product does not use or interpret
prescription-only diagnostic devices (e.g., implanted CGM data is ingested as user health
data, not medically interpreted by Helix — the user's existing clinical management remains
with their clinician).

**Gate 5 — Screening and escalation framing, not clinical management framing.**
Red-flag alerts route to "seek clinical evaluation now" — not "here is your diagnosis and
treatment plan." The product augments the user's ability to have an informed conversation
with a clinician; it does not substitute for that clinician.

### Copy constraints (non-negotiable)

The following language or close variants must appear in the product:

- **In the app**: "Not a diagnosis · augments, doesn't replace, your clinician" (present
  on every answer screen and the home screen footer).
- **In the health score (ADR-016)**: "This is a wellness orientation aid, not a medical
  risk score or diagnosis. Always discuss significant changes with your healthcare provider."
- **In Escalation Guardian outputs (ADR-009)**: "This value is outside the typical reference
  range. This is not a diagnosis. Please contact your healthcare provider or, if you believe
  this is urgent, seek emergency care."
- **In the Cognitum Seed outputs (ADR-014)**: "This is a screening signal based on
  non-medical radar sensing. It does not diagnose sleep apnea or any condition. Please
  discuss with your physician if you have concerns."
- **In all marketing and app store descriptions**: Language reviewed by regulatory counsel
  before submission.

These are not legal boilerplate to be buried in a terms page; they are required copy,
product-team enforced, present in the UI at the point of contact.

### The regulated track (separately gated)

If Helix adds features that cross into SaMD territory — for example:

- Arrhythmia detection from ECG or PPG with clinical diagnostic intent
- A glucose management advisory system tied to CGM readings
- An AI system that recommends specific medication changes
- Any feature marketed to clinicians for clinical decision-making

...those features are built on a separate regulated track with:

1. Regulatory counsel engaged from specification phase.
2. Clinical governance review of all algorithms and thresholds.
3. Pre-submission meeting with FDA before development is complete (for 510(k)/De Novo track).
4. Quality system documentation (21 CFR Part 820 / ISO 13485).
5. Separate versioning, release, and post-market surveillance pipeline.
6. Distinct app store listing or clearly separated feature module, depending on counsel's
   guidance on how FDA views the combined product.

This track is not a later-phase aspiration; the pipeline for it should be established at
or before Phase 3 (see §8 of the spec) when wearable integration deepens.

### Clinical governance and medical advisory board

The wellness/device boundary is not a legal question alone; it requires ongoing clinical
judgment. Helix requires a medical advisory board (MAB) with:

- At least one primary care / internal medicine physician.
- At least one clinical informatics or health IT specialist.
- At least one functional/integrative medicine practitioner (aligned with Helix's use case).
- Periodic review of red-flag thresholds (ADR-009), evidence tier assignments (ADR-006),
  and escalation copy.

MAB review is a release gate for any feature that involves: (a) clinical reference ranges,
(b) red-flag thresholds, (c) conditions or diagnoses mentioned in copy, or (d) any claim
about the efficacy of an intervention.

---

## Alternatives Considered

### Alternative A: Regulated SaMD track from day one

Build and launch under FDA SaMD regulatory controls, pursuing a De Novo or 510(k)
clearance before or contemporaneously with launch.

**Not adopted for Phase 0–1** because the time-to-market impact (12–36+ months for
regulatory clearance) is disproportionate to the Phase 0–1 feature set, which is genuinely
within the wellness safe harbor. The regulated track is preserved for specific features that
need it (see "regulated track" above).

### Alternative B: No regulatory concern — just be careful in copy

Operate as if the wellness safe harbor applies without formal regulatory strategy, relying
on careful product copy to prevent regulatory action.

**Rejected** because copy alone does not determine regulatory status; the *function* of
the software is what FDA evaluates. A reactive posture (adjusting copy in response to FDA
inquiry) is riskier than proactive regulatory strategy. Regulatory counsel must be engaged
before launch regardless of positioning.

### Alternative C: Comply voluntarily with IEC 62304 and ISO 14971 for all features

Apply the full SaMD development lifecycle (IEC 62304 software lifecycle, ISO 14971 risk
management) to every feature, even those clearly in the wellness category.

**Partially adopted**: applying IEC 62304 risk-classification thinking to code quality and
ISO 14971 risk management to clinical-adjacent features is good practice and provides
documentation evidence of the wellness positioning. However, full compliance overhead
applied to every wellness feature creates an unnecessary burden. The MAB and regulated-track
pipeline apply 62304/14971 where it matters.

---

## Consequences

### Positive

- Faster, lower-risk Phase 0–1 launch without pre-market submission delays.
- A defensible, reviewable copy and design record if FDA inquiry occurs.
- Clear internal rules that prevent engineers and product managers from inadvertently
  shipping features that constitute diagnosis without realizing it.
- A credible regulated track for future high-value clinical features.

### Negative

- Constrains some high-value features that users may want (e.g., "does this pattern
  indicate X?") until they are built and reviewed on the regulated track.
- Ongoing MAB engagement and legal review create cost and time overhead that a purely
  wellness product would not face.
- The boundary between "useful wellness intelligence" and "diagnosis" is genuinely ambiguous
  in some cases (e.g., HRV patterns associated with overtraining vs. cardiac risk). These
  require case-by-case clinical governance review, not a one-time architectural decision.

### Mitigations

- Invest in the MAB early (Phase 0), not at Phase 3 when features are already built.
- Build a copy review checklist and integrate it into the release gate process.
- Retain regulatory counsel on retainer, not just on engagement-by-engagement basis, so
  review cycles can be fast.
- Darwin Mode (ADR-018) fitness function includes clinical-safety compliance gates: any
  evolved configuration that fails a Gate 1–5 check is rejected even if it improves
  grounding or coverage scores.

---

## Open Questions

1. **Non-US regulatory analogues**: The EU MDR (Medical Device Regulation 2017/745) and
   UK MHRA have their own SaMD frameworks; CE marking may be required for EU users. How
   analogous is the EU MDR wellness exclusion to the FDA general wellness safe harbor?
   Engage EU regulatory counsel before targeting EEA users.

2. **FTC jurisdiction**: Even below the FDA SaMD threshold, FTC Section 5 (unfair or
   deceptive acts) and the FTC Act's health claim standards apply to wellness marketing.
   Are all Helix marketing claims substantiated? Engage FTC-experienced counsel for
   marketing review.

3. **Darwin Mode and regulatory risk**: If Darwin Mode (ADR-018) mutates prompt scaffolds
   or verifier thresholds that affect clinical copy, does the evolved version need
   re-review? Answer: yes. Darwin Mode's Gate 1–5 checks must be integrated into the
   DRACO fitness function so that evolved configurations are constrained to not violate
   wellness positioning.

4. **Cognitum Seed FCC/RF considerations**: mmWave radar hardware is subject to FCC
   Part 15 and potentially 47 CFR Part 15B. Ensure the hardware partner has appropriate
   FCC authorization for the intended device class (ADR-014 scope).

5. **2026 FDA CDS guidance changes**: The January 2026 final guidance superseded the
   2022 guidance with broadened scope and new transparency requirements. Confirm with
   regulatory counsel whether the updated guidance changes any analysis above.

---

## References

| # | Source | Evidence | URL |
|---|--------|----------|-----|
| 1 | FDA: General Wellness Policy for Low Risk Devices (2016/2019) | [A] | https://www.fda.gov/regulatory-information/search-fda-guidance-documents/general-wellness-policy-low-risk-devices |
| 2 | FDA: General Wellness guidance PDF download | [A] | https://www.fda.gov/media/90652/download |
| 3 | Arnold & Porter: FDA 2022 CDS Final Guidance analysis | [A] | https://www.arnoldporter.com/en/perspectives/advisories/2022/10/fda-releases-significantly-revised-final-clinical |
| 4 | Ropes & Gray: Is your CDS software a medical device? 2022 | [A] | https://www.ropesgray.com/en/insights/alerts/2022/10/is-your-clinical-decision-support-software-a-medical-device |
| 5 | Faegre Drinker: Key updates in FDA's 2026 General Wellness and CDS guidance | [A] | https://www.faegredrinker.com/en/insights/publications/2026/1/key-updates-in-fdas-2026-general-wellness-and-clinical-decision-support-software-guidance |
| 6 | Arnold & Porter: FDA "cuts red tape" on CDS and wearables 2026 | [A] | https://www.arnoldporter.com/en/perspectives/advisories/2026/01/fda-cuts-red-tape-on-clinical-decision-support-software |
| 7 | Jones Day: A relaxing 2026 — FDA updates General Wellness and CDS guidance | [B] | https://www.jonesday.com/en/insights/2026/01/a-relaxing-2026-fda-updates-general-wellness-and-clinical-decision-support-software-guidance |
| 8 | DLA Piper: Your CDS software may now be regulated by FDA as a medical device (2022) | [B] | https://www.dlapiper.com/en-us/insights/publications/2022/09/your-clinical-decision-support-software-may-now-be-regulated-by-fda-as-a-medical-device |
| 9 | Akin: FDA issues key guidance updates for digital health and wellness | [B] | https://www.akingump.com/en/insights/blogs/eye-on-fda/fda-issues-key-guidance-updates-for-digital-health-and-wellness |
| 10 | TactionSoft: FDA SaMD compliance guide | [C] | https://www.tactionsoft.com/blog/fda-samd-software-as-medical-device-compliance-guide/ |
| 11 | PMC: US FDA regulation of clinical software in the era of AI/ML | [B] | https://pmc.ncbi.nlm.nih.gov/articles/PMC12264609/ |
