# ADR-002: Ruflo as the Orchestration Meta-Harness

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-003, ADR-005, ADR-006, ADR-007, ADR-008, ADR-009, ADR-011, ADR-013, ADR-017, ADR-018, ADR-019

---

## Context

### Why orchestration is not optional

Helix is irreducibly multi-step and multi-specialty. A single conversation about "why am I tired in the afternoons?" requires:

1. Retrieving 6–10 relevant biomarker time-series from RuVector (ferritin trend, iron saturation, deep-sleep percentage, resting HR trajectory).
2. Computing numeric deltas and range crossings *deterministically* — an LLM must not be asked to do this arithmetic.
3. Drafting a grounded narrative answer that cites only facts already retrieved.
4. Independently re-deriving each factual claim in that draft against the original stored datums (verification).
5. Checking whether any retrieved value crosses a red-flag threshold before the answer reaches the user.
6. Gating all outbound content through a PII scanner so no PHI leaks to any external model.
7. Recording the reasoning trajectory for the SONA/ReasoningBank learning loop so the next similar question is handled better.

No monolithic prompt can safely own all seven steps without conflating concerns, creating hallucination risk at the arithmetic layer, and bypassing the verification gate. Decomposing into specialist agents is a structural necessity, not a preference. **[A]**

### The hallucination anatomy of single-model health chat

The canonical failure mode — a single LLM receiving a pile of health-record text and answering in one pass — conflates three distinct failure modes:

- **Numeric fabrication**: the model invents or miscalculates trends, percentages, and deltas. LLMs are unreliable over time-series arithmetic. **[A]**
- **Citation confabulation**: the model composes an answer that sounds grounded ("your ferritin is low at 28") but has no mechanism to verify that 28 is the *actual stored value* versus a plausible confabulation.
- **Scope creep**: without an explicit escalation guardian, the model tips into diagnostic claims or optimization advice when values are actually dangerous.

Each of these maps to a specific agent boundary in the Helix swarm — the Trend/Numeric agent (ADR-007), the Verifier/Critic agent (ADR-008), and the Escalation Guardian (ADR-009).

### Ruflo as the substrate

Ruflo is the ruvnet agent meta-harness: a multi-topology swarm coordinator with hooks, AIDefence-gated PII scanning, SONA/ReasoningBank learning, HIPAA-mode audit trails, and 3-tier cost-aware model routing (WASM booster → Haiku → Sonnet/Opus, per ADR-026/ADR-019). It provides:

- **Hierarchical-mesh swarm topology** with Raft consensus for leader-maintained authoritative state. **[A]**
- **12 background workers** (audit, optimize, consolidate, benchmark, ultralearn, and others) available for offline health-graph maintenance tasks.
- **Hooks** (`pre-task`, `post-task`, `session-start`, `session-end`, `pre-edit`, `post-edit`) that are machine-executed, not prompt-based, enabling reliable automation regardless of conversational context.
- **SONA trajectory recording** for per-question learning; **ReasoningBank** for distillation of successful reasoning patterns into HNSW-indexed retrievable verdicts.
- **AIDefence** — multi-layer defense against prompt injection, PII egress, and adversarial input. **[A]**
- **Ed25519 witness-signed audit entries** satisfying HIPAA §164.312(b) audit-control requirements. **[B]**
- **Federation support** for PII-stripped cohort signals (ADR-011) with behavioral trust scoring.

The MetaHarness layer (ADR-017) mints Helix as a branded, independently versioned harness that pins the Ruflo/RuVector kernel at a known-good version while remaining upgradeable. The product is the harness; the model is a replaceable detail.

---

## Decision

### Adopt Ruflo as the Helix orchestration meta-harness with a ten-agent specialist swarm

Helix runs a **hierarchical swarm** with a maximum of 12 concurrent agents (6–8 active for most queries), using **Raft consensus** for outputs that are clinically meaningful. The swarm is initialized at session start and the router holds state across a conversation.

#### Swarm initialization

```bash
npx @claude-flow/cli@latest swarm init \
  --topology hierarchical \
  --max-agents 12 \
  --strategy specialized \
  --consensus raft \
  --namespace helix-{user_id}
```

#### The ten specialist agents

| Agent | Role | Model tier | Key guarantee |
|---|---|---|---|
| **Router / Coordinator** | Receives user input, decomposes into sub-tasks, routes to agents, assembles final response | Haiku | Never fabricates; only routes |
| **Ingestion agents** (one per connector, spawned on demand) | Pull data from connectors (FHIR, HealthKit, Health Connect, wearable APIs, lab feeds), normalize into Ruflo task queue | Haiku / WASM booster | Idempotent; fault-isolated per connector |
| **Normalization agent** | Maps raw inbound facts to canonical ontologies (ADR-004); attaches provenance; routes un-mappable to review queue | Haiku | Never silently coerces; rejects → queue |
| **Trend / Numeric agent** | Deterministic computation: slopes, deltas, reference-range crossings, correlations, z-scores, change-points | WASM booster (Tier 1) | Code only — no LLM arithmetic on health data |
| **Functional-Medicine (FM) Analyst** | Retrieves grounded facts from RuVector, composes insights and recommendations; must cite every claim | Sonnet | Produces draft only; cannot publish without Verifier |
| **Verifier / Critic agent** | Re-derives each claim in the FM Analyst draft against original RuVector datums; rejects or downgrades uncorroborated claims; applies evidence tiering (ADR-006) | Sonnet (different provider family from Analyst where feasible) | Gate — no output reaches user without passing here |
| **Escalation Guardian** | Monitors retrieved values against red-flag threshold registry; short-circuits to urgent-care guidance when triggered; suppresses optimization advice during escalation (ADR-009) | Haiku (rule-first, LLM for narrative only) | 100% recall target on life-safety flags |
| **Experiment Planner** | GOAP-based n-of-1 experiment design and tracking; generates measurement plan, expected effect size, duration, and retest schedule | Sonnet | Deferred until Phase 4 roadmap |
| **Ambient Sensing agent** | Runs on / with the Cognitum Seed; first-pass signal extraction and anomaly detection for mmWave vitals; emits screening flags into vault (ADR-014) | WASM booster (on-device) | Raw sensor data never transmitted |
| **Visualization agent** | Translates graph state into 3D twin color-coding, 0–100 score decomposition, trend sparklines, and explanatory imagery (ADR-015/016) | Haiku / WASM booster | Grounded-only — renders actual data, never fabricated |
| **Privacy / PII Gate (AIDefence)** | Scans every egress path (federation, any external LLM call, visualization export) for PII; blocks or strips before transmission | WASM (rule + ML) | No raw PHI leaves the vault boundary |

#### Answer pipeline (happy path)

```
User question
    │
    ▼
[Router] decomposes → parallel sub-tasks
    │
    ├─► [Trend/Numeric agent]  ──────── deterministic facts
    │        (code, no LLM)
    │
    ├─► [RuVector retrieval]  ──────── grounded datums with provenance
    │
    ▼
[FM Analyst]  ──── draft answer (cites retrieved facts only)
    │
    ▼
[Verifier/Critic]  ──── re-derives each claim
    │          ├── supported → passes
    │          └── unsupported → dropped / abstention appended
    ▼
[Escalation Guardian]  ──── checks for red-flag values
    │          ├── clear → answer released to user
    │          └── red-flag → escalation pathway, optimization suppressed
    ▼
[PII Gate]  ──── scans outbound content
    │
    ▼
User sees cited, verified, evidence-tiered answer
```

#### Swarm topology and consensus

- **Topology**: `hierarchical` with the Router as queen; prevents drift by maintaining a single authoritative state holder.
- **Consensus**: `raft` — the Verifier/Critic is effectively the Raft leader for clinical output; the FM Analyst is a proposer. No claim reaches the user without Verifier acknowledgement.
- **Anti-drift**: Ruflo's hierarchical topology enforces role boundaries. Agents cannot modify RuVector directly; all writes go through the Normalization agent or Ingestion agents, which enforce provenance attachment.

#### Hooks wiring

| Hook | Trigger | Action |
|---|---|---|
| `session-start` | User opens Helix | Restore session state from RuVector; import recent vault updates; prime ReasoningBank with last N trajectories for this user |
| `pre-task` | Any agent receives a task | AIDefence scan of input; reject if prompt injection detected; PII check before routing to external model |
| `post-task` | Agent completes a task | Store trajectory step in SONA; if task was an answer, run `hooks_post-task --train-neural true` |
| `pre-edit` | Normalization agent writes to vault | Validate provenance fields present; enforce schema |
| `post-edit` | Any vault write completes | Append witness-signed audit entry; update HNSW indexes |
| `session-end` | User closes session / timeout | Compress trajectory; distill patterns via ReasoningBank `consolidate` worker; persist session RVF |

#### SONA / ReasoningBank learning loop

The swarm learns per-user and per-pattern over time:

1. **Trajectory recording**: every question–answer cycle records a trajectory (`trajectory-start` → multiple `trajectory-step` → `trajectory-end`) capturing: query intent, retrieved datums, analyst draft, verifier verdict, final answer.
2. **Verdict judgment**: the Verifier's verdict (supported / partially supported / abstained / escalated) is the ground-truth label for the trajectory.
3. **Pattern distillation**: `ReasoningBank` runs distillation after every N closed trajectories, extracting reusable reasoning patterns (e.g., "low-ferritin + reduced deep-sleep → energy pattern; relevant LOINC codes; standard evidence tier") and indexing them in HNSW.
4. **Pattern retrieval**: on subsequent similar questions, the Router retrieves top-k patterns before spawning agents, pre-seeding the FM Analyst with the validated reasoning scaffold.
5. **Experience replay**: `ultralearn` background worker replays past trajectories periodically to reinforce correct patterns and decay stale ones.

Net effect: Helix gets measurably better at reasoning about this specific user's health over time, without retraining a model.

#### AIDefence PII gating

Every agent boundary that crosses the vault perimeter is gated:

- **Inbound external data**: `aidefence_scan` checks for injection patterns in raw connector payloads before they enter the normalization pipeline.
- **LLM calls**: if an external model (cloud frontier) is used, `aidefence_has_pii` screens the assembled context; any PII is stripped or pseudonymized before the call.
- **Federation egress**: all outbound federation payloads pass `transfer_detect-pii`; raw values are replaced with differential-privacy-noised aggregates. Raw records never leave the vault (ADR-011).
- **Visualization export**: any shared image or summary is scanned before transmission.

AIDefence uses a multi-layer stack: rule-based PII patterns (regexes for SSN, DOB, MRN formats), ML-based contextual detection, and a prompt-injection classifier. The system is adaptive — `aidefence_learn` ingests blocked patterns and re-weights the classifier. **[A - re: pattern of rule+ML defense layers]**

#### HIPAA-mode audit trail

Every agent action on health data produces an append-only, hash-chained audit entry:

```
{
  "timestamp": "ISO-8601",
  "agent_id": "helix-verifier-001",
  "action": "VERIFY_CLAIM",
  "resource_type": "lab_result",
  "resource_id": "loinc:3016-3:2026-04-10",
  "verdict": "SUPPORTED",
  "evidence_tier": 1,
  "previous_hash": "sha256:...",
  "signature": "ed25519:..."
}
```

This satisfies HIPAA §164.312(b) (audit controls) and §164.312(c)(1) (integrity). The hash chain is verifiable by the user and, in the event of dispute, by a clinician or regulator. The user *always* owns the audit log; Helix cannot delete it.

---

## Alternatives Considered

### Alternative 1: Monolithic single-prompt architecture

A single LLM receives the full health record as context and answers in one pass, as in ChatGPT Health's current model.

**Rejected because:**
- LLMs perform unreliably on arithmetic over time-series health data — trends, deltas, and range crossings will be fabricated or miscalculated at non-trivial rates. **[A]**
- No structural verification gate — the model checks its own work, which is not independent.
- No escalation guardrail that fires *before* the user sees dangerous output.
- Cannot produce a HIPAA-compliant hash-chained audit trail of individual reasoning steps.
- Learning is limited to in-context examples; no SONA/ReasoningBank accumulation.
- Privacy boundary is a single point of failure; any prompt-injection breach exposes everything.

### Alternative 2: LangChain / LangGraph or similar Python orchestration framework

A Python-based agent framework with custom health-domain chains.

**Rejected because:**
- No native WASM / on-device path — violates the local-first privacy model (ADR-001, ADR-013).
- Python runtime is a significant dependency on mobile/edge; ruvLLM's WASM path requires Rust/WASM integration.
- No built-in PII-gating, AIDefence, or hash-chained audit. These would require significant bespoke construction — replicating what Ruflo already provides.
- No SONA/ReasoningBank integration; per-user learning would need a separate custom layer.
- Dependency on a Python orchestration framework conflicts with the ruvnet all-Rust mandate (project CLAUDE.md).
- External library surface area increases supply-chain risk for a healthcare product.

### Alternative 3: Custom Rust orchestrator built from scratch

Write a purpose-built Helix orchestrator in Rust that directly manages agent lifecycles, hooks, and audit trails.

**Rejected because:**
- Ruflo already provides the validated, production-tested machinery for swarm coordination, hooks, SONA, ReasoningBank, AIDefence, and audit. Building this from scratch is 6–12 months of foundational work before any health-domain logic can be written.
- Maintenance burden: every upgrade to the consensus protocol, PII scanner, or hook system would fall on the Helix team rather than being pulled from upstream.
- The MetaHarness (ADR-017) model explicitly solves the "stay current while owning the brand" problem. A custom orchestrator discards that leverage.

---

## Consequences

### Positive

- **Clean role separation**: each agent owns a specific slice of the pipeline; bugs and hallucinations are localized and debuggable.
- **Architectural anti-hallucination**: the Verifier gate and deterministic numeric agent structurally prevent the two most common failure modes in health chat.
- **Built-in privacy and security**: AIDefence PII gating, hash-chained audit, and Ed25519 witness signatures are provided by the substrate, not bolted on.
- **Per-user learning**: SONA/ReasoningBank means Helix compounds — it reasons better about a specific user's patterns over time without model retraining.
- **Multi-provider LLM routing**: Ruflo's 3-tier routing (WASM → Haiku → Sonnet) minimizes cost and maximizes on-device computation, supporting ADR-013 and ADR-019.
- **Darwin Mode compatible**: the harness configuration (retrieval parameters, routing tiers, verifier thresholds) is exactly what Darwin Mode mutates and tests — Ruflo is the surface Darwin optimizes (ADR-018).

### Negative

- **Operational complexity**: 10-agent swarm is significantly more complex than a single-prompt system. Debugging requires distributed tracing across agent boundaries.
- **Latency**: multi-agent pipeline adds latency vs. single-pass. Mitigation: parallel retrieval and numeric computation; on-device Tier-1 tasks; async non-blocking hooks.
- **Dependency risk**: Ruflo is a fast-moving open-source project. Pin versions explicitly; maintain a vendor-risk plan; track upstream releases.
- **Eval set requirement**: SONA/ReasoningBank and Darwin Mode are only as good as the health-eval set used to judge trajectory outcomes and evolve the harness (ADR-018). Building a high-quality, curated health eval set is a significant ongoing effort.

### Mitigations

| Risk | Mitigation |
|---|---|
| Agent boundary bugs causing data leakage | AIDefence scans every agent boundary; vault writes require provenance schema validation |
| Latency exceeds interactive threshold | Tier-1 WASM booster for all deterministic tasks; stream partial responses where safe |
| Ruflo upstream breaking change | Semantic-version pin in MetaHarness; pre-upgrade integration test suite |
| Verifier approves a fabricated claim | Verifier uses a different model family from the Analyst; human-review queue for low-confidence outputs |

---

## Open Questions

1. **Model family independence for Verifier**: which specific Sonnet/Opus variants from which providers will be used to ensure the Verifier and Analyst are genuinely different model families? This requires ADR-019 routing configuration.
2. **Red-flag threshold registry**: who maintains the clinical thresholds the Escalation Guardian monitors? Requires medical advisory board governance input (ADR-009).
3. **SONA trajectory retention policy**: how long are raw trajectories retained before compression? What is the user-controlled deletion policy to comply with right-to-erasure requirements?
4. **Concurrent multi-device sessions**: if a user has Helix open on phone + tablet simultaneously, how does the Router maintain Raft consistency across two session contexts?
5. **Audit log accessibility**: should the user be able to export the full audit log in a standard format (e.g., FHIR AuditEvent resource) for portability and clinical review?

---

## References

- Ruflo meta-harness documentation and swarm topology ADRs: `/home/ruvultra/projects/ruvector/docs/adr/` **[A]**
- HIPAA Security Rule, 45 CFR §164.312 (audit controls, integrity, access control): https://www.hhs.gov/hipaa/for-professionals/security/ **[B]**
- SMART on FHIR App Launch v2.2.0 (patient-mediated access model): https://build.fhir.org/ig/HL7/smart-app-launch/app-launch.html **[A]**
- "Medical Graph RAG: Towards Safe Medical LLM via Graph Retrieval-Augmented Generation" (arXiv:2408.04187) — evidence that structured retrieval over knowledge graphs improves grounding vs. flat-context LLM **[B]**
- OpenAI ChatGPT Health launch (January 7, 2026) — single-model health chat; differentiator benchmark for Helix **[A - publicly announced product]**
- ADR-007 (Deterministic numeric/trend engine), ADR-008 (Verifier/critic), ADR-009 (Escalation Guardian), ADR-017 (MetaHarness mint), ADR-018 (Darwin Mode), ADR-019 (Cost-aware routing) — all Helix-internal, cross-referenced
- RuVector ADR-028 (eHealth Platform Architecture) — prior art for GNN/GraphRAG + HIPAA pattern in ruvnet stack **[A]**
