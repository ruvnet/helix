# Helix — Capabilities Integration Loop (ruvnet ecosystem)

**Loop:** `/loop 10m review ruvnet/ruvector + ruvnet/ruview (+rvdna) for capabilities → implement with ADRs →
test+validate → push to ruvnet/helix main frequently — until SOTA`
**Repo:** https://github.com/ruvnet/helix · **Started:** 2026-06-25

## Source repos reviewed
- **ruvnet/ruvector** — vector + GNN memory DB; OCR (ai-ocr), GraphRAG, ONNX embeddings, HNSW, RaBitQ, WASM.
  → candidates: OCR lab-PDF ingestion (ADR-012), vector/GraphRAG semantic retrieval over the health graph (ADR-003).
- **ruvnet/ruview** — WiFi-CSI contactless sensing: breathing/HR, sleep-stage + apnea screening, fall/distress/
  bed-exit semantic states, Ed25519-attested, edge-only. → realizes ADR-014 ambient sensing + ADR-009 escalation.
- **ruvnet/rvdna** (requested) — genomics: variant calling, lineage, translate, score. → Helix genome ingestion
  (Tier A5 / §7.4 user-owned VCF), a `helix-genome` adapter; screening-grade, GINA-aware, never a clinical verdict.

## Definition of SOTA (exit criteria)
Each adopted capability has: (1) an ADR, (2) a real tested crate, (3) clippy/fmt clean + audit clean,
(4) wired into the pipeline/UI where it fits, (5) pushed to main. Loop ends when the high-value capabilities
from all three repos are integrated and validated.

## Capability backlog (ordered)
| # | Capability | Source | ADR | Crate | Status |
|---|-----------|--------|-----|-------|--------|
| 1 | WiFi-CSI ambient sensing + escalation | ruview | ADR-020 | `helix-sensing` | ✅ done |
| 2 | Genome ingestion + pharmacogenomics (GINA-aware) | rvdna | ADR-021 | `helix-genome` | ✅ done |
| 3 | OCR lab-PDF ingestion (connector degradation) | ruvector | ADR-022 | `helix-ocr` | ✅ done |
| 4 | Vector / GraphRAG semantic retrieval | ruvector | ADR-023 | `helix-retrieval` | ✅ done |

## Ledger
- **Iter 1 (2026-06-25):** reviewed ruvector + ruview. ADR-020 (WiFi-CSI ambient sensing, RuView backend) +
  `helix-sensing` crate: RuView reading → ProvRecords (RUVW-* research codes, AmbientSensing, capped 0.5
  confidence) + screening flags mapping semantic states (possible-distress/fall-risk → Critical; elderly-
  inactivity/apnea → Urgent; ambient context → none) to the Escalation Guardian (ADR-009); rejects unsigned/
  non-finite; non-diagnostic framing throughout. 94 tests green; clippy+fmt clean. Pushed to main.

- **Iter 2 (2026-06-25):** ADR-021 (genome ingestion & pharmacogenomics, rvDNA backend) + `helix-genome`:
  pharmacogenomic phenotypes (CYP2D6/CYP2C19 → Metabolizer) → ProvRecords (GENO-PGX-*, Derived, 0.6 conf) +
  "verify with your prescriber" advisories (non-normal metabolizers only; decision-support, never a dosing
  directive); biomarker risk → GENO-RISK-* records (0.4 conf, band + ancestry caveat); GINA-aware privacy
  note; excluded from federation; rejects empty/out-of-range/NaN. 100 tests green; clippy+fmt clean. Pushed.

- **Iter 3 (2026-06-25):** ADR-022 (OCR lab ingestion, RuVector OCR) + `helix-ocr`: `extract → gate →
  record|queue`. The sanity gate queues (never coerces) candidates that are non-finite, unit-less, below an
  OCR-confidence floor, or physiologically implausible (a misread "28"→"2800000" is caught); survivors become
  OcrExtraction records with confidence capped at 0.8 (below a clean feed), code=None for later ADR-004
  normalization. Unblocks the real primary lab path (Quest/Labcorp have no consumer APIs). 107 tests; clean. Pushed.

- **Iter 4 (2026-06-25):** ADR-023 (semantic retrieval, RuVector HNSW/GraphRAG) + `helix-retrieval`. The
  retrieval contract: direct concept-code matches + vector neighbours + one graph hop, fused + recency-blended
  (180-day half-life) + deduped + top_k. Every result carries its score AND reason (direct/vector/graph) for
  audit (ADR-005) + verifier (ADR-008). Hard line: **retrieval is recall, not grounding** — candidates still
  pass the grounding gate. Embedder/Index injected (RuVector at edge), pure + testable. "Why am I tired?"
  surfaces ferritin+sleep+vitamin-D though they share no code. 112 tests; clean. Pushed.

## All 4 backlog capabilities integrated ✅
ADR-020 sensing (ruview) · ADR-021 genome (rvdna) · ADR-022 OCR (ruvector) · ADR-023 retrieval (ruvector).
Each: ADR + tested crate + clippy/fmt clean + anti-hallucination guardrails (capped confidence, screening-not-
diagnosis, provenance-required, recall≠grounding). Pushed to ruvnet/helix main each iteration.

## Remaining polish (SOTA hardening, not new load-bearing capability)
1. `cargo audit` re-check + property tests on the new adapters (sensing/genome/ocr/retrieval).
2. Expose the adapters via wasm; live "Ambient / Genome / Import" UI panels + screenshots.
3. Refresh COVERAGE.md to map ADR-020..023 → crates; then close the loop.
