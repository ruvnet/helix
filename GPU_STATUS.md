# Helix — Local-GPU Capability Loop

**Loop:** `/loop 5m using local NVIDIA GPU, implement remaining Helix capabilities in Rust, with ADRs,
test/validate, push to ruvnet/helix main — until complete`
**GPU:** RTX 5080 (16 GB, CUDA 13). **In-stack inference:** ruvLLM v2.1.0 (:8080) serving Qwen2.5-3B-Instruct;
ollama (:11434) fallback. **Rust ML available:** candle-core, ort, tokenizers, ndarray (workspace lock).

## Scope (what local GPU actually unblocks — Rust only, never Python)
| # | Capability | ADR | Crate | Status |
|---|-----------|-----|-------|--------|
| 1 | On-device LLM analyst (grounded compose) | ADR-026 | `helix-llm` | ✅ done (ruvLLM+ollama, GPU-validated) |
| 2 | Learned MiniLM text embedder (helix-retrieval) | ADR-027 | `helix-embed` | ⬜ next (candle/ort + MiniLM) |
| 3 | Learned visual encoder (helix-visual) | ADR-028 | `helix-vision` | ⬜ |
| 4 | Live connector clients (FHIR/wearable) — Rust scaffold | ADR-029 | `helix-connect` | ⬜ (sandbox/mock; real APIs need partner auth) |
| 5 | Federation network transport scaffold | ADR-030 | `helix-fed` | ⬜ (over helix-cohort primitive) |

Not GPU-unblockable (stay spec/seam): real hardware (mmWave/WiFi firmware, Seed), 3D WebGL twin,
regulatory/clinical sign-off.

## Ledger
- **Iter 1 (2026-06-26):** ADR-026 + `helix-llm` — on-device LLM analyst. LLM narrates the already-grounded
  facts (claims + deterministic trend + citations); it does NOT retrieve (ADR-005) or compute (ADR-007).
  Number-guard rejects any output value not in the facts → falls back to the deterministic template; backend
  is a trait. **ruvLLM is the in-stack default** (ADR-013), ollama fallback. Validated end-to-end on the
  RTX 5080: ruvLLM narrated a grounded ferritin answer, number-guard passed. Honest note: the guard blocks
  fabricated *values*, not added qualitative claims (e.g. "anemia") — that's the Verifier's job (ADR-008).
  6 unit tests + 1 GPU integration test (`--ignored`). clippy/fmt clean.
