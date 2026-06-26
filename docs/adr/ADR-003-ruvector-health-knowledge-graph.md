# ADR-003: RuVector as Memory and Personal Health Knowledge Graph

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001, ADR-002, ADR-004, ADR-005, ADR-007, ADR-008, ADR-012, ADR-013

---

## Context

### What grounded health answering requires from memory

Helix's anti-hallucination guarantee (ADR-005) rests on a single architectural commitment: every factual claim in an answer must resolve to a stored datum with source, timestamp, units, reference range, and confidence. This demands a memory substrate with two distinct capabilities that most data stores provide separately:

1. **Vector retrieval**: fuzzy semantic search over embedded health facts — "find the recent lab results semantically related to iron metabolism" — with sub-millisecond latency and provenance metadata intact.
2. **Graph reasoning**: traversal of explicit typed relationships — "trace from the ferritin value to the condition (iron-deficiency anemia) to its associated symptoms to the relevant medications" — to enable the kind of multi-hop reasoning a functional-medicine clinician applies.

Neither capability alone is sufficient. A pure vector store answers similarity queries but cannot traverse typed relationships or enforce ontological constraints. A pure knowledge graph is expressive but too rigid for embedding-based retrieval of free-text subjective logs, clinical notes, and wearable-signal descriptions. **[A]**

### The temporal dimension

Health data is inherently longitudinal. A single ferritin value means little; a trend of three quarterly draws with a downward slope is clinically significant. An HbA1c of 5.8% matters differently depending on whether it is rising or falling. The memory substrate must treat every metric as a **time series** and make temporal reasoning — slopes, deltas, change-points, reference-range crossing moments — a first-class operation, not an afterthought computed at query time over raw data. **[A]**

### Provenance as a first-class citizen

The provenance model is not a metadata tag on a record. It is the mechanism by which every answer can be traced back to its source. Each stored fact must carry:
- **Source identifier**: connector ID, system name, and feed version.
- **Collection timestamp**: when the value was measured or recorded (not when it was ingested).
- **Measurement method and units**: UCUM-normalized (ADR-004).
- **Reference range**: population-standard or personalized, with source and date.
- **Confidence score**: a [0,1] float reflecting the reliability of the measurement (e.g., wearable HRV < clinic ECG HRV).
- **Evidence tier**: Tier 1–4 per ADR-006.
- **FHIR resource type and ID**: enabling export to a FHIR R4/R5 bundle for clinician handoff.

Without this, the Verifier/Critic agent (ADR-008) has no ground truth to check claims against.

### Why RuVector

RuVector is a Rust-native, multi-substrate vector + graph memory database with a WASM path for on-device deployment. Benchmarks from the ruvector codebase confirm:
- HNSW search k=10 on 384-dim vectors: **61µs p50** on ARM64 (Apple M2 NEON); **72µs p50** on x86_64 (AVX2). **[A — directly measured]**
- 16,400 QPS on a single ARM64 node for k=10 search.
- Tiered quantization: 1× (f32 hot), 4× (SQ8 warm), 16× (PQ cool), 32× (binary cold) with automatic access-pattern-driven tier migration.
- GraphSAGE and GCN forward passes in-database (ADR-028 demonstrates this in a 50M-patient eHealth context).
- WASM path available with memory-only storage and scalar fallback — enabling on-device retrieval with no network call (ADR-013).
- PostgreSQL extension available for deployments requiring SQL-level access and RLS-based multi-tenancy.
- RDF triple store with in-database SPARQL for ontology-level cross-mapping.
- REDB-backed persistent storage on native targets with ACID transactions, memory-mapped vectors, and path-traversal-protected file I/O.

For Helix specifically, the single-codebase, cross-platform nature of RuVector means the health vault can run on:
- A mobile device (WASM + memory-only, for on-device inference, ADR-013).
- A local server or NAS in the user's home (native with REDB persistence).
- A future optional encrypted cloud sync tier (RuVector-Postgres with per-user RLS).

The WASM path is the key differentiator against any Pythonic or cloud-native vector DB — it is the only path that satisfies ADR-001's local-first, user-owned-vault guarantee without shipping a native daemon. **[A]**

---

## Decision

### Use RuVector as the unified memory substrate and health knowledge graph engine

The Helix health vault is implemented on top of five logical RuVector subsystems that share a common identity and provenance model:

#### 1. HNSW vector store (semantic embedding layer)

**What it holds**: embedded representations of every health fact — lab result, clinical note chunk, medication event, wearable data point, subjective log entry, Cognitum Seed vital signal.

**Embedding model**: `all-MiniLM-L6-v2` (384 dimensions, ONNX, runs on-device) for general health text; `BioClinicalBERT-384` for clinical notes where MIMIC-III pre-training yields better recall on medical terminology. Both produce 384-dim vectors matching the HNSW index dimension.

**Index configuration**:
```
dimensions: 384
metric: cosine
m: 24                  # higher than default 16; optimizes medical-recall > 0.95
ef_construction: 200
ef_search: 100
max_elements: 10_000_000   # 10M for a single user over 10+ years
```

**What the vector layer enables**:
- "Retrieve facts semantically related to iron metabolism" — top-k cosine search over the embedded fact space.
- "Find clinical notes that mention fatigue in the last 6 months" — time-filtered HNSW search.
- "Find the most similar past health patterns to my current ferritin trajectory" — trajectory embedding similarity.
- Provenance metadata is stored alongside each vector entry (source, timestamp, LOINC/RxNorm/SNOMED codes, reference range, confidence, FHIR resource ID).

#### 2. Time-series index (longitudinal numeric layer)

**What it holds**: per-metric sorted arrays of (timestamp, value, unit, source_id) tuples for every quantitative health measurement.

**Structure**: a secondary sorted index maintained by RuVector alongside the HNSW entries, keyed by canonical metric identifier (LOINC code + UCUM unit):

```
metric_ts_index[loinc:3016-3:uIU/mL] = [
    (2025-03-10, 2.1, uIU/mL, quest-lab-001),
    (2025-09-15, 1.8, uIU/mL, quest-lab-001),
    (2026-04-10, 3.4, uIU/mL, labcorp-002),
]
```

**What the time-series layer enables** (all computed deterministically by the Trend/Numeric agent, ADR-007 — the LLM never does this arithmetic):
- Slope computation: linear regression over the N most recent draws.
- Delta computation: percentage change between any two draws.
- Change-point detection: structural break detection using PELT or BOCPD algorithms.
- Reference-range crossing detection: when did the value cross the upper or lower bound?
- Rolling statistics: 30/90/365-day mean, min, max, SD.
- Correlation computation: Pearson/Spearman between two metric time-series.

#### 3. GNN / GraphRAG health knowledge graph

**What it holds**: the typed, directed health knowledge graph linking entities and relationships across all health domains.

**Entity types** (nodes):
```
Biomarker        { loinc_code, name, system, unit, reference_range, body_system }
Condition        { snomed_code, icd10_code, name, status, onset_date }
Medication       { rxnorm_code, ndc_code, name, dose, route, frequency, status }
Supplement       { name, rxcui_if_known, dose, form, status }
Intervention     { type, description, start_date, end_date, outcome }
WearableSignal   { device_type, metric_type, sample_rate, unit }
SubjectiveState  { type [mood, energy, symptom, pain], severity, onset, notes }
GenomicVariant   { rsid, gene, allele, clinical_significance }
BodySystem       { name, snomed_code }
LabResult        { loinc_code, value, unit, collected_at, ordering_provider }
ClinicalEvent    { type [encounter, immunization, procedure], date, provider, facility }
```

**Relationship types** (edges, directed, typed):
```
BIOMARKER_INDICATES_CONDITION     (strength: [0,1], evidence_tier)
CONDITION_AFFECTS_BIOMARKER
MEDICATION_TREATS_CONDITION
MEDICATION_INTERACTS_WITH_MEDICATION (severity: [minor, moderate, major, contraindicated])
SUPPLEMENT_AFFECTS_BIOMARKER
INTERVENTION_CHANGES_BIOMARKER    (direction: [up, down, stabilizes], lag_days)
WEARABLE_CORRELATES_WITH_BIOMARKER (r_value, p_value, lag_days)
SUBJECTIVE_CORRELATES_WITH_WEARABLE
GENOMIC_VARIANT_PREDISPOSES_TO_CONDITION (penetrance, evidence_grade)
BIOMARKER_BELONGS_TO_BODY_SYSTEM
CONDITION_INVOLVES_BODY_SYSTEM
```

**How it is built**: the graph is constructed incrementally by the Normalization agent (ADR-002) as facts arrive. Population-level edges (e.g., "low ferritin BIOMARKER_INDICATES_CONDITION iron-deficiency anemia") are seeded from curated clinical-knowledge sources at initialization. User-specific edges are added as the system observes correlations (e.g., "low deep-sleep CORRELATES WITH elevated resting HR with r=0.67 over 90 days") — these are clearly labeled as user-specific Tier-1 evidence, not population generalizations.

**Reasoning operations**:
- **Multi-hop traversal**: "Given ferritin=28 (low), traverse BIOMARKER_INDICATES_CONDITION → iron-deficiency anemia → CONDITION_AFFECTS_BIOMARKER → which other biomarkers should be checked?"
- **Interaction detection**: "Are any of the user's active medications connected by MEDICATION_INTERACTS_WITH_MEDICATION with severity ≥ moderate?"
- **Pathway reasoning**: "What is the shortest path from 'chronically elevated cortisol' to 'reduced bone density' through the health graph?"
- **GNN-enhanced embeddings**: GraphSAGE forward passes refine node embeddings by aggregating neighborhood context, so a biomarker's embedding reflects not just its own values but its graph neighborhood (conditions it indicates, medications that affect it). This enables "find biomarkers in a similar clinical position to ferritin for this patient" via embedding similarity over graph-refined vectors.

**GraphRAG retrieval for the FM Analyst**:
When the FM Analyst receives a question, it issues a GraphRAG retrieval that combines:
1. HNSW cosine search for semantically similar facts (dense retrieval).
2. Keyword/LOINC/SNOMED matching for exact code lookup (sparse retrieval).
3. Graph traversal from the top-k retrieved nodes to expand context by 1–2 hops (relationship-grounded context).
4. RRF fusion to merge and re-rank all results.

The FM Analyst receives a structured context of provenance-tagged facts — never raw text blobs — which enables the Verifier to re-derive each claim against a discrete datum rather than searching through paragraphs.

#### 4. Numeric engine (deterministic computation layer)

A separate in-process computation layer, accessed by the Trend/Numeric agent, that runs all quantitative health computation as **deterministic code** — not LLM inference. Implemented in Rust, compiled to WASM for on-device use.

Operations:
- `slope(metric_id, window_days, min_points) -> (slope, r2, p_value)`
- `delta_pct(metric_id, from_date, to_date) -> f64`
- `range_crossing_events(metric_id, low, high) -> [(date, direction)]`
- `change_points(metric_id, algorithm) -> [(date, confidence)]`
- `correlate(metric_a, metric_b, lag_days, window) -> (r, p, n)`
- `zscore(value, population_mean, population_sd) -> f64`
- `rolling_stats(metric_id, window_days) -> Stats`

All outputs are tagged with their computation method, input date range, and number of data points — so the FM Analyst can cite them correctly ("resting HR slope: +0.8 bpm/week over 21 days, n=21, r²=0.72").

#### 5. Ontology / FHIR layer (normalization bridge)

RuVector's in-database RDF triple store (demonstrated in ADR-028 for 31.4M medical-ontology triples) holds:
- LOINC term definitions and LOINC-SNOMED mappings.
- RxNorm drug hierarchy and ingredient relationships.
- SNOMED CT concept hierarchy (loaded from IHTSDO-licensed N-Triples export).
- ICD-10-CM code tree with SNOMED crosswalks.
- UCUM unit graph.

SPARQL queries from the Normalization agent resolve incoming raw terms to canonical codes:
```sparql
# Map "TSH" (Quest local code) to LOINC
SELECT ?loinc_code ?ucum_unit
WHERE {
  ?concept skos:altLabel "TSH" .
  ?concept loinc:term_code ?loinc_code .
  ?concept loinc:example_ucum_units ?ucum_unit .
}
```

FHIR R4 resource shapes are stored as type schemas in RuVector metadata, enabling:
- **Inbound**: FHIR resources arriving from SMART on FHIR connectors are mapped to graph nodes directly.
- **Outbound**: any subset of the health graph can be serialized to a FHIR R4 Bundle for clinician handoff, EMR import, or patient-mediated sharing (SMART Health Links).

#### Health graph schema summary

```
[BodySystem] ←── BELONGS_TO ────────────────────────────────┐
     ↑                                                        │
     │ INVOLVES                                               │
[Condition] ←─── INDICATES ──── [Biomarker] ←── AFFECTS ────┘
     ↑                               │
     │ TREATS                        │ CORRELATES_WITH
[Medication] ──── INTERACTS ────── [WearableSignal]
     │           (drug-drug)              │
     │ AFFECTS                           │ CORRELATES_WITH
[Biomarker] ◄────────────────── [SubjectiveState]
     │
     │ PREDISPOSES (via SNOMED pathway)
[GenomicVariant] ─────────────────────────────────────► [Condition]
```

#### Provenance model (per-fact schema)

Every node and edge in the health graph carries:
```rust
struct HealthProvenance {
    source_connector_id: String,      // "labcorp-api-v2" | "oura-ring-v3" | "cgm-dexcom"
    source_record_id:    String,      // original ID in the source system
    collection_at:       DateTime,    // when the value was measured
    ingested_at:         DateTime,    // when Helix ingested it
    measurement_method:  Option<String>,
    canonical_code:      CanonicalCode,   // LOINC | RxNorm | SNOMED | ICD10
    unit:                Option<UcumUnit>,
    reference_range:     Option<RangeSpec>,   // { low, high, source, date }
    confidence_score:    f32,          // [0,1]
    evidence_tier:       EvidenceTier, // Tier1 | Tier2 | Tier3 | Tier4
    fhir_resource_type:  Option<FhirResourceType>,
    fhir_resource_id:    Option<String>,
    connector_version:   SemVer,
    hash:                [u8; 32],     // SHA-256 of (value + source + collection_at)
}
```

The hash enables tamper detection: if a stored value is modified after ingestion, the hash will not match.

#### WASM on-device path

For mobile deployment (ADR-013), RuVector compiles to WASM32 with:
- Memory-only storage (no REDB; state is in-memory, persisted via RVF export on session end).
- Scalar fallback for distance computation (no AVX2/NEON in WASM).
- The `all-MiniLM-L6-v2` ONNX model runs in-browser/in-app via WASM for embedding generation.
- The numeric engine (slopes, deltas, correlations) is compiled to WASM separately.
- GraphSAGE forward pass is available in WASM but is compute-bounded; for on-device use, pre-computed graph-refined embeddings are cached in the hot tier.

The WASM path means a user's health data is never transmitted to generate a retrieval result — the entire retrieval pipeline (embed query → HNSW search → graph traversal → numeric computation) executes locally.

---

## Alternatives Considered

### Alternative 1: Generic cloud vector database (Pinecone, Weaviate, Qdrant)

Pinecone, Weaviate, or Qdrant as the semantic retrieval layer, with a separate relational database for structured health data.

**Rejected because:**
- All are cloud-resident services — fundamentally incompatible with ADR-001's user-owned, local-first vault. Sending health data to a third-party vector service exposes it to subpoena, data breach, and commercial repurposing.
- No WASM path — cannot run on-device (ADR-013).
- No in-database GNN / GraphRAG — a separate service would be required for graph reasoning, creating another PHI-handling endpoint and a split provenance model.
- Split provenance: a separate vector DB + relational DB + graph DB stack would make it impossible to enforce a single, unified provenance model across all fact types.
- Cost: per-query cloud pricing for every health-data retrieval is antithetical to the "frontier quality at local cost" model.

### Alternative 2: Knowledge graph only (Neo4j, RDF triple store)

Use a dedicated graph database as the sole memory layer; represent all health facts as triples; retrieve via Cypher/SPARQL.

**Rejected because:**
- No vector embedding layer — cannot perform semantic search over clinical notes, subjective logs, or mmWave-derived vital descriptions. These are inherently natural-language and require embedding-based retrieval.
- Embedding search would require a separate vector service, reintroducing the split-provenance and privacy problems.
- Neo4j is Java-based with no WASM path.
- Schema rigidity: a triple store requires all facts to be pre-mapped to a formal ontology before they can be queried. In practice, many health facts arrive in free-form text that needs embedding first, structured second — the Normalization agent handles this progressively.
- RuVector's in-database SPARQL triple store provides the graph expressiveness of a dedicated RDF store while co-locating it with the vector engine in a single privacy boundary.

### Alternative 3: Relational database with pgvector

PostgreSQL + pgvector for vector similarity, with a separate time-series column, and application-level graph logic.

**Rejected because:**
- pgvector does not provide the GNN layer (GraphSAGE, GCN forward passes), which are required for multi-hop reasoning and graph-refined embeddings.
- No WASM path — PostgreSQL does not run on-device.
- Application-level graph logic means PHI-handling graph traversal moves out of the database and into a process that is harder to audit and secure.
- RuVector's PostgreSQL extension (ADR-028) actually provides a superset of pgvector capabilities including GNN, SPARQL, hyperbolic embeddings, and self-healing — so if a PostgreSQL deployment is needed in the future, it is RuVector-Postgres, not pgvector, that is the right choice.

---

## Consequences

### Positive

- **Sub-millisecond retrieval** (61µs p50 for k=10 HNSW search) enables real-time conversational health Q&A.
- **Unified provenance model**: every fact, regardless of source, carries the same structured provenance fields — enabling the Verifier agent to check any claim against its exact source datum.
- **Graph reasoning + vector retrieval in one substrate**: the FM Analyst retrieves multi-hop clinical context without fan-out to separate services.
- **WASM path**: the full retrieval pipeline runs on-device, satisfying ADR-001 and ADR-013.
- **Tiered quantization**: health data is automatically tiered (f32 hot → SQ8 warm → PQ cool → binary cold) as it ages, controlling storage growth on-device.
- **FHIR export**: any subset of the graph serializes to FHIR R4 for clinician handoff, appointment prep exports, or SMART Health Links sharing.
- **Self-improving graph**: GraphSAGE-refined embeddings and user-specific correlation edges mean the health graph becomes more accurate representations of the specific user over time.

### Negative

- **Health graph schema design is significant up-front work**: the entity and relationship type definitions must be clinically defensible, ontology-aligned, and maintained as clinical knowledge evolves. This requires medical advisory input (ADR-009).
- **Ontology mapping dependency**: the FHIR/LOINC/SNOMED layer in the triple store must be kept current with quarterly code system releases (ADR-004).
- **GNN compute on-device**: GraphSAGE forward passes are compute-bounded on mobile; pre-computed embeddings must be cached in the hot tier for real-time use.
- **Schema migration**: as the health domain model evolves, migrating existing graph data to a new schema requires careful versioning (RuVector RVF format versions help, but migration logic must be written).

### Mitigations

| Risk | Mitigation |
|---|---|
| Recall degradation as graph grows | HNSW m=24, ef_construction=200, ef_search=100; self-healing `ReindexPartition` triggers below 0.95 recall |
| Graph schema staleness | Quarterly schema review with medical advisory board; versioned schema releases tied to MetaHarness releases |
| On-device storage limits | Automatic tiered quantization; cold-tier binary compression; user-configurable data retention windows |
| Tampered vault data | Per-fact SHA-256 hash in provenance; Ed25519-signed audit log; coherence engine detects structural inconsistencies |

---

## Open Questions

1. **Embedding model selection**: should Helix use `all-MiniLM-L6-v2` (smaller, faster, WASM-friendly) or `BioClinicalBERT-384` (better medical recall) as the default? Or should the model be configurable per-domain (clinical notes → BioClinicalBERT; wearable signals → MiniLM)? Requires benchmarking on a Helix-specific health eval set.
2. **Graph edge population**: which curated clinical-knowledge sources will populate the population-level knowledge edges at initialization? Options include UMLS, SNOMED CT IS-A hierarchy, NLM drug interaction database, OpenFDA adverse events. Licensing and download procedures differ per source.
3. **Cross-user graph isolation**: how is the personal health graph strictly isolated from other users' graphs, particularly in a shared-device or family scenario? RuVector namespace isolation needs explicit testing.
4. **FHIR R5 readiness**: the spec cites FHIR R4/R5. The ontology/FHIR layer should be validated against FHIR R5 resource shapes as R5 adoption accelerates among major EHR vendors.
5. **Subjective log embedding**: free-text symptom logs and mood entries are high-noise inputs. What quality filters are applied before embedding and inserting into the graph?

---

## References

- RuVector core architecture (ADR-001, ruvector codebase): HNSW 61µs p50, 16,400 QPS, tiered quantization, WASM path **[A — directly measured in ruvector codebase]**
- RuVector eHealth Platform Architecture (ADR-028): GNN/GraphRAG in clinical context, SPARQL ontology layer, schema design, HIPAA alignment **[A]**
- "Retrieval-Augmented Generation in Biomedicine: A Survey of Technologies, Datasets, and Clinical Applications" (arXiv:2505.01146, 2025): structured KG retrieval outperforms flat-context RAG for clinical reasoning **[B]**
- "Medical Graph RAG: Towards Safe Medical LLM via Graph Retrieval-Augmented Generation" (arXiv:2408.04187): graph-structured retrieval reduces hallucination in medical LLM outputs **[B]**
- "Biomedical Knowledge Graph: A Survey of Domains, Tasks, and Real-World Applications" (arXiv:2501.11632): comprehensive review of health KG architectures **[B]**
- Hamilton et al. (2017). "Inductive Representation Learning on Large Graphs." NeurIPS. (GraphSAGE) **[A — foundational paper]**
- Malkov & Yashunin (2018). "Efficient and robust approximate nearest neighbor search using HNSW." arXiv:1603.09320. **[A — foundational paper]**
- HL7 FHIR R4 Specification: https://hl7.org/fhir/R4/ **[A]**
- SMART Health Cards and Links IG v1.0.0: https://build.fhir.org/ig/HL7/smart-health-cards-and-links/ **[A]**
- RuVector ADR-029 (RVF Canonical Binary Format) — persistence format for health vault state **[A]**
