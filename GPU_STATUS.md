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

- **Iter 2 (2026-06-26):** ADR-027 + `helix-embed` — learned MiniLM text embeddings on the GPU. all-MiniLM-L6-v2
  (384-dim, ruvnet standard) served locally (ollama, on-device, ADR-013) behind a `TextEmbedder` trait;
  `LearnedEmbedder` adapts it to `helix_retrieval::Embedder` (wires the real encoder into ADR-023), degrading
  to an empty vector on backend failure. Validated on GPU: a fatigue query embeds at 0.483 to fatigue/ferritin
  text vs 0.098 to an unrelated potassium-recipe sentence — real semantic recall. 4 unit + 1 GPU integration
  test; clippy/fmt clean. Embeddings = recall; grounding (ADR-005) stays strict.

- **Iter 3 (2026-06-26):** ADR-028 + `helix-vision` — learned visual encoder on the GPU (by composition):
  a vision model (moondream) constrained to LAYOUT-ONLY description → MiniLM embedding (ADR-027). Encodes
  appearance, never interpretation (ADR-025/010). **Value-guard** rejects any caption containing a digit and
  falls back to a neutral token. GPU-validated end-to-end over the real medical corpus: the guard PROVABLY
  fired — moondream tried to read lab-report values, the guard caught the digits and forced the neutral
  fallback (the safety property working). Honest limitation: caption quality on the synthetic SVG images is
  weak (moondream returns garbage on the synthetic x-ray), so retrieval is degenerate here; a true in-process
  CLIP/ColPali encoder (candle, GPU) behind the same `VisionCaptioner`/embedder traits is the quality upgrade.
  5 unit tests + 1 GPU integration test; clippy/fmt clean. base64 inlined (no new dep).
