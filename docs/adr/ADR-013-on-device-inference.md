# ADR-013: On-Device Inference Where Feasible

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001 (User-owned vault), ADR-002 (Ruflo meta-harness), ADR-003 (RuVector WASM path), ADR-010 (Wellness/SaMD boundary), ADR-011 (Federation), ADR-014 (Cognitum Seed), ADR-019 (Cost-aware routing)

---

## Context

### The privacy problem with cloud LLM inference over health data

The dominant architecture for AI-powered health products routes user queries and context
to cloud-hosted LLMs. The user's question, the retrieved health facts used as context, and
frequently additional personal identifiers (account ID, session metadata) transit to a
cloud inference endpoint, are processed by the model, and a response is returned.

This architecture creates several distinct risks:

**[A] Privacy exposure by design.** Sending health data to a cloud inference endpoint means
that data leaves the encrypted vault (ADR-001), crosses network boundaries, and is processed
by a third-party model service. Even if the provider has strong data-handling commitments,
the data has left the user's control. A health-specific query like "given my current
ferritin, testosterone, and sleep data, what is likely driving my fatigue?" requires
transmitting health values that are precisely the class of sensitive information that
ADR-001 is designed to protect.

**[B] Inference-time data is not end-to-end encrypted to the model.** Current cloud LLM
architectures require that inference-time context be in plaintext at the point of GPU
processing. Homomorphic encryption over LLM inference is not yet computationally practical
at interactive latency for large models. This means that a cloud endpoint, by necessity,
processes the health context in cleartext.

**[A] Regulatory classification risk.** Sending personal health data to a cloud LLM
operated by a third party may create a HIPAA Business Associate relationship (if the product
were a covered entity) or may trigger GDPR data-processor classification with corresponding
DPA requirements. For a DTC product that voluntarily adopts HIPAA-grade controls (as in
Helix's positioning), cloud LLM calls with health context require a BAA-equivalent
agreement with the model provider. Most LLM providers do not provide healthcare BAAs for
standard API access tiers.

**[B] Subpoena reachability.** Cloud-resident inference logs may be subpoena-reachable,
depending on the provider's data retention policy. This is the same structural risk that
the Helix spec calls out for ChatGPT Health: cloud-resident, encrypted, but reachable.

**[B] Third-party training on query data.** Some cloud LLM providers use API queries for
model training or fine-tuning by default, requiring explicit opt-out. Sending a user's
health context to such a provider without their knowledge constitutes sharing of sensitive
health data for a secondary purpose without informed consent.

### The state of on-device LLM inference in 2025–2026

**[A]** On-device LLM inference matured significantly in 2025–2026. Key developments:

**Model quantization**: 4-bit quantization in GGUF format (Q4_K_M and Q5_K_M variants)
reduces memory footprint by approximately 75% with minimal quality degradation. A 7B
parameter model that would require ~14 GB in FP16 runs in approximately 4 GB of device
RAM in Q4_K_M, bringing it within reach of recent flagship phones and all modern laptops.

**ExecuTorch (Meta)**: Reached 1.0 GA in October 2025 and v1.1.0 in January 2026.
ExecuTorch provides a 50 KB base runtime supporting 12+ hardware backends including
Apple Core ML (iPhone/Mac), Qualcomm QNN/Hexagon NPU (Android), Arm XNNPACK, and
experimental WASM/JavaScript. LoRA inference and 4-bit HQQ quantization are now supported.

**WebLLM**: Enables in-browser LLM inference via WebGPU and WASM, with models including
Llama 3, Phi-3-mini, and Mistral 7B running at interactive speed in a browser context
without any server-side inference.

**Medical-domain small models**: HuatuoGPT (medical dialogue adaptation), BioMistral (biomedical
LLM), and fine-tuned variants of Phi-3-mini and Gemma-2-2B are available for medical and
health-adjacent tasks. These are not frontier-quality reasoners, but for grounded retrieval
over structured health data with well-formed prompts, they are meaningfully capable.

**ruvLLM**: Helix's substrate (Ruflo + RuVector) includes ruvLLM — a local inference engine
designed for the ruvnet stack with a WASM path, MicroLoRA fine-tuning support, and tight
integration with the RuVector memory layer. This is the preferred on-device inference
engine for Helix in the Ruflo/RuVector substrate context.

**[A]** A key architectural advantage of on-device inference: health data that remains
in the decrypted-RAM context during an inference pass never traverses a network. The
privacy boundary is the device boundary, which is under the user's physical control.

### The hybrid necessity

On-device inference at 3B–7B parameters has real quality limitations for:

- Complex multi-step reasoning over large health knowledge graphs
- Free-form synthesis requiring frontier model-level coherence
- Medical literature synthesis spanning multiple clinical domains simultaneously
- Tasks where the cost of a wrong answer is high (e.g., Escalation Guardian threshold
  evaluation in ambiguous cases — though this is handled deterministically, not by LLM)

These tasks benefit from frontier model quality. The architectural question is not
"on-device or cloud" but "which tasks go where, with what consent model."

---

## Decision

Helix prefers on-device inference for all health-data-touching analysis tasks. Cloud
inference is reserved for tasks that explicitly cannot be handled by on-device models,
is gated behind explicit user consent at the point of data transmission, and passes through
the Ruflo AIDefence PII gate before any health context leaves the device.

### Model tier architecture

```
TIER 0 — Deterministic, no model (always on-device)
  Scope: trend computation, range crossings, numeric statistics, percent changes,
         correlations, anomaly detection against thresholds
  Implementation: Ruflo Trend/Numeric agent (ADR-007) — code, not LLM
  Privacy: perfect — no model involved, no inference, no data egress
  Why: LLMs are unreliable at arithmetic over time series; this tier handles all
       quantitative work

TIER 1 — Small on-device LLM (default for most queries)
  Scope: grounded Q&A using retrieved health facts, proactive insight composition,
         evidence tiering labels, user-facing narrative generation from structured data,
         local summarization of trends, daily briefing text
  Model: ruvLLM WASM path, or ExecuTorch-deployed 3B–7B parameter quantized model
         (Phi-3-mini-4K Q4_K_M, Gemma-2-2B-IT Q5_K_M, or BioMistral-7B-Q4_K_M)
  Hardware: CPU (all devices); NPU-accelerated (Qualcomm Hexagon, Apple Neural Engine)
            when available via ExecuTorch backend
  Latency target: < 2 seconds first-token for typical health query on 2024+ flagship phone
  Privacy: data stays on device; no network egress
  Context window: 4K–8K tokens (sufficient for structured retrieval from RuVector)

TIER 2 — Cloud inference with explicit consent and PII gate (complex reasoning)
  Scope: multi-step reasoning over large health graphs; complex synthesis across many
         clinical domains; advanced functionality that requires frontier model quality
  Trigger: Ruflo router determines Tier 1 model confidence is below threshold, or the
           task classification exceeds Tier 1's scope
  Consent gate: user is shown the prompt that will be sent (or a summary), including
                which health data values will be included; explicit per-session consent
                required before transmission
  PII gate: AIDefence agent strips identifiers (name, DOB, account ID, geolocation,
            provider names) before transmission; health values are sent but not linked
            to the user's identity in the outbound payload
  Model: cloud frontier (Claude Sonnet/Opus via Anthropic API, or equivalent)
  Data handling requirement: provider must have a healthcare data handling agreement
                             (BAA-equivalent) or explicit "no training on API inputs"
                             commitment for the access tier used
  Logging: transmitted health context is logged locally (user can inspect and delete),
           but Helix's backend does not receive or retain the health context

TIER 3 — Emergency escalation (Escalation Guardian, ADR-009)
  Scope: red-flag value detection, urgent-care routing
  Implementation: deterministic thresholds (Tier 0) + Escalation Guardian rule engine;
                  NOT an LLM inference task
  Cloud use: if network is available, Escalation Guardian may look up current urgent-care
             resources (location lookup, not health data transmission)
  Privacy: no health data transmitted for the escalation itself
```

### ruvLLM WASM path specifics

The ruvLLM WASM path runs the Tier 1 inference engine as a compiled WebAssembly module,
enabling:

- Browser-based Helix deployment (no native app required for web interface)
- Consistent runtime across iOS, Android, macOS, Windows, and Linux via WASM
- Sand-boxed execution that cannot access other browser/app data
- No dependency on native GPU bindings (though WASM SIMD and WebGPU paths are available
  for performance when permitted by the runtime)

**WASM model format**: GGUF Q4_K_M or GGUF Q5_K_M (4- and 5-bit quantized), loaded
into the WASM heap. Model files are downloaded once and cached locally (encrypted at rest
using the same vault KEK as user data — see ADR-001). Model updates occur on app update
cycles, not automatically.

**Tokenization and prompt construction**: health facts retrieved from RuVector are
serialized as structured JSON (not raw FHIR, to avoid token overhead), passed to the WASM
runtime, and processed entirely within the WASM sandbox. No health data leaves the
WASM sandbox during Tier 1 inference.

### Consent-gated cloud escalation flow

When Tier 2 cloud inference is warranted:

1. Ruflo router raises a `CloudEscalationRequest` with the task classification,
   the proposed prompt summary, and the list of health data fields that would be included.
2. The UI presents a consent modal: "To answer this question well, Helix would like to
   use a cloud AI service. This would send the following information: [list of data fields].
   Your name and account details are not sent. Do you consent for this session?"
3. User confirms, cancels, or selects "Use on-device only (may be less accurate)."
4. If confirmed, AIDefence performs PII stripping; the stripped payload is transmitted.
5. The Ruflo Verifier/Critic agent (ADR-008) validates the cloud response against the
   locally-held ground truth before the response reaches the user.
6. A session log entry records that cloud inference occurred (timestamp, data domains
   sent, model used) — the user can view and delete this log.

The Verifier/Critic step (step 5) is critical: the cloud model's response is not presented
to the user directly. It is grounded against the user's own vault data exactly as on-device
responses are. This prevents the cloud model from introducing hallucinated health claims
that bypass the Helix anti-hallucination pipeline (ADR-005).

### Model selection and health domain considerations

**[B]** No publicly available small model (under 7B parameters) as of early 2026 has been
validated for clinical-quality reasoning over personal health data. Models described as
"medical" (BioMistral, HuatuoGPT) are fine-tuned for biomedical NLP tasks and may have
specific strengths in terminology and literature, but they have not undergone clinical
validation comparable to the scrutiny applied to FDA-regulated diagnostic software.

**[C]** For Helix's use case — grounded Q&A over structured retrieved facts, not open-ended
clinical reasoning — small models are adequate because the difficult reasoning work is done
in retrieval (RuVector, ADR-003), ontology mapping (ADR-004), and numeric computation
(ADR-007). The LLM's role is compositional: assembling a coherent narrative from
pre-retrieved, pre-computed, structured facts. This is a lower-complexity task than
unconstrained clinical reasoning, and small models perform it reliably.

**Darwin Mode integration (ADR-018)**: The model tier routing policy is a candidate for
Darwin Mode optimization. Darwin Mode can safely evolve the Tier 1/Tier 2 boundary
(the confidence threshold for escalation) against the DRACO fitness function, subject to
the constraint that cloud escalation consent is never bypassed.

### Cognitum Seed on-device inference (ADR-014)

The Cognitum Seed edge device runs its own on-device inference for:
- mmWave signal processing and feature extraction
- First-pass anomaly detection (irregular breathing patterns, motion anomalies)
- Normalization of raw radar signals to derived vitals (respiration rate, HR estimate)

Seed inference happens on the Seed's embedded processor; derived vitals (not raw radar data)
are transmitted to the user's primary device and inserted into the vault. The Seed's
inference pipeline never has network connectivity in the Helix architecture — it is
air-gapped from the cloud, communicating only with the user's primary device via local
protocol (BLE or local Wi-Fi).

---

## Alternatives Considered

### Alternative A: Cloud-only inference with BAA

Route all health-data-touching queries to a cloud frontier model under a formal healthcare
data processing agreement (BAA or BAA-equivalent) with the model provider.

**Not adopted as the primary path** for four reasons: (1) BAAs are available from
very few LLM providers at the API tier, and those available often exclude use cases
involving sensitive data categories (genomics, mental health); (2) even with a BAA,
health data leaves the vault on every query, which contradicts ADR-001's local-first
model; (3) cloud-only removes the offline capability that is central to Helix's product
positioning; (4) the Tier 2 consent-gated cloud path provides BAA-equivalent access
where frontier quality is genuinely needed, without making it the default.

### Alternative B: On-device fine-tuned model (per-user personalization)

A base model is fine-tuned on-device against the user's health data using MicroLoRA
(ruvLLM) to create a personalized model that knows the user's data "by heart."

**Not adopted for Phase 0–1** because: (1) per-user fine-tuning on device creates model
storage overhead (multiple LoRA adapters); (2) fine-tuning on health data creates risk
of the model "memorizing" specific values in a way that could leak through the model
weights; (3) the Tier 0 + Tier 1 retrieval-grounded architecture achieves personalization
via RAG (retrieving the user's actual data) rather than model memorization, which is both
safer and more transparent. MicroLoRA personalization is preserved as a potential Phase 5
capability (ADR-013 v2) once the privacy implications are fully analyzed.

### Alternative C: Federated learning for model quality improvement

Train a global health-domain-adapted model via federated learning across Helix users,
with gradient aggregation (no raw data sharing). Distribute the improved model to devices.

**Deferred, not rejected.** FL for model improvement is a legitimate long-term option
for improving Tier 1 model quality without cloud data sharing. Its complexity (secure
aggregation, poisoning defense, gradient DP) makes it Phase 5+ work. The immediate
priority is making Tier 1 on-device inference good enough via retrieval grounding rather
than model quality alone.

### Alternative D: Hybrid privacy-preserving cloud inference (trusted execution environments)

Use cloud inference within a Trusted Execution Environment (TEE, e.g., AMD SEV-SNP,
Intel TDX) where the cloud provider provably cannot access the plaintext health context.
TEE-backed LLM inference is an emerging research area.

**Interesting but not yet production-viable.** TEE-hosted LLM inference at interactive
latency with full attestability is not yet broadly deployed by major model providers. Track
this as a future option that could eliminate the plaintext-at-inference limitation of
current cloud models.

---

## Consequences

### Positive

- Health data-touching queries default to staying on-device; users' health information does
  not leave their vault for routine analysis.
- Offline capability: Tier 0 and Tier 1 inference work with no network access. A user on
  a plane can ask about their health data and get a grounded, cited response.
- Lower marginal cost per query for the business: on-device inference at scale is
  significantly cheaper than cloud frontier API calls (marginal cost is device electricity,
  not API tokens).
- Explicit consent-gate for cloud escalation ensures users know when their health context
  is being transmitted; this is a meaningful differentiator from cloud-default competitors.
- Verifier/Critic applied to cloud responses prevents the cloud model from introducing
  hallucinations that bypass the anti-hallucination pipeline.

### Negative

- Tier 1 on-device model quality ceiling is real. Complex multi-hop health reasoning
  (e.g., drug-drug interaction analysis across a 12-medication stack, multi-system
  correlation spanning cardiology and endocrinology) may produce inferior responses
  compared to frontier cloud models.
- Model storage overhead: a Q4_K_M 7B model file is approximately 4 GB. Combined with the
  RuVector WASM runtime and vault, this is a meaningful storage footprint. Model packaging
  and partial loading strategies are required.
- Latency on older or lower-end devices: Tier 1 inference on CPU without NPU may be slow
  (5–15 seconds per response on older mid-range hardware). Graceful degradation (shorter
  context windows, smaller model variants, or a soft prompt to offer cloud escalation)
  required for these devices.
- ExecuTorch and ruvLLM WASM are fast-moving substrates; pin versions and track upstream
  releases closely (same vendor risk note as Ruflo/RuVector in the broader project).

### Mitigations

- Streaming token output (display tokens as they arrive) makes latency perception better
  even on slower devices.
- Model selection at first-run: profile the device's inference speed and auto-select the
  best model variant (3B vs 7B) for the available RAM and NPU.
- Explicit user option to "always prefer on-device" or "always ask before cloud" — the
  latter being the default and the former being a stricter privacy setting.
- Darwin Mode (ADR-018) learns the Tier 1/Tier 2 routing boundary from real usage:
  over time, the router gets better at knowing which questions Tier 1 handles well.

---

## Open Questions

1. **Model licensing for health data**: Health-domain fine-tuned models (HuatuoGPT,
   BioMistral) may carry academic or research-only licenses incompatible with a commercial
   product. Verify license terms for any medical-domain model before inclusion in the
   distribution. Base models (Phi-3-mini, Gemma, Llama 3) have commercial licenses; verify
   health fine-tunes separately.

2. **BAA requirement for cloud Tier 2**: Even with explicit user consent, if a cloud
   model provider processes health information on behalf of a Helix user, does this create
   a HIPAA Business Associate relationship for Helix (if Helix were a covered entity) or
   for the model provider? What BAA-equivalent agreement does the provider need to offer?
   Engage privacy counsel before enabling Tier 2 for US users.

3. **Minimum device spec floor**: What is the minimum device spec at which Tier 1 inference
   is usable (< 5s first-token)? Is there a floor below which only Tier 0 is offered,
   with a graceful prompt to cloud escalation? Define the device capability floor before
   shipping Tier 1.

4. **Darwin Mode and model routing**: When Darwin Mode (ADR-018) evolves the Tier 1/Tier 2
   routing boundary, does it need to re-validate that cloud escalation consent is still
   surfaced for all Tier 2 invocations? Yes — this must be a hard constraint in the Darwin
   fitness function, not an evolved variable.

5. **Cognitum Seed model updates**: The Seed's on-device signal processing models are
   OTA-updatable (ADR-014). What is the update signature and verification model? Ed25519
   witness-signing (consistent with ADR-017 MetaHarness) should be applied to Seed
   firmware and model updates.

---

## References

| # | Source | Evidence | URL |
|---|--------|----------|-----|
| 1 | Octomil: On-Device LLM Inference — The Definitive 2025–2026 Guide | [A] | https://docs.octomil.com/blog/on-device-llm-inference-2025-2026/ |
| 2 | daily.dev: Running LLMs Locally in 2026 — Ollama, llama.cpp | [B] | https://daily.dev/blog/running-llms-locally-ollama-llama-cpp-self-hosted-ai-developers/ |
| 3 | V. Chandra: On-Device LLMs State of the Union, 2026 | [B] | https://v-chandra.github.io/on-device-llms/ |
| 4 | arXiv: WebLLM — High-Performance In-Browser LLM Inference Engine (2024) | [A] | https://arxiv.org/pdf/2412.15803 |
| 5 | arXiv: Optimizing LLMs Using Quantization For Mobile Execution (2024) | [A] | https://arxiv.org/html/2512.06490v1 |
| 6 | arXiv: Survey of Small Language Models (2024) | [B] | https://arxiv.org/pdf/2410.20011 |
| 7 | Casper: Prompt Sanitization for Protecting User Privacy in Web-Based LLMs | [B] | https://arxiv.org/pdf/2408.07004 |
| 8 | HHS.gov: HIPAA Business Associate Guidance | [A] | https://www.hhs.gov/hipaa/for-professionals/privacy/guidance/business-associates/index.html |
| 9 | FDA 2026 CDS guidance: transparency requirements for AI/ML recommendations | [A] | https://www.faegredrinker.com/en/insights/publications/2026/1/key-updates-in-fdas-2026-general-wellness-and-clinical-decision-support-software-guidance |
| 10 | AccountableHQ: HIPAA covered entity definition and Business Associate scope | [B] | https://www.accountablehq.com/post/hipaa-covered-entity-definition-45-cfr-160-103-plain-english-guide-with-exclusions-and-edge-cases |
