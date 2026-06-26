# ADR-028: Learned Visual Encoder for Medical-Document Retrieval (local GPU)

**Status**: Proposed
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-025 (visual RAG), ADR-027 (text embeddings), ADR-010 (SaMD), ADR-013 (on-device), ADR-005 (grounding)

---

## Context

ADR-025 (`helix-visual`) built visual RAG with a **perceptual descriptor** floor
and a pluggable `VisualEmbedder`. The real recall quality wants a *learned*
visual encoder. The local GPU makes one feasible on-device — but ADR-025's hard
line stands: **a visual encoder for Helix encodes appearance, never clinical
interpretation.** It must not "read" or diagnose the image.

A vision model is available on the GPU (ollama, `moondream`). Used naively it
would describe findings ("possible infiltrate") — a diagnosis from pixels, which
ADR-010/025 forbid. Used *correctly* — constrained to **document type and visual
layout only** ("lab report table", "x-ray image", "line chart") — it yields a
semantic representation of *what the document looks like*, which is exactly the
retrieval signal we want.

## Decision

Add `helix-vision`: a **learned visual encoder by composition** — a GPU vision
model constrained to layout-only description, embedded by the MiniLM encoder
(ADR-027). Image → layout caption → vector. Behind a trait so a true CLIP/ColPali
in-process encoder (candle, GPU) drops in later.

1. **Layout-only, never findings.** The vision prompt asks only for document
   TYPE and visual layout in a few words, and explicitly forbids any value,
   finding, or diagnosis. This keeps the encoder on the *appearance* side of
   ADR-025/010.
2. **Value-guard.** The caption is rejected if it contains any digit — a layout
   description has no numbers; a number means the model started reading content.
   On rejection, fall back to a neutral type token (or the perceptual descriptor,
   ADR-025).
3. **On-device.** The vision model and the embedder both run on the local GPU;
   the image never leaves the device (ADR-013).
4. **Recall, not grounding.** The vector ranks *which documents look alike*
   (ADR-025); it surfaces a document for a human + the grounding gate (ADR-005),
   and asserts nothing clinical.
5. **Pluggable.** `VisionCaptioner` (the GPU vision backend) and `TextEmbedder`
   (ADR-027) are both traits — a CLIP image encoder, a different vision model, or
   a cloud-escalated path (ADR-019) all swap in without changing the contract.

## Alternatives Considered

- **Let the vision model describe findings.** Rejected outright — diagnosis from
  pixels (ADR-010/025).
- **In-process CLIP/ColPali now (candle, GPU).** The eventual right encoder, but
  the weight download + CUDA build are heavyweight; deferred behind the same
  trait. The layout-caption encoder ships the capability today, safely.
- **Perceptual descriptor only (ADR-025).** Kept as the guard's fallback; the
  learned encoder adds semantic-type recall on top.

## Consequences

**Positive.** Real, learned, semantic visual retrieval on-device; the value-guard
makes content-reading detectable and blockable; composes the GPU vision model with
the ADR-027 embedder; the trait keeps a true CLIP encoder one swap away.

**Negative.** Caption-based encoding is coarser than a native visual embedder;
depends on the vision model's layout fidelity; an extra model in VRAM.

**Mitigations.** Strict prompt + value-guard + neutral fallback; swap CLIP behind
the trait when eval demands; ADR-019 routes model tier.

## Open Questions

- In-process CLIP/ColPali (candle, GPU) vs. the caption pipeline — trait makes it
  swappable; pick per eval.
- Caption vocabulary control for stable, discriminative layout tokens.
- How visual-encoder matches reconcile with OCR records (ADR-022) and perceptual
  matches (ADR-025) in one ranked view.

## References

- ollama `moondream` vision model on the local GPU (layout-only prompting). **[A]**
- ColPali / CLIP — learned visual document encoders (the in-process target). **[A]**
- Helix ADR-025 (visual RAG, appearance≠interpretation), ADR-027 (embeddings), ADR-010 (SaMD), ADR-013. **[A]**

> Architectural/product guidance, not legal or medical advice. The encoder represents appearance; it never interprets or diagnoses an image.
