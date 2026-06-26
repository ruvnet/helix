# ADR-006: Evidence Tiering & Explicit Abstention Policy

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-005, ADR-007, ADR-008, ADR-009, ADR-010, ADR-016, ADR-018

---

## Context

### The conflation problem in health guidance

Functional-medicine and longevity guidance spans a vast spectrum of evidentiary strength.
At one end: a person's own measured ferritin level, with a lab reference range established
by large population studies. At the other: "biohacker lore" — protocols derived from
n-of-1 experiments shared on podcasts, extrapolated from animal studies, or simply
accreted as community consensus. The danger is not that either end of the spectrum is
necessarily wrong — it is that conflating them, presenting both with equal confidence,
misleads the user about how much weight to place on a recommendation.

Most health AI systems conflate these tiers silently. A general LLM trained on PubMed
abstracts, health forums, functional-medicine blog posts, and clinical guidelines treats
all of it as roughly co-equal training signal. The model can assert "your ferritin should
be above 50 for optimal energy" (a functional-medicine heuristic debated in the
literature) with the same confidence as "your ferritin is 28, below the Quest Diagnostics
reference range of 30–400" (a directly measured fact from the user's own lab). The user
has no way to distinguish these claims.

### Evidence-based medicine's established hierarchy

Evidence-based medicine (EBM) has maintained a formal evidence hierarchy for decades. The
canonical Oxford CEBM hierarchy, and the widely-used GRADE framework, both rank evidence
types in order of reliability [A]:

**Level 1**: Systematic reviews and meta-analyses of randomized controlled trials (RCTs) —
the highest level; aggregated across many independent, controlled studies [A].

**Level 2**: Individual RCTs — controlled comparison with random assignment; considered
strong evidence for causal claims [A].

**Level 2b/3**: Systematic reviews of cohort studies; individual cohort studies —
observational, not controlled; confounding is possible [A].

**Level 4**: Case-control studies, case series — retrospective, lower causal confidence [A].

**Level 5**: Expert opinion, mechanistic reasoning, traditional practice — the lowest level
of formal evidence; may reflect consensus among practitioners but is not empirically
validated [A].

The GRADE framework (BMJ 2004, widely adopted in clinical guideline production) further
distinguishes between high, moderate, low, and very-low certainty evidence, and explicitly
instructs guideline developers to communicate these distinctions to clinicians and patients
so they can calibrate their decision-making accordingly [A].

Helix's four-tier scheme maps onto this EBM framework, adapted for the personal health
intelligence context where the user's own measurements constitute a distinct and primary
tier not present in population-level EBM:

| Helix Tier | Name                       | EBM analogue                           | Weight     |
|------------|----------------------------|----------------------------------------|------------|
| Tier 1     | Your data                  | N/A (individual, not population)       | Primary    |
| Tier 2     | Reference standards        | Level 1–2 population norms/guidelines  | Strong     |
| Tier 3     | Peer-reviewed literature   | Level 2–4 (study design dependent)     | Moderate   |
| Tier 4     | Heuristic / emerging lore  | Level 5 / expert opinion               | Weak       |

### Why abstention is a safety property, not a limitation

A 2026 Nature npj Digital Medicine review, "When silence is safer: a review and
decision-theoretic framework for LLM abstention in healthcare," formalizes two distinct
motivations for LLM abstention [A]:

1. **Uncertainty-driven abstention**: the model should decline to answer when its
   confidence is low or when the available data is insufficient to support a credible claim.

2. **Safety-driven abstention**: the model should decline to provide information that,
   even if technically correct, could cause harm without appropriate clinical context.

The paper establishes that "in healthcare, confidently stated inaccurate medical advice
can cause significant harm, making the ability to abstain especially important" — and
that for high-stakes medical Q&A, an abstention with a clear reason and a next step is
strictly preferable to a confident wrong answer [A].

ClinDet-Bench (arXiv 2602.22771, 2026) specifically evaluates LLMs on "judgment
determinability" in clinical decision-making — the ability to know when a case is
underdetermined and abstain appropriately, rather than forcing a decision. It found that
most LLMs fail this property: they over-answer rather than abstain [B].

The parallel concept in clinical decision support is the "uncertain deferral" — a system
that, rather than producing a recommendation, routes to a human reviewer or explicitly
flags its uncertainty. AHRQ's clinical decision support principles and the ONC's guidance
on Appropriate Use Criteria both recognize deferral as a first-class outcome, not an
error state [B].

### Why Tier 4 content requires explicit labeling (not suppression)

Functional-medicine and longevity optimization literature includes a significant volume
of plausible, practitioner-popular guidance that is not yet validated in RCTs but may be
useful to the user. Completely suppressing this content would make Helix less useful than
reading a functional-medicine book. The correct design is: *label it accurately, display
it with visually distinct lower weight, and never allow it to be dressed up as established
medical fact.*

The failure mode to avoid is "epistemic laundering" — taking a Tier-4 community
heuristic and, by placing it in the mouth of a confident-sounding AI, making it appear
to be Tier-2 clinical guidance.

---

## Decision

### The four evidence tiers, fully defined

**Tier 1 — Your Data**
Direct measurements from the user's own health record, carrying a complete ProvRecord
(ADR-005). This is the highest-confidence tier in Helix: the fact is specific to this
user, at this point in time, from a known source with a known measurement method. Examples:
ferritin 28 ng/mL measured by Quest Diagnostics on 2026-06-10; average deep sleep
42 min/night from Oura Gen3 over the last 30 days; fasting glucose 94 mg/dL from a
continuous glucose monitor calibrated by fingerstick.

Tier-1 claims may only be made when a valid ProvRecord exists and the datum is within its
staleness window (ADR-005). A Tier-1 claim carries inline source attribution.

**Tier 2 — Reference Standards and Clinical Guidelines**
Population reference ranges used to contextualize Tier-1 values; clinical guidelines from
recognized bodies (e.g., AHA, USPSTF, Endocrine Society); diagnostic criteria from
ICD-10/DSM-5 used only for context, not for diagnosis. Examples: the reference range
"ferritin 30–400 ng/mL" as established by Quest Diagnostics' population cohort; the
AHA guideline that LDL-C below 70 mg/dL is recommended for high-risk cardiovascular
patients.

Tier-2 content must cite its source (guideline body, version/year) and the population to
which the reference range applies. This matters because "optimal" ferritin ranges for
a marathon athlete differ from those for a sedentary adult, and Helix should not present
a single-population range as universal.

**Tier 3 — Peer-Reviewed Literature**
Published studies that support a recommendation or association, not yet at the level of
formal guidelines or systematic reviews. These must be cited (author, journal, year, DOI
if available) and the evidence quality noted: RCT, cohort, case-control, mechanistic.
Effect sizes and population characteristics must be surfaced where available: "In a
16-week RCT of 148 sedentary adults (Smith et al., 2023), 400 IU vitamin D
supplementation raised serum 25(OH)D by 12 ng/mL on average."

Tier-3 recommendations always include a caveat about applicability to this specific user's
characteristics.

**Tier 4 — Heuristic / Emerging / Community ("Biohacker Lore")**
Practitioner consensus not yet validated in controlled studies; optimization heuristics
from functional-medicine communities; protocols derived from small case series or
extrapolated from mechanistic/animal research; emerging interventions with biological
plausibility but limited human data. These are always labeled "emerging / low evidence"
in the UI, displayed in a visually distinct lighter style, and accompanied by a disclosure:
"This is based on functional-medicine practice, not established clinical evidence. Discuss
with your doctor before acting on it."

Tier-4 content is never the primary recommendation when a Tier-1 or Tier-2 basis exists
for the same question.

### The abstention policy

The Functional-Medicine Analyst is explicitly trained (via Ruflo SONA/ReasoningBank
patterns, ADR-002) to recognize when the available evidence is insufficient to make a
useful, honest claim — and to say so. Abstention is measured, tracked, and rewarded (see
Metrics). The following conditions trigger abstention:

**Missing data abstention**: A user asks about a metric for which no Tier-1 measurement
exists in their vault. Response: "I don't have a [metric] value on file. Would you like
to add it to your next panel request?" Always paired with an actionable suggestion.

**Stale data abstention**: The most recent measurement for a required metric is beyond
its staleness window (ADR-005). Response: "Your last [metric] is [N] months old — beyond
the window I rely on for current assessment. This is worth retesting. I can help you
draft a retest request." Optionally: "Your value at the time was [value] — here is what
that meant then." (Past-tense framing, not current-state claim.)

**Low confidence abstention**: OCR-derived or device-derived data carries `confidence < 0.50`.
Response: "I have a value for [metric] but it may not be accurate — it was extracted from
[PDF / device signal] and I'm not confident in the reading. Please verify with a direct
test before acting on it."

**Conflicting data abstention**: Two sources provide materially different values for the
same metric in the same time window (e.g., two different lab providers reporting different
ferritin values). Response: "I have conflicting values for [metric] from [source A] and
[source B]. I won't draw conclusions until these are reconciled. Which lab result should I
use?" De-duplication is attempted at ingestion (ADR-004), but the analyst does not silently
resolve clinical conflicts.

**Underdetermined clinical question**: The user asks a question that requires evidence or
data the system does not have, and which cannot be answered reliably from Tier 3 or Tier 4
alone without risking epistemic laundering. Response: "This question is beyond what your
current data allows me to answer well. [Specific reason.] [Suggested next step.]"

### Gap notice design principles

Gap notices — all abstention outputs — follow these design rules:

1. **Never a dead end.** Every gap notice includes at least one actionable next step
   (retest, add a data source, consult a clinician, upload a recent lab).

2. **Reason stated.** The reason for abstention is explicit: missing data, stale data,
   low confidence, or conflicting data. Users should not experience abstention as opaque
   refusal.

3. **Visually distinct.** Gap notices use an amber/caution visual style distinct from
   green (grounded answer), red (red-flag escalation), and blue (general information).

4. **Not apologetic.** "I don't have that yet" is neutral and honest, not "I'm sorry, I
   can't help with that." The framing emphasizes the user's agency to close the gap.

5. **Time-boxed.** Where a retest timeline is clinically appropriate, suggest it: "ferritin
   retesting is typically meaningful at 8–12 weeks after any intervention."

### Tier-4 disclosure boilerplate

All Tier-4 content is accompanied by one of the following disclosures, selected by context:

- Short form (in-line): "(emerging / low evidence — discuss with your doctor)"
- Full form (expandable): "This recommendation is based on functional-medicine practitioner
  consensus and/or preclinical research. It has not been validated in controlled clinical
  trials in populations similar to yours. Treat it as a hypothesis worth exploring with
  your clinician, not a protocol to follow without professional guidance."

The full form is shown on first exposure per user session and on demand; the short form
appears inline for subsequent references.

### How the UI signals evidence tiers

Each piece of guidance in the Helix UI carries a visual evidence tier chip:

| Tier | Chip color / label               | Tooltip on tap                              |
|------|----------------------------------|---------------------------------------------|
| 1    | Teal · "YOUR DATA"              | "Based directly on your [source] measurement" |
| 2    | Blue · "CLINICAL GUIDELINE"     | "From [body name], [year] guideline"          |
| 3    | Purple · "PUBLISHED STUDY"      | "From [author et al., journal, year]"         |
| 4    | Amber · "LOW EVIDENCE"          | "Emerging / community practice — not validated" |

Evidence tier chips appear on: inline recommendations, answer cards, digital twin
region overlays, and the 0–100 score decomposition (ADR-016). The health score never
includes Tier-4 contributions — it is calculated from Tier-1 and Tier-2 inputs only.

### Rewarding abstention: metrics and the Darwin fitness function

Abstention is a first-class success state in Helix. The Darwin Mode fitness function
(ADR-018) includes `abstention_correctness` as a positive metric: the fraction of
"I don't have that" responses that correspond to genuine data gaps in the eval set,
rather than retrieval failures that incorrectly returned no results.

High `abstention_correctness` is rewarded. Low `abstention_correctness` (abstaining when
data exists) is penalized — it indicates retrieval failure, not honest uncertainty. The
two failure modes are tracked separately:

- **True abstention**: data is genuinely absent or stale — correct behavior.
- **False abstention (retrieval failure)**: data exists but was not retrieved — a bug,
  tracked in the DRACO/grounding score and addressed via retrieval improvements.

The model routing policy (ADR-019) also uses abstention quality as a signal: a cheaper
model that abstains correctly on data-gap cases is preferred over a more expensive model
that fills gaps with plausible-sounding hallucinations.

---

## Alternatives Considered

### Alternative A: Single evidence label ("supported" / "not supported") rather than four tiers

A simpler binary label — a claim is either supported by data or not — avoids the
complexity of four tiers. Products like ChatGPT Health use a broadly similar approach:
physician-tuned responses without an explicit tiering system surfaced to the user.

Rejected because: this conflates qualitatively different types of evidence. A Tier-1
ferritin measurement and a Tier-4 optimization heuristic are both "supported" in a binary
system, but they warrant very different levels of confidence. The EBM hierarchy exists
precisely because the medical community determined that binary "evidence / no evidence"
is insufficient for clinical decision-making. Helix serves users who are making real
health decisions; they deserve the same epistemic clarity practitioners expect.

### Alternative B: Suppress Tier-4 content entirely

Only surface guidance with Tier-1, Tier-2, or Tier-3 backing. Suppress anything that
cannot be grounded in peer-reviewed literature or clinical guidelines.

Rejected because: a significant portion of the functional-medicine / longevity optimization
domain that Helix's target users care about operates in Tier-3 and Tier-4 territory.
Suppressing it would reduce utility substantially. The correct solution is not suppression
but labeling — so the user can apply their own epistemic discount, the same way a
knowledgeable practitioner would. Helix trusts the user to make decisions; it equips
them to do so honestly.

### Alternative C: Let the Analyst decide dynamically when to abstain (no explicit policy)

Rather than a rule-based abstention policy, allow the LLM to decide contextually when
to abstain based on its confidence. This is the "chain-of-thought" approach: the model
reasons about its uncertainty and chooses.

Rejected because: ClinDet-Bench (arXiv 2602.22771) found that most LLMs systematically
over-answer rather than abstain in clinical contexts, even when instructed to express
uncertainty. The tendency to produce a plausible response outweighs the tendency to
abstain. A rule-based policy enforced at the retrieval and verification layer (ADR-005,
ADR-008) is more reliable than a LLM self-assessment of its own uncertainty — which is
itself a calibration problem. The policy is the guardrail; LLM judgment is supplementary.

---

## Consequences

### Positive

- **Epistemic honesty.** Users receive responses that are calibrated to the actual
  strength of the evidence. They can distinguish "this is a fact from my lab" from
  "this is a functional-medicine hypothesis." This is the product's honest positioning.
- **Lower liability.** Tier-4 disclosures and the abstention policy reduce the risk of
  a user acting on a recommendation as if it were established medical guidance when it
  is not. Combined with the wellness positioning (ADR-010), this creates a defensible
  clinical-safety posture.
- **Darwin-improvable.** Abstention correctness is a measurable metric that Darwin Mode
  (ADR-018) can optimize. The tiering system creates clear labeled training signal.
- **Clinician credibility.** Users who show the "prep for my appointment" summary to
  their clinician can point to evidence tiers. A clinician seeing "Tier-3 study, 2023
  cohort, n=148" next to a recommendation is more likely to engage constructively than
  if they see a confident unsourced assertion.

### Negative

- **User perception risk.** Some users want confident answers and may find evidence
  tiers or gap notices "wishy-washy." Research on UX of uncertainty communication is
  mixed; some populations respond poorly to expressed uncertainty even when it is honest
  and appropriate. Mitigation: design tier chips as information-dense but visually compact;
  lead with the *recommendation* (what to do), follow with the tier label; the label should
  feel like a trust signal, not a hedge.
- **Tier classification complexity.** Classifying a given piece of guidance into the
  correct tier requires maintaining a knowledge base of clinical guidelines (Tier 2),
  literature citations (Tier 3), and community heuristics (Tier 4). This is ongoing
  work — guidelines are updated, literature accumulates, and the Tier-4/Tier-3 boundary
  shifts as heuristics get validated. A knowledge-management process is required.
- **Tier inflation risk.** Without governance, the Analyst may drift toward presenting
  Tier-3 studies as if they are Tier-2 guidelines (because it sounds more authoritative).
  The Verifier agent (ADR-008) must check tier assignments as part of its verification
  pass, and the Darwin fitness function must penalize tier inflation.

### Mitigations

- Maintain a versioned library of Tier-2 guideline sources (AHA, USPSTF, Endocrine
  Society, etc.) with their publication dates, used by the normalization layer to
  consistently classify Tier-2 content.
- Medical advisory board (§7.3) reviews tier classifications quarterly for core domains
  (cardiometabolic, metabolic, sleep, hormones) and before any new domain is added.
- Darwin Mode eval set (ADR-018) includes tier-classification test cases with known
  correct tier labels, so the fitness function catches tier inflation.

---

## Open Questions

1. **Sub-tiering of Tier 3.** A cohort study with n=10,000 and a mechanistic study with
   n=12 are both "Tier 3" in the current scheme. Should Tier 3 expose study design and
   sample size in the chip tooltip? Proposed: yes — "Published Study (RCT, n=148)" vs.
   "Published Study (cohort, n=32)" gives the user more to work with without adding a
   fifth tier.

2. **Tier-2 source authority.** Different clinical bodies sometimes disagree on reference
   ranges (e.g., the AHA and functional-medicine practitioners use different LDL-C targets).
   How should Helix handle conflicting Tier-2 sources? Proposed: surface the conflict,
   show both, and note the body that produced each.

3. **Tier-4 content gatekeeping.** Who decides what enters the Tier-4 knowledge base?
   Unlimited Tier-4 content from community sources risks making Helix a platform for
   unvalidated health claims. Proposed: Tier-4 content must be reviewed by the medical
   advisory board before inclusion; third-party community contributions are flagged "user-
   contributed" within Tier 4 and carry an additional disclaimer.

4. **Abstention rate target.** What is the acceptable range of abstention rates? If the
   system abstains on 40% of questions, users may lose trust. If it abstains on 5%, it
   may be over-answering. Proposed: pilot with a 10–20% abstention rate target (questions
   where the user has insufficient data to answer well), measured against the eval set.

---

## References

- [A] "When silence is safer: a review and decision-theoretic framework for LLM abstention in healthcare,"
  npj Digital Medicine (2026): https://www.nature.com/articles/s41746-026-02882-1
- [A] "The Levels of Evidence and their role in Evidence-Based Medicine" (PMC 3124652):
  https://pmc.ncbi.nlm.nih.gov/articles/PMC3124652/
- [A] Hierarchy of Evidence — Wikipedia (overview with CEBM and GRADE references):
  https://en.wikipedia.org/wiki/Hierarchy_of_evidence
- [A] Levels of Evidence — EBSCO Research Starters (Health and Medicine):
  https://www.ebsco.com/research-starters/health-and-medicine/hierarchy-evidence
- [B] ClinDet-Bench: Beyond Abstention, Evaluating Judgment Determinability of LLMs in Clinical
  Decision-Making (arXiv 2602.22771): https://arxiv.org/pdf/2602.22771
- [B] "Evaluating Medical LLMs by Levels of Autonomy: A Survey Moving from Benchmarks to
  Applications" (arXiv 2510.17764): https://arxiv.org/pdf/2510.17764
- [B] Advances in Clinical Decision Support Systems — 2023 Literature Review (PMC 12020640):
  https://www.ncbi.nlm.nih.gov/pmc/articles/PMC12020640/
- [B] Appropriate Use Criteria Program — Clinical Decision Support Mechanisms (HHS Guidance Portal):
  https://www.hhs.gov/guidance/document/appropriate-use-criteria-program-clinical-decision-support-mechanisms
- [C] "An Explainable Agentic AI Framework for Uncertainty-Aware and Abstention-Enabled Acute
  Ischemic Stroke Imaging Decisions" (arXiv 2601.01008): https://arxiv.org/pdf/2601.01008

---

*This document provides architectural guidance, not legal, regulatory, or medical advice.
Helix is a decision-support tool, not a diagnostic authority. Evidence tier labels are
informational aids; they do not substitute for clinical judgment.*
