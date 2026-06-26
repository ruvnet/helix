# ADR-026: On-Device LLM Analyst — Grounded Compose Step (local GPU)

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-005 (grounding), ADR-007 (deterministic numerics), ADR-008 (verifier), ADR-013 (on-device inference), ADR-019 (routing), ADR-002 (FM-analyst)

---

## Context

Through ADR-005/007/008 the pipeline produces a **GroundedAnswer**: grounded
claims (every value cited), deterministically-computed trend facts, and an
escalation verdict. Until now the analyst's natural-language *composition* — the
plain-English phrasing a person reads — has been a deterministic template (a
seam). The spec always intended a real LLM here (ADR-002 FM-analyst), but
*strictly as a narrator of already-grounded facts*, never as a reasoner that
retrieves or computes (ADR-005/007 forbid that).

The local machine now has a **GPU (RTX 5080, 16 GB, CUDA 13)** running
**ollama** with `qwen2.5-coder` models on an OpenAI-compatible endpoint. That
makes a real on-device LLM analyst achievable **without sending any health data
to the cloud** (ADR-013) — the model runs locally on the GPU.

The danger is obvious: an LLM is exactly the component that hallucinates. So the
integration must make it structurally impossible for the LLM to add information.

## Decision

Add `helix-llm`: an **on-device LLM narrator** for the compose step, behind hard
anti-hallucination constraints.

1. **Facts in, prose out — never reasoning.** The LLM receives the *finished*
   grounded facts (claims, computed trend, citations) and is instructed to
   **restate only those facts** in calm plain language. It does not retrieve
   (ADR-005), does no arithmetic (ADR-007), and is given no raw data to reason over.
2. **Number-guard backstop.** After generation, every numeric token in the
   output must already appear in the input facts. If the model introduces a
   number that isn't in the facts, the output is **rejected** and Helix falls
   back to the deterministic template. The LLM can rephrase; it cannot invent a
   value.
3. **No new claims, no advice beyond the facts.** The system prompt forbids
   diagnoses, recommendations, or any assertion not in the facts; the Verifier
   (ADR-008) still gates clinically meaningful output downstream.
4. **On-device, local GPU, in-stack.** The backend is a trait over any local
   OpenAI-compatible endpoint; the **default is ruvLLM** (the ruvnet-native
   on-device engine ADR-013 names — v2.1.0 serving `Qwen2.5-3B-Instruct` on the
   RTX 5080), with **ollama as a fallback**. No health data leaves the device
   (ADR-013). The cloud-frontier escalation path (ADR-019, consent + PII-gate)
   plugs in behind the same trait for the hardest narration only.
5. **Deterministic by default.** Temperature 0 for reproducibility; the narrator
   is a phrasing layer, not a creative one.

## Alternatives Considered

- **Keep the deterministic template only.** Rejected: robotic phrasing; the spec
  wants a real analyst voice. (The template stays as the guard's fallback.)
- **Let the LLM read the dossier and answer freely.** Rejected outright — that is
  the exact hallucination failure ADR-005/007/008 exist to prevent.
- **Cloud LLM.** Rejected as default: sends health data off device (ADR-013).
  Allowed only as a consented, PII-gated escalation (ADR-019).

## Consequences

**Positive.** A real, fluent analyst voice at $0 cloud cost, fully on-device on
the GPU; the number-guard makes fabricated values impossible to surface; the
deterministic pipeline remains the source of truth.

**Negative.** LLM latency (hundreds of ms on local GPU) vs. an instant template;
the guard can reject good outputs (then we fall back, which is safe); model
quality varies; GPU/VRAM is a resource.

**Mitigations.** Temperature 0 + a tight prompt minimize rejection; the fallback
is always safe; ADR-019 routes only when a local model suffices; cache narrations.

## Open Questions

- The exact number-guard tokenizer (units, ranges, dates) — start strict, relax
  with evidence.
- Which model tier per narration (qwen 7b/14b/32b) under ADR-019 cost routing.
- How narration caching interacts with the emergent-time memory decay.

## References

- ollama OpenAI-compatible local inference; `qwen2.5-coder` on RTX 5080 (CUDA 13). **[A]**
- Helix ADR-005 (grounding), ADR-007 (deterministic numerics), ADR-008 (verifier), ADR-013 (on-device). **[A]**

> Architectural/product guidance, not legal or medical advice. The LLM narrates grounded facts; it never diagnoses or invents.
