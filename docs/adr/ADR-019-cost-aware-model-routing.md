# ADR-019: Cost-Aware Model Routing Under Privacy Constraints

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-005, ADR-006, ADR-008, ADR-013, ADR-017, ADR-018, ADR-001

---

## Context

### The cost problem for health AI at the frontier

Routing every Helix query to a frontier cloud model (Claude Opus, GPT-4o, Gemini Ultra) achieves
the highest reasoning quality ceiling but is economically non-viable for a personal health
intelligence product:

- A health dossier generates continuous queries: proactive insights on new data, ambient-sensing
  anomaly investigation, background ontology mapping of lab results, periodic trend computation.
  Many of these are not high-complexity reasoning tasks.
- Frontier model pricing (~$0.003–0.015 per 1K output tokens) means that a user who imports
  a year of Apple Health data and runs a normalization pass against LOINC/RxNorm will consume
  hundreds of thousands of tokens before the first conversational query.
- Most tasks in the Helix pipeline are structurally simpler than "reason over a complex health
  question": PII scanning (AIDefence), ontology code lookup, trend magnitude computation,
  structured FHIR parsing, red-flag threshold comparison. These tasks can be handled by smaller
  models or deterministic code.

The naive alternative (route everything to the cheapest model) sacrifices quality where quality
is non-negotiable: the Functional-Medicine Analyst reasoning over a complex biomarker pattern,
the Verifier re-deriving a claim under ambiguous source data, the Escalation Guardian deciding
whether a cardiac signal is a red-flag.

The solution is a **learned cost-aware router**: a classifier that, given a task embedding,
predicts which model tier will handle it within the quality bar at the lowest cost. **[A]** —
grounded in `docs/adr/ADR-252-fastgrnn-training-pipeline.md` and `docs/adr/ADR-256-metaharness-sdk-evaluation.md`.

### Privacy amplifies the case for on-device routing

A health product that sends every query to a cloud frontier model has a structural privacy
vulnerability: all health data that appears in prompt context — biomarker values, medication
names, symptom descriptions, genomic information — is transmitted to the model provider's
infrastructure. Even with contractual privacy guarantees and encryption in transit, this exposure
is architecturally inconsistent with ADR-001 (user-owned, local-first encrypted vault) and
ADR-013 (on-device inference where feasible).

The cost-aware router therefore has a privacy dimension: routing to an on-device model (ruvLLM /
WASM path) keeps health data local. Routing to a cloud frontier model exposes that data to a
third-party infrastructure. This means on-device routing is simultaneously the lower-cost and
the higher-privacy option — the two objectives reinforce each other.

Cloud frontier model calls must be:
- Gated by explicit user consent for the specific call type.
- PII-gated: the AIDefence agent scrubs identifiers from the prompt context before transmission.
- Logged in the HIPAA-mode audit trail.
- Never used for the PII-gate agent itself (circular dependency: the agent that strips PII cannot
  route through a path that exposes PII).

Evidence grade: **[A]** — grounded in ADR-001, ADR-013, and the AIDefence PII-gating architecture
in the Helix spec §3.

### The DRACO matrix and the learned routing policy

The routing policy is learned — not hand-coded. The mechanism, analogous to the Tiny Dancer
FastGRNN router in ruvector (ADR-252), is:

1. **Generate a DRACO matrix.** Each row is `{task_embedding, quality_per_model_tier}` — the
   embedding of a task query, paired with quality scores (on the DRACO fitness scale from ADR-018)
   obtained by running the same task through each model tier and evaluating the output against the
   health-eval set.
2. **Train a FastGRNN classifier.** `TrainingDataset::from_draco` derives binary labels: does
   the cheap model (haiku / on-device ruvLLM) stay within tolerance of the quality bar? The
   FastGRNN router learns to predict this from the task embedding.
3. **Route at inference time.** Given a new task, compute its embedding, run it through the
   trained FastGRNN router, and route to the cheapest model that the router predicts will meet
   the quality bar.

In the ruvector codebase, this pipeline is implemented in `ruvector-tiny-dancer-core` with the
DRACO adapter and safetensors persistence fixed by ADR-252. The FastGRNN router runs inference on
all eight platform targets (linux/macos/windows x86_64 + arm64 + WASM) and produces a single
routing decision in sub-millisecond wall time. **[A]** — grounded in ADR-252 and cli.js Tiny
Dancer implementation.

For Helix, the Tiny Dancer router from ruvector is the canonical model-routing primitive. The
`@metaharness/router` concept described in the product spec §13 ("a learned router sends each
task to the cheapest model that clears the quality bar") is implemented via Tiny Dancer, not via
a separate `@metaharness/router` package. ADR-256 explicitly establishes this: "Promote the
router we already have as the harness's cost-optimal router. Use Tiny Dancer (ADR-252) as the
canonical model-routing primitive." **[A]**

### The three-tier model hierarchy for Helix

The Helix harness uses the three-tier model routing established in ADR-026 (ruvector) and
carried into the Helix context:

| Tier | Model | Latency | Cost | Use cases in Helix |
|---|---|---|---|---|
| **Tier 1** | On-device ruvLLM / WASM | <1ms–50ms | $0 per call | PII scanning, simple field extraction, FHIR schema parsing, ontology code lookup, threshold comparison, structured output formatting |
| **Tier 2** | Haiku (or equivalent small cloud model) | ~500ms | ~$0.0002/call | Lab panel normalization, wearable data ingestion, trend magnitude labeling, symptom-to-data mapping for simple queries |
| **Tier 3** | Sonnet / Opus (or equivalent frontier model) | 2–8s | $0.003–0.015/call | Functional-medicine analyst reasoning over complex patterns, verifier/critic independent derivation, Escalation Guardian red-flag assessment, cross-biomarker correlation reasoning, complex graph traversal queries |

The router learns, from Helix's own eval logs, which task types reliably clear the quality bar at
Tier 1 or Tier 2. Tasks that the router assigns to Tier 1 or 2 but that score below the quality
bar are re-routed to the next tier (escalation). Tasks that consistently fail at Tier 2 and require
Tier 3 contribute negative training signal to the DRACO matrix, reinforcing the Tier 3 assignment.

### The quality bar — and why it is non-negotiable

The quality bar is the minimum DRACO fitness score (from ADR-018) that a response must achieve
to be delivered to the user. It is not a soft preference; it is an architectural guardrail.

The cost optimization objective is: *route to the cheapest tier that clears the quality bar.* Not:
*route to the cheapest tier.* The bar is set first; cost is minimized subject to the bar.

This means:

- A task routed to Tier 1 that does not clear the bar is escalated to Tier 2. The router is
  updated: the DRACO matrix gains a row indicating this task type is not Tier-1 sufficient.
- A Tier 2 response that does not clear the verifier's grounding check (ADR-008) is escalated
  to Tier 3. The verifier's judgment is the ground truth; the router defers to it.
- A Tier 3 response that does not clear the bar is not delivered. The abstention policy (ADR-005,
  ADR-006) applies: the user receives "I don't have sufficient grounded data to answer this"
  rather than a below-bar response from a frontier model.

The coupling of cost-aware routing with the faithfulness bar ensures that cost optimization is
always a secondary objective. Routing never overrides the faithfulness requirement established in
ADR-005, ADR-006, and ADR-018.

### Consent and PII gating for cloud escalation

Every Tier 3 (cloud frontier model) call requires:

1. **Explicit user consent, per call type.** The first time a query type escalates to Tier 3,
   the user is notified: "This query requires a more capable model. It will send de-identified
   health context to [provider]. Do you want to proceed?" The user can approve, deny, or set a
   standing preference per query type. Standing preferences are stored in the local vault and
   respected on all subsequent calls of that type.
2. **PII stripping before transmission.** The AIDefence PII gate agent processes the prompt
   context before any Tier 3 call. The strip operation removes: names, dates of birth, addresses,
   provider names, insurance IDs, and any genomic identifiers. The stripped context is what is
   transmitted; the original context (with PII) never leaves the local vault.
3. **Audit logging.** Every Tier 3 call is logged in the HIPAA-mode audit trail: timestamp,
   query type, model provider, PII-strip confirmation, user consent record.

The AIDefence PII gate runs on-device at Tier 1 and is never itself routed to a cloud model.
This prevents the circular exposure problem: the agent responsible for stripping PII cannot
itself route through a cloud path that would expose PII to the very infrastructure it is stripping
it from.

### Memory that ages correctly — emergent-time decay

A health intelligence product must weight evidence by recency. A ferritin value from six years ago
is less relevant than one from last month. A resting heart rate trend from before a medication
change is less informative than one from after. The Helix harness uses an **emergent-time decay
model** (powered by `@ruvector/emergent-time`, linked via the `@metaharness/kernel` peer
dependency) to assign decay weights to signals in the memory retrieval layer.

The decay model parameters are one of the seven Darwin Mode mutation surfaces (ADR-018). Specifically,
the `memory-decay-rate` surface encodes half-life values per signal type:

| Signal type | Default half-life | Rationale |
|---|---|---|
| Wearable HRV (daily) | 45 days | High-frequency signal; trends emerge quickly |
| Lab panel (quarterly) | 180 days | Reference: most panels repeat at 3–12mo |
| Medication start/stop | No decay | Permanent event; always relevant |
| Genomic variant | No decay | Immutable signal |
| CGM / continuous glucose | 30 days | Context-specific; dietary changes alter baseline |
| Sleep stage summary | 60 days | Seasonal variation; moderate decay |
| Subjective symptom log | 30 days | Highly variable; short-term relevance |
| mmWave ambient vitals | 7 days | High-frequency; rapid trend detection |

These defaults are the baseline; Darwin Mode can tune them within bounds. The bounds prevent
the optimizer from setting a decay rate that would effectively discard a signal that the health-eval
set ground truth relies on (which would score poorly on the coverage component of DRACO).

The emergent-time decay is integrated into the retrieval layer: when the Functional-Medicine Analyst
issues a vector search query, the returned results are weighted by their decay-adjusted relevance.
A result that is semantically highly similar to the query but old will score lower than a
semantically similar and recent result. This keeps "what do we know about this person now" distinct
from "what did we know about this person years ago."

Evidence grade for emergent-time: **[B]** — `@ruvector/emergent-time` is confirmed as a transitive
dependency of `@metaharness/kernel` (ADR-256), but its specific API and decay model parameters are
not independently verified in documentation at document date. The decay concept itself is standard
in longitudinal time-series analysis.

---

## Decision

Helix deploys a three-tier, learned cost-aware router implemented via the Tiny Dancer FastGRNN
classifier (ADR-252), with on-device inference (ruvLLM / WASM) preferred, cloud escalation
gated by consent and PII-stripping, and the faithfulness quality bar as a non-negotiable floor
that routing cannot override.

### The routing policy lifecycle

The routing policy is not static: it learns from Helix's own evaluation logs.

**Bootstrap** (Phase 0–1): The router uses a heuristic initial policy derived from the task
taxonomy defined in the Helix agent roster (ADR-017). Simple structural tasks → Tier 1; moderate
reasoning tasks → Tier 2; complex multi-step health reasoning → Tier 3. This heuristic is
hand-coded in the harness's `routing-tiers` mutation surface.

**Calibration** (Phase 1–2): As Helix accumulates eval log entries (from both the health-eval
set in Darwin Mode and from anonymized task-outcome pairs in production), a DRACO matrix is
assembled. `tiny-dancer train` learns the FastGRNN classifier.

**Deployment** (Phase 2+): The trained classifier replaces the heuristic policy. The `routing-tiers`
mutation surface in Darwin Mode can now tune the classifier's confidence thresholds (not the
model weights), allowing evolution to adjust the boundary between tiers without retraining.

**Ongoing learning**: Every eval run (from Darwin Mode or manual QA) contributes new DRACO matrix
rows. `tiny-dancer train` is re-run on the accumulated matrix periodically (quarterly or after
a major product change). The updated model is reviewed before deployment.

### Routing decision path

```
task_query
    │
    ▼
[ embedding ] — ruvector HNSW encode
    │
    ▼
[ Tiny Dancer router ]
    │
    ├── score > threshold_tier1? ──YES──► Tier 1 (on-device ruvLLM / WASM)
    │                                         │
    │                                         ▼
    │                                   DRACO score?
    │                                         │
    │                           pass ─────────┤────────── fail
    │                            │             │
    │                            ▼             ▼
    │                        deliver      escalate to Tier 2
    │
    ├── score > threshold_tier2? ──YES──► Tier 2 (Haiku / small cloud)
    │                                         │
    │                                         ├── consent + PII gate
    │                                         ▼
    │                                   DRACO score?
    │                           pass ─────────┤────────── fail
    │                            │                         │
    │                            ▼                         ▼
    │                        deliver              escalate to Tier 3
    │
    └── else ──────────────────────────► Tier 3 (Sonnet / Opus)
                                               │
                                               ├── consent (per call type)
                                               ├── PII gate (AIDefence, on-device)
                                               ├── audit log entry
                                               ▼
                                         DRACO score?
                                 pass ─────────┤────────── fail
                                  │                         │
                                  ▼                         ▼
                              deliver                  abstain (ADR-005/006)
```

### Configuration encoding in the harness

The routing policy is encoded in the harness's `routing-tiers` surface file:

```json
{
  "tier1_threshold": 0.85,
  "tier2_threshold": 0.65,
  "quality_bar": 0.72,
  "on_device_model": "ruvllm-health-small-q4",
  "tier2_model": "claude-haiku-3-5",
  "tier3_model": "claude-sonnet-4-5",
  "consent_required_for": ["tier2", "tier3"],
  "pii_gate_required_for": ["tier2", "tier3"],
  "always_tier1": ["pii_scan", "threshold_compare", "field_extract", "fhir_schema_parse"],
  "always_tier3": ["red_flag_escalation", "complex_correlation", "verifier_derivation"]
}
```

The `always_tier1` and `always_tier3` lists are hardcoded overrides: the router's learned policy
cannot route red-flag escalation below Tier 3, and PII scanning cannot route above Tier 1. These
overrides are outside the Darwin Mode mutation surface and require a code change and governance
review to modify.

### Guardrail: cost optimization never overrides faithfulness

The following invariants are enforced in code, not convention:

1. The `quality_bar` parameter cannot be set below the minimum grounding threshold established
   in ADR-005 (every factual claim must resolve to a stored datum with provenance). A router
   configuration with `quality_bar < grounding_minimum` is rejected at harness load time with
   a `HELIX_QUALITY_BAR_VIOLATED` error.

2. Darwin Mode's `routing-tiers` mutation surface is bounded: `tier1_threshold` cannot be
   lowered below 0.80, and `quality_bar` cannot be lowered below 0.70, regardless of DRACO
   score improvement. A mutation that would lower these below the floor is rejected by the
   `validateGeneratedCode` gate.

3. The Verifier/Critic agent (ADR-008) runs at Tier 3 regardless of the router's tier assignment
   for the fm-analyst. The verifier is never downgraded to a cheaper tier: independent derivation
   requires the verifier to have sufficient reasoning capability to re-derive complex health claims.

4. The Escalation Guardian (ADR-009) runs at Tier 3. The classification of a red-flag value
   cannot be delegated to a tier that might miss the pattern. This is a patient-safety invariant.

---

## Alternatives Considered

### Alternative 1: Always route to the frontier model

Route every Helix query to Claude Opus or equivalent. Maximum quality ceiling; no routing
complexity.

**Why rejected.** Economically non-viable at scale (continuous background tasks will cost
thousands of dollars per user-year at frontier pricing). Architecturally incompatible with
ADR-001 (local-first) and ADR-013 (on-device preference) since all queries expose health data
to cloud infrastructure. Provides no quality advantage for structurally simple tasks (PII scan,
FHIR parsing, threshold comparison) where a Tier 1 on-device model performs identically.

### Alternative 2: Rule-based routing — hard-code which agents go to which tier

Define a fixed taxonomy: ingestion agents always use Tier 2; the fm-analyst always uses Tier 3;
PII scan always uses Tier 1. No learning, no router.

**Why rejected.** Rule-based routing is static — it cannot improve as the model landscape changes,
as new on-device models become capable of handling tasks previously requiring cloud, or as the
eval set reveals that a particular task type that was assumed to need Tier 3 is actually handled
well by Tier 2. The learned router can discover these improvements; static rules cannot.
Rule-based routing also cannot tune task boundaries: the line between "moderate reasoning" (Tier 2)
and "complex reasoning" (Tier 3) is not a sharp threshold — it is a distribution over embedding
space that a learned classifier captures better than a human-written taxonomy.

That said, the `always_tier1` and `always_tier3` hardcoded overrides in the routing configuration
retain the rule-based approach for tasks where the safety or privacy requirement is categorical
(PII scanning, red-flag escalation). These are not replaced by the learned router; they are
layered on top of it.

### Alternative 3: Use the `@metaharness/router` package

Adopt the `@metaharness/router` package (if it exists as a separate release from `metaharness`)
as the routing layer, instead of Tiny Dancer.

**Why rejected.** ADR-256 establishes that Tiny Dancer's FastGRNN router is the canonical
routing primitive for this stack: it is already shipped, benchmarked, tested on eight platform
targets, and implements the DRACO-based routing pattern that `@metaharness/router` conceptually
describes. Taking `@metaharness/router` as a runtime dependency would be circular (the
metaharness ecosystem depends on `@ruvector/ruvllm`) and would conflict with ADR-150's
removability constraint. The Tiny Dancer approach delivers the same learned routing capability
without the dependency risk.

### Alternative 4: Consent-free cloud routing with contractual privacy guarantees

Route to cloud models without per-call user consent, relying on contractual guarantees from the
model provider (data processing agreements, HIPAA BAA).

**Why rejected.** Contractual guarantees are weaker than architectural guarantees. A HIPAA BAA
is not a technical control; it is a liability agreement. A system designed so that health data
*cannot* leave the device without explicit user consent and PII-stripping is stronger than a
system where health data leaves the device but the provider has signed a promise not to misuse it.
ADR-001 establishes that Helix's privacy posture is architectural. Consent + PII-gating is the
architectural expression of that posture in the routing layer.

---

## Consequences

### Positive

- "Frontier-quality at a fraction of the cost": the learned router identifies which tasks can be
  handled by cheaper or on-device models, dramatically reducing per-user inference cost while
  preserving quality on complex health reasoning tasks.
- Privacy-by-default: on-device routing preference means health data stays local in the common
  case. Cloud frontier calls are the exception, not the rule.
- The quality bar is structurally enforced: cost optimization cannot degrade faithfulness below
  the floor, and the verifier/escalation guardian are never downgraded.
- The routing policy learns continuously: as on-device models improve and the eval log accumulates,
  the router discovers tasks it can handle without cloud escalation. Cost falls over time without
  manual re-engineering.
- Audit trail: every cloud frontier call is logged with consent record, PII-strip confirmation,
  and DRACO outcome score.

### Negative

- Bootstrap requires a heuristic initial policy (Phase 0–1) before the learned classifier is
  ready. The heuristic may be conservative (over-routing to Tier 3) until calibrated.
- The DRACO matrix requires the health-eval set (ADR-018 gating condition). Until the eval set
  exists, the learned policy cannot be trained, and the heuristic bootstrap policy applies.
- Per-call consent for Tier 3 creates friction. Users who want seamless cloud escalation must
  set standing permissions; users who do not want cloud escalation at all must configure the
  router accordingly. UX design must make this clear without overwhelming onboarding.
- The PII-strip step adds latency to every Tier 2/3 call (estimate: 50–150ms on-device). This
  latency is visible to users for interactive queries.
- Tiny Dancer's FastGRNN router was designed for the ruvector benchmark task distribution. Helix's
  health task distribution may differ significantly; the bootstrap heuristic and the first DRACO
  matrix need to cover enough of the Helix task space to train a meaningful classifier.

### Mitigations

| Risk | Mitigation |
|---|---|
| Bootstrap over-routes to Tier 3 | Acceptable until calibrated; Phase 1–2 calibration plan |
| Eval set dependency for learned router | Bootstrap heuristic covers Phase 0–4; learned router is Phase 5+ |
| Consent friction | Standing preferences per query type; sensible defaults (consent once per type) |
| PII-strip latency | Async pre-strip on prompt assembly; cache stripped contexts for repeated templates |
| Task distribution mismatch | Seed the DRACO matrix with synthetic eval-set examples across all Helix task types |

---

## Open Questions

1. **On-device model for Tier 1.** Which ruvLLM model (and quantization level) serves as the
   Tier 1 on-device model? The target is a model that fits in <2 GB on a mobile device,
   handles simple structured tasks, and runs in <100ms on the WASM path. What is the current
   best candidate?

2. **DRACO matrix seed.** How many DRACO matrix rows are needed before the FastGRNN classifier
   produces reliable routing decisions? The ruvector training pipeline (ADR-252) does not specify
   a minimum dataset size. What is the minimum before the learned router is trustworthy enough
   to replace the bootstrap heuristic?

3. **Provider for Tier 2.** The harness configuration names `claude-haiku-3-5` as the Tier 2
   model. Is this the correct current Haiku version? Should the harness support multiple Tier 2
   provider options (Haiku, Gemini Flash, GPT-4o-mini) with the router selecting among them,
   or is provider selection fixed?

4. **Standing consent UX.** What is the onboarding flow for establishing standing consent for
   Tier 2/3 calls? At what granularity is consent given — per query type, per agent, per
   session, or globally?

5. **PII-strip completeness.** The AIDefence PII gate removes explicit identifiers. Does it also
   handle quasi-identifiers (combinations of rare biomarker values, rare genomic variants, or
   rare condition combinations that could re-identify even without explicit PII)? What is the
   verification approach?

6. **Emergent-time API.** The `@ruvector/emergent-time` package is linked transitively through
   `@metaharness/kernel`. Is there a stable public API for setting decay coefficients per signal
   type, or does this require direct access to the package? What version should be targeted?

---

## References

- `docs/adr/ADR-252-fastgrnn-training-pipeline.md` — Tiny Dancer FastGRNN, DRACO matrix, safetensors, `TrainingDataset::from_draco`. **[A]**
- `docs/adr/ADR-256-metaharness-sdk-evaluation.md` — Tiny Dancer as canonical routing primitive; `@metaharness/router` not needed; emergent-time transitive dependency. **[A]**
- `npm/packages/ruvector/bin/cli.js` lines 1879–1950 — Tiny Dancer `train` and `route` CLI surface. **[A]**
- `docs/adr/ADR-266-metaharness-darwin-integration.md` — routing-tiers as a Darwin mutation surface. **[A]**
- `helix/docs/Helix-PHI-ADR-Product-Spec.md` §13 (cost-aware routing, memory that ages correctly), §6 ADR-019, §3 (agent roster + PII gate), ADR-001, ADR-005, ADR-006, ADR-008, ADR-013. **[A]**
- ADR-026 (ruvector) — Three-tier model routing: Agent Booster / Haiku / Sonnet-Opus tiers. **[A]** (local ADR)
- `@ruvector/emergent-time` — Decay-weighted memory; linked via `@metaharness/kernel` peer dep. **[B]** — existence confirmed (ADR-256); API details not independently verified.
- FastGRNN (Kusupati et al., 2018) — architecture underlying Tiny Dancer's routing classifier. **[B]** — published paper; not directly cited in the ruvector repo documentation.
- "LLM routing for cost-quality tradeoff" — published work on learned LLM routing (RouteLLM, FrugalGPT, 2023–2024). **[B]** — relevant external literature; not specifically implemented in this stack but conceptually grounded in the same approach.
