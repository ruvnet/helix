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
| 2 | Genome ingestion (VCF → records, GINA-aware) | rvdna | ADR-021 | `helix-genome` | ⬜ next |
| 3 | OCR lab-PDF ingestion (connector degradation) | ruvector | ADR-022 | `helix-ocr` | ⬜ |
| 4 | Vector / GraphRAG semantic retrieval | ruvector | ADR-023 | `helix-retrieval` | ⬜ |

## Ledger
- **Iter 1 (2026-06-25):** reviewed ruvector + ruview. ADR-020 (WiFi-CSI ambient sensing, RuView backend) +
  `helix-sensing` crate: RuView reading → ProvRecords (RUVW-* research codes, AmbientSensing, capped 0.5
  confidence) + screening flags mapping semantic states (possible-distress/fall-risk → Critical; elderly-
  inactivity/apnea → Urgent; ambient context → none) to the Escalation Guardian (ADR-009); rejects unsigned/
  non-finite; non-diagnostic framing throughout. 94 tests green; clippy+fmt clean. Pushed to main.

## Next picks
1. ADR-021 + `helix-genome`: rvdna VCF/variant → ProvRecords; user-owned, GINA-aware, screening not diagnosis (§7.4).
2. Expose helix-sensing + helix-neural via wasm; add a live "Ambient" data-source panel in the UI.
3. ADR-022 OCR lab ingestion; ADR-023 semantic retrieval. Re-run audit after any new deps.
