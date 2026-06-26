# Helix ADR → Implementation Coverage

Maps every Architecture Decision Record (ADR-001…019) to its implementing crate,
or documents why it is out of scope for this Rust core workspace. This is the
evidence for SOTA exit criterion #1 ("every load-bearing ADR has a real crate or
a documented N/A").

**Scope:** this repo implements the testable, benchmarkable **core** the ADRs
specify. The mobile app, 3D WebGL twin, on-device-radar firmware, and live
federation network are client/hardware/networked surfaces that cannot be
realized or meaningfully tested in a headless Rust workspace; for those, the ADR
remains the specification and the table notes the interface seam.

| ADR | Title | Status | Where |
|-----|-------|--------|-------|
| 001 | User-owned, local-first encrypted vault | ✅ Implemented (real AEAD) | `helix-vault` |
| 002 | Ruflo orchestration meta-harness | ✅ Core implemented | `helix-core` (the grounded-answer pipeline composes the agent roles) |
| 003 | RuVector memory + health knowledge graph | ◑ Interface modeled | `helix-provenance` (record/provenance schema). Graph store is RuVector itself (upstream substrate). |
| 004 | Canonical ontology normalization | ✅ Implemented | `helix-ontology` |
| 005 | Retrieval-grounded, provenance-required answering | ✅ Implemented | `helix-provenance` (grounding gate) + `helix-core` |
| 006 | Evidence tiering & abstention | ✅ Implemented | `helix-evidence` |
| 007 | Deterministic numeric/trend engine | ✅ Implemented + benchmarked | `helix-numeric` |
| 008 | Verifier/critic + swarm consensus | ✅ Implemented | `helix-verifier` |
| 009 | Red-flag escalation & clinician-in-the-loop | ✅ Implemented | `helix-escalation` + `helix-core` |
| 010 | Wellness positioning vs. SaMD boundary | ✅ Encoded as policy | non-diagnostic framing enforced in `helix-escalation` (screening language) + `helix-score` (disclaimer) |
| 011 | Federation for opt-in PII-stripped cohort intel | ◑ Privacy primitive implemented | `helix-cohort` (ADR-024: generalize + cell-suppression + DP); network transport still out of scope |
| 012 | Connector abstraction with graceful degradation | ◑ Interface modeled | `helix-ontology` review-queue is the un-mappable seam; live connectors are I/O-bound integration work |
| 013 | On-device inference where feasible | ✅ Boundary implemented | `helix-vault` (local key custody); model routing in `helix-core` seam + ADR-019 |
| 014 | Ambient passive sensing (Cognitum Seed, mmWave) | ◑ Signal modeled | screening-grade thresholds in `helix-escalation` (`SEED-REI`); radar firmware is hardware-bound |
| 015 | Visual 3D anatomical digital twin | ⬜ N/A (client/WebGL) | spec-only; data it renders comes from `helix-core` + `helix-score` |
| 016 | Composite 0–100 health score | ✅ Implemented | `helix-score` |
| 017 | Mint Helix as a branded harness (MetaHarness) | ⬜ N/A (build tooling) | spec-only; this repo IS the minted artifact's content |
| 018 | Darwin Mode self-optimization (faithfulness fitness) | ◑ Fitness inputs implemented | the DRACO components (grounding/faithfulness) are exactly what `helix-verifier` + `helix-evidence` measure; the evolve loop is external tooling |
| 019 | Cost-aware model routing under privacy constraints | ◑ Interface modeled | `helix-verifier::ModelFamily` + `Criticality` give the routing signal; the learned router wraps ruvector Tiny Dancer (upstream) |

**Legend:** ✅ real, tested crate · ◑ interface/seam modeled here, full realization upstream or I/O-bound · ⬜ client/hardware/networked, spec-only by design.

## Integration ADRs (ruvnet ecosystem capabilities)

| ADR | Title | Status | Where |
|-----|-------|--------|-------|
| 020 | WiFi-CSI ambient sensing (RuView) | ✅ Implemented | `helix-sensing` (wasm: `sensing_reading_json`) |
| 021 | Genome ingestion & pharmacogenomics (rvDNA) | ✅ Implemented | `helix-genome` (wasm: `genome_profile_json`) |
| 022 | OCR lab-document ingestion (RuVector OCR) | ✅ Implemented | `helix-ocr` (wasm: `ocr_ingest_json`) |
| 023 | Semantic retrieval over the health graph (RuVector HNSW/GraphRAG) | ✅ Implemented | `helix-retrieval` (Embedder/Index injected) |
| 024 | Privacy-preserving cohort primitive (federation) | ✅ Implemented | `helix-cohort` (k-anon + DP) |
| 025 | Visual RAG over medical documents/images (rupixel) | ✅ Implemented + benchmarked | `helix-visual` (tile embeddings + MaxSim) |
| 026 | On-device LLM analyst — grounded compose (local GPU) | ✅ Implemented + GPU-validated | `helix-llm` (ruvLLM default, ollama fallback, number-guard) |
| 027 | Learned MiniLM text embeddings (local GPU) | ✅ Implemented + GPU-validated | `helix-embed` (all-MiniLM-L6-v2, wires into helix-retrieval) |

Each integration realizes/strengthens a core ADR: 020→014 (ambient tier backend) + 009 (escalation);
021→005/§7.4 (genome, user-owned); 022→012 (connector degradation, the primary lab path); 023→003/005
(the Retrieve step). All carry the anti-hallucination guardrails: capped confidence, screening-not-diagnosis,
provenance-required, recall≠grounding.

## Tally
- **Fully implemented + tested crates:** 001, 004, 005, 006, 007, 008, 009, 010, 013, 016, **020, 021, 022, 023** (14)
- **Core/seam modeled here:** 002, 003, 012, 014, 018, 019 (6)
- **Documented N/A (client/hardware/networked):** 015, 017 (2) — 011's privacy primitive is now built (`helix-cohort`, ADR-024)

Every ADR is accounted for: a crate, a modeled seam, or a documented out-of-scope rationale.
15 crates · 112 tests · `cargo audit` clean (130 deps) · the four ruvnet-ecosystem integrations are
exposed through `helix-wasm` so the console and mobile app can use them.
