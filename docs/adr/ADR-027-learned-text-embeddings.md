# ADR-027: Learned MiniLM Text Embeddings for Semantic Retrieval (local GPU)

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-023 (semantic retrieval), ADR-003 (RuVector), ADR-013 (on-device inference), ADR-005 (grounding)

---

## Context

ADR-023 built the semantic-retrieval *contract* (`helix-retrieval`) with an
injected `Embedder` trait, but the reference embedder was a placeholder — the
real recall quality depends on a learned text encoder. The local **GPU** now
makes a real one feasible on-device.

The ruvnet stack already standardizes on **all-MiniLM-L6-v2** (384-dim) for
semantic embeddings (RuVector's ONNX MiniLM; ADR-210 upstream). Running it
locally on the GPU keeps health text on-device (ADR-013) while giving
`helix-retrieval` genuine semantic recall — so "why am I tired?" pulls
fatigue-related records by *meaning*, not just shared concept codes.

## Decision

Add `helix-embed`: a learned text embedder behind a trait, wired into
`helix-retrieval`.

1. **all-MiniLM-L6-v2, 384-dim, on-device.** The default backend is the local
   GPU MiniLM (served via ollama's embeddings endpoint today; the in-stack ruvLLM
   / RuVector ONNX-MiniLM / `ruv_embedder` paths plug in behind the same trait).
   No text leaves the device (ADR-013).
2. **Embeddings are recall, not grounding.** The vectors decide *what the analyst
   may look at* (ADR-023); every retrieved record still passes the ADR-005
   grounding gate. A semantic neighbour is never, by itself, a claim.
3. **Pluggable + degradation-safe.** `TextEmbedder` is a trait (deterministic
   stub in tests, GPU backend in prod). A `helix_retrieval::Embedder` adapter
   embeds queries and record text; on backend failure it degrades to an empty
   vector (no spurious matches) rather than erroring the whole answer.
4. **Cosine on normalized vectors.** Similarity is cosine; record text is a
   compact, provenance-preserving rendering (`concept value unit`), never raw PHI
   beyond what's already in the record.

## Alternatives Considered

- **Keep the reference embedder.** Rejected: weak recall; the GPU makes a real
  encoder cheap and on-device.
- **Cloud embedding API.** Rejected as default: sends health text off device
  (ADR-013); allowed only consented + PII-gated (ADR-019).
- **A bigger LLM embedding model.** Deferred: MiniLM-384 is the stack standard,
  fast, and sufficient; a larger encoder plugs in behind the trait if needed.

## Consequences

**Positive.** Real semantic recall on-device at $0 cloud cost; standardized on the
ruvnet MiniLM-384; drops straight into `helix-retrieval` without changing its
contract; the grounding gate keeps it honest.

**Negative.** Embedding adds latency vs. exact match; depends on a local model
being served; recall quality is bounded by MiniLM.

**Mitigations.** Batch + cache embeddings; the ANN (RuVector HNSW) caps
candidates; degrade safely on backend failure; swap a stronger encoder behind the
trait if eval demands.

## Open Questions

- In-process ONNX (ort/candle, true single-process GPU) vs. the HTTP embeddings
  endpoint — the trait makes this swappable; pick per deployment.
- Embedding caching + invalidation as records change.
- Record-text rendering for best recall without leaking more than the record holds.

## References

- all-MiniLM-L6-v2 (sentence-transformers, 384-dim); RuVector ONNX MiniLM; ADR-210 upstream. **[A]**
- Helix ADR-023 (retrieval contract), ADR-013 (on-device), ADR-005 (grounding). **[A]**

> Architectural/product guidance, not legal or medical advice. Embeddings drive recall; grounding stays strict.
