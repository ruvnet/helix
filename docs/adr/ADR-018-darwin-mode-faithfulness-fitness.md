# ADR-018: Darwin Mode Self-Optimization with Faithfulness as the Fitness Function

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-005, ADR-006, ADR-008, ADR-013, ADR-017, ADR-019

---

## Context

### The product improvement problem

A health intelligence product has two improvement trajectories: (1) retrain or swap the underlying
model, and (2) improve the harness configuration that the model operates within. Model retraining
or swapping is expensive, slow, requires clinical validation of the new model's behavior, and
introduces systemic change risk. Harness configuration changes — retrieval parameters, routing
tier thresholds, prompt scaffolds, verifier gate thresholds, memory decay rates — are smaller,
more isolated, more reversible, and more directly targeted at the specific behaviors this product
cares about.

The question is whether harness configuration can be improved *automatically* — without requiring
a human to enumerate and test every change — and if so, what objective should guide the
improvement.

### Why evolutionary self-optimization is the right mechanism

Manual grid search over harness configuration is O(n^k) in the parameter space: if there are k
tunable surfaces each with n candidate values, the search space grows exponentially. Darwin Mode
(the `@metaharness/darwin` package, `npx metaharness-darwin evolve`) solves this by applying
genetic algorithm + simulated annealing to intelligently sample the space, running each candidate
against a held-out eval set in a sandbox, and keeping only changes that measurably improve the
target score. **[A]** — grounded in `harnesses/timesfm-harness/.claude/skills/evolve/SKILL.md`,
`docs/adr/ADR-266-metaharness-darwin-integration.md`, and the `@metaharness/darwin` package
documentation cited therein.

In the ruvector benchmark context (ADR-266), Darwin Mode evolves over 32 mutation surfaces
(HNSW M, efConstruction, RaBitQ bits, Matryoshka dimensions, etc.) against a 4-component scoring
function. In Helix's health context, the mutation surfaces and scoring function are different —
but the mechanism is identical.

The timesfm-harness live example demonstrates the safety properties that make this acceptable in a
production context: the deterministic mutator is air-gapped (no network, no API key), every mutation
passes a `validateGeneratedCode` gate that rejects any change adding new imports, network access,
filesystem access, shell access, env access, or dependencies, and the mutation runs in a sandbox
before any result is archived. Nothing is promoted without measured improvement against a test. **[A]**

### Why faithfulness — not engagement — is the right fitness function

Most AI product optimization implicitly or explicitly optimizes for engagement: response length,
user interaction rate, satisfaction ratings. In a consumer health context, engagement optimization
is actively dangerous:

- A model that hallucinates confident-sounding health advice will often score higher on user
  satisfaction in the short term than one that says "I don't have data on that." People prefer
  confident answers, even wrong ones.
- Engagement metrics reward specificity regardless of whether the specificity is grounded.
  "Your ferritin is likely low based on your symptoms" is more engaging than "I don't have a
  recent ferritin value." Only the second is honest.
- A fitness function that maximizes engagement will evolve the harness toward telling users what
  they want to hear. In a domain where false reassurance can delay diagnosis or encourage
  dangerous self-treatment, this is a patient safety issue.

The alternative is a fitness function that measures **faithfulness** — how accurately the system's
outputs can be traced back to the user's own data, with correct provenance, accurate evidence
tiering, and appropriate abstention when data is absent.

Evidence grade for the engagement-vs-faithfulness tradeoff in health AI: **[A]** — extensively
documented in the clinical NLP and medical AI literature. The 2022–2024 evaluation of large
language models in clinical settings (multiple papers, NEJM AI, JAMA Network Open) consistently
found that general-purpose models optimized for human preference ratings produced confidently wrong
medical claims at significantly higher rates than abstain-heavy systems. **[B]** — specific papers
not cited here; claim is well-supported in the research literature but not verified against a
specific citation within the ruvector repo.

### DRACO-style fitness — the five components

The DRACO fitness framework as used in this project (project-specific term; not a published
external standard) is a composite quality score built from five components. In the ruvector/Helix
context, DRACO originates from the Tiny Dancer training pipeline (ADR-252), where a DRACO matrix
encodes rows of `{embedding, per_model_quality_scores}` used to train the FastGRNN router. The
"style" of multi-dimensional quality scoring in DRACO maps directly onto the faithfulness fitness
function Helix needs. **[A]** — grounded in `docs/adr/ADR-252-fastgrnn-training-pipeline.md`.

For Helix's Darwin Mode, the DRACO-style fitness score has five components:

| Component | What it measures | Weight (provisional) |
|---|---|---|
| **Grounding** | Fraction of factual claims that resolve to a stored datum with provenance | 0.35 |
| **Coverage** | Fraction of relevant available data points surfaced in the response | 0.20 |
| **Balance** | Calibration: are confidence levels correct relative to data quality? | 0.15 |
| **Cleanliness** | Absence of hedging-as-fabrication (speculative claims dressed up as cautious claims) | 0.15 |
| **Faithfulness** | Agreement between the response and what the verifier agent independently re-derives from the source data | 0.15 |

The composite: `score = 0.35*grounding + 0.20*coverage + 0.15*balance + 0.15*cleanliness + 0.15*faithfulness`

Note that "faithfulness" in the component list is the narrow verifier-agreement measure, while the
overall fitness function is itself the faithfulness objective. Grounding is the highest-weighted
component because un-sourced claims are the most dangerous failure mode; coverage ensures the
system is not evading the grounding requirement by simply saying less; balance and cleanliness
catch subtler failure modes (over-confidence, hedge-wrapped hallucination).

### Fusion: verifier and judge from different model families

Darwin Mode evaluates each candidate harness configuration by running it against the eval set and
scoring the outputs. The scoring has two stages:

1. **The verifier agent** independently re-derives each claim from the source data in RuVector.
   This is the Verifier/Critic agent from ADR-008, running on the eval set responses.
2. **The judge** assigns the DRACO component scores for balance, cleanliness, and faithfulness
   based on the verifier's output and the eval set ground truth.

The architecturally critical requirement: **the verifier and judge must be drawn from a different
model family than the synthesizer** (the fm-analyst agent that produced the response). This is
the fusion principle. If the verifier is from the same model family as the synthesizer, it will
share the same systematic biases — it will fail to catch exactly the classes of error that the
synthesizer is prone to making. A GPT-family synthesizer verified by a GPT-family verifier is not
independent; a GPT-family synthesizer verified by a Claude-family verifier (or vice versa) is.

Evidence grade: **[B]** — well-supported in the AI evaluation literature (the "LLM-as-judge"
problem) and consistent with standard practices in NLP evaluation (multiple annotator families).
Not independently verified in a Helix-specific context.

In practice for Helix: the fm-analyst (synthesizer) runs on one model tier; the verifier and DRACO
judge run on a different provider or model family. The specific assignment is determined at harness
configuration time and encoded in the agent roster.

### The eval set — the hardest asset

Darwin Mode cannot safely evolve if there is no held-out eval set to evolve against. An eval set
for Helix requires:

1. **Representative health queries.** Questions that span the kinds of reasoning Helix is asked to
   do: biomarker trend interpretation, medication interaction queries, sleep data correlation, lab
   panel review, symptom-to-data mapping.
2. **Ground-truth responses.** For each query, a curated "correct" response that demonstrates
   proper grounding, correct evidence tiering, appropriate abstention where data is absent, and
   correct escalation where a red-flag value is present.
3. **Source data fixtures.** Each query must be paired with a fixed synthetic health data
   fixture so the eval is deterministic: the same fixture + query must always produce the same
   ground truth.
4. **Clinical governance.** Ground-truth responses must be reviewed by the medical advisory board.
   An eval set curated without clinical review is worse than no eval set — it will evolve the
   harness toward unreviewed heuristics.
5. **Ongoing maintenance.** As medical knowledge evolves, as reference ranges are updated, and as
   the product adds new data types, the eval set must be updated. A stale eval set will evolve
   the harness against outdated ground truth.

Building this eval set well takes time and clinical-governance resources. It is **the gating
dependency for Darwin Mode**. The Helix roadmap (§8) explicitly defers Darwin Mode to Phase 5
("turn on Darwin Mode once a curated health-eval set exists"). This decision supports that deferral.

Evidence grade for the eval-set dependency: **[A]** — grounded in the Helix product spec §8 and in
the standard practice of machine learning evaluation where a contaminated or absent eval set
renders optimization meaningless or actively harmful (the Goodhart's Law problem: when a measure
becomes a target, it ceases to be a good measure).

### What Darwin Mode evolves — the mutation surfaces for Helix

Darwin Mode mutates one surface per generation. The 7 mutation surfaces for the Helix harness,
analogous to the 7 surface files in the timesfm-harness evolve skill, are:

| Surface | What it controls | Example mutation |
|---|---|---|
| `retrieval-policy` | HNSW efSearch, similarity threshold, GraphRAG hop depth | Increase efSearch from 100 to 150 |
| `routing-tiers` | Model tier thresholds for the cost-aware router (ADR-019) | Lower the haiku/sonnet boundary |
| `prompt-scaffolds` | System message templates for the fm-analyst | Add structured output format hint |
| `verifier-thresholds` | Confidence thresholds at which verifier downgrades a claim | Tighten grounding threshold from 0.80 to 0.85 |
| `abstention-policy` | When to abstain vs. answer with low confidence | Increase staleness cutoff from 12mo to 9mo |
| `memory-decay-rate` | The emergent-time decay coefficient for aging signals | Reduce half-life for wearable HRV from 60d to 45d |
| `evidence-tier-weights` | How evidence tiers modulate recommendation confidence | Reduce weight of Tier-4 heuristic recommendations |

Each mutation is a single targeted change. Only the mutated surface is changed; all other surfaces
are held constant. The candidate harness runs against the health-eval set in a sandbox, and the
DRACO score is computed. If the score improves above a statistical significance threshold (to guard
against noise), the mutation is archived as a successful descendant and becomes the new baseline
for the next generation.

---

## Decision

Enable Darwin Mode for Helix as a harness-level evolution capability, subject to the following
invariants. The capability is **gated until the curated health-eval set exists** and is **not
active in Phase 0–4 of the roadmap**.

### The Darwin loop mechanics

```bash
# Full evolution run (requires health-eval set)
npm run evolve \
  --sandbox real \
  --generations 10 \
  --children 4 \
  --fitness draco-health \
  --eval-set ./evals/health-eval-v1.json

# Dry run (mock substrate, no eval execution, for CI smoke testing)
npm run evolve:dry
```

Or via the CLI skill:

```bash
npx metaharness-darwin evolve . \
  --sandbox real \
  --generations 10 \
  --children 4 \
  --fitness-config ./config/draco-health.json
```

One generation of the Darwin loop:

1. **Mutate.** The deterministic mutator selects one surface file and applies one targeted change
   (parameter nudge, threshold adjustment, prompt addition/removal). The `validateGeneratedCode`
   gate rejects any change that adds imports, network access, filesystem access, shell access, env
   access, or new dependencies.
2. **Sandbox.** The candidate harness (unchanged kernel + mutated surface) runs against the
   health-eval set fixtures. The fm-analyst processes each query against the paired synthetic
   health data fixture. Responses are collected.
3. **Score.** The verifier agent (different model family from fm-analyst) re-derives each claim
   from the fixture. The DRACO judge assigns component scores. The composite DRACO score is
   computed.
4. **Select.** If the composite score improves by more than a threshold (default: 0.02 standard
   deviations above the current baseline, to guard against noise), the mutation is archived. If
   not, it is discarded. The elite mutations from the generation become the baseline for the next.
5. **Checkpoint.** All generation results (baseline score, each child's score and mutation,
   pass/fail determination) are written to `docs/darwin/evolution-runs/YYYY-MM-DD-run-N.json`.
   Checkpoints are committed to the repo as the audit record of what evolved.

### Safe by default

- **Deterministic mutator.** No LLM is invoked to generate mutations. The mutator applies
  pre-defined parameterized changes to pre-defined surfaces. Air-gapped: no network, no API key
  required.
- **`validateGeneratedCode` gate.** Every mutation output is checked for new imports, network
  calls, filesystem access, shell invocations, and new dependencies before it is allowed to run.
- **Sandbox.** Each candidate runs in isolation; nothing is promoted to the baseline without passing
  the eval.
- **Statistical gate.** Score improvement must exceed the noise threshold; random-walk improvement
  is rejected.
- **Archive-only, no auto-promote.** The evolution run produces an archive of successful
  descendants. Promotion to the main harness requires human review of the archive and an explicit
  merge decision. Darwin Mode does not automatically modify the production harness.
- **No model change.** The underlying model (Claude, on-device ruvLLM, etc.) is frozen. Darwin
  Mode evolves harness configuration, never model weights.
- **Opt-out.** `npm run evolve -- --no-darwin` or the `HELIX_NO_DARWIN=1` environment variable
  disables evolution entirely and falls back to the static configuration. Opt-out is recorded in
  the audit log.

### Gating condition

Darwin Mode is not activated until ALL of the following are true:

1. The curated health-eval set exists at `evals/health-eval-v1.json` with at least 50 curated
   (query, fixture, ground-truth-response) triples across all major query types.
2. The eval set has been reviewed and approved by the Helix medical advisory board.
3. The DRACO fitness function has been calibrated against human evaluation: DRACO component
   weights have been validated by comparing DRACO scores against clinical expert rankings on a
   calibration set of at least 20 (query, response) pairs.
4. The Phase 1 Functional-Medicine Analyst + Verifier pipeline is shipped and validated (grounding
   rate at baseline before evolution).
5. A manual review of at least one full evolution run archive has been completed and approved.

Until all five conditions are met, `npm run evolve` exits with a `HELIX_DARWIN_NOT_READY` error
and a message explaining which gating conditions remain open.

---

## Alternatives Considered

### Alternative 1: Optimize for user satisfaction ratings

Use thumb-up/thumb-down feedback and session continuation as the fitness signal, evolving the
harness toward configurations that produce responses users prefer.

**Why rejected.** This is the engagement trap. Health users prefer confident answers. An optimizer
that learns from user satisfaction will produce more confident answers regardless of grounding,
which is precisely the hallucination vector this product is designed to prevent. User satisfaction
is a lagging indicator of trust, and trust in a health product can be built on a foundation of
false confidence. Evidence tiering and abstention (ADR-006) will always score lower on immediate
user satisfaction than a confident guess; this is a feature, not a bug.

### Alternative 2: Static configuration — no evolution

Fix the harness configuration at Phase 0 and improve it manually via code review and human-in-the-
loop testing. Darwin Mode is not used at all.

**Why rejected.** Static configuration is appropriate for early phases (0–4) when the eval set
does not yet exist. But as the product accumulates real usage and the eval set is built, manual
configuration of 7 inter-related surfaces becomes a coordination problem: a change to retrieval
thresholds may interact with verifier thresholds in non-obvious ways, and manual search will
not efficiently find the jointly optimal configuration. Darwin Mode's value compounds with
configuration complexity.

### Alternative 3: Use online learning — update the harness from live user sessions

Continuously update harness configuration based on the outcomes of real user interactions,
using the live product as the eval environment.

**Why rejected.** Online learning in a health context means the product evolves on patient data
and real health queries. A mutation that degrades grounding on a real query could give a real
user a false answer about their health. The sandbox + held-out eval approach is the safe-by-design
alternative: mutations are tested on synthetic fixtures before any deployment. Live usage data can
*inform* the eval set (by identifying gap query types) but cannot serve as the evolution environment.

### Alternative 4: Optimize for BLEU/ROUGE score against reference responses

Use automated text-similarity metrics against the ground-truth responses in the eval set.

**Why rejected.** Text-similarity metrics do not measure grounding. A response that paraphrases
the ground truth without citing the user's data may score well on BLEU/ROUGE while failing on
grounding entirely. DRACO-style fitness is semantically meaningful — it measures whether claims
are traceable to source — whereas text-similarity is a syntactic proxy. DRACO is harder to
implement (requires the verifier pipeline to be running during eval) but measures the thing that
actually matters for a health product.

---

## Consequences

### Positive

- The harness continuously improves toward the property this domain requires most (faithfulness),
  without retraining a model or shipping risky updates to users.
- Improvement is provable: every evolution run produces a checkpoint archive with before/after
  DRACO scores, a complete audit trail of what changed and why it was kept.
- The fitness function is domain-appropriate: grounding is the highest-weighted component, which
  means the optimizer will always prioritize reducing un-sourced claims over other improvements.
- Safe by default: air-gapped mutator, `validateGeneratedCode` gate, sandbox-only execution,
  no auto-promotion.
- The fusion principle (different model families for synthesizer vs. verifier/judge) ensures the
  evaluation is genuinely independent.

### Negative

- Darwin Mode is not available until Phase 5 of the roadmap (after the health-eval set exists).
  This deferral is a consequence of the eval set gating condition, not a limitation of the mechanism.
- Building and maintaining the health-eval set is significant ongoing work (clinical governance
  review, fixture generation, ground-truth curation). This is the hardest asset to build well.
- The DRACO fitness function requires the verifier pipeline to be running during every evolution
  run, which adds time and cost to each generation. An evolution run of 10 generations x 4 children
  = 40 verifier evaluations of the full eval set. With a 50-triple eval set, that is 2,000 verifier
  calls per run.
- Statistical gating means small real improvements may be missed (false negatives). A mutation
  that genuinely improves grounding by 0.8% will not survive the noise threshold. This is
  intentional: we prefer stability to marginal gains.

### Mitigations

| Risk | Mitigation |
|---|---|
| Eval set not built | Phase 5 gate; CLI error on attempt to run without eval set |
| Eval set becomes stale | Quarterly clinical governance review; versioned eval set files |
| DRACO miscalibration | Calibration step required before gate opens; human ranking comparison |
| Evolution cost | Start with small eval set (50 triples); run biweekly, not daily |
| Archive drift | Evolution run archives committed to repo; human review before promotion |

---

## Open Questions

1. **Eval set construction.** What is the process for generating synthetic health data fixtures
   that are realistic enough to produce meaningful eval responses but contain no real patient data?
   Who generates the fixtures, and what review process governs their inclusion?

2. **Medical advisory board composition.** Who reviews the eval set ground-truth responses?
   What clinical specialties must be represented to cover the query types Helix handles?

3. **DRACO component weights.** The weights (0.35/0.20/0.15/0.15/0.15) are provisional. The
   calibration step requires comparison against clinical expert rankings; what is the plan for
   conducting that calibration, and who are the clinical experts?

4. **Verifier family.** Which specific model family should serve as the verifier/judge? The
   requirement is "different from the synthesizer." If the fm-analyst runs on Claude (Anthropic
   family), should the verifier run on a GPT-family model, a Gemini-family model, or a local
   model such as ruvLLM? What are the cost and privacy implications of each choice?

5. **Evolution run frequency.** How often should evolution runs be triggered in Phase 5? Weekly
   (analogous to ADR-266's `darwin-evolution.yml` weekly CI trigger) or less frequently?

6. **Archive promotion process.** Who reviews the evolution archive before a successful mutation
   is merged into the main harness? What is the sign-off requirement (lead engineer, clinical
   governance, both)?

---

## References

- `harnesses/timesfm-harness/.claude/skills/evolve/SKILL.md` — live Darwin Mode skill: air-gapped mutator, validateGeneratedCode gate, sandbox, measured improvement requirement. **[A]**
- `docs/adr/ADR-266-metaharness-darwin-integration.md` — 32 mutation surfaces, genetic algorithm, ADR-150 compliance, scoring policy. **[A]**
- `docs/adr/ADR-252-fastgrnn-training-pipeline.md` — DRACO matrix concept, FastGRNN training for routing, `TrainingDataset::from_draco`. **[A]**
- `docs/adr/ADR-265-ruvector-comprehensive-benchmark-suite.md` — 4-component scoring function (0.4*recall + 0.3*QPS + 0.2*memory + 0.1*latency); analogous to DRACO health scoring. **[A]**
- `helix/docs/Helix-PHI-ADR-Product-Spec.md` §13, §6 ADR-018, §8 (Phase 5 deferral), §10 (Darwin win rate metric). **[A]**
- ADR-008 — Verifier/Critic agent + swarm consensus (the verifier pipeline that Darwin scoring depends on). **[A]** (within Helix spec)
- ADR-005, ADR-006 — Grounded answering and evidence tiering; these are the behaviors Darwin Mode optimizes for. **[A]** (within Helix spec)
- LLM-as-judge evaluation bias literature (2023–2024): models used as judges show systematic preference for outputs from the same model family. **[B]** — well-supported in research but not cited to a specific paper in this document.
- Goodhart's Law in ML evaluation: "When a measure becomes a target, it ceases to be a good measure." — foundational principle behind the eval set gating requirement and the choice of DRACO over engagement metrics. **[A]** (general principle), **[B]** (health-AI specific applications).
