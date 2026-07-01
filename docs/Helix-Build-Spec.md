# HELIX — Onboarding & Proactive Analyst: Technical Build Specification

> **Companion to** [`Helix-PHI-ADR-Product-Spec.md`](./Helix-PHI-ADR-Product-Spec.md) (the product/ADR spec, ADR-001–036).
> **This document** turns that vision into a buildable plan for two concrete outcomes the owner asked for:
> **(1)** a turnkey **onboarding system** that pulls in *all* of a person's health data, and
> **(2)** a **proactive functional-medicine analyst** that studies it and produces curated daily insights.
> **Status:** v0.1.0 — Proposed (awaiting go/no-go on Phase 0)
> **Prepared by:** ISO Vision LLC · **Date:** 2026-06-30
> **Form factor decision:** Local desktop/web app (Rust engine + local web wizard), local-first. *Not* mobile-first for v1.
> **Grounding:** RuVector/RVF, Ruflo, agentic-flow, agent-harness-generator — verified against rUv's real source via the RuvNet Brain (paths cited inline). Repo state verified by a read-only crate survey (2026-06-30).

---

## 0. How to read this

This is a **delivery spec**, not a re-statement of the product vision. It assumes the product spec's ADRs and adds new ones (**ADR-037+**). It is deliberately honest about what is **already built**, what is **not**, and what is **verify-at-build-time**. The one rule: *nothing here is asserted as done unless the survey confirmed it in code.*

---

## 1. Honest current-state assessment (survey-grounded, 2026-06-30)

The workspace is **~10,000 lines of clean, tested Rust across 27 library crates**. Zero `todo!()`/`unimplemented!()`. The **analyst brain is real**; the **plumbing to real, persistent user data is not**.

### 1.1 What is genuinely built and tested
| Capability | Crate | Reality |
|---|---|---|
| End-to-end grounded analyst loop | `helix-pipeline` | **Real.** `analyze()` runs abstain → escalate → deterministic numerics → ground-every-claim → tiered recommendation, proven in `tests/pipeline.rs` (3 outcomes) — but **in-memory, on test fixtures**. |
| The datum + provenance schema | `helix-provenance` | **Real.** `ProvRecord {source, measured_at, method, code, concept, value, unit, reference_range, confidence}` — the true "event with time + provenance." Citation gate rejects fabricated/dangling refs at construction. |
| Deterministic stats | `helix-numeric` | **Real.** mean/slope/%-change/range-crossings/pearson/CUSUM, property-tested, sub-µs. |
| Evidence tiering + abstention | `helix-evidence` | **Real.** Tier 1–4, staleness → GapNotice. |
| Red-flag escalation | `helix-escalation` | **Real.** Versioned registry, 5 cited rules (K⁺, Hgb, glucose, SpO₂, Seed REI). |
| 0–100 score, focus, bio-age, timeline | `helix-score`, `helix-focus`, `helix-bioage`, `helix-timeline` | **Real.** Decomposable score, focus rules, Levine PhenoAge (NHANES-validated), score-over-time. |
| Encryption primitive | `helix-vault` | **Real crypto** (XChaCha20-Poly1305, zeroize) — **but in-memory `BTreeMap` only.** |
| Browser app + WASM | `helix-wasm`, `ui/`, `mobile/` | **Real.** 20+ wasm-bindgen fns; a working vanilla-JS console + PWA that runs the real pipeline and **already file-imports Apple Health `export.xml`, FHIR, 23andMe, and lab-photo OCR** — displayed, **not persisted**. |

### 1.2 What is NOT built (the gaps that block the owner's asks)
1. **No persistence of any kind.** The vault is in-memory; **nothing survives closing the app.** No database, no `.rvf`, no file store. *(This is why "I don't know how to hook my data in" is the correct instinct — the data currently evaporates.)*
2. **No real vector index or graph.** `helix-retrieval::Index` is an **injected trait with no implementation**. No HNSW/RVF/GraphRAG is wired.
3. **No ontology tables.** `helix-ontology` enforces *policy* over pre-scored candidates; there is **no LOINC/RxNorm/SNOMED/UCUM dictionary or unit converter**. LOINC codes elsewhere are hardcoded literals.
4. **No native app, CLI, or server.** Libraries + tests + examples + a browser demo. The only "runs the product" path is WASM in a tab.
5. **Connectors are thin.** Only FHIR + Apple Health/23andMe **file** parsing. **No Quest, Walgreens, EMR, Lose It, or CSV.**
6. **LLM/embeddings need an external local server** (ollama/ruvLLM over HTTP); no in-process model.
7. ⚠️ **The five `*_STATUS.md` docs overstate reality** — they describe trait *seams* ("RuVector HNSW", "rvDNA") as if the backends are present. They are not dependencies of this workspace. Treat those docs as intent, not status.

---

## 2. Gap analysis — owner asks → what must be built

| The owner wants… | Blocked by gap(s) | Net new work |
|---|---|---|
| "Load **all** my data in — Walgreens, Apple Health, scale, Quest, EMR, Lose It" | #1 persistence, #5 connectors | Persistent store + 4–5 new connectors |
| "…and have it **stick**" | #1 persistence | Disk-backed encrypted vault (ADR-037) |
| "A **holistic event map** of any/all health elements" | #1, #2 | Persistent `ProvRecord` event store + graph (ADR-040) |
| "Study **trends**, see what's going on" | — (mostly built) | Wire `helix-numeric`/`helix-timeline` over persisted data |
| "**Proactive daily insights** & recommendations" | #1, #6, no driver | Daily briefing engine (ADR-042) + local LLM narration |
| "Integrate **Lose It** (food intake)" | #5 | Lose It connector (ADR-041) |
| "Curated, personalized, relevant" | — (built: evidence/focus/verifier) | Real data + LLM narrator |

**Reading:** the intelligence is largely done. The build is **storage + connectors + a driver + an app shell** — well-scoped, not speculative.

---

## 3. Target architecture — local desktop/web app

### 3.1 Shape
```
┌───────────────────────────────────────────────────────────────┐
│  LOCAL DESKTOP/WEB APP  (Tauri shell — Rust core + web UI)     │
│                                                               │
│  ┌───────────────┐   Onboarding wizard  ┌──────────────────┐  │
│  │  Web UI        │◀────────────────────▶│  Rust engine      │ │
│  │  (reuse ui/)   │   grounded answers    │  (helix-* crates) │ │
│  └───────────────┘   daily briefing      └────────┬─────────┘  │
│                                                    │            │
│   Connectors (helix-connect)          Analyst (helix-pipeline) │
│   AppleHealth·Quest·Walgreens·EMR·LoseIt      │                │
│        │  normalize (helix-ontology + tables) │                │
│        ▼                                        ▼               │
│   ┌─────────────────── PERSISTENCE (NEW) ──────────────────┐   │
│   │  redb  → sealed ProvRecords (helix-vault at rest)      │   │
│   │  .rvf  → semantic index (helix-retrieval::Index)       │   │
│   └────────────────────────────────────────────────────────┘  │
│                                                               │
│   Local LLM/embed: ruvLLM/ollama (HTTP) or ONNX-MiniLM via RVF │
│   Daily driver: launchd/cron → analyze() → briefing           │
└───────────────────────────────────────────────────────────────┘
      Nothing leaves the machine. User holds the keys (ADR-001).
```

### 3.2 Storage tier (the decision that was open — now grounded)
Two tiers, because RVF and relational lookup are different jobs (verified: RVF's public API returns only `id + distance`, no record iteration — `rulake/docs/research/rvf-backend-blocker.md`):

- **`.rvf` (RuVector Format) — the semantic index.** `rvf-runtime` v0.3.0 `RvfStore::{create,open,ingest_batch,query,compact}`, single crash-safe file with a witness chain (`ruvector/crates/rvf/rvf-runtime`; format ref `cognitum-seed/docs/seed/rvf-format.md`). Implements the empty `helix-retrieval::Index` trait. This is the RVF-first, zero-server vector store — the same backend AgentDB itself uses (`agentdb/src/backends/rvf/RvfBackend.ts`).
- **`redb` — the encrypted record store.** Pure-Rust embedded ACID KV. Holds `helix-vault`-sealed `ProvRecord`s at rest (closes gap #1) and backs the event-map/timeline exact lookups RVF can't serve. Sanctioned by the global storage hierarchy for Rust KV when AgentDB/TS would be overkill (and it would be, in a Rust desktop app).

> **Not** SQLite-by-default, **not** a cloud DB, **not** Pinecone/pgvector — per RVF-first. Vectors → RVF; encrypted structured records → redb.

### 3.3 App runtime — **Tauri** (lead) vs. local Axum server (fallback)
- **Tauri (recommended):** Rust backend + web frontend in one native, local-first desktop app; secure file access for `export.zip` drops; small footprint; Rust-first (honors the workspace). **Reuses the existing `ui/` almost as-is.**
- **Axum + browser (fallback):** a `localhost` Rust server the browser hits. Simpler to start, but it's a running server, not an app. Choose only if Tauri build friction appears.
- **Embeddings/LLM:** keep ruvLLM/ollama (already validated in-repo) for narration; **prefer local ONNX MiniLM via RVF** for *embeddings* to drop the server dependency on the retrieval path (ADR-044).

---

## 4. Data-source integration matrix

> **The 80/20 insight:** the scale, **Lose It**, and often Quest/EMR **already sync into Apple Health**, so one *"Export All Health Data"* `export.zip` (an `export.xml` + clinical CDA/FHIR records) likely already carries 70–80% of everything. We build the Apple Health rail first, then add the sources that *don't* funnel through it.

| Source | Primary mechanism | Format | Already in Apple Health? | Connector status | Fallback (ADR-012) |
|---|---|---|---|---|---|
| **Apple Health** | "Export All Health Data" → `export.zip` | `export.xml` + `clinical-records/*.json` (FHIR) + CDA | — (it *is* the hub) | **Parser exists** (`helix-connect::parse_apple_health`, ~14 HK types) — **needs: persist + more HK types + clinical-records FHIR** | n/a |
| **Scale (⚠️ model TBD)** | Vendor app → Apple Health (weight, body-fat, BMI) | via `export.xml` (`BodyMass`, `BodyFatPercentage`) | **Usually yes** | Covered by Apple Health rail if it syncs | Vendor CSV export |
| **Lose It (food)** | (a) Apple Health nutrition summary; (b) Lose It email **CSV** export (detailed foods) | HK dietary types + CSV | Partial (summary yes; per-food no) | **New (ADR-041)** — CSV parser for the food log | Manual |
| **Quest** | MyQuest export; some regions → Apple Health Records / patient FHIR | **PDF** (+ OCR), sometimes FHIR | Sometimes | **New** — PDF→OCR→LOINC (`helix-ocr` gate exists) or FHIR (`parse_observation` exists) | PDF/OCR |
| **EMR (doctor)** | SMART-on-FHIR OAuth **or** Apple Health Records **or** CCDA/Blue-Button download | FHIR JSON / CCDA XML | If provider connected in Health app | **Partial** — `FhirConnector` exists but **no partner OAuth**; CCDA parser new | User export + CCDA/PDF |
| **Walgreens** | Patient portal export; no clean consumer API | PDF / manual | Rarely | **New** — export parse + manual entry | Manual + barcode |
| **Genome (bonus)** | User-owned raw file | 23andMe raw `.txt` / VCF | No | **Exists** (`parse_23andme_raw`) | — |

**Verify-at-build-time:** exact Lose It CSV columns; whether the owner's scale syncs to Apple Health (needs model); current Quest/EMR patient-access surface; Walgreens export availability. Each is isolated behind a connector so a missing one degrades gracefully rather than blocking.

---

## 5. The onboarding wizard flow ("anybody can use it")

A guided, source-by-source flow in the local app. For each source: **explain → show exact steps → accept the file → parse → normalize → seal → confirm what came in.**

1. **Welcome & consent.** "Everything stays on this device. You hold the key." Generate/derive the vault key from a passphrase (ADR-037).
2. **Apple Health (start here).** Step-by-step with the real taps: *Health app → profile photo → Export All Health Data → AirDrop/save `export.zip` → drop it here.* Parse → normalize → seal → show a live tally ("312 values, 9 medications, 18 months of sleep" — the Figure 2 dossier, now from *your* file).
3. **Lose It.** *Lose It → Settings → Export Data → email CSV → drop the CSV here.*
4. **Quest.** *MyQuest → download report PDF → drop it* (OCR+LOINC), or connect via FHIR if available.
5. **EMR.** *Health app connected provider* (fastest), or *portal → download CCDA/Blue Button → drop it.*
6. **Walgreens.** Export or manual/barcode for the med list.
7. **Review & first insight.** Show the assembled event map and generate the **first grounded daily briefing** — proving the loop end-to-end on real data.

Every step is optional and re-runnable; the wizard tracks coverage and nudges the highest-value missing source next.

---

## 6. The holistic health event-map (ADR-040)

The "event map of any and all health elements" = a **persistent, unified `ProvRecord` store** (redb) with three read models over it:
- **Timeline** — every metric as a time series (`helix-numeric` slopes/change-points; `helix-timeline` for score-over-time).
- **Graph** — biomarker ↔ condition ↔ medication ↔ intervention ↔ subjective-state edges (the `ruvector-graph` Cypher/temporal model is the production target; a minimal in-repo adjacency is the MVP).
- **Semantic index** — `.rvf` embeddings so "why am I tired?" retrieves *your* relevant records (`helix-retrieval` over the RVF `Index` impl).

Memory that **ages correctly** is already shipped upstream and should be adopted, not rebuilt: `ruvector-temporal-coherence` (**ADR-211 ACCEPTED**) — temporal decay + coherence gating so recent/reinforced signals outrank stale one-offs.

---

## 7. The proactive daily-briefing engine (ADR-042)

The "give me useful insights every day" loop, local-first:
1. **Trigger** — a `launchd`/cron job (local equivalent of `ruvector` ADR-096's Cloud Scheduler) fires each morning, or on new-data import.
2. **Diff** — detect what changed since yesterday (new records, crossings, trend shifts) via `helix-numeric` on the persisted store.
3. **Analyze** — run `helix-pipeline::analyze()` per focus area (`helix-focus` picks them): abstain if data's thin, escalate red-flags, ground every claim.
4. **Narrate** — `helix-llm` (ruvLLM/ollama) turns *grounded facts only* into plain language; the number-guard blocks any figure not in the facts.
5. **Verify** — `helix-verifier` cross-family check on clinically meaningful claims before display.
6. **Deliver** — a glanceable briefing: what changed, what's trending, what (if anything) needs attention, each claim citing *your* record. Red-flags route to "see a clinician" (ADR-009), never optimization tips.

This is exactly the "functional-medicine specialist in your pocket" — assembled from parts that already exist, once real data persists beneath them.

---

## 8. New ADRs (037+)

| ADR | Title | One-line decision |
|---|---|---|
| **037** | Persistent encrypted vault (redb) | Give `helix-vault::VaultStore` a redb disk backend; passphrase-derived key; sealed `ProvRecord`s at rest. Closes gap #1. |
| **038** | RVF-backed semantic index | Implement `helix-retrieval::Index` on `rvf-runtime` (`.rvf`); metadata JSON carries record id. Closes gap #2 (vector half). |
| **039** | Local desktop/web app shell (Tauri) | Wrap the Rust engine + existing `ui/` in Tauri; local-first; secure file drops. Closes gap #4. |
| **040** | Unified health event-map | Persistent `ProvRecord` store + timeline + graph + semantic read models. The "holistic event map." |
| **041** | Connectors: Quest, Walgreens, EMR (CCDA), Lose It | Extend `helix-connect` (per ADR-012/029) with file-drop parsers + graceful degradation. Closes gap #5. |
| **042** | Proactive daily-briefing engine | Local scheduler → diff → analyze → narrate → verify → grounded briefing. |
| **043** | Ontology dictionary loading | Load real LOINC/RxNorm/SNOMED/UCUM tables to feed `helix-ontology`'s candidate scorer + unit conversion. Closes gap #3. |
| **044** | Local ONNX-MiniLM embeddings via RVF | Replace the ollama HTTP embedding call with in-process ONNX MiniLM (offline, no server) on the retrieval path. |

*(Each gets a full Context/Decision/Consequences ADR file under `docs/adr/` as its phase begins.)*

---

## 9. Phased build plan (verifiable milestones)

**Phase 0 — MVP vertical slice (prove the whole loop on real data).** ADR-037, -038, -039 (min), -040 (min).
- Persist: redb vault + `.rvf` index wired; vault sealed into the pipeline path.
- Ingest the owner's **real Apple Health `export.zip`** → normalize → seal → index.
- Answer **one real grounded question** about the owner's own data, and render the **first daily briefing**.
- **Definition of done (provable):** close and reopen the app → the data is still there → a cited answer about *your* ferritin/sleep/etc. re-derives from *your* stored records. Screenshot + record count as proof.

**Phase 1 — Onboarding wizard + the rest of the connectors.** ADR-041, -043.
- The guided source-by-source wizard; Quest (PDF/OCR), Lose It (CSV), EMR (CCDA/FHIR), Walgreens; real ontology tables.
- **Done when:** a non-technical person loads ≥3 sources unaided and sees a correct unified dossier.

**Phase 2 — Proactive daily analyst.** ADR-042, -044.
- Daily scheduler, day-over-day diffing, verified briefing, local ONNX embeddings, red-flag routing.
- **Done when:** a scheduled run produces a correct, cited briefing from *changed* data with a verifier pass — and abstains where data is thin.

**Phase 3 — Hardening & the vision layer.** Backup/recovery UX, the 3D twin (`helix-visual`), Darwin Mode (`helix-evolve`) once a real eval set exists, opt-in cohort (`helix-cohort`/`helix-fed`).

Each phase runs under the repo's QA gate (`cargo test`/`clippy -D warnings`) and ends green before the next begins.

---

## 10. Risks, open questions, verify-at-build-time

- **`vertical:health` MetaHarness template unverified.** ADR-017 assumes it; source shows only `minimal`/`vertical:devops`. **Verify or author it** before relying on it.
- **RVF read-back gap.** No public `read_all_vectors()` (rulake note) — fine for our design (RVF = ANN only; redb = records), but pin `rvf-runtime` and re-check on upgrade.
- **Connector realities:** Lose It CSV schema; the scale's Apple-Health sync (needs model); Quest/EMR patient-access surface; Walgreens export. All isolated; missing ones degrade, don't block.
- **On-device LLM quality ceiling** for narration; keep numerics deterministic (never let the model do arithmetic — ADR-007).
- **Clinical safety** stays non-negotiable: escalation thresholds and evidence tiering gate everything; wellness-not-diagnosis framing (ADR-010).

---

## 11. What I need from you to start Phase 0

1. **Your real Apple Health `export.zip`** (Health app → profile → *Export All Health Data*). This is the MVP's fuel.
2. **Your scale's make/model** — so I can confirm it rides in via Apple Health or needs its own parser.
3. **A Lose It CSV export** (when convenient) — to lock the food-log parser to the real columns.

Approve this spec (or redirect any part) and I'll begin **Phase 0**: wiring persistence and proving the end-to-end loop on your actual data.

> *This document provides architectural and delivery guidance, not legal, regulatory, or medical advice. Engage regulatory counsel and clinical governance before shipping diagnostic or treatment-recommending features.*
