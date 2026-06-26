# ADR-022: OCR Lab-Document Ingestion (RuVector OCR backend)

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-012 (connector degradation), ADR-004 (ontology normalization), ADR-005 (provenance), ADR-006 (abstention)

---

## Context

ADR-012 made **graceful degradation** the connector contract: when a clean API
isn't available, fall back to user-initiated export + **PDF/OCR import**. The
iter-7 research for ADR-004/012 confirmed this is not a corner case — **Quest
Diagnostics and Labcorp have no consumer APIs**, so for most users the lab PDF
*is* the primary ingestion path, not a fallback. **[A]**

**[ruvnet/ruvector](https://github.com/ruvnet/ruvector)** ships **OCR** in-stack
(its topics include `ocr` / `ai-ocr`), so Helix can extract structured values
from scanned/exported lab documents without a new external dependency or sending
documents to a cloud OCR service (which would violate ADR-001/013). **[A]/[B]**

The danger: OCR is messy. A misread decimal point or unit turns a normal value
into a red-flag (or hides one). So the OCR path must be the **most conservative**
ingestion route — low default confidence, mandatory normalization through ADR-004
(confident-or-queued, never coerced), and aggressive sanity checks before any
extracted value is allowed to become a usable record.

## Decision

Add an **OCR lab-ingestion adapter** (`helix-ocr`) that turns RuVector-OCR output
into candidate records, gated hard:

1. **On-device OCR.** Documents are OCR'd locally via RuVector; the document and
   its text never leave the device (ADR-001/013).
2. **Extraction → candidates, not records.** OCR produces *candidate* analytes
   (label, value, unit, ref-range text, a per-field OCR confidence). A candidate
   is not a record until it passes the gate.
3. **Sanity gate before acceptance.** Reject/queue a candidate when: OCR
   confidence is below floor; the value is non-numeric or non-finite; the unit is
   missing/unparseable; or the value is physiologically implausible for the
   analyte. Borderline → **human-review queue** (ADR-004), never silent coercion.
4. **Conservative provenance.** Accepted candidates become `ProvRecord`s with
   `method = OcrExtraction`, the parsed value/unit/range, and a **confidence
   derived from the OCR confidence and capped** — always lower than a structured
   feed for the same analyte, so the analyst and Verifier (ADR-008) weight it
   accordingly.
5. **Then normalize.** Accepted candidates still flow through ADR-004 ontology
   normalization (LOINC mapping) like any other source; un-mappable labels queue.

Implemented as a pure transform: `extract → gate → record/queue`, with the OCR
engine itself injected (RuVector on device; a stub in tests) so the gate logic is
deterministic and exhaustively testable.

## Alternatives Considered

- **Cloud OCR (Textract/Document AI).** Rejected: sends health documents off
  device (ADR-001/013) and adds a vendor dependency.
- **Trust OCR output directly as records.** Rejected: a single misread digit
  could fabricate a red-flag or hide one — exactly the hallucination-adjacent
  failure ADR-005/006 exist to prevent.
- **LLM-parses-the-PDF.** Rejected for the numeric extraction itself (ADR-007:
  LLMs are unreliable at exact numeric transcription); deterministic parsing +
  the sanity gate is safer. An LLM may *assist label matching* upstream of the gate.

## Consequences

**Positive.** Unblocks the most common real ingestion path (lab PDFs) with no new
external dependency and no document egress; conservative confidence keeps OCR data
honest; the human-review queue makes failure visible instead of silent.

**Negative.** OCR/layout parsing is brittle across lab formats; the plausibility
ranges need curation per analyte; review-queue volume could be high early on.

**Mitigations.** Per-analyte plausibility ranges curated and versioned; capped
confidence + Verifier gate; surface "imported from a scan — confirm" in the UI so
the user can correct an OCR record; iterate format coverage behind the gate.

## Open Questions

- Source of per-analyte plausibility ranges (reuse the ADR-009 red-flag registry's
  outer bounds as a coarse plausibility filter?).
- How much layout/table structure RuVector-OCR returns vs. raw text (affects
  parser design).
- UX for confirming/correcting OCR-imported values.

## References

- ruvnet/ruvector — OCR / ai-ocr in-stack (repo topics + modules). **[A]/[B]**
- Helix ADR-012 (connector degradation; Quest/Labcorp have no consumer APIs — PDF/OCR primary), ADR-004, ADR-005, ADR-007. **[A]**

> Architectural/product guidance, not legal or medical advice. OCR-imported values are conservative-confidence and user-confirmable.
