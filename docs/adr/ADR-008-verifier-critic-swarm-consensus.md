# ADR-008: Verifier/Critic Agent & Swarm Consensus for Clinical Outputs

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-002, ADR-005, ADR-006, ADR-007, ADR-009, ADR-010, ADR-018

---

## Context

### A single agent can be confidently wrong

The fundamental limitation of any single-pass LLM inference in a health context is that
confidence and correctness are decorrelated. A model can assert a clinically significant
claim — a trend, a recommendation, a risk association — with high fluency and apparent
certainty while being factually incorrect, insufficiently grounded, or pitched at an
inappropriate evidence tier. The Functional-Medicine Analyst in Helix, however carefully
prompted and constrained by the provenance-required architecture (ADR-005), is still a
single agent. A single agent can fail.

The pattern of using an independent second agent to verify the first is well-established
in both software engineering (code review, pair programming) and in clinical medicine
(second opinion, peer review, pharmacist review of physician prescriptions). Applied to
LLM systems, this pattern is now backed by a growing research literature.

### Multi-agent verification reduces hallucination

MARCH: Multi-Agent Reinforced Self-Check for LLM Hallucination (arXiv 2603.24579, 2026)
demonstrated that a multi-agent reinforcement architecture, where a separate check-agent
reviews synthesizer outputs against source material, reduces hallucination rates
significantly relative to single-agent inference and outperforms self-consistency (same
model, multiple samples) alone [A]. The key finding is that reinforced self-check — where
the critic can modify or suppress the synthesizer's output — provides stronger guarantees
than soft-voting approaches.

VerifiAgent: a Unified Verification Agent in Language Model Reasoning (arXiv 2504.00406,
2025) proposed a two-level verification architecture: meta-verification of solution
completeness and consistency, plus fine-grained tool-based verification of individual
claims. This matches the Helix architecture precisely — the Verifier checks both the
holistic response structure and each individual claim against its ProvRecord [A].

"Mitigating LLM Hallucinations Using a Multi-Agent Framework" (MDPI Information, 2025)
reviewed current multi-agent approaches and found that "having separate agents with
distinct roles — one for generation, one for verification — consistently outperforms
single-agent generation with self-review, because the verifier is not susceptible to
the same biases as the generator" [A].

MASLab: A Unified and Comprehensive Codebase for LLM-based Multi-Agent Systems (arXiv
2505.16988, 2025) provides an open taxonomy of multi-agent verification patterns,
distinguishing: (a) sequential critique (verifier after generator), (b) parallel sampling
with aggregation (multiple generators, voted), and (c) adversarial debate (generator and
verifier argue toward consensus). The Helix architecture uses primarily (a) with elements
of (c) for the highest-stakes clinical outputs [B].

### The "different model family" principle

The weakest form of verification uses the same model to verify its own output. Research
consistently shows this fails to catch the systematic errors of that model family —
the verifier reproduces the same blind spots as the generator. ADR-018 (Darwin Mode)
explicitly specifies that "the verifier and judge are drawn from different model families
than the synthesizer." This principle is grounded in adversarial ML research: a verifier
trained on similar data distribution as the generator is vulnerable to the same
distributional artifacts.

"When Agents Disagree: The Selection Bottleneck in Multi-Agent LLM Pipelines" (arXiv
2603.20324, 2026) analyzed disagreement patterns in multi-agent LLM systems and found
that agents from the same model family tend to agree on the same errors, while agents
from different model families disagree on each other's systematic failure modes, making
cross-family verification more effective at catching a broader class of errors [B].

Self-consistency hallucination detection research (EmergentMind, aggregating multiple
studies) found that purely self-consistency-based approaches plateau at AUROC ~0.74–0.76
on hallucination detection. Integrating cross-model probing — having a second, distinct
model family check the first — consistently improved detection performance beyond this
ceiling [B]. In health applications, the ceiling is unacceptable; cross-family verification
is the Helix standard.

### Ruflo swarm consensus as the coordination layer

Ruflo's built-in swarm coordination (ADR-002) provides the multi-agent orchestration
infrastructure: the Analyst and Verifier run as distinct agents in a Ruflo swarm, with
the Verifier operating under explicit role separation (no shared state with the Analyst
during the verification pass). Ruflo's consensus mechanism (Raft for deterministic outputs,
PBFT-style for high-stakes clinical verdicts) arbitrates between agent outputs and gates
the final response.

---

## Decision

### The Verifier/Critic agent: role and scope

The Verifier/Critic is an independent Ruflo agent that runs *after* the Functional-Medicine
Analyst produces a draft response, *before* that draft is surfaced to the user. The
Verifier is a distinct agent instance, using a different model from a different model
family than the Analyst (see Model Selection below), and operates without access to the
Analyst's reasoning trace — only the Analyst's output draft, the original query, the
numeric facts payload (ADR-007), and the retrieved ProvRecords (ADR-005).

The Verifier performs three passes:

**Pass 1 — Claim enumeration and attribution check.**
Decompose the Analyst draft into atomic claims (matching the claim extraction step in
ADR-005, step 5). For each claim:
- Is it attributed to a ProvRecord? If not: mark as ungrounded.
- Is the cited ProvRecord present in the retrieved set? If not: mark as phantom citation.
- Is the ProvRecord within its staleness window? If not: mark as stale-data claim.
- Does the claim accurately represent the ProvRecord value? Check numeric values
  against the numeric facts payload (ADR-007) — if a number in the Analyst draft
  differs from the payload, mark as numeric discrepancy.

**Pass 2 — Evidence tier verification.**
For each claim, verify that the evidence tier label (ADR-006) is appropriate:
- Tier-1 claims must be backed by a ProvRecord from the user's own data.
- Tier-2 claims must cite a recognized clinical guideline or reference standard.
- Tier-3 claims must cite a peer-reviewed publication.
- Tier-4 claims must carry the required disclosure language.
- Any claim that is labeled Tier 1 but backed only by parametric LLM knowledge
  (no ProvRecord): down-grade to Tier 3 at best, or mark as ungrounded.

**Pass 3 — Clinical appropriateness check.**
- Does the response contain any content that could constitute a diagnosis or treatment
  recommendation beyond the wellness/decision-support scope (ADR-010)? Flag for
  suppression or reformulation.
- Does the response trigger any red-flag threshold (ADR-009)? If yes: flag immediately
  for escalation; optimization content must be suppressed.
- Is the response internally consistent? (e.g., does it assert a "declining trend" while
  citing a value that increased month-over-month?) Inconsistencies are flagged.
- Is the confidence level of the response calibrated to the evidence strength? (e.g., is
  a Tier-4 recommendation expressed with the same certainty as a Tier-1 finding?) If not:
  flag for reformulation.

### Verifier output: verdicts and actions

The Verifier produces a structured verdict for each claim in the draft:

```
ClaimVerdict {
  claim_text:   String
  verdict:      Verdict         // Verified | Ungrounded | PhantomCitation |
                                 // StaleData | NumericDiscrepancy | TierMismatch |
                                 // ClinicalOverreach | RedFlagTrigger
  action:       Action          // Pass | Drop | Downgrade | Reformulate | Escalate
  reason:       String          // human-readable explanation (for audit log)
  replacement:  Option<String>  // suggested replacement text if Reformulate
}
```

The Verifier's verdict set is passed to Ruflo's consensus layer. The consensus outcome
determines the final response:

- **All claims Verified**: response passes with a verification badge.
- **One or more claims Dropped**: they are removed from the response; gap notices are
  inserted in their place.
- **One or more claims Reformulated**: the replacement text is substituted.
- **RedFlagTrigger**: the Escalation Guardian (ADR-009) takes over. The Analyst's
  optimization content is suppressed entirely.
- **ClinicalOverreach**: the specific claim is suppressed or reframed as a suggestion
  to discuss with a clinician.

The user sees only the post-verification response. The Verifier's verdict set, the Analyst
draft, and the diff between them are written to Ruflo's HIPAA-mode audit log (ADR-002)
for full traceability.

### Ruflo swarm consensus: when it activates

Swarm consensus (using Ruflo's built-in Raft/PBFT mechanisms) is invoked for a subset
of outputs that meet the "clinically meaningful" threshold. Not every response requires
full multi-agent consensus; that would impose unacceptable latency. The threshold is:

**Consensus required (full Verifier + consensus gate):**
- Any response that asserts an out-of-range biomarker value.
- Any response that makes a trend claim about a metric tracked in the user's standing
  health model (cardiometabolic, sleep, hormones, metabolic).
- Any recommendation that includes a specific intervention (supplement dose, lifestyle
  change, retest timeline).
- Any response that explicitly or implicitly references a health risk.
- Any response generated while the Escalation Guardian has an active watch on a metric.

**Lightweight verification only (Verifier single-pass, no full consensus):**
- General informational responses (e.g., "what is HRV?").
- Navigation responses (e.g., "here is your sleep data for last month").
- Administrative responses (e.g., "I've updated your goal for ferritin").

**No verification required:**
- Responses that contain no factual claims (e.g., "sure, let me pull that up").

The "clinically meaningful" classification is made by a fast routing agent at the front
of the pipeline, using a lightweight classifier trained on the Ruflo eval set.

### Model selection: verifier from a different model family

The Analyst and the Verifier must use models from different families. In practice:

| Role        | Example model tier (ADR-019)           | Constraint                           |
|-------------|----------------------------------------|--------------------------------------|
| Analyst     | Tier 2/3: on-device model or cloud     | The primary reasoning model          |
| Verifier    | Different model family from Analyst    | Never the same model architecture    |

If the Analyst uses a model from provider family A (e.g., a Claude-class model), the
Verifier uses a model from provider family B (e.g., an open-weights model) or vice
versa. Darwin Mode (ADR-018) maintains this cross-family constraint as a non-negotiable
invariant in its mutation space — evolved configurations that violate same-family
verification are rejected.

For on-device inference (ADR-013), where the model selection is constrained by device
capability, the Verifier may use a smaller model from a different architecture. Even a
smaller cross-family model provides meaningful independent verification — it catches
systematic errors specific to the Analyst model family while adding minimal latency.

The model routing policy (ADR-019) is aware of the cross-family requirement and routes
the Analyst and Verifier tasks accordingly.

### Latency and cost management

Full verification adds latency. Target end-to-end latencies including verification:

| Response class           | Target total latency | Approach                               |
|--------------------------|----------------------|----------------------------------------|
| Standard analytical Q&A  | < 4 seconds          | Analyst + Verifier in overlapping pipeline |
| Clinically-flagged output | < 6 seconds          | Full consensus gate; acceptable for safety |
| Simple navigation / info | < 1.5 seconds        | No Verifier pass required              |

To achieve these targets:

- **Parallel claim extraction.** Claim enumeration (ADR-005 step 5) and the Verifier's
  Pass 1 preparation begin while the Analyst is still generating the second half of the
  draft (streaming generation).
- **Cached ProvRecord lookups.** The Verifier reuses the same retrieved ProvRecords as
  the Analyst (already in working memory); no second retrieval pass is needed.
- **Incremental verdict.** The Verifier produces verdicts per claim as each is extracted,
  rather than waiting for the full draft. Claims verified early can begin UI rendering
  while the tail of the draft is still being verified.
- **Lightweight Pass 3.** The clinical appropriateness check uses a fast rule-based
  classifier (Escalation Guardian pattern matching, ADR-009) for the first screen, with
  LLM-level judgment only for flagged cases.

Cost: the Verifier model (cross-family, smaller tier acceptable for most verification
tasks) is routed to a lower-cost model by ADR-019's cost-aware router. Verification adds
approximately 30–60% to raw inference cost for clinically-flagged responses. This is
acceptable given the safety value; the Darwin Mode fitness function (ADR-018) optimizes
the cost/faithfulness tradeoff over time.

### The Verifier in Darwin Mode evolution (ADR-018)

Darwin Mode (ADR-018) uses a "synthesizer + verifier + judge from different model families"
architecture for evaluating whether an evolved configuration improves the DRACO fitness
score. The Helix Verifier agent's verdict data is a training signal: configurations that
reduce the rate of Verifier-flagged claims in the eval set are kept; configurations that
increase it are discarded. Over time, this creates evolutionary pressure toward a
synthesizer that generates fewer claims requiring Verifier intervention — not by disabling
verification but by improving the Analyst.

The judge (ADR-018's eval layer) is a third, distinct model family — separate from both
the Analyst and the Verifier — providing the most independent possible assessment of
response quality.

### Audit trail

Every Verifier pass is logged in Ruflo's HIPAA-mode audit trail (ADR-002):
- Analyst draft (redacted of PII for the audit log; full version retained in encrypted
  local audit store on device, ADR-001).
- Verifier verdict set.
- Consensus outcome.
- Diff: which claims were dropped, reformulated, or escalated.
- Model IDs for both Analyst and Verifier.
- Total latency for the verification pass.

This audit trail supports the "prep for my appointment" feature: the user can show a
clinician not just the final response but the evidence trail that produced it.

---

## Alternatives Considered

### Alternative A: Single-agent self-review ("reflect and verify" prompt pattern)

Ask the Analyst to review its own output in a second pass: "Given the data you retrieved,
verify each claim in your response." This is common in chain-of-thought and reflection
prompting literature.

Rejected because: self-review by the same model is systematically weak. A model that
generated a hallucinated claim with high confidence is unlikely to flag that same claim
during self-review — the same weights that produced the hallucination process the
review. ClinDet-Bench (arXiv 2602.22771) found that self-review improved factual
accuracy only marginally and failed to catch systematic errors specific to the model
family. MARCH (arXiv 2603.24579) found that reinforced self-check outperformed self-review
by a substantial margin, and that cross-model verification outperformed same-model
self-check. Helix's health domain demands the stronger approach [A].

### Alternative B: Human clinical review for all outputs (clinical editor in the loop)

Every response reviewed by a licensed clinician before reaching the user. Some
clinical AI platforms use this for high-stakes outputs.

Rejected because: it is incompatible with the product's real-time, conversational,
always-available design. Human clinical review adds minutes to hours of latency and
is not scalable to millions of conversational responses. The correct role for clinical
human judgment in Helix is in governance (curating red-flag thresholds, reviewing
the evidence tier library, approving Darwin Mode evolution changes — ADR-009 §7.3),
not in-line response review. Red-flag escalation (ADR-009) routes the user to a
clinician when the stakes genuinely demand it.

### Alternative C: Ensemble voting (multiple Analyst instances, majority vote)

Generate 3–5 Analyst responses in parallel, use majority voting to select the response
that appears most often. Similar to RAGAS's self-consistency-based faithfulness approach.

Rejected because: majority voting does not guarantee grounding — it selects the *most
common* response, which may be the most plausible hallucination if all instances share
the same parametric knowledge bias. Self-consistency research found an AUROC ceiling of
~0.74–0.76 for this approach [B]. More critically: in health data, the right answer is
not necessarily the most common answer — it is the answer grounded in *this user's*
specific data. A claim-level provenance check (the Verifier's approach) is stronger than
majority voting over unanchored prose. Ensemble voting also multiplies compute cost by
3–5x without the targeted precision of a single cross-family verifier.

---

## Consequences

### Positive

- **Catches fabrication before the user sees it.** The primary safety value: hallucinated
  claims, phantom citations, and numeric discrepancies are intercepted and removed before
  reaching the user. This is the most direct implementation of the "architecturally
  anti-hallucination" product promise.
- **Audit trail for clinical accountability.** Every response has a verifiable chain of
  custody: what the Analyst proposed, what the Verifier accepted, dropped, or reformulated,
  and what the user saw. This is the evidentiary foundation for the product's credibility
  with clinicians.
- **Independent of Analyst model quality.** As the Analyst model is upgraded or replaced
  (ADR-019 routing), the Verifier provides a consistent quality floor. New model
  configurations must pass the Verifier before Darwin keeps them.
- **Darwin feedback loop.** Verifier verdicts are training signal. The product improves
  over time toward a synthesizer that generates fewer flaggable claims.

### Negative

- **Latency.** The Verifier adds 1–3 seconds to response time for clinically-flagged
  outputs. This is acceptable given the safety rationale but must be communicated to
  users (e.g., a "verifying..." UI state) so they don't interpret the pause as an error.
- **Verifier calibration.** A poorly calibrated Verifier that flags too aggressively
  degrades the experience (many helpful claims dropped). One that flags too leniently
  misses the errors it exists to catch. The Verifier's calibration is tracked in the
  Darwin eval set and tuned accordingly.
- **Cross-family model dependency.** Requiring a second, cross-family model increases
  infrastructure complexity and cost. On-device paths (ADR-013) must provision a suitable
  cross-family Verifier model; this has download and storage implications.
- **Audit log size.** Storing full Analyst drafts and Verifier verdict sets for every
  response adds significant local storage. Mitigation: HIPAA audit logs are retained for
  90 days on-device, then compressed and optionally migrated to user-controlled encrypted
  backup (ADR-001).

### Mitigations

- UI design: show a brief "cross-checking sources..." indicator during the Verifier pass.
  Transparent about the architecture; builds trust.
- Verifier calibration: maintain a labeled eval set of (Analyst draft, correct verdict)
  pairs. Darwin Mode tracks Verifier F1 on this set and flags degradation.
- Storage: audit logs are JSON-line format, compressed at rest; median response log is
  ~2–4 KB; 90-day retention at 10 responses/day = ~4 MB, well within mobile storage bounds.

---

## Open Questions

1. **Verifier model selection automation.** How should the cross-family constraint be
   enforced operationally as new models become available? Proposed: maintain an explicit
   registry of model families with a "Analyst-compatible" vs. "Verifier-only" flag; Darwin
   Mode (ADR-018) uses this registry to construct valid configurations.

2. **Verifier for Tier-4 content.** Tier-4 claims by definition lack strong grounding.
   Should the Verifier treat "this is an unvalidated heuristic, labeled as such" as
   verified or unverified? Proposed: Tier-4 claims are Verified if and only if the
   disclosure language is present and the claim is not labeled at a higher tier than 4.
   The Verifier's job is to ensure tier labeling is accurate, not to suppress Tier-4.

3. **Partial verification for long responses.** A detailed "standing health model" summary
   may contain 20–30 claims. Full claim-level verification for every claim may be slow.
   Proposed: apply full verification to the top N claims by clinical significance (ranked
   by a priority score based on evidence tier and metric importance), and lightweight
   rule-based checks for the remainder.

4. **Verifier disagreement logging.** When the Verifier drops or reformulates a claim,
   should the user be told? Proposed: no — surfacing every verification event would be
   noisy and may undermine confidence. Instead, verification events are in the audit log
   and available via "view source." The user sees the final, verified response. The audit
   trail is available on request.

---

## References

- [A] MARCH: Multi-Agent Reinforced Self-Check for LLM Hallucination (arXiv 2603.24579):
  https://arxiv.org/pdf/2603.24579
- [A] VerifiAgent: a Unified Verification Agent in Language Model Reasoning (arXiv 2504.00406):
  https://arxiv.org/pdf/2504.00406
- [A] "Mitigating LLM Hallucinations Using a Multi-Agent Framework," MDPI Information 2025:
  https://www.mdpi.com/2078-2489/16/7/517
- [B] "When Agents Disagree: The Selection Bottleneck in Multi-Agent LLM Pipelines"
  (arXiv 2603.20324): https://arxiv.org/pdf/2603.20324
- [B] MASLab: A Unified and Comprehensive Codebase for LLM-based Multi-Agent Systems
  (arXiv 2505.16988): https://arxiv.org/pdf/2505.16988
- [B] Self-Consistency Hallucination Detection (EmergentMind topic survey):
  https://www.emergentmind.com/topics/self-consistency-based-hallucination-detection
- [B] "LLM-based Agents Suffer from Hallucinations: A Survey of Taxonomy, Methods, and
  Directions" (arXiv 2509.18970): https://arxiv.org/html/2509.18970v1
- [C] "Attention Knows Whom to Trust: Attention-based Trust Management for LLM Multi-Agent
  Systems" (arXiv 2506.02546): https://arxiv.org/pdf/2506.02546
- [C] ClinDet-Bench: Beyond Abstention (context for self-review failures in clinical LLM):
  https://arxiv.org/pdf/2602.22771

---

*This document provides architectural guidance, not legal, regulatory, or medical advice.
The Verifier/Critic agent augments the safety of the system but cannot guarantee the
correctness of every response. Helix is a decision-support tool, not a diagnostic
authority. Engage clinical governance before building diagnostic or treatment-recommending
features.*
