# ADR-009: Red-Flag Escalation & Clinician-in-the-Loop

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-005, ADR-006, ADR-007, ADR-008, ADR-010, ADR-014, ADR-016, ADR-018

---

## Context

### When optimization tips are the wrong response

Helix is built to help people optimize their health. But some values are not optimization
problems — they are emergencies. A critically low hemoglobin, a dangerously elevated
potassium, a pattern of nocturnal breathing pauses detected by the Cognitum Seed: these
are not inputs to a recommendation engine. They are signals that require a clinician, now.

The failure mode to prevent is not subtle: an optimization-oriented AI, presented with
a critically abnormal lab value, might respond with a supplement recommendation. That
is the wrong response in every dimension — it treats a safety situation as a wellness
question, it delays necessary care, and it actively misleads the user about the severity
of what they are looking at. The Escalation Guardian exists to prevent this failure mode
at the architectural level.

### Clinical "critical values": the established medical standard

Clinical laboratories have maintained formal "critical value" programs since at least the
1970s (Lundberg's original "panic values" concept). A critical value is a result so far
outside the expected range that a delay in clinical review could pose immediate risk to
the patient. Laboratory personnel are required to report critical values to a treating
clinician within a defined window — typically 30 minutes from result availability [A].

The published evidence base for specific critical value thresholds was examined by
"Establishing an Evidence Base for Critical Laboratory Value Thresholds" (American Journal
of Clinical Pathology, 2014, doi:10.1093/ajcp/aqu069) — one of the most rigorous
analyses of the evidence behind commonly cited threshold values. Key findings: many
thresholds in use are based on institutional convention rather than prospective studies,
making the choice of threshold an active medical governance question rather than a fixed
technical parameter. This directly motivates ADR-009's medical advisory governance
requirement [A].

Commonly cited adult critical value examples from major academic medical centers (Cleveland
Clinic Laboratories, Brown University Health) include [A]:

**Electrolytes and metabolic:**
- Potassium: critical high ≥ 6.0 mmol/L (risk: life-threatening cardiac arrhythmia,
  particularly in the context of ECG changes or renal disease); critical low ≤ 2.5 mmol/L
- Sodium: critical high ≥ 160 mmol/L (severe hypernatremia); critical low ≤ 120 mmol/L
  (severe hyponatremia; risk: cerebral edema, seizures)
- Glucose: critical low < 40–50 mg/dL (severe hypoglycemia; risk: loss of consciousness,
  irreversible brain injury); critical high > 500–600 mg/dL (hyperglycemic crisis)
- Bicarbonate (CO2): critical low ≤ 10 mmol/L (metabolic acidosis); critical high
  ≥ 40 mmol/L (severe metabolic alkalosis)
- Calcium, total: critical high ≥ 13–14 mg/dL (hypercalcemic crisis); critical low
  ≤ 6–6.5 mg/dL (tetany, seizure risk)
- Magnesium: critical low ≤ 1.0 mg/dL (arrhythmia risk)

**Hematologic:**
- Hemoglobin: critical low < 6–7 g/dL (severe anemia requiring clinical evaluation and
  likely transfusion consideration); critical high varies (polycythemia context)
- Hematocrit: critical low < 15–18%
- Platelet count: critical low < 20–50 × 10⁹/L (hemorrhagic risk)
- INR / prothrombin time: critical high > 5.0 (hemorrhagic risk, especially in
  anticoagulated patients)
- PTT: critical high > 100 seconds

**Cardiovascular and respiratory:**
- Troponin (high-sensitivity): any value above the 99th-percentile URL is actionable;
  rising serial troponins are a red flag requiring clinical evaluation within hours.
- BNP / NT-proBNP: critical elevations vary by institutional protocol but a markedly
  elevated BNP (> 500 pg/mL in the acute context) warrants urgent clinical attention.
- Arterial blood gas: pH < 7.20 or > 7.60; pO₂ < 50 mmHg in an arterial sample;
  pCO₂ > 70 mmHg.
- Fingertip SpO₂ (pulse oximetry): sustained SpO₂ < 90% in a person without known
  baseline hypoxemia.

**Ambient / wearable red flags (non-lab):**
- Cognitum Seed (ADR-014): patterns consistent with severe sleep-disordered breathing
  (respiratory event index > 30 events/hour; respiratory pauses > 20 seconds regularly).
- Resting HR: sustained resting HR > 100 bpm (tachycardia) or < 40 bpm (bradycardia)
  not explained by known medication/athletic baseline.
- CGM: glucose < 54 mg/dL sustained (Level 2 hypoglycemia per ADA definition); glucose
  > 250 mg/dL sustained in an insulin-using individual.

**Other critical values requiring urgent attention:**
- TSH: critical low < 0.01 mIU/L (possible thyrotoxicosis); critical high > 10 mIU/L
  with symptoms.
- PSA: rapid doubling; values > 10 ng/mL with no established baseline (not critical per
  se, but high-priority for urological follow-up).
- Creatinine: doubling from baseline within 48 hours (consistent with AKI definition).

These examples are illustrative. Specific thresholds must be reviewed and approved by the
medical advisory board before deployment (see Medical Advisory Governance below).

### Abstention and deferral in clinical decision support

The concept of deferral — explicitly routing to a clinician rather than generating a
recommendation — is a design pattern in clinical decision support with its own literature.
AHRQ and ONC CDS guidance both frame the ability to route a clinical question to a human
expert as a first-class outcome of a CDS system, not a failure [B]. The 2026 npj Digital
Medicine review on LLM abstention in healthcare ("When silence is safer") establishes
that safety-driven abstention — declining to provide information that, even if correct,
could mislead without clinical context — is a mandatory design property in medical AI [A].

For red-flag values, the correct system behavior is not abstention (which implies "I don't
know") but *escalation* (which implies "I know this is significant and it requires a
clinician"). The distinction is important for UX: abstention is a gentle gap notice;
escalation is an unambiguous, prioritized call to action.

### "Augments not replaces" is a regulatory and safety framing

The wellness/decision-support positioning (ADR-010) creates the regulatory space for Helix
to operate. Within that space, the "augments not replaces a clinician" framing is not
merely a disclaimer — it is the product's honest characterization of what it can and
cannot do. The Escalation Guardian is the mechanism that makes this framing concrete:
when a value reaches the clinical threshold that requires professional judgment, the system
explicitly says so and routes the user toward it. The product does not attempt to
substitute for the clinician in that moment; it routes to one.

This framing aligns with ADR-010's SaMD boundary: the Escalation Guardian's outputs are
never diagnoses. They are urgency notifications with a clear action: "see a clinician now."
The specific diagnosis — what the lab value means and what to do about it — remains the
clinician's domain.

---

## Decision

### The Escalation Guardian: role in the Ruflo swarm

The Escalation Guardian is a dedicated Ruflo agent that runs in parallel with every
pipeline pass and has priority authority to preempt the Analyst and Verifier. It operates
as a sensor-style agent: it watches for specific conditions (threshold crossings, pattern
matches) and when conditions are met, it fires an escalation event that modifies the
pipeline's output mode.

The Escalation Guardian is the *last line of defense* in the anti-hallucination pipeline:
even if the Analyst produces a perfectly grounded, Verifier-approved optimization
recommendation, if the Escalation Guardian fires on any value in the data context, that
optimization content is suppressed and the escalation takes over.

The Guardian watches three input streams:

1. **Lab and measurement values**: every ProvRecord value is compared against the
   red-flag threshold registry at ingestion time and again at response time. A value
   that exceeds a critical threshold fires an escalation regardless of when it was measured.

2. **Trend and numeric facts** (from ADR-007): trends that project toward a critical
   threshold, even if the current value is not yet critical, fire a "watch" flag (below
   full escalation) that promotes the metric in the user's standing health model.

3. **Ambient vitals** (Cognitum Seed, ADR-014): pattern matches on respiration signals,
   resting HR streams, and CGM continuous data. These are evaluated against wearable-class
   thresholds (lower specificity than lab values) with appropriate screening-signal framing.

### Red-flag threshold registry

The threshold registry is a versioned, human-editable configuration that specifies:

```
RedFlagThreshold {
  metric_id:       String        // LOINC code or Helix internal ID
  metric_name:     String
  trigger_type:    TriggerType   // AbsoluteHigh | AbsoluteLow | TrendProjection | PatternMatch
  critical_low:    Option<f64>   // value at or below this → Level 2 (critical) escalation
  warning_low:     Option<f64>   // value at or below this → Level 1 (urgent) escalation
  warning_high:    Option<f64>   // value at or above this → Level 1 (urgent) escalation
  critical_high:   Option<f64>   // value at or above this → Level 2 (critical) escalation
  unit:            String        // UCUM
  population_note: String        // e.g., "adult; adjust for pediatric / CKD / anticoagulation"
  escalation_text: String        // what to say to the user at this threshold
  action_text:     String        // the specific action the user should take
  advisory_ref:    String        // reference to the medical advisory decision that set this
  last_reviewed:   ISO date      // date of last medical advisory review
  version:         String        // semantic version of this threshold entry
}
```

Every threshold entry is version-controlled (git), reviewed by the medical advisory board
before deployment, and linked to the clinical reference that motivates the threshold value.
Thresholds cannot be changed unilaterally by Darwin Mode (ADR-018): they are a
governance-controlled input, not a self-optimizing parameter.

### Escalation levels and user experience

The Escalation Guardian fires at two levels:

**Level 1 — Urgent (orange/amber):**
Triggered by values at or beyond warning thresholds. The response:
- Prominently flags the specific metric and value.
- States clearly: "This value is outside the range where I can give you optimization
  guidance without you first speaking to a clinician."
- Recommends a specific action: "Schedule an appointment with your doctor, or call your
  provider's nurse line, within the next 1–3 days."
- Suppresses any optimization recommendation related to the flagged metric, but does
  not suppress all other content.
- Offers to help prepare a summary of the flagged value and relevant context for the
  clinical appointment ("prep for my appointment" ADR, §2 Tier D).

**Level 2 — Critical (red):**
Triggered by values at or beyond critical thresholds. The response:
- Takes over the entire UI surface — no other content is shown alongside the escalation.
- States clearly and specifically: "[Metric] is at a critical value ([value] [unit]).
  This requires medical attention now. Do not wait."
- Provides immediate action options:
  - "Call 911 / emergency services" (if applicable to the specific threshold, e.g.,
    suspected hypoglycemic emergency, suspected acute cardiac event).
  - "Call your doctor or go to urgent care now."
  - "If you feel symptoms [list symptom context], call 911."
- Suppresses all optimization content and all other response content entirely.
- Optionally: if ICE (in-case-of-emergency) contact information is stored, offers to
  initiate that contact.
- Logs the escalation event in the audit trail (ADR-002) with timestamp, metric, value,
  and escalation level.

The Escalation Guardian is designed to be impossible to accidentally dismiss. The escalation
UI must require explicit acknowledgment ("I understand; I will seek care") before the user
can navigate elsewhere in the app. This is the one Helix UI pattern where engagement
optimization explicitly applies — but the engagement goal is to confirm the user has seen
and acknowledged the warning, not to increase session time.

### Suppression of optimization content on red-flag firing

This rule is absolute: **when the Escalation Guardian fires at any level, optimization
recommendations related to the flagged domain are suppressed.**

The reasoning: presenting "here's how to optimize your potassium intake" to a user with
a critically elevated potassium is not just unhelpful — it actively misleads them about
the severity of the situation. The user might act on the optimization tip rather than
seeking emergency care. This failure mode has a real-world precedent in clinical alerts
fatigue, where the presence of a minor recommendation alongside a critical alert reduces
the user's perception of the critical alert's urgency.

Implementation: the Escalation Guardian fires before the Analyst's optimization content
is drafted. If a Level 2 escalation fires during query processing, the Analyst is not
invoked for optimization content; only the Escalation Guardian's response is generated.
If a Level 1 escalation fires, the Analyst is invoked for non-related-domain content but
is given an explicit constraint: no recommendations related to [metric domain] in this
response.

### Trend-based watch flags (pre-critical monitoring)

In addition to threshold-based escalation, the Escalation Guardian maintains a watch list:
metrics that are within normal range but trending toward a warning threshold, with a
projected time to threshold breach (from ADR-007 trend computation):

```
WatchFlag {
  metric_id:         String
  current_value:     f64
  current_date:      ISO date
  warning_threshold: f64
  projected_breach:  Option<ISO date>  // null if trend doesn't project to threshold
  days_to_breach:    Option<u32>
  trend_confidence:  f32               // R² from linear regression
  notice_text:       String            // e.g., "Your ferritin is declining and may fall 
                                       // below range in approximately 6 weeks at the 
                                       // current rate."
}
```

Watch flags surface in the standing health model and the 0–100 health score (ADR-016) as
amber indicators, without the urgency framing of a threshold breach. They prompt proactive
action ("consider discussing ferritin trend with your doctor at your next visit; a retest
in 8 weeks would clarify whether the trend is continuing") rather than emergency action.

Watch flags are available to the Analyst as context. The Analyst may reference them in
responses, with the constraint that trend projections carry appropriate uncertainty
communication (ADR-006, ADR-007) and the framing: "discuss with your clinician" rather
than "you will reach a critical value."

### Ambient vital escalation (Cognitum Seed, ADR-014)

Cognitum Seed ambient signals require different treatment than lab values:

1. **Lower specificity**: mmWave-derived physiological signals have higher false-positive
   rates than laboratory measurements. A single night of anomalous respiration patterns
   does not warrant the same escalation urgency as a critically low hemoglobin.

2. **Framing requirement**: Seed outputs are screening signals, not diagnostic findings
   (ADR-014). The escalation language must reflect this:
   - Not: "Your Helix detected sleep apnea."
   - Correct: "Your Helix has detected patterns in your overnight breathing that are
     consistent with sleep-disordered breathing. This pattern has appeared on N of the
     last M nights. Sleep-disordered breathing is diagnosed by sleep study, not by
     ambient sensing. We recommend discussing this pattern with your doctor, who may
     refer you for a sleep evaluation."

3. **Persistence requirement**: Ambient-signal escalation requires a minimum pattern
   duration to reduce false positives. The exact persistence threshold (e.g., N nights
   in a row, or N of M nights over a window) is set by the medical advisory board and
   stored in the threshold registry.

4. **Integration with lab escalation**: If a user also has a low hemoglobin (known to
   increase sleep-disordered breathing risk) and the Seed detects relevant patterns,
   the Analyst and Escalation Guardian coordinate: the Analyst may reference both signals
   together with appropriate framing. The conjunction of a lab-based and an ambient-based
   signal increases the urgency of the "discuss with clinician" recommendation without
   crossing into diagnosis.

### Medical advisory governance

The red-flag threshold registry is not a static technical artifact — it requires ongoing
clinical governance. Responsibilities:

**Medical Advisory Board (MAB):** A standing body of licensed clinicians (target
composition: 3–5 physicians spanning internal medicine, cardiology, endocrinology,
sleep medicine, and a clinical laboratory specialist). The MAB:
- Reviews and approves every new threshold before it enters the registry.
- Reviews the registry annually for alignment with updated clinical guidelines.
- Adjudicates disputed escalation cases (events flagged as false positives by users or
  the Darwin eval set).
- Approves changes to the ambient escalation persistence requirements.
- Reviews any population adjustments to thresholds (pediatric, CKD-adjusted, etc.).

**Threshold update process:**
1. Proposed change submitted with clinical reference.
2. MAB review (synchronous for critical-value changes, async for watch-flag changes).
3. MAB approval documented in `advisory_ref` field.
4. Version bump to threshold registry; deployed as a governed update (not via Darwin Mode).
5. Retrospective review after 30 days to assess escalation rate and any false-positive
   reports from users.

**Out-of-scope for Darwin Mode:** Darwin Mode (ADR-018) self-optimizes Helix's
configuration toward faithfulness. Escalation thresholds are explicitly excluded from the
Darwin mutation space. A self-optimizing system that adjusts clinical escalation thresholds
based on user engagement feedback is not safe. Thresholds are fixed by clinical governance;
they are not hyperparameters.

### "Augments not replaces" framing: implementation

Every Helix response — not just escalation events — carries a footer: "Not a diagnosis ·
augments, doesn't replace, your clinician." This is not a disclaimer in the legal sense;
it is an honest statement of the product's role.

For escalation events, this framing is elevated to the primary message, not the footer:
the escalation output explicitly says what Helix can tell the user (a value is outside a
concerning threshold), what it cannot tell them (what this means for their specific
clinical situation), and who can tell them (their clinician). The escalation is designed
to increase the likelihood of timely clinical contact, not to substitute for it.

This framing connects directly to ADR-010 (wellness positioning vs. SaMD boundary):
Helix's escalation outputs are patient-empowerment tools — giving the user the information
they need to have a productive, urgent clinical conversation — not clinical decision support
outputs directed at a clinician. The difference matters both for regulatory compliance and
for the user's own mental model of what Helix is.

---

## Alternatives Considered

### Alternative A: Hard suppression of all health data when any red flag is detected

When a red flag fires, suppress access to all health data and optimization content until
the user acknowledges consulting a clinician. Only restore access after a clinician
confirmation flow.

Rejected because: this is paternalistic, unworkable in practice, and potentially harmful.
A user with a slightly elevated potassium who is already under physician management does
not benefit from having their health data locked. The "suppress related optimization
content" rule is appropriately scoped: it prevents misleading optimization guidance
adjacent to a red-flag value, without preventing the user from continuing to use the
product for unrelated health areas. Full lockout also creates a problematic UX incentive
to dismiss escalations in order to regain access.

### Alternative B: All escalations delivered identically regardless of severity

Use a single escalation tier rather than two (warning vs. critical). Simpler to implement
and avoid ambiguity about thresholds.

Rejected because: a single severity level forces a binary choice between under-reacting
(calling everything "urgent care now" including mild lab abnormalities) and over-reacting
(calling only true emergencies an escalation, missing clinically important but non-emergency
values). The two-tier system matches the "critical value / action value" distinction used
by clinical laboratories, preserves proportionality, and reduces alert fatigue — a known
problem in clinical decision support [B].

### Alternative C: Rely on user-reported symptoms rather than threshold-based detection

Only escalate when the user reports symptoms alongside an abnormal value, not on
threshold-based detection alone. This avoids false-positive escalations for asymptomatic
laboratory abnormalities.

Rejected because: many dangerous laboratory values are initially asymptomatic. A potassium
of 6.5 mmol/L may cause no symptoms a user would recognize — but carries meaningful cardiac
arrhythmia risk. Critically low hemoglobin develops gradually; the user may have adapted
to their symptoms and not report them as acute. The escalation architecture exists precisely
to catch the cases where the user does not know to be alarmed. Symptom-only triggering
would miss the most dangerous cases.

---

## Consequences

### Positive

- **The most important safety guarantee.** Red-flag recall — the fraction of dangerous values
  that trigger escalation — is the primary safety metric (§10). The Escalation Guardian
  architecture is designed to achieve 100% recall on values meeting critical thresholds.
- **Trust with clinicians.** A product that reliably routes patients to clinical care when
  appropriate builds trust with the medical community, enabling the "prep for my appointment"
  feature to function as a bridge rather than a barrier between Helix and clinicians.
- **Legal and regulatory protection.** Explicit escalation with a clear "see a clinician"
  action, combined with the wellness positioning (ADR-010), creates a strong defense
  against claims that Helix provided reckless health guidance. The escalation event,
  the user's acknowledgment, and the action recommendation are all in the audit trail.
- **User trust.** Users who see Helix correctly identify a critical value and urgently
  route them to care develop deep trust in the product — the system demonstrated it
  was watching their back when it mattered.

### Negative

- **Threshold governance is ongoing work.** The medical advisory board is a real human
  process that costs time, money, and coordination. Threshold maintenance is not a one-time
  task; it is an ongoing clinical operation. Budget for it from day one.
- **False positives erode trust.** An escalation that fires when the user knows the value
  is explained (e.g., an athlete with a physiological resting HR of 38 bpm triggering a
  bradycardia alert) is experienced as noise. Population-adjusted thresholds and user-context
  inputs (e.g., "I am a competitive cyclist with a baseline RHR of 38") are necessary to
  avoid this. The threshold registry must support per-user overrides, requiring MAB approval
  for any user-specific adjustment to a critical threshold.
- **Level 2 UX is stressful.** A full-screen red escalation is designed to be taken
  seriously — but if it fires on a false positive, it is a genuinely alarming experience
  for the user. False positives at Level 2 must be minimized; the registry should be
  conservatively calibrated initially and adjusted based on real-world rates.

### Mitigations

- Start with a conservative threshold set (higher critical thresholds, lower false-positive
  rate) and broaden as the MAB gains confidence in real-world performance.
- Implement a "this value is explained" flow: user can mark a value as "known/managed by
  my clinician" with a note, which suppresses escalation for that specific metric for a
  specified duration (requires re-confirmation after the duration expires).
- Track false-positive escalation rate as a product metric; review with MAB when rate
  exceeds 5% of escalation events.

---

## Open Questions

1. **Emergency services integration.** Should Helix offer a "call 911" button directly
   within Level 2 escalation screens? Technical implementation is trivial; the product,
   liability, and governance implications are not trivial. This requires legal review and
   MAB sign-off before implementation.

2. **Medication context for thresholds.** A potassium of 5.8 mmol/L in a patient on
   spironolactone is clinically different from the same value in a patient on no medications.
   Should Helix's medication data (ADR-004, RxNorm) inform threshold calibration?
   Proposed: yes, in Phase 2 — medication-context-adjusted thresholds require MAB review
   for each medication class and must not create a pathway to medication management advice
   (ADR-010 SaMD boundary).

3. **Escalation from third-party connected devices.** If a smartwatch reports a detected
   AFib episode, should Helix escalate? The smartwatch's detection algorithm is a black box
   and carries its own false-positive rate. Proposed: treat device-reported clinical
   findings (AFib detection, irregular HR, fall detection) as Level 1 screening signals,
   not Level 2 critical values, with appropriate screening-not-diagnosis framing (same
   as Cognitum Seed). Full MDx classification escalation requires lab or clinical
   confirmation.

4. **Clinician notification pathway.** Should Helix have a "notify my doctor" feature for
   red-flag events — sending a structured summary to the user's clinician directly? This
   is technically feasible via FHIR messaging or HealthKit export. It requires HIPAA-grade
   data handling, explicit user consent, and careful regulatory analysis of whether it
   creates a care coordination relationship that changes the product's regulatory status
   (ADR-010). Proposed: deferred to Phase 3 with regulatory counsel.

---

## References

- [A] "Establishing an Evidence Base for Critical Laboratory Value Thresholds," American
  Journal of Clinical Pathology, 2014 (doi:10.1093/ajcp/aqu069):
  https://academic.oup.com/ajcp/article/142/5/617/1760784
- [A] Critical Values in Laboratory Medicine — Acute Care Testing review:
  https://acutecaretesting.org/en/articles/critical-values-in-laboratory-medicine
- [A] Cleveland Clinic Laboratories — Critical and Urgent Values & Results:
  https://clevelandcliniclabs.com/laboratory-resources/policies-procedures/critical-and-urgent-values-results/
- [A] Brown University Health — Critical Values List:
  https://www.brownhealth.org/centers-services/laboratories/health-care-provider-resources/critical-values-list
- [A] "When silence is safer: a review and decision-theoretic framework for LLM abstention
  in healthcare," npj Digital Medicine (2026):
  https://www.nature.com/articles/s41746-026-02882-1
- [A] Visualization of Critical Limits and Critical Values Facilitates Interpretation
  (PMC 11899349): https://pmc.ncbi.nlm.nih.gov/articles/PMC11899349/
- [B] Clinical Decision Support — Agency for Healthcare Research and Quality (AHRQ):
  https://www.ahrq.gov/cpi/about/otherwebsites/clinical-decision-support/index.html
- [B] Clinical Decision Support — Office of the National Coordinator for Health IT (ONC):
  https://www.healthit.gov/topic/safety/clinical-decision-support
- [B] Blood Test Results Explained: Critical Values Guide:
  https://www.kantesti.net/blood-test-results-explained-critical-values/
- [C] ClinDet-Bench — abstention evaluation in clinical LLM decision-making (arXiv 2602.22771):
  https://arxiv.org/pdf/2602.22771
- [C] ADA Standards of Medical Care in Diabetes (glucose alert values, Level 2 hypoglycemia
  definition < 54 mg/dL): https://doi.org/10.2337/dc23-S006

---

*This document provides architectural guidance, not legal, regulatory, or medical advice.
The Escalation Guardian is designed to route users toward clinical care; it does not
provide clinical care, diagnose conditions, or substitute for professional medical judgment.
Engage regulatory counsel and a medical advisory board before deploying escalation features
in any product used by patients. Threshold values cited here are illustrative examples;
all production thresholds must be reviewed and approved by licensed clinicians.*
