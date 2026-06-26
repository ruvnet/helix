# Helix — Personal Health Intelligence (PHI)

> Your entire body — every record, signal, and result — assembled into one living dossier.
> **Private. Continual. Visual. And built so it can't make things up.**

[![Helix management console](docs/ui/screenshots/dashboard.png)](https://ruvnet.github.io/helix/ui/)

<p align="center">
  <a href="https://ruvnet.github.io/helix/ui/#ask"><img src="docs/ui/screenshots/answer.png" alt="Grounded, cited answer — open the live demo" width="49%" /></a>
  <a href="https://ruvnet.github.io/helix/mobile/"><img src="docs/ui/screenshots/mobile.png" alt="Helix mobile PWA — open the live demo" width="22%" /></a>
</p>

<p align="center"><sub>↑ the screenshots are clickable — they open the <a href="https://ruvnet.github.io/helix/">live demo</a> (real Rust pipeline in WebAssembly)</sub></p>

<p align="center"><b><a href="https://ruvnet.github.io/helix/">▶ Live demo</a></b> · the UI runs the real Rust pipeline compiled to WebAssembly</p>


Helix is a mobile-first *"functional-medicine specialist in your pocket."* It ingests
everything a person can know about their own body — EMR records, pharmacy history, phone
and wearable telemetry, genome, lab panels, sleep, recovery, nutrition, subjective logs,
and **always-on ambient sensing** — normalizes it into a single longitudinal **personal
health knowledge graph**, and puts a conversational, multi-agent analyst on top of it.

The differentiator is not "another health chatbot." **Every answer is grounded in the
user's own data, traceable to its source, graded for evidence quality, and bounded by an
explicit refuse-when-unknown policy.**

Built on the **ruvnet stack**:

- **RuVector** — self-learning vector + GraphRAG memory DB (the substrate of *context*).
- **Ruflo** — multi-agent meta-harness with PII-gating, learning loops, and audit trails
  (the substrate of *reasoning*).
- **Cognitum Seed** — always-on, contactless mmWave edge sensing (the *continuous signal*).
- **MetaHarness / Darwin Mode** — the whole stack is minted as a branded harness that
  self-optimizes toward *faithfulness* while the underlying model stays frozen.

## Repository layout

```
helix/
├── README.md                          ← this file
└── docs/
    ├── Helix-PHI-ADR-Product-Spec.md  ← the full product spec (v1.0.0, with diagrams)
    └── adr/
        ├── README.md                  ← ADR index
        └── ADR-001 … ADR-019          ← 19 detailed Architecture Decision Records
```

## Start here

1. **[The product spec](docs/Helix-PHI-ADR-Product-Spec.md)** — vision, capability wish
   list, reference architecture, anti-hallucination design, roadmap, and the
   differentiation vs. ChatGPT Health.
2. **[The ADR index](docs/adr/README.md)** — the 19 load-bearing architecture decisions,
   each researched and evidence-graded.

The three decisions that make or break a product in this space:
**anti-hallucination / data-grounding** ([ADR-005](docs/adr/ADR-005-retrieval-grounded-provenance-answering.md)–[008](docs/adr/ADR-008-verifier-critic-swarm-consensus.md)),
**privacy & data ownership** ([ADR-001](docs/adr/ADR-001-user-owned-local-first-vault.md), [011](docs/adr/ADR-011-federation-pii-stripped-cohort.md), [013](docs/adr/ADR-013-on-device-inference.md)),
and **clinical safety** ([ADR-009](docs/adr/ADR-009-red-flag-escalation-clinician-in-loop.md), [010](docs/adr/ADR-010-wellness-vs-samd-boundary.md)).

## What's built

This is no longer just a spec — the testable core is implemented in Rust and validated.

- **22 crates · 153 tests · 30 ADRs** · clippy/fmt clean · `cargo audit` clean.
- **Anti-hallucination core**: provenance grounding (`helix-provenance`), deterministic
  numerics (`helix-numeric`), evidence tiering (`helix-evidence`), red-flag escalation
  (`helix-escalation`), verifier (`helix-verifier`), the grounded-answer pipeline
  (`helix-pipeline`), and a real AEAD vault (`helix-vault`).
- **On-device AI on the local GPU** (Rust, never Python): the LLM analyst narrates grounded
  facts via **ruvLLM** with a number-guard (`helix-llm`), real **MiniLM** text embeddings
  (`helix-embed`), and a **layout-only** visual encoder (`helix-vision`) — each GPU-validated.
- **ruvnet-ecosystem integrations**: WiFi-CSI sensing (`helix-sensing`/RuView), genome +
  pharmacogenomics (`helix-genome`/rvDNA), OCR ingestion (`helix-ocr`), semantic retrieval
  (`helix-retrieval`), visual RAG (`helix-visual`/rupixel), privacy-preserving cohort
  (`helix-cohort`), FHIR connectors (`helix-connect`), and federation transport (`helix-fed`).
- **Web console + WASM mobile app** running the real pipeline in-browser — **[live demo](https://ruvnet.github.io/helix/)**.

Every integration keeps the discipline: the LLM narrates (never reasons), embeddings/visual
are recall (not grounding), genomics/cohort data is privacy-gated, and nothing diagnoses.

## Status

**v0.1.0 — implemented core, Proposed ADRs.** The Rust core is built, tested, and
GUI/GPU-validated. The ADRs are grounded by multi-source research but have not been
ratified by regulatory counsel or a clinical advisory board. Genuinely out of scope for
this workspace (and flagged in [`docs/COVERAGE.md`](docs/COVERAGE.md)): live partner-API
auth, the federation aggregator network, physical sensing hardware, the 3D WebGL twin, and
clinical/regulatory sign-off.

---

*Prepared by ISO Vision LLC. This repository provides architectural and product guidance,
**not** legal, regulatory, or medical advice. Engage regulatory counsel and clinical
governance before building any diagnostic or treatment-recommending features.*
