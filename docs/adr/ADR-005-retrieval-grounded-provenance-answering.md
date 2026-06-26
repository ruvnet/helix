# ADR-005: Retrieval-Grounded, Provenance-Required Answering

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-003, ADR-004, ADR-006, ADR-007, ADR-008, ADR-010, ADR-013

---

## Context

### The danger of ungrounded health claims

Large language models (LLMs) produce fluent, confident prose regardless of whether the
underlying claims are factually supported. In most domains this is an inconvenience. In
personal health intelligence it is a safety hazard: a hallucinated ferritin trend, an
invented drug interaction, or a fabricated reference-range crossing can cause a user to
over- or under-react, delay necessary care, or make a supplement decision on false data.

The academic record on this is now unambiguous. A 2025 medRxiv preprint, "Medical
Hallucination in Foundation Models and Their Impact on Healthcare," found systematic
hallucination patterns across leading foundation models on clinical Q&A, noting that
models confidently fabricate citations, invent dosage figures, and misstate reference
ranges — not in edge cases but with alarming regularity on standard benchmarks [A].

A large-scale systematic expert evaluation published at arXiv 2511.06738 ("Rethinking
Retrieval-Augmented Generation for Medicine") assessed RAG systems on clinical Q&A and
found that while retrieval-augmented generation measurably reduces hallucination rates
relative to pure parametric generation, faithfulness failures persist even with retrieval
unless the system enforces per-claim attribution rather than document-level retrieval [A].

The MEGA-RAG framework (PMC12540348, 2025) introduced multi-evidence guided answer
refinement specifically for the biomedical domain, recognizing that single-pass retrieval
is insufficient: conflicting evidence must be surfaced, not silently resolved by the
synthesizing model, and every final claim must be traceable to a specific source passage
[B].

RAGAS — the canonical RAG faithfulness evaluation framework — operationalizes this as
follows: a "faithfulness" score is computed by decomposing the generated response into
atomic assertions and checking each assertion against retrieved context, dividing
supported assertions by total assertions. A faithfulness score below 1.0 means the model
asserted something not grounded in its context [A]. The same metric architecture is used
by TruLens, deepchecks, and the healthcare-specific RAG evaluation suites reviewed in
FutureAGI (2026) [B].

The eTracer system (arXiv 2601.03669, 2025) demonstrates claim-level grounding for text
generation: individual sentences are traceable to source passages via span-level
attribution, rather than the weaker document-level attribution that most RAG systems
provide. This is the standard Helix adopts [B].

A critical nuance from arXiv 2412.18004 ("Correctness is not Faithfulness"): a response
can be factually correct while being unfaithful to retrieved context (the model "knew"
the answer from parametric weights, not from retrieved data). In a health context, this
is still a failure — the user cannot audit a parametric answer, only a provenance-traced
one. Helix therefore requires faithfulness to *the user's own data*, not just to general
medical knowledge [A].

### Why retrieval from the user's own vault is the only acceptable source

General medical knowledge retrieved from the web or a clinical literature corpus cannot
answer "why am I tired?", because that question depends on *this user's* ferritin trend,
*this user's* deep sleep pattern, *this user's* medication timing. Generic retrieval
produces generic answers — precisely what Helix is built to replace. The retrieval
substrate is the user's own personal health knowledge graph in RuVector (ADR-003),
populated by normalized, ontology-mapped data (ADR-004).

Every datum in the RuVector vault carries a provenance record (see Decision section)
attached at ingestion time. The Functional-Medicine Analyst retrieves specifically tagged
nodes. No answer can reference a claim whose provenance record is absent.

---

## Decision

### Core rule: no backing datum → no claim

The Functional-Medicine Analyst operates under an absolute constraint: every factual
assertion in a response must resolve to at least one stored measurement node in RuVector
that carries a complete provenance record. If a claim cannot be grounded — because the
measurement does not exist, is outside its staleness window, or its provenance is
incomplete — the claim is suppressed. The system issues a gap notice instead (see
ADR-006 for the abstention policy).

This rule is enforced architecturally, not by prompt instruction alone. The Verifier/Critic
agent (ADR-008) re-derives every clinically meaningful claim from the same retrieval path
and rejects drafts containing ungrounded assertions before they surface to the user.

### Provenance record schema

Every measurement node stored in RuVector carries the following provenance fields,
attached at the point of ingestion (ADR-004 normalization layer) and immutable thereafter:

```
ProvRecord {
  source_system:     string        // e.g. "Quest Diagnostics", "Oura Gen3", "Apple HealthKit"
  source_connector:  string        // internal connector ID, version-pinned
  ingested_at:       ISO-8601 UTC  // when Helix received and stored this datum
  measured_at:       ISO-8601 UTC  // when the measurement was actually taken
  loinc_code:        string?       // LOINC code if applicable (labs, vitals)
  rxnorm_code:       string?       // RxNorm if medication
  value_raw:         string        // raw value as received, unparsed
  value_numeric:     f64?          // parsed numeric value if applicable
  unit_ucum:         string        // UCUM-canonical unit string
  reference_range:   { low: f64?, high: f64?, population: string, source: string }
  measurement_method: string?      // e.g. "venipuncture", "photoplethysmography"
  confidence:        f32           // [0.0, 1.0] — lower for OCR/inferred values
  staleness_window:  Duration      // domain-specific; see Staleness Policy below
  provenance_hash:   sha256        // hash of (source_system, measured_at, loinc_code, value_raw)
}
```

The `provenance_hash` provides a stable identifier that the UI presents to the user as
a "source fingerprint" — they can match it against the original lab report to confirm
no data was silently mutated.

### Staleness policy

Different data types have different valid windows. The Analyst refuses to assert a fact
from a datum whose `measured_at` is older than its `staleness_window`:

| Data type                          | Default staleness window |
|------------------------------------|--------------------------|
| Lab panel (fasting lipids, CBC)    | 18 months                |
| Hormone panel (testosterone, TSH)  | 12 months                |
| Micronutrient (ferritin, Vit D)    | 12 months                |
| HbA1c                              | 6 months                 |
| CGM / continuous glucose           | 30 days (trend window)   |
| Wearable sleep / HRV               | 7 days (rolling window)  |
| Ambient vitals (Cognitum Seed)     | 24 hours (live signal)   |
| Resting HR (rolling average)       | 14 days                  |
| Body weight / composition          | 30 days                  |

These defaults are configurable per user based on medical advisory guidance and can be
tightened by the Verifier agent if a Tier-2 clinical guideline specifies a shorter window.
When a datum is within window but approaching expiry, the UI surfaces a "value aging"
notice rather than a hard abstention.

### Retrieval plumbing: how grounding is enforced end-to-end

The grounded answering pipeline runs in the following order. No step may be skipped.

1. **Query decomposition.** The Analyst decomposes the user's question into a set of
   explicit data requirements — a list of measurement types and time windows needed to
   answer the question. This decomposition is stored as a query manifest.

2. **Retrieval from RuVector.** Each data requirement triggers a vector + metadata
   lookup in RuVector's HNSW index, filtered by LOINC code and staleness window.
   Returned nodes include their full ProvRecord. The Trend/Numeric agent (ADR-007)
   runs deterministic computation over retrieved time-series nodes and returns numeric
   facts — slopes, deltas, reference-range crossings — as a structured payload.

3. **Evidence-tier annotation.** Each retrieved node is tagged with its evidence tier
   (ADR-006) before the Analyst receives the context. The Analyst never sees raw data
   without tier labels.

4. **Draft synthesis.** The Analyst composes a draft response from the structured
   payload — numeric facts, evidence tier labels, and ProvRecord summaries. The draft
   must contain inline citation markers in the format `[{source_system}, {measured_at|YYYY-MM}]`
   adjacent to every factual claim.

5. **Claim extraction.** A lightweight extraction pass decomposes the draft into atomic
   assertions (comparable to RAGAS decomposition). Each assertion is matched to exactly
   one citation marker. Assertions without a marker are flagged as ungrounded.

6. **Verifier gate.** The Verifier/Critic agent (ADR-008) receives: the original query
   manifest, the retrieved ProvRecords, the numeric facts payload, and the draft with
   claim-to-citation mappings. The Verifier independently re-derives each claim. Claims
   that cannot be re-derived are dropped or replaced with gap notices. The gate produces
   a verified claim set.

7. **Escalation check.** The Escalation Guardian (ADR-009) scans the verified claim set
   for red-flag values. If a red flag fires, optimization content is suppressed and the
   red-flag routing takes over.

8. **Final response.** Only verified, cited claims reach the user. The UI renders inline
   citations as tappable links — tapping opens the source measurement card with the full
   ProvRecord, including the original raw value and the provenance hash.

### Inline citation format (UI surface)

Each claim in the UI response is tagged with a citation chip rendered adjacent to the
claim text. The chip shows: source system, measurement date, and evidence tier. Example:

```
Your ferritin was 28 ng/mL, below the reference range of 30–400 ng/mL.
  ↳ [Quest Diagnostics · Jun 2026 · Tier 1 — your data]

Your deep sleep decreased by 22% on nights following late training sessions.
  ↳ [Oura Gen3 · last 30 days · Tier 1 — your data]
```

Tapping a citation chip opens a source card showing: the full ProvRecord, the reference
range and its source population, the staleness window and days remaining, and a link to
the original import if the original file is stored locally (ADR-001).

### What the system says when grounding fails

If a required datum is absent, stale, or carries confidence below 0.50, the claim is
replaced with a structured gap notice rather than a best-guess. Gap notices take three
forms:

- **Missing data**: "I don't have a ferritin value on file. Would you like to add it to
  your next panel request?"
- **Stale data**: "Your last vitamin D was 14 months ago — outside the window I rely on
  for current assessment. Consider retesting."
- **Low confidence**: "Your sleep data for the last 3 nights is low-quality (device not
  worn). I'm not drawing conclusions from it."

Gap notices are surfaced in a visually distinct style (amber, not error-red) and are
offered as actionable prompts (what to do to fill the gap), not passive disclaimers.
Abstention is framed as a feature of honesty, not a limitation. See ADR-006 for the
full abstention policy and reward structure.

### Graph-aware retrieval

RuVector's GNN/GraphRAG layer (ADR-003) enables the Analyst to traverse relationships
across the personal health knowledge graph — for example: `fatigue symptom → low ferritin
node → iron-absorption pathway → medication known to reduce iron absorption`. This
relationship traversal is grounded: every edge in the graph is labelled with the source
data that established it (e.g., a Labcorp ferritin value and an RxNorm medication entry).
The Analyst may reason over paths in the graph but may only assert claims whose terminal
nodes carry valid ProvRecords. Inferred relationships must be labelled as inferences with
lower confidence, not asserted as facts.

### Parametric knowledge is context, not evidence

The LLM's parametric knowledge (training data) may inform *which* retrieval queries to
form and *how* to interpret reference ranges, but parametric claims must never enter the
final response without a corresponding retrieved ProvRecord. The framing "studies show..."
or "generally, low ferritin is associated with..." may appear only in Tier-3 or Tier-4
context blocks (ADR-006), clearly labelled as literature or heuristic — never conflated
with the user's own Tier-1 measurements.

---

## Alternatives Considered

### Alternative A: Prompt engineering alone ("instruct the model to cite its sources")

Many health AI products attempt to enforce grounding via system-prompt instructions: "only
answer from the provided context" or "cite your sources." Evaluation studies consistently
find this insufficient — models follow the instruction partially but hallucinate when the
context is incomplete, when the question pattern matches a common parametric answer, or
when the instruction conflicts with confident parametric knowledge [A, RAGAS, eTracer].

Rejected because: prompt instructions are not architecturally enforced; the Verifier gate
does not exist in this model; the failure mode (confident hallucination) is silent and
undetectable by the user. In the health domain, silent confidence is more dangerous than
explicit uncertainty.

### Alternative B: Citation-at-document-level rather than claim-level

A weaker form of RAG cites the source document but does not link each individual claim
to a specific passage or measurement node. This is the approach used by most general-purpose
RAG deployments. RAGAS research shows document-level attribution allows the model to cite
a document that does not actually support the specific claim made [A].

Rejected because: in a health context, "your ferritin" and "population average ferritin"
can appear in the same document. Document-level citation does not tell the user which of
those the claim is based on. The claim-level ProvRecord mapping is required for the user
to exercise meaningful audit of any answer.

### Alternative C: Retrieval from general medical literature corpus only

An alternative architecture would retrieve from medical literature (PubMed, clinical
guidelines) rather than from the user's personal data. This is effectively what ChatGPT
Health does — answer general health questions well from a tuned model with access to
connected EHR data, but the primary reasoning draws on general knowledge.

Rejected because: the core product thesis (§1.2) is that complete, structured, longitudinal
*personal* context produces qualitatively better guidance than general LLM knowledge.
Literature retrieval is useful for Tier-3 evidence labels on recommendations, but it is
not a substitute for retrieval from the user's own measurements. General knowledge cannot
answer "why am I specifically tired"; it can only answer "why do people generally get tired
in the afternoons" — a different question.

---

## Consequences

### Positive

- **Trust and auditability.** Every claim the user reads can be traced to a specific
  measurement, dated and sourced, that they can independently verify. This is the
  foundational trust mechanism — and a genuine differentiator versus products that
  present LLM-synthesized prose without provenance.
- **Core differentiator.** The grounding architecture, when functioning correctly,
  produces answers that are demonstrably more reliable than pure-parametric alternatives.
  The faithfulness property is measurable (RAGAS/DRACO score in ADR-018) and improvable.
- **Audit trail.** Every response is accompanied by the full set of ProvRecords consulted,
  the query manifest, and the Verifier's output — stored in Ruflo's HIPAA-mode audit log
  (ADR-002). This trail supports "prep for my appointment" summaries and is the user's
  evidence record.
- **Gap detection as value.** The system explicitly identifies what the user does not
  know about their own health — which measurements are missing or stale. This drives
  actionable retest recommendations, which is itself a health-optimization service.

### Negative

- **Higher "I don't have that" rate.** The system will refuse to answer some questions
  that a general LLM would answer confidently (and often incorrectly). Users accustomed
  to chatbot-style responses may initially interpret abstention as evasion. UX framing
  is critical: gap notices must be designed as actionable next steps, not dead ends.
- **Retrieval plumbing complexity.** The full pipeline (decomposition → retrieval →
  numeric computation → synthesis → claim extraction → verification → escalation check)
  adds latency relative to a single-pass LLM call. Target end-to-end latency for a
  standard analytical response: under 4 seconds on-device (ADR-013), under 2 seconds
  cloud-routed (ADR-019).
- **Schema maintenance.** The ProvRecord schema must be maintained as new data sources
  are added. Each new connector (ADR-012) must populate all required fields; connectors
  that cannot provide measured_at or unit_ucum must go into a review queue rather than
  silently ingesting with incomplete provenance.
- **OCR / PDF provenance gaps.** Lab PDFs imported via OCR fallback (ADR-012) carry
  lower confidence scores (typically 0.70–0.85 depending on OCR quality) and may have
  imprecise measured_at fields (date-of-report, not date-of-draw). These values are
  flagged in the UI and excluded from time-series trend claims.

### Mitigations

- Latency: the Trend/Numeric agent (ADR-007) runs in parallel with the synthesis draft,
  not sequentially; the critical path is retrieval + verification, not numeric computation.
- UX: gap notices are designed with a "what to do" action — "add to panel" or "re-upload
  recent lab" — so the user always has a path forward.
- OCR confidence: OCR-derived values are usable for snapshot views but excluded from
  trend calculations that require precise measured_at; this is enforced in the query
  manifest filter logic.

---

## Open Questions

1. **Provenance hash verification UX.** How prominently should the provenance hash be
   surfaced in the UI? Power users may want to verify it; general users may find it
   confusing. Proposed: hidden by default, accessible via "view source" gesture.

2. **Cross-source reconciliation provenance.** When Apple Watch HR and Whoop HR disagree
   for the same time window, the de-duplication layer (B5, ADR-004) must choose or
   merge. The merged value's ProvRecord must reference *both* source records and the
   reconciliation method. Schema needs a `provenance_chain` array for multi-source values.

3. **Parametric knowledge guard.** Enforcing "no parametric claim in the final response"
   requires the claim extraction step to detect when the model's output diverges from
   retrieved context. This is a hard problem — the Verifier catches it post-hoc, but an
   earlier detect-and-suppress stage would improve reliability. Consider integrating a
   lightweight faithfulness classifier (RAGAS-style) at the draft stage.

4. **Confidence decay over time.** Should `confidence` in the ProvRecord decay over time
   as the measurement ages toward its staleness window, rather than being a hard cutoff?
   A sigmoid decay model would allow "this value is 10 months old; confidence 0.70; I can
   mention it but won't assert it as current."

5. **Clinical advisory review.** The staleness windows above are engineering defaults.
   A medical advisory board (ADR-009, §7.3) should review and ratify them before launch,
   particularly for hormone panels and HbA1c where retest frequency is clinically debated.

---

## References

- [A] "Medical Hallucination in Foundation Models and Their Impact on Healthcare" (medRxiv 2025):
  https://www.medrxiv.org/content/10.1101/2025.02.28.25323115v1.full
- [A] "Rethinking Retrieval-Augmented Generation for Medicine: A Large-Scale, Systematic Expert Evaluation":
  https://arxiv.org/pdf/2511.06738
- [A] MEGA-RAG — multi-evidence guided answer refinement for public health (PMC 2025):
  https://pmc.ncbi.nlm.nih.gov/articles/PMC12540348/
- [A] RAGAS: Automated Evaluation of Retrieval Augmented Generation (ResearchGate):
  https://www.researchgate.net/publication/393020278_RAGAs_Automated_Evaluation_of_Retrieval_Augmented_Generation
- [A] "Correctness is not Faithfulness in Retrieval Augmented Generation" (arXiv 2412.18004):
  https://arxiv.org/pdf/2412.18004
- [B] eTracer: Towards Traceable Text Generation via Claim-Level Grounding (arXiv 2601.03669):
  https://arxiv.org/pdf/2601.03669
- [B] "Toward Faithful Retrieval-Augmented Generation with Sparse Autoencoders" (arXiv 2512.08892):
  https://arxiv.org/pdf/2512.08892
- [B] Best 5 RAG Evaluation Tools for Healthcare AI Applications in 2026 (FutureAGI):
  https://futureagi.com/blog/best-healthcare-rag-evaluation-2026/
- [B] RAG Evaluation Metrics: Answer Relevancy, Faithfulness, and Accuracy (Deepchecks):
  https://deepchecks.com/rag-evaluation-metrics-answer-relevancy-faithfulness-accuracy/
- [C] "Retrieval-Augmented Generation: A Comprehensive Survey of Architectures" (arXiv 2506.00054):
  https://arxiv.org/html/2506.00054v1

---

*This document provides architectural guidance, not legal, regulatory, or medical advice.
Helix is a decision-support tool, not a diagnostic authority. Engage clinical governance
before building diagnostic or treatment-recommending features.*
