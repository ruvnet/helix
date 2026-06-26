# Helix — Architecture Decision Records

This directory holds the load-bearing architecture decisions for **Helix — Personal
Health Intelligence (PHI)**, a private, local-first, anti-hallucination personal health
intelligence platform built on the ruvnet stack (Ruflo + RuVector + Cognitum Seed +
MetaHarness/Darwin).

Each ADR follows **Context → Decision → Alternatives Considered → Consequences → Open
Questions → References**. Factual and technical claims carry an inline evidence grade —
**[A]** strong/primary · **[B]** secondary/reporting · **[C]** inferred/weak — and the
regulatory content is architectural/product guidance, **not legal or medical advice**.

The product spec these decisions implement is [`../Helix-PHI-ADR-Product-Spec.md`](../Helix-PHI-ADR-Product-Spec.md).

> **A note on cross-references.** ADRs cross-reference each other by their native number
> (e.g. "see ADR-005"). A handful of citations point at the **upstream `ruvnet/ruvector`
> repository's** ADRs (e.g. *RuVector ADR-028*, *ADR-252*, *ADR-256*, *ADR-150*) — these are
> external substrate citations to the platform Helix is built on, not Helix-internal ADRs.

## Index

| ADR | Title | Theme |
|-----|-------|-------|
| [001](ADR-001-user-owned-local-first-vault.md) | User-Owned, Local-First Encrypted Health Vault | Privacy / data ownership |
| [002](ADR-002-ruflo-orchestration-meta-harness.md) | Ruflo as the Orchestration Meta-Harness | Platform |
| [003](ADR-003-ruvector-health-knowledge-graph.md) | RuVector as Memory & Personal Health Knowledge Graph | Platform |
| [004](ADR-004-canonical-ontology-normalization.md) | Canonical Ontology Normalization (LOINC/RxNorm/SNOMED/UCUM/FHIR) | Platform |
| [005](ADR-005-retrieval-grounded-provenance-answering.md) | Retrieval-Grounded, Provenance-Required Answering | Anti-hallucination (core) |
| [006](ADR-006-evidence-tiering-abstention.md) | Evidence Tiering & Explicit Abstention Policy | Anti-hallucination (core) |
| [007](ADR-007-deterministic-numeric-trend-engine.md) | Deterministic Numeric/Trend Engine | Anti-hallucination (core) |
| [008](ADR-008-verifier-critic-swarm-consensus.md) | Verifier/Critic Agent & Swarm Consensus for Clinical Outputs | Anti-hallucination (core) |
| [009](ADR-009-red-flag-escalation-clinician-in-loop.md) | Red-Flag Escalation & Clinician-in-the-Loop | Clinical safety |
| [010](ADR-010-wellness-vs-samd-boundary.md) | Wellness Positioning vs. SaMD Regulatory Boundary | Regulatory |
| [011](ADR-011-federation-pii-stripped-cohort.md) | Federation for Opt-In, PII-Stripped Cohort Intelligence | Privacy / network effects |
| [012](ADR-012-connector-abstraction-graceful-degradation.md) | Connector Abstraction with Graceful Degradation | Platform / ingestion |
| [013](ADR-013-on-device-inference.md) | On-Device Inference Where Feasible | Privacy / compute |
| [014](ADR-014-ambient-sensing-cognitum-seed.md) | Ambient Passive Sensing via the Cognitum Seed (mmWave) | Sensing |
| [015](ADR-015-visual-3d-digital-twin.md) | Visual Health-Intelligence Layer (3D Anatomical Digital Twin) | Visual |
| [016](ADR-016-composite-health-score.md) | Composite 0–100 Health Score — Transparent, Decomposable | Visual |
| [017](ADR-017-mint-branded-harness-metaharness.md) | Mint Helix as a Branded Harness via MetaHarness | Self-optimization |
| [018](ADR-018-darwin-mode-faithfulness-fitness.md) | Darwin Mode Self-Optimization with Faithfulness Fitness | Self-optimization |
| [019](ADR-019-cost-aware-model-routing.md) | Cost-Aware Model Routing Under Privacy Constraints | Self-optimization |
| [020](ADR-020-wifi-csi-ambient-sensing.md) | WiFi-CSI Contactless Ambient Sensing (RuView backend) | Sensing / integration |
| [021](ADR-021-genome-ingestion-rvdna.md) | User-Owned Genome Ingestion & Pharmacogenomics (rvDNA backend) | Genomics / integration |
| [022](ADR-022-ocr-lab-ingestion.md) | OCR Lab-Document Ingestion (RuVector OCR backend) | Ingestion / integration |
| [023](ADR-023-semantic-retrieval.md) | Semantic Retrieval over the Health Graph (RuVector HNSW/GraphRAG) | Retrieval / integration |
| [024](ADR-024-privacy-preserving-cohort.md) | Privacy-Preserving Cohort Feature Extraction (federation primitive) | Privacy / federation |
| [025](ADR-025-visual-rag-medical-documents.md) | Visual RAG over Medical Documents & Images (rupixel backend) | Visual retrieval / integration |
| [026](ADR-026-on-device-llm-analyst.md) | On-Device LLM Analyst — Grounded Compose Step (local GPU, ruvLLM) | LLM / on-device |
| [027](ADR-027-learned-text-embeddings.md) | Learned MiniLM Text Embeddings for Semantic Retrieval (local GPU) | Embeddings / on-device |
| [028](ADR-028-learned-visual-encoder.md) | Learned Visual Encoder for Medical-Document Retrieval (local GPU) | Visual encoder / on-device |
| [029](ADR-029-connector-clients.md) | Live Connector Clients — FHIR/SMART + Wearables (Rust, sandbox-first) | Connectors / ingestion |
| [030](ADR-030-federation-transport.md) | Federation Transport — Opt-In Cohort Contribution (Rust, privacy-gated) | Federation / privacy |
| [031](ADR-031-longitudinal-health-score-timeline.md) | Longitudinal Health-Score Timeline | Dashboard / visual |
| [032](ADR-032-evidence-based-focus-areas.md) | Evidence-Based "Focus Areas" & Vitals Panel (non-diagnostic) | Dashboard / clinical safety |
| [033](ADR-033-dashboard-updates-recommendations.md) | Dashboard Updates & Recommendations (evidence-tiered, grounded) | Dashboard / anti-hallucination |
| [034](ADR-034-biological-age-estimate.md) | Biological / Medical Age Estimate from Routine Labs | Dashboard / biomarkers |
| [035](ADR-035-darwin-parameter-evolution.md) | Darwin-Style Parameter Evolution (safety-frozen) | Self-optimization / safety |

## Status

All 35 ADRs are **Proposed** (v1.0.0). They are derived from the v1.0.0 product spec and
grounded by multi-source research; they have not yet been ratified against an
implementation or reviewed by regulatory counsel / a clinical advisory board.
