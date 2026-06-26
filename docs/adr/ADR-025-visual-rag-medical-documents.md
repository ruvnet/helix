# ADR-025: Visual RAG over Medical Documents & Images (rupixel backend)

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-022 (OCR ingestion), ADR-023 (semantic retrieval), ADR-005 (grounding), ADR-010 (SaMD), ADR-001 (vault), ADR-015 (visual)

---

## Context

ADR-022 ingests lab documents by **OCR** — text extraction. But OCR *flattens*
the page: tables, charts, reference-range columns, and the spatial layout that
makes a lab report legible are lost or mangled, and many medical artifacts
(imaging reports, ECG strips, scanned forms, photos of a rash) have little
extractable text at all. The user still needs to *find* them: "show me the lab
report with my lipid panel", "the imaging report from last spring".

**[ruvnet/rupixel](https://github.com/ruvnet/rupixel)** is exactly this: a Rust
port of **PixelRAG / ColPali-style visual RAG** on the RuVector ANN substrate
(HNSW + IVF-Flat). It renders documents to **screenshot tiles** and retrieves
over **visual embeddings**, so "retrieve over what a page *looks like*, not just
its text." It ships a client-side WASM MiniLM demo and a metaharness benchmark
CLI. **[A]**

The safety line is sharp and non-negotiable: **visual retrieval finds
similar-looking documents — it does not interpret or diagnose them.** Helix must
never say "this X-ray shows X." Visual RAG is an *organization and recall* tool
(ADR-023's "recall, not grounding", applied to pixels), feeding the same
grounding gate (ADR-005) and the same non-diagnostic framing (ADR-010).

## Decision

Add a **visual-RAG layer** (`helix-visual`) for medical documents and images,
ColPali-faithful and gated:

1. **Tile embeddings + late interaction (MaxSim).** Each document/image is split
   into a grid of **tiles**; each tile gets a perceptual visual descriptor.
   Retrieval scores a query against a document by **MaxSim** — for every query
   tile, the best-matching document tile, averaged (ColPali late interaction) —
   so layout and local structure survive, not just a global average.
2. **On-device, pixels-in-vault.** Tiling + embedding run locally; the document
   pixels and any PHI burned into them stay in the encrypted vault (ADR-001/013).
   Only derived embeddings are indexed.
3. **Retrieval, never interpretation.** A result is "a document that *looks*
   like this", with a similarity score — tagged as visual recall, **never** a
   reading or diagnosis (ADR-010). It surfaces the document for a human (and the
   grounding gate, ADR-005); it does not assert clinical content from pixels.
4. **Complements, not replaces, OCR.** Visual RAG and OCR (ADR-022) are
   layered: OCR extracts values where it can; visual RAG finds and ranks the
   right *document* even when OCR can't read it. They share the review queue.
5. **Pluggable embedder.** The perceptual descriptor is the in-crate, dependency-
   light reference (deterministic, benchmarkable); a learned ColPali/MiniLM-class
   encoder (rupixel on RuVector, WASM at the edge) plugs in behind the same
   `VisualEmbedder` trait.

## Alternatives Considered

- **OCR-only (ADR-022).** Rejected as sufficient: loses tables/charts/layout and
  fails on image-only artifacts; can't *find* a document it can't read.
- **A vision model that reads/interprets the image.** Rejected outright: that is
  diagnosis from pixels — the SaMD line (ADR-010) and the hallucination risk
  (ADR-005) exist precisely to forbid it. Helix retrieves and surfaces; clinicians
  interpret.
- **Cloud visual embedding.** Rejected: sends medical images off device
  (ADR-001/013). rupixel is edge-first by design.

## Consequences

**Positive.** The user can find any medical document by appearance; tables and
charts survive retrieval; image-only artifacts become findable; on-device privacy
preserved; ColPali-grade recall in pure Rust.

**Negative.** Visual embeddings are heavier than text; the reference perceptual
descriptor is weaker than a learned encoder (it ranks by appearance, not
semantics); MaxSim is O(query_tiles × doc_tiles) per candidate.

**Mitigations.** Tile grid is tunable (recall vs. cost); the descriptor is a
floor that a learned encoder replaces behind the trait; ANN (RuVector HNSW/IVF)
caps the candidate set before MaxSim; benchmark the hot path and keep it
allocation-light.

## Open Questions

- Tile grid + descriptor dimensionality for the privacy/utility/cost trade-off.
- When to render a *stored* artifact to tiles vs. ingest an already-rastered scan.
- How visual matches and OCR records reconcile in the same dossier view.

## References

- ruvnet/rupixel — pixel-native visual RAG on RuVector (HNSW + IVF-Flat); ColPali/PixelRAG lineage. **[A]**
- ColPali (Faysse et al., 2024) — late-interaction visual document retrieval. **[A]**
- Helix ADR-022 (OCR), ADR-023 (retrieval, recall≠grounding), ADR-010 (SaMD), ADR-001 (vault). **[A]**

> Architectural/product guidance, not legal or medical advice. Visual retrieval surfaces documents; it never interprets or diagnoses an image.
