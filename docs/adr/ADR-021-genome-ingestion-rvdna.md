# ADR-021: User-Owned Genome Ingestion & Pharmacogenomics (rvDNA backend)

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001 (user-owned vault), ADR-005 (provenance), ADR-006 (evidence/abstention), ADR-009 (escalation), ADR-010 (SaMD), §7.4 (the 23andMe lesson)

---

## Context

Genome data is the most sensitive data a person has, and §7.4 / ADR-001 make the
rule non-negotiable: Helix stores genomic data **user-owned and local-first**,
never a third-party direct-to-consumer vault (the 23andMe bankruptcy is the
cautionary tale). But ADR-005's roadmap (Tier A5) also wants the genome to be
*usable* — pharmacogenomics and risk context are genuinely actionable.

**[ruvnet/rvdna](https://github.com/ruvnet/rvdna)** is an AI-native genomics
engine in pure Rust + WASM, in-stack (HNSW on RuVector). Relevant capabilities: **[A]**
- **23andMe-style genotype parsing/calling** — from the user's *own* raw file.
- **Pharmacogenomics** — CYP2D6 / CYP2C19 star-alleles → metabolizer phenotype,
  the highest-actionability genomic signal (it changes how a clinician doses
  common drugs).
- **20-SNP biomarker risk scoring** + streaming anomaly detection.
- **Variant calling**, protein prediction, epigenomic modeling.
- **64-dim genomic profile vectors** with HNSW similarity (cohort matching).
- The **`.rvdna`** cognitive-container format.

rvDNA runs on the edge via WASM, so it fits Helix's local-first, on-device-first
posture (ADR-013) — the genome can be analyzed without leaving the device.

## Decision

Adopt rvDNA as Helix's **genome analysis backend**, behind a `helix-genome`
adapter, under strict guardrails:

1. **User-owned, on-device.** The raw genotype file and the `.rvdna` container
   live only in the user's encrypted vault (ADR-001). Analysis runs locally
   (WASM, ADR-013). Helix the company never holds or can sell the corpus.
2. **GINA-aware, privacy-first.** Genomic-derived records are flagged as
   genetic data with a standing privacy note (GINA limits employer/insurer
   discrimination but does **not** govern collection — design to the strictest
   standard; genomic records are excluded from federation by default, ADR-011).
3. **Risk/decision-support, not diagnosis.** Biomarker risk scores are
   probabilistic context with a band + the population caveat; pharmacogenomic
   phenotypes are **"discuss with your prescriber"** advisories, never a dosing
   directive (ADR-010). Nothing here is a clinical verdict.
4. **Provenance + capped confidence.** Derived genomic facts become `ProvRecord`s
   (method `Derived`, source `rvdna`, `GENO-*` codes — never a clinical LOINC,
   ADR-004), at a confidence reflecting array/imputation limits. No backing
   genotype → no claim (ADR-005).
5. **Pharmacogenomic flags feed the analyst, gently.** A non-normal metabolizer
   phenotype emits an advisory the analyst surfaces when relevant medications are
   in the dossier (the ADR-C1 interaction map) — as a prompt to verify with a
   clinician, with optimization suppression only when paired with a real red flag
   (ADR-009).

## Alternatives Considered

- **No genome support.** Rejected: leaves the most actionable precision-medicine
  signal (pharmacogenomics) on the table; ADR-005 Tier A5 wants it.
- **Cloud genomics API.** Rejected outright: re-creates the 23andMe failure mode
  (§7.4) and violates ADR-001/013.
- **Treat risk scores as diagnoses.** Rejected: polygenic/array risk is
  probabilistic and population-bound; presenting it as a verdict misleads (ADR-006/010).

## Consequences

**Positive.** Real, in-stack, edge genome analysis; the highest-value genomic
signal (CYP2D6/CYP2C19) wired into the medication-interaction surface; full
user-ownership and on-device privacy preserved; cohort similarity available
without exposing raw data (HNSW on derived vectors only).

**Negative.** Genotyping arrays have coverage/imputation gaps; risk scores carry
ancestry-bias caveats; pharmacogenomic interpretation is itself an evolving
clinical field — over-claiming is a real liability.

**Mitigations.** Capped confidence + explicit band/caveat on every risk; star-allele
phenotypes framed as "verify with prescriber"; ancestry caveat surfaced; genomic
records excluded from federation; clinical-governance review of the
pharmacogenomic rule set (not Darwin-mutable).

## Open Questions

- Which biomarker risk traits to surface at MVP (and with what ancestry caveats)?
- Curating the pharmacogenomic phenotype→advisory mapping with clinical pharmacy input.
- `.rvdna` container lifecycle inside the vault (versioning, re-analysis on model updates).

## References

- ruvnet/rvdna — genomics engine (README: genotyping, CYP2D6/CYP2C19, biomarker risk, HNSW). **[A]**
- CPIC / PharmGKB — pharmacogenomic phenotype→action evidence (to ground the advisory set). **[A]**
- GINA (Genetic Information Nondiscrimination Act) — scope and limits. **[A]**
- Helix ADR-001 / §7.4 (the 23andMe lesson), ADR-005, ADR-010, ADR-011. **[A]**

> Architectural/product guidance, not legal or medical advice. Genomic risk/pharmacogenomics is decision-support; engage clinical and legal counsel.
