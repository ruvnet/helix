# ADR-023: Semantic Retrieval over the Health Graph (RuVector HNSW/GraphRAG)

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-003 (RuVector memory/graph), ADR-005 (grounded answering), ADR-008 (verifier), ADR-006 (abstention)

---

## Context

ADR-005 makes retrieval the foundation of grounded answering: the analyst may
only compose from the records the **Retrieve** step actually pulled. So far the
pipeline (`helix-core`) is handed a pre-filtered record set by concept code — an
exact match. That misses the connective reasoning the product promises: *"why am
I tired?"* should surface ferritin **and** deep-sleep **and** vitamin D, which
share no code, only a clinical relationship.

**[ruvnet/ruvector](https://github.com/ruvnet/ruvector)** is exactly this engine:
**HNSW** vector search, GraphRAG, and GNN relationship reasoning over
provenance-tagged facts (topics: `vector`, `gnn`, `graph`, `mincut`, `llm-inference`).
ADR-003 already names it the analytic layer; ADR-023 specifies the **retrieval
contract** Helix builds on top of it. **[A]**

The retrieval layer must not become a hallucination vector itself: similarity is
*recall*, not *grounding*. A semantically-retrieved record is still subject to
the ADR-005 grounding gate and the ADR-008 verifier — retrieval only decides
*what the analyst is allowed to look at*, never what it may assert.

## Decision

Add a **semantic retrieval layer** (`helix-retrieval`) that selects the candidate
record set for the analyst, with these guarantees:

1. **Recall, then ground.** Retrieval returns a ranked candidate set (by vector
   similarity + graph proximity); every candidate still passes the ADR-005
   grounding gate before it can back a claim. Retrieval widens what's *considered*,
   never what's *asserted*.
2. **Deterministic, explainable scoring.** Each candidate carries its similarity
   score and the *reason* it was retrieved (direct concept match, vector
   neighbour, or graph-linked) so the Retrieve step is auditable (ADR-005) and the
   Verifier (ADR-008) can see why a record is in scope.
3. **Provenance-preserving.** Retrieval operates over `ProvRecord`s and never
   strips provenance; the embedding is computed from the record, the record
   travels with the result.
4. **Bounded & recency-aware.** `top_k` caps the candidate set; ties and ranking
   incorporate recency so a fresh value outranks a stale one of equal similarity
   (consistent with ADR-006 staleness handling).
5. **Engine injected.** The actual vector index (RuVector HNSW on device/WASM) is
   behind an `Embedder` + `Index` trait; this crate owns the *contract and the
   ranking/explanation policy*, so it is pure and fully testable with a stub
   embedder. RuVector plugs in at the edge.

## Alternatives Considered

- **Exact concept-code retrieval only (status quo).** Rejected: misses
  cross-concept relationships, which is the product's core value (§4 graph-aware
  reasoning).
- **Let the LLM pick relevant records from the whole dossier.** Rejected: that
  re-introduces the failure ADR-005/007 prevent (the model selecting/ignoring
  data unaccountably); deterministic retrieval + explicit scores is auditable.
- **Pure vector similarity, no graph.** Rejected as insufficient: clinical
  relationships (biomarker↔symptom↔medication) are graph edges RuVector can
  traverse; ignoring them yields spurious or missing associations.

## Consequences

**Positive.** Cross-concept, relationship-aware recall (the "connect the dots"
the product is for); every candidate explainable and still grounded/verified;
runs on-device via RuVector WASM (ADR-013).

**Negative.** Embedding quality and graph-edge curation drive recall quality;
similarity can surface spurious neighbours (handled by the downstream gate, but
adds verifier load); `top_k` tuning trades recall vs. cost.

**Mitigations.** The grounding gate (ADR-005) + verifier (ADR-008) are the
backstop — retrieval can be permissive because assertion is strict; surface the
retrieval reason in the audit trail; tune `top_k` per query criticality (ADR-019
cost routing).

## Open Questions

- Embedding model for health records on-device (dimensionality vs. WASM size).
- Graph-edge construction policy (which biomarker↔condition↔med edges, and from
  what evidence) — overlaps ADR-003 schema work.
- Fusion weighting of vector similarity vs. graph proximity vs. recency.

## References

- ruvnet/ruvector — HNSW / GraphRAG / GNN reasoning (repo topics + ADRs). **[A]**
- Helix ADR-003 (RuVector analytic layer), ADR-005 (grounded answering), ADR-008 (verifier), ADR-006 (staleness). **[A]**

> Architectural/product guidance, not legal or medical advice. Retrieval is recall; grounding and verification remain strict.
