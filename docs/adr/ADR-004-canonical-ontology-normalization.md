# ADR-004: Canonical Ontology Normalization Layer

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-002, ADR-003, ADR-005, ADR-007, ADR-009, ADR-012

---

## Context

### The identity crisis in consumer health data

The same concept — a thyroid-stimulating hormone measurement — arrives at Helix in a dozen forms depending on the source:

| Source | Raw form |
|---|---|
| Quest Diagnostics PDF | "TSH" with value "2.1", units "uIU/mL", no code |
| Labcorp portal feed | Local test code "004259", name "TSH", units "mIU/L" |
| Apple Health Records (FHIR) | `Observation.code.coding[0].code = "3016-3"`, system `http://loinc.org` |
| Oura API | Not provided — HR only; TSH not in scope |
| Physician note (text) | "TSH within normal limits at 2.1" |
| Function Health PDF | "Thyroid Stimulating Hormone (TSH)" with value "2.1 µIU/mL" |

Without normalization, these six records are six distinct objects in the health graph. Queries for "thyroid function" would miss records identified by local lab codes; trend computation would fail because unit strings don't match; drug-interaction checks would be blind to context identified only in clinical notes; and the FM Analyst would face a fundamentally ambiguous data model. **[A]**

This is not a corner case — it is the default state of consumer health data. Every connector in the integration matrix (ADR-012) produces a different identifier space, unit convention, and terminology dialect. The normalization layer is the part that turns a pile of heterogeneous health facts into a coherent, queryable corpus.

### The cost of silent coercion

The alternative to principled normalization is silent best-effort mapping: guess what "TSH" means and insert it with a best-guess LOINC code. This approach fails catastrophically in health contexts:

- A medication identified by trade name rather than RxNorm may be silently mapped to the wrong drug in a drug-interaction check.
- A lab result whose units are guessed wrong (µg/dL vs µmol/L) produces a value that appears in range but is actually dangerous.
- A condition identified by ICD-10 revision (ICD-10 vs ICD-10-CM) may map to the wrong diagnostic concept.

Silent coercion converts unknown data quality into false confidence. Helix's anti-hallucination contract (ADR-005) requires that if a datum cannot be reliably mapped to a canonical code, it does not enter the analytic graph as if it were canonical. It goes to a human-review queue instead. **[A]**

### The five canonical code systems

The normalization layer targets five standard code systems that together cover the full health-data domain Helix ingests:

**LOINC** (Logical Observation Identifiers Names and Codes)
- Governs: laboratory tests, clinical observations, vital signs, clinical document sections, questionnaires.
- Scale: 100,000+ observation terms. Labs are the dominant use case: "TSH" maps to LOINC `3016-3` (Thyrotropin [Units/volume] in Serum or Plasma).
- Licensing: free download from loinc.org; no per-use fee; requires attribution. LOINC is not covered by the UMLS Metathesaurus license requirement — it can be downloaded and used independently. The LOINC-SNOMED Part Mapping (mapping between LOINC parts and SNOMED CT concepts) is licensed separately but the core LOINC table is free. **[A — from loinc.org/kb/license]**
- Release cadence: twice yearly (June, December). Code additions are backward-compatible; codes are never deleted, only deprecated.
- Quest Diagnostics and Labcorp are both listed LOINC adopters with their own LOINC mapping tables published at loinc.org/adopters. **[A]**

**RxNorm**
- Governs: drugs, medications, clinical drug components, dose forms, drug interactions.
- Scale: 120,000+ concepts, 500,000+ relationships (routes, ingredients, dose forms, brand↔generic links).
- Licensing: available via the NLM UMLS Metathesaurus license (free to US institutions and individuals; requires registration at uts.nlm.nih.gov). The RxNorm API (rxnav.nlm.nih.gov) is freely accessible without UMLS license for basic lookups. **[A]**
- Supplements and OTC products: RxNorm covers prescription and OTC medications; dietary supplement coverage is incomplete (notable gap for Helix's supplement-logging use case). Unmapped supplements go to the review queue with a flag for manual RxNorm lookup or manual SNOMED supplement code assignment.
- Personal access tokens for the NLM APIs are not time-limited; the API is publicly available. Rate limits apply (verify current limits at build time). **[B]**

**SNOMED CT** (Systematized Nomenclature of Medicine — Clinical Terms)
- Governs: clinical findings, diagnoses, body structures, procedures, organisms, substances, observable entities.
- Scale: 350,000+ active concepts, 1.5M+ relationships. The most expressive of the five systems; SNOMED covers conditions (e.g., "Iron deficiency anaemia" SNOMED `87522002`), symptoms, procedures, and body structures with a full IS-A hierarchy.
- Licensing: managed by SNOMED International. Free for use in countries with a national affiliate agreement (the US affiliate is the NLM; US users can access SNOMED CT free via the UMLS Metathesaurus or VSAC). Commercial software distributed internationally requires explicit SNOMED International affiliate licensing for each target country. **[A — from IHTSDO affiliate license terms]** For Helix, the app itself would require an affiliate license if distributed internationally; verify with counsel. US MVP can rely on NLM UMLS free access.
- SNOMED CT is loaded into RuVector's RDF triple store as N-Triples (downloadable from IHTSDO or the NLM VSAC). The IS-A hierarchy enables hierarchical reasoning: "find all diabetes subtypes" traverses SNOMED's `is_a` relationships.

**ICD-10 / ICD-10-CM**
- Governs: disease classification for clinical documentation, billing, and epidemiology.
- Scale: ICD-10-CM (US clinical modification, CDC) has 72,000+ codes; ICD-10 (WHO international version) is the root taxonomy.
- Licensing: ICD-10-CM is maintained by the CDC NCHS and is freely available in the public domain (US) — no license required. ICD-10 (WHO) has a WHO copyright; use in software should be verified with WHO licensing terms for international deployment. **[A — from CDC NCHS ICD-10-CM documentation]**
- Release cadence: ICD-10-CM releases annually (October). Major code additions and deletions occur; the normalization layer must track the active code set version.
- Relationship to SNOMED: the NLM maintains a SNOMED CT → ICD-10-CM crosswalk map (available via VSAC). This crosswalk is used by the normalization pipeline to map clinical findings (SNOMED) to billing codes (ICD-10) where both are relevant.

**UCUM** (Unified Code for Units of Measure)
- Governs: measurement units for all quantitative health values — lab results, vital signs, wearable metrics.
- Scale: covers all SI units, derived units, and non-SI units used in healthcare.
- Licensing: maintained by the Regenstrief Institute; free download and use, no commercial license required. **[A]**
- The UCUM specification (unitsofmeasure.org) is the canonical reference. FHIR requires UCUM for all `Quantity` resources with a unit.
- Critical for Helix: without UCUM normalization, a ferritin of 28 in "ng/mL" and a ferritin of 28 in "µg/L" (which is numerically equivalent) would appear as different values in the time-series index, breaking trend computation.

**FHIR R4/R5 as the interchange model**
- FHIR (Fast Healthcare Interoperability Resources) is used by Helix as the canonical **interchange format** — the shape data takes when moving between the connector layer and the health knowledge graph.
- FHIR R4 is the dominant version in deployed EMR systems (US EHR vendors required by 21st Century Cures Act). FHIR R5 is the current specification; adoption is accelerating.
- Key FHIR resource types Helix uses: `Observation` (labs, vitals), `MedicationRequest` / `MedicationStatement` (meds), `Condition` (diagnoses), `Immunization`, `AllergyIntolerance`, `Patient`, `DiagnosticReport`, `DocumentReference` (for clinical notes), `Procedure`.
- SMART on FHIR provides the OAuth 2.0 authorization layer for patient-mediated access to EMR FHIR endpoints (PKCE required for public clients). **[A]**
- SMART Health Links (SHL) enable the user to share a signed, versioned bundle of their FHIR records with a clinician or another app — a key mechanism for ADR-001's user-controlled data model. **[A]**

---

## Decision

### Normalize all health data to LOINC / RxNorm / SNOMED CT / ICD-10 / UCUM, with FHIR R4/R5 as the interchange model, before entry into the analytic health knowledge graph. Un-mappable data routes to a human-review queue rather than being silently coerced.

#### Normalization pipeline

Every fact entering the Helix health graph traverses the following pipeline, implemented in the Ruflo Normalization agent (ADR-002):

```
Raw inbound fact (from any connector)
        │
        ▼
┌─────────────────────────────┐
│   1. Extraction             │
│   Parse structured fields:  │
│   name, value, unit, date,  │
│   source code (if any)      │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│   2. Code lookup            │
│   Try exact code match:     │
│   - If source provides LOINC│
│     code → validate & accept│
│   - If source provides RxNorm│
│     code → validate & accept│
│   - If source provides SNOMED│
│     → validate & accept     │
│   - Else → go to step 3     │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│   3. Fuzzy NLP mapping      │
│   Query RuVector RDF store  │
│   via SPARQL + embedding    │
│   similarity:               │
│   - Match "TSH" → LOINC     │
│     3016-3 (confidence 0.97)│
│   - Match "metformin 500mg" │
│     → RxNorm 861007         │
│   - Match "type 2 diabetes" │
│     → SNOMED 44054006       │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│   4. Confidence gating      │
│   confidence ≥ 0.85:        │
│     accept with code +      │
│     confidence score        │
│   0.60 ≤ confidence < 0.85: │
│     accept with NEEDS_REVIEW│
│     flag; surfaced in UI    │
│   confidence < 0.60:        │
│     → human review queue   │
│     (never enters graph)    │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│   5. Unit normalization     │
│   Map raw unit string to    │
│   UCUM expression:          │
│   "uIU/mL" → "[IU]/mL"     │
│   "mg/dL"  → "mg/dL"       │
│   "bpm"    → "/min"        │
│   Convert to canonical UCUM │
│   unit for the LOINC code   │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│   6. Provenance attachment  │
│   Attach full HealthProvenance│
│   struct (ADR-003):         │
│   source, timestamp, code,  │
│   unit, ref range, tier,    │
│   confidence, FHIR shape    │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│   7. FHIR resource shaping  │
│   Wrap as FHIR R4 resource  │
│   (Observation, Medication- │
│   Statement, Condition, etc.)│
│   for graph insertion and   │
│   export compatibility      │
└──────────────┬──────────────┘
               │
               ▼
        Health knowledge graph
        (RuVector, ADR-003)
```

#### Un-mappable data → human-review queue

Data that fails confidence gating (step 4, confidence < 0.60) is routed to a **structured human-review queue** rather than being silently dropped or coerced:

```rust
struct ReviewQueueEntry {
    raw_input:          String,         // original text/value as received
    source_connector:   String,
    collection_at:      DateTime,
    best_candidate:     Option<CodeCandidate>,  // highest-confidence match
    confidence:         f32,
    reason:             ReviewReason,   // NoMatch | AmbiguousMatch | UnitConflict | ...
    priority:           ReviewPriority, // P1 (potentially significant) | P2 | P3
    suggested_action:   String,         // "Map to LOINC 3016-3?" or "Create custom entry"
}
```

The queue is surfaced to the user in the Helix UI as a notification: "3 items from your recent Labcorp import need a quick review." The user can accept the suggested mapping, manually enter the correct code, or dismiss (which marks the entry as intentionally un-mapped and prevents re-queuing). A dismissed entry is stored as a raw, un-normalized fact — visible in the user's data but excluded from trend computation and graph reasoning until resolved.

Priority P1 entries (e.g., a medication that could not be RxNorm-mapped, raising drug-interaction-check risk) trigger a more urgent notification.

#### Code system versioning

Each code system release is tracked in the RuVector RDF triple store metadata:
```
{
  "loinc_version":    "2.78",   // June 2026 release
  "rxnorm_version":   "2026AA",
  "snomed_version":   "2026-01-31",
  "icd10cm_version":  "FY2026",
  "ucum_version":     "2.1"
}
```

When a new code system release is ingested, a `code_system_migration` background worker:
1. Checks for deprecated codes in the existing health graph.
2. Maps deprecated codes to their active successors where the ontology provides a replacement mapping.
3. Flags codes with no successor for the review queue.
4. Updates the RDF triple store atomically (within a RuVector REDB transaction).

All stored facts retain the code system version at the time of ingestion in their provenance record, enabling historical queries that are not affected by code system drift.

#### Worked example: mapping a TSH result end-to-end

**Input** (from Quest PDF via OCR, connector `quest-pdf-ocr-v1`):
```
Test: TSH
Value: 2.1
Units: uIU/mL
Reference Range: 0.450 - 4.500 uIU/mL
Date: 2026-04-10
```

**Step 1 — Extraction**: name="TSH", value=2.1, unit="uIU/mL", reference_low=0.45, reference_high=4.5, date=2026-04-10.

**Step 2 — Code lookup**: no source-provided LOINC code in PDF → go to step 3.

**Step 3 — Fuzzy NLP mapping**:
- SPARQL query to RuVector RDF store: `SELECT ?code WHERE { ?c loinc:longCommonName "Thyrotropin [Units/volume] in Serum or Plasma" }` → candidate LOINC `3016-3`.
- Embedding similarity: embedding("TSH") cosine-similar to embedding("Thyrotropin") in the LOINC concept space → score 0.96.
- Combined confidence: 0.97.

**Step 4 — Confidence gating**: 0.97 ≥ 0.85 → accepted.

**Step 5 — Unit normalization**: "uIU/mL" → UCUM `[IU]/mL` (micro-international-units per milliliter). LOINC 3016-3 example UCUM unit is `[IU]/mL` — match confirmed.

**Step 6 — Provenance attachment**:
```json
{
  "source_connector_id": "quest-pdf-ocr-v1",
  "collection_at": "2026-04-10",
  "canonical_code": { "system": "LOINC", "code": "3016-3", "display": "Thyrotropin [Units/volume] in Serum or Plasma" },
  "unit": "[IU]/mL",
  "reference_range": { "low": 0.45, "high": 4.5, "unit": "[IU]/mL", "source": "quest-pdf" },
  "confidence_score": 0.97,
  "evidence_tier": 1,
  "connector_version": "1.2.0",
  "hash": "sha256:a3f9..."
}
```

**Step 7 — FHIR resource shaping**:
```json
{
  "resourceType": "Observation",
  "id": "helix-obs-tsh-20260410",
  "status": "final",
  "code": { "coding": [{ "system": "http://loinc.org", "code": "3016-3", "display": "Thyrotropin [Units/volume] in Serum or Plasma" }] },
  "valueQuantity": { "value": 2.1, "unit": "IU/mL", "system": "http://unitsofmeasure.org", "code": "[IU]/mL" },
  "referenceRange": [{ "low": { "value": 0.45 }, "high": { "value": 4.5 } }],
  "effectiveDateTime": "2026-04-10"
}
```

**Result in graph**: a `LabResult` node with LOINC code 3016-3, value 2.1 [IU]/mL, full provenance, FHIR resource ID. The time-series index for `loinc:3016-3:[IU]/mL` gains this data point. The Trend/Numeric agent can now compute a TSH slope across multiple draws.

#### Domain-specific mapping notes

| Domain | Primary code system | Common challenges | Fallback |
|---|---|---|---|
| Labs | LOINC | Local lab codes (Quest, Labcorp); PDF OCR errors in test names | Fuzzy NLP on LOINC long common names; Quest/Labcorp LOINC mapping tables |
| Medications | RxNorm | Trade names vs generic; supplements not in RxNorm | NLM RxNorm API lookup by name; supplement → SNOMED substance code |
| Diagnoses (clinical) | SNOMED CT | Free-text clinical notes; ICD-10 vs SNOMED disambiguation | NLP entity extraction → SNOMED concept disambiguation |
| Diagnoses (billing/admin) | ICD-10-CM | Revision differences (ICD-9 legacy records); specificity mismatches | SNOMED-ICD10 crosswalk; ICD-9-CM to ICD-10-CM GEMs mapping |
| Units | UCUM | Non-standard unit strings; missing units in wearable APIs | UCUM synonym table; default UCUM per LOINC example unit |
| Wearable metrics | LOINC (where available) | Most wearable metrics not in LOINC (e.g., HRV, recovery score) | Internal Helix vocabulary for proprietary metrics; not mixed into LOINC space |
| Genomic variants | dbSNP rs-IDs; HGVS notation | No LOINC/SNOMED for individual variants; ClinVar for clinical significance | Store as GenomicVariant node with rsid; map to condition via ClinVar SNOMED crosswalk |

---

## Alternatives Considered

### Alternative 1: Store data as raw text and normalize at query time

Accept all health data as-is (raw strings, local codes, arbitrary units) and normalize only when the FM Analyst makes a query — embedding-based fuzzy matching at retrieval time.

**Rejected because:**
- The Trend/Numeric agent (ADR-007) requires structured, unit-consistent time-series data. Computing a TSH trend over three draws requires that all three have the same canonical code and UCUM unit; this cannot be guaranteed at query time without pre-normalization.
- The Verifier/Critic agent (ADR-008) must check claims against stored data. If the stored data is a raw string "TSH 2.1 uIU/mL" rather than a structured datum with provenance, the Verifier cannot reliably re-derive the claim "your TSH is 2.1."
- Drug-interaction detection requires canonical RxNorm identifiers. Raw medication names ("levothyroxine 50 mcg") cannot be reliably cross-referenced against a drug-interaction knowledge base without normalization.
- Normalization errors compound silently over time — each query may produce different mappings from the same raw data.

### Alternative 2: Single ontology (LOINC for everything)

Map all health data to LOINC codes, using LOINC's broad coverage to handle medications, conditions, and units.

**Rejected because:**
- LOINC does not govern medications (RxNorm is the standard), conditions/diagnoses (SNOMED CT and ICD-10), or units (UCUM). Forcing everything into LOINC produces incorrect or missing codes for large swaths of health data.
- Drug-interaction databases (NLM, DrugBank) are keyed on RxNorm. Medications mapped only to LOINC observation terms cannot be cross-referenced against interaction databases.
- Clinical safety: a medication stored with only a LOINC observation code (e.g., the LOINC code for "medication administration") carries no pharmacological identity — drug-interaction checks would be blind to it.

### Alternative 3: Use FHIR-only coding (store raw FHIR resources, rely on FHIR to carry codes)

Ingest all data as FHIR resources and rely on the FHIR resource's embedded codes without additional normalization.

**Rejected because:**
- Many data sources do not produce FHIR. Lab PDFs, wearable APIs, genomic VCF files, and manual entries require normalization *before* they can be expressed as valid FHIR resources.
- FHIR resources can contain any coding system — the `code.coding` array can have zero, one, or many codes from different systems. A FHIR `Observation` might carry only a local lab code with no LOINC code; accepting it as-is defeats the purpose of normalization.
- FHIR is the interchange format; LOINC/RxNorm/SNOMED/ICD-10/UCUM are the semantic identifiers. Both are needed.

---

## Consequences

### Positive

- **Query reliability**: every health fact in the graph shares the same identifier space — trend computation, drug-interaction detection, and graph traversal all work without code-space disambiguation at query time.
- **Clinician legibility**: LOINC/RxNorm/SNOMED/ICD-10 are the code systems clinical professionals and EHR systems understand. A Helix export (FHIR bundle) is immediately legible to a clinician's EMR.
- **FHIR interoperability**: normalized data exports as valid FHIR R4 bundles for appointment prep summaries, SMART Health Links sharing, and future EMR integration.
- **Provenance completeness**: every normalized fact carries canonical code + UCUM unit + reference range + confidence — enabling the Verifier to check any claim against a discrete, unambiguous datum.
- **Review queue as safety net**: un-mappable data is never silently coerced; the queue surfaces data-quality problems to the user for resolution, maintaining trust.

### Negative

- **Normalization is labor-intensive and ongoing**: the LOINC/RxNorm/SNOMED mapping tables must be refreshed with each code system release (LOINC twice yearly, SNOMED twice yearly, ICD-10-CM annually, RxNorm monthly). This is a durable operational requirement.
- **Supplement and wearable coverage gaps**: RxNorm's supplement coverage is incomplete; wearable-specific metrics (HRV, recovery score, sleep stages as proprietary vendor constructs) do not map to standard codes. These require either an internal Helix vocabulary extension or a LOINC ballot for new terms.
- **SNOMED CT licensing complexity for international distribution**: international deployment requires affiliate licensing per country. For the Phase 0–1 US MVP, NLM/UMLS free access is sufficient.
- **Review queue UX burden**: if too many facts land in the review queue (especially from OCR-heavy PDF sources), users experience friction. Requires continuous improvement of the fuzzy NLP mapper and PDF OCR post-processing.
- **Code system version drift**: a health graph built across multiple years spans multiple LOINC/SNOMED releases. Facts ingested under different versions may require migration when codes are deprecated.

### Mitigations

| Risk | Mitigation |
|---|---|
| Lab PDF OCR mapping failures | Publish and maintain Lab-local-code → LOINC crosswalk tables for Quest/Labcorp (public on loinc.org/adopters); use as primary lookup before fuzzy NLP |
| SNOMED international licensing | US MVP uses NLM UMLS free access; international roadmap requires SNOMED International affiliate agreement; get counsel before non-US launch |
| Supplement RxNorm gaps | Seed an internal supplement vocabulary mapped to SNOMED substance codes; flag as "non-standard code" in provenance; do not mix with RxNorm space |
| Review queue overflow | Lower the P1 priority threshold for medications (never miss a drug); raise the P3 threshold for wearable signals (trend loss is recoverable); monitor queue length as a quality metric |

---

## Open Questions

1. **UMLS license registration**: does Helix (as a commercial product) require a commercial UMLS license for RxNorm and SNOMED CT access, or does the free NLM individual/institution license cover commercial app use? This requires legal counsel to confirm before distribution.
2. **Wearable metric vocabulary**: which body owns the standard LOINC codes for wearable metrics (HRV, SpO₂ as reported by a consumer device, sleep stage as classified by a wearable algorithm)? Some of these LOINC codes exist; others are pending ballots. Track LOINC ballot outcomes for the wearable domain.
3. **ClinVar integration for genomics**: should genomic variants be linked to SNOMED/ICD-10 conditions via the ClinVar crosswalk, or via a different knowledge base (e.g., ClinGen, PharmGKB for pharmacogenomics)? This is an ADR-003 schema question with licensing implications.
4. **Ambient vital signal codes**: Cognitum Seed-derived metrics (respiration rate, restlessness index, sleep-disordered-breathing signal) — do standard LOINC codes exist for contactless mmWave-derived measurements, or do they require an internal Helix vocabulary extension?
5. **FHIR R5 transition timeline**: when does the FHIR layer need to support R5 resource shapes in addition to R4? Track ONC and major EHR vendor roadmaps.

---

## References

- LOINC License: https://loinc.org/kb/license/ — free to use, no per-use fee, attribution required **[A]**
- LOINC Adopters (Quest Diagnostics, Labcorp): https://loinc.org/adopters/ — both publish LOINC mapping tables **[A]**
- RxNorm / UMLS Metathesaurus License: https://uts.nlm.nih.gov/uts/login — free registration, covers RxNorm **[A]**
- NLM RxNorm API: https://rxnav.nlm.nih.gov/ — freely accessible for drug name lookup **[A]**
- SNOMED CT Affiliate License: https://www.snomed.org/snomed-ct/get-snomed — free for NLM UMLS users (US); international requires affiliate agreement **[A]**
- ICD-10-CM (CDC NCHS): https://www.cdc.gov/nchs/icd/icd-10-cm.htm — public domain in US **[A]**
- UCUM Specification: https://unitsofmeasure.org/ — free download, no commercial license required **[A]**
- HL7 FHIR R4 Specification: https://hl7.org/fhir/R4/ **[A]**
- SMART Health Cards and Links IG v1.0.0: https://build.fhir.org/ig/HL7/smart-health-cards-and-links/ **[A]**
- "SNOMED CT, LOINC, and RxNorm" (NLM, 2018): https://data.lhncbc.nlm.nih.gov/public/mor/pubs/pdf/2018-ybmi-ob.pdf — comprehensive overview of the three primary clinical terminologies **[B]**
- RuVector ADR-028 (eHealth Platform Architecture) — prior art for RDF triple store with 31.4M medical-ontology triples in RuVector **[A]**
- LOINC-SNOMED CT Part Mapping (IHTSDO/Regenstrief): https://loinc.org/collaboration/snomed-ct/ — bridge between LOINC parts and SNOMED CT concepts **[A]**
- NLM ICD-10-CM/GEMs crosswalk (ICD-9 to ICD-10 mapping): https://www.cms.gov/Medicare/Coding/ICD10/2018-ICD-10-CM-and-GEMs **[A]**
