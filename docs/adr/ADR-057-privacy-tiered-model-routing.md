# ADR-057: Privacy-Tiered Model Routing (Connected vs Sealed)

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-013, ADR-019, ADR-026, ADR-048, ADR-056

---

## Context

### One seam, not two products

Helix already has three pieces that this ADR must unify rather than duplicate:

- **ADR-005/006/007** establish that the analytical work — retrieval, evidence tiering,
  reference-range and trend math, red-flag detection — is deterministic and grounded,
  independent of any model. A model never reasons or retrieves; it narrates.
- **ADR-026** shipped `helix-llm`: an on-device narrator behind an `LlmBackend` trait,
  with a **number-guard** that rejects any output introducing a value not present in the
  input facts, falling back to a deterministic template on rejection.
- **ADR-019** already designed a three-tier, learned (Tiny Dancer FastGRNN) cost/privacy
  router with a non-negotiable quality-bar floor and consent-gated cloud escalation.
  **ADR-013** already established on-device-preferred inference with the same escalation
  pattern.

What is missing is the **concrete hardware ladder** underneath ADR-019's Tier 1/2/3
abstraction, and a **single user-facing switch** — not three technical tiers exposed to
the user, but two modes people can actually reason about: whether their data leaves the
device at all. This ADR names that ladder, decides the switch, and states a hard,
testable invariant for the "data never leaves" mode.

### Grounding check for the Rung 0 (bare Seed) hardware ceiling

**Confirmed, [A], from direct inspection of the Cognitum Seed platform:**

- The Seed is a **Pi Zero 2 W**, 512 MB LPDDR2 RAM, Cortex-A53 @ 1 GHz — confirmed in
  `cognitum-seed/docs/seed/ADR-094-pi-zero-sparse-llm-cog.md` and
  `cognitum-seed/docs/cognitum-seed-specs.md`. The Cog Store's own promotional copy states
  the operating envelope for its 90 edge apps: **max 3 concurrent cogs**, **~388 KB average
  binary**, **~1.7 MB RAM per cog**, 100% local, written in Rust
  (`cognitum-seed/scripts/cognitum/promo-video-script.md`, "KEY STATS FOR LOWER THIRDS").
- **A general-purpose chat-LLM does not fit this envelope, and this has been measured, not
  assumed.** ADR-094 evaluated model candidates for Pi Zero 2 W: `Phi-3-mini 3.8B Q4` and
  `Llama-3.2-1B Q4` both **OOM**; only ultra-narrow sparse-attention micro-models
  (`SmolLM2-135M Q4`, `Qwen2.5-0.5B Q4`) are viable, and only as a single dedicated cog, not
  alongside anything else. **ADR-096 measured the result on live hardware**: a real
  `SmolLM2-135M-Instruct-Q4_K_M.gguf` loaded and generated coherent tokens, at
  **~0.08–0.14 tok/s** baseline (a 12-token completion took **83 seconds**); the
  optimization target after two tranches of work is only **3–10 tok/s**. Even at the
  stretch target, a several-sentence grounded narration would take tens of seconds to
  minutes, from a 135M-parameter model whose fluency is far below what ADR-026's narrator
  role needs. **[A]**
- Conclusion, stated honestly: it is not strictly true that *no* transformer of any size
  can execute on Rung 0 — a single-purpose, non-conversational 135M–0.5B sparse-attention
  model can, per ADR-094/096. It *is* true that no general-purpose, responsive **chat-LLM
  narrator** is viable there. Rung 0 therefore runs the deterministic pipeline plus a
  **classification-only** micro-model, not a generative one.
- **The micro-model that does fit** is a **FastGRNN classifier** (Tiny Dancer) — already
  real and wired on the Seed platform, not proposed: `cognitum-seed/src/cognitum-agent/src/
  neural_router.rs` implements "Tiny Dancer FastGRNN routing integration" and is used today
  to decide whether an incoming task/signal warrants further processing. **[A]** for
  FastGRNN/Tiny Dancer specifically. The `ruv-fann` crate name used loosely for "the
  ruvnet micro-net family" is **not** independently confirmed as a Seed dependency in this
  scan — grade **[C]** for that specific crate name; the decision below routes on FastGRNN,
  which is confirmed.

### Grounding check for Rung 1 (local node) and its honest quality gap

**[B, Proposed]** `agent-harness-generator/docs/adrs/ADR-150-tailscale-local-frontier-
concurrent-benchmarks.md` establishes the exact pattern Rung 1 needs: a bigger local box
the user owns (Mac Studio/mini, or a Pi-5-class appliance) runs `ruvllm serve` / Ollama /
llama.cpp, bound to a Tailscale (or LAN) address, exposing an OpenAI-compatible
`/v1/chat/completions` endpoint — "over Tailscale it looks exactly like the OpenAI API...
but free and air-gapped." A `Qwen2.5-Coder-32B` GGUF fits in 48 GB unified memory alongside
other workloads with Metal acceleration.

**The same ADR is the source of the honest caveat this decision must carry**: measured
results (SWE-bench code-repair, not a health benchmark) show local 7B and 14B models
resolving at **~¼–⅓ of the hosted-frontier rate** — `qwen2.5-coder:14b` + repair scored
**6.7%** against hosted `deepseek` at **29.3%** on the same harness and corpus (full-300
SWE-bench Lite); an earlier stratified sample put local 7B at "**~⅓–¼**" of the hosted
rate. ADR-150's own conclusion: **"the model is the binding constraint,"** not the harness.
**[B]** — this is a coding-benchmark measurement, not a health-narration measurement, and
is cited here as a directional signal about the local-vs-frontier capability gap, not as a
Helix-specific quality number. It is the honest reason Rung 1 is *capability-limited*
relative to Rung 2, even though it is architecturally equivalent (same OpenAI-compatible
seam, zero third-party egress either way).

### Grounding check for the routing seam itself

- **Helix side, [A], directly inspected**: `crates/helix-llm/src/lib.rs` ships a tested
  `LlmBackend` trait, a `LocalLlmBackend` with `ruvllm()` (default,
  `http://127.0.0.1:8080/v1`, `Qwen/Qwen2.5-3B-Instruct`) and `ollama()`
  (`http://127.0.0.1:11434/v1`) presets, and a passing number-guard test suite (5 unit
  tests, `cargo test -p helix-llm`). **No non-local backend exists in code today, and a
  workspace-wide grep confirms zero callers outside the crate itself** — `helix-llm` is
  built and tested in isolation; nothing in the Helix pipeline invokes `compose()` yet.
  This ADR is **design-intent**, not a description of a wired feature — graded honestly,
  matching the discipline ADR-150 itself models ("no number is claimed until a real served
  run exists").
- **ADR-019 (this repo)** already specifies the abstraction this ADR's ladder slots under:
  a three-tier FastGRNN router (Tiny Dancer) with a hardcoded, Darwin-immutable floor —
  `quality_bar` cannot be lowered below the grounding minimum, and consent + AIDefence PII
  gating wrap every cloud call.
- **[B, Proposed]** `agentic-flow/docs/adr/ADR-073-metaharness-router-cost-optimal-model-
  routing.md`: `agentic-flow`'s `ModelRouter` (`src/router/router.ts`) already picks
  providers/models across **anthropic / openrouter / onnx / gemini / ollama** by static
  config rules today, and ADR-073 proposes layering a cost-optimal, quality-bar-gated mode
  on top via `@metaharness/router` / `@ruvector/tiny-dancer` (already a dependency).
- **[A], shipped and read directly**: `open-claude-code/v2/src/optimize/router.mjs` is a
  live, zero-dependency cost-cascade router already in production use elsewhere in the
  stack. Its `DEFAULT_LADDER` (`claude-haiku-4-5` → `claude-sonnet-4-6` → `claude-opus-4-6`,
  cheapest → most capable) and its deterministic `estimateComplexity()` heuristic are the
  concrete precedent for "cheapest model that clears a quality bar, escalate on failure" —
  the same thesis ADR-019 already adopted for Helix via Tiny Dancer.
- **ADR-056 (this repo)** confirms a paired Cognitum Seed exposes a real REST+MCP surface
  (`https://cognitum.local:8443`) reachable over the local network with no cloud round
  trip — a plausible auto-discovery target for detecting a Rung-1 local node, reusing an
  already-implemented capability rather than inventing new pairing/discovery machinery.
- **ADR-048 (this repo)** already requires AIMDS/AIDefence at every inbound/outbound LLM
  surface. Any Connected-mode (Rung 2) call is a cloud escalation in ADR-013/019's sense
  and must pass through the same gates: consent, PII-strip, audit log.

---

## Decision

**Helix exposes exactly two user-facing modes — Connected and Sealed — over one
model-routing seam. In both modes, all analytical work (retrieval, evidence tiering,
reference ranges, trend math, red-flag escalation) stays deterministic and local per
ADR-005/006/007; the model's only job, in every mode, is to narrate already-grounded
facts, bounded by helix-llm's number-guard (ADR-026).**

Underneath the two user-facing modes is a three-rung **privacy-tiered ladder**. Capability
rises and privacy falls as the ladder climbs; a hard egress gate protects the bottom two
rungs.

### The ladder

| Rung | Name | What runs | Egress | Narrator quality |
|---|---|---|---|---|
| **0** | Sealed / bare Seed | Deterministic engine + FastGRNN (Tiny Dancer) classification-only micro-model. **No chat-LLM.** | **Zero.** Pi Zero 2 W (512 MB RAM, max 3 concurrent cogs) cannot host a responsive general-purpose transformer — measured, not assumed (ADR-094/096: best-case micro-LLM candidate manages ~0.1–10 tok/s). | Grounded, cited, structured findings + templated language only. |
| **1** | Sealed / local node | A real local LLM (7–32B GGUF via Ollama / ruvLLM / llama.cpp) on hardware the user owns — a Pi-5-class appliance or their own Mac — exposed OpenAI-compatibly over LAN/Tailscale. | **Zero third-party egress.** Traffic never leaves the user's own network/tailnet. | Real generative narration; honestly ~¼–⅓ of frontier quality on current 7–14B local models per the measured coding-benchmark signal (ADR-150) — a directional caveat, not a health-specific number. |
| **2** | Connected | A frontier model (e.g. Claude Opus 4.8) via provider API, for maximum capability. | The grounded prompt leaves the device. | Highest quality narration/reasoning-adjacent phrasing available. |

**User-facing surface is exactly two switches, not three:**

- **Sealed** — auto-selects the best rung the user's *available* hardware can host: Rung 1
  if a local LLM node is detected (paired Cognitum Seed running a local model, or a
  user-declared local endpoint), Rung 0 otherwise. The user never manually picks 0 vs 1;
  Helix probes for a local node and falls back gracefully.
- **Connected** — Rung 2. Requires the same consent + PII-minimization + AIDefence gate
  already specified in ADR-013/019/048 before any prompt leaves the device.

Exposing the three technical rungs directly to the user was considered and rejected (see
Alternatives) — the ladder is an internal routing concept; the product surface is binary.

### The hard Sealed-mode egress invariant

**In Sealed mode, the model endpoint MUST resolve to a local address. Any non-local
endpoint is refused.** This is the privacy guarantee, and it must be testable, not
advisory:

- The backend selected while in Sealed mode is checked against an allowlist before every
  call: `127.0.0.1` / `localhost`, RFC 1918 private ranges, the Tailscale CGNAT range
  (`100.64.0.0/10`), and mDNS `.local` names resolved and re-checked against the same
  ranges (guards against DNS rebinding to a public IP under a `.local`-looking name).
- A configured base URL that does not resolve into this allowlist is rejected at
  construction time — Sealed mode does not silently fall through to Connected; it refuses
  and falls back further down the ladder (to Rung 0's deterministic template) rather than
  ever calling out.
- This mirrors ADR-013's framing exactly: an architectural guarantee, not a contractual
  one. "Sealed" must mean the code cannot dial out, the same way ADR-013 argues a BAA is a
  weaker guarantee than a system that structurally cannot transmit.
- The check lives in the routing seam (below), not in each caller — one enforcement point,
  testable in isolation (unit tests can assert `is_local_endpoint("https://api.anthropic.
  com/v1")` → `false`, `is_local_endpoint("http://127.0.0.1:8080/v1")` → `true`,
  `is_local_endpoint("http://100.x.x.x:11434/v1")` → `true`).

### The routing seam

The seam is `helix-llm`'s `LlmBackend` trait (ADR-026), extended — not replaced — with a
mode-aware selector, backed by the same primitives ADR-019 already named:

1. **`SealedBackend`** — wraps today's `LocalLlmBackend` (already shipped, tested), adding
   the egress allowlist check described above. Auto-detection between Rung 0 and Rung 1:
   probe for a reachable local OpenAI-compatible endpoint (a paired Cognitum Seed's
   `cognitum.local:8443` MCP surface per ADR-056, or a user-configured local URL); on
   success, narrate via Rung 1; on failure/timeout, fall back to Rung 0 — deterministic
   template + FastGRNN-classified structured findings, no generative call at all.
2. **`ConnectedBackend`** — a new implementation of the same `LlmBackend` trait, routed
   through ADR-019's Tier 3 path: explicit per-call-type consent, AIDefence PII-strip
   (ADR-048), audit log entry, then the frontier provider call.
3. **Mode selection** picks which backend implementation `compose()` (ADR-026) receives.
   The number-guard, the deterministic-facts-in contract, and the system prompt are
   **identical across all three rungs and both modes** — only the backend changes. This is
   what makes it one seam: nothing about grounding, retrieval, or the anti-hallucination
   pipeline varies by rung.
4. **ADR-019's learned FastGRNN router** governs *within* Connected mode which cloud tier
   (Tier 2 small model vs Tier 3 frontier) a given task escalates to; this ADR does not
   change that policy — it adds the Sealed/Connected product-level switch and the Rung 0/1
   hardware-detection layer beneath ADR-019's Tier 1.

### Configuration sketch (illustrative, not yet implemented)

```json
{
  "user_mode": "sealed",
  "sealed_allowlist": ["127.0.0.1", "localhost", "10.0.0.0/8", "172.16.0.0/12",
                        "192.168.0.0/16", "100.64.0.0/10", "*.local"],
  "sealed_rung1_probe": { "endpoints": ["cognitum.local:8443", "user_configured"],
                          "timeout_ms": 500 },
  "connected_requires": ["consent_per_call_type", "aidefence_pii_gate", "audit_log"],
  "rung0_model": "fastgrnn-classifier (tiny-dancer)",
  "rung1_models": ["qwen2.5-coder:14b", "qwen2.5-coder-32b (Mac, 48GB unified)"],
  "rung2_model": "claude-opus-4-8"
}
```

---

## Alternatives Considered

### Alternative 1: Two separate products/codebases (air-gapped SKU, cloud SKU)

Ship a fully air-gapped build and a separate cloud-enabled build. Rejected: doubles
engineering and QA surface for no privacy benefit over a single seam with a hard mode
switch; defeats the "one codebase, one seam" simplicity ADR-019/026 already established.
A single binary with a testable invariant is a stronger, cheaper guarantee than two
binaries a user has to trust were built correctly.

### Alternative 2: Expose all three rungs to the user directly

Let the user manually pick Rung 0/1/2 instead of Sealed/Connected. Rejected: the rung a
device can reach is a hardware fact, not a preference — most users cannot usefully reason
about "does my Seed have a paired local LLM node." Sealed auto-selecting the best rung
available already matches ADR-013's "profile the device, auto-select the best model
variant" pattern. Power users retain visibility (the active rung is always shown in the
UI), but the *decision* is automatic.

### Alternative 3: Soft/advisory local-only preference in Sealed mode (no hard gate)

Treat "Sealed" as a UI preference that biases routing toward local models without a code-
level refusal of non-local endpoints. Rejected: this is exactly the "contractual, not
architectural" guarantee ADR-013 argues is weaker. A misconfiguration (wrong base URL,
a Rung-1 probe that returns a public address) would silently leak health context with no
error. The hard allowlist check makes the guarantee testable and fails closed.

### Alternative 4: Sealed mode never detects Rung 1; always Rung 0 only

Simplify by making Sealed mean only the bare-Seed rung, requiring a separate manual step
to use a local LLM node. Rejected: this throws away real, zero-additional-privacy-cost
capability that a user's own hardware (Mac, Pi-5 appliance) can provide. The entire point
of a three-rung ladder under a two-mode switch is that Sealed should get the best
available quality without the user manually managing hardware detection.

---

## Consequences

### Positive

- **One seam, testable privacy.** A single `LlmBackend` extension point (ADR-026) serves
  all three rungs and both user modes; the privacy guarantee is a unit-testable allowlist
  check, not a design intention scattered across callers.
- **Never worse than deterministic.** Rung 0's total absence of a chat-LLM does not
  degrade safety — the deterministic pipeline (ADR-005/006/007) and the number-guard
  (ADR-026) already make the model an optional narrator, not a required reasoner. A user
  on a bare Seed still gets grounded, cited, structured findings.
- **Honest capability ladder.** Naming the Rung 1 quality gap explicitly (¼–⅓ of frontier,
  from a real measurement, even if from a different benchmark domain) prevents Helix from
  overselling "local LLM = same quality as cloud."
- **Reuses real, already-built capability.** Cognitum Seed's REST/MCP discovery (ADR-056),
  the Tiny Dancer FastGRNN router (ADR-019, and confirmed live on the Seed via
  `neural_router.rs`), and `helix-llm`'s existing trait/number-guard are all real
  components this decision arranges rather than reinvents.
- **Consistent user mental model.** "Sealed" and "Connected" map directly to a single
  question — does health data ever leave this device — which is the question users
  actually have, rather than a three-way technical choice they cannot evaluate.

### Negative

- **helix-llm has no cloud backend and no callers yet.** This ADR is design-intent: the
  `ConnectedBackend` implementation, the mode-selection layer, the egress allowlist check,
  and the Rung-1 auto-probe are all unbuilt. Until then, helix-llm remains an isolated,
  tested crate that nothing in the pipeline invokes.
- **Rung 0 has zero generative narration.** Users on a bare Seed with no phone or local
  node get templated language only — a real capability gap versus Rung 1/2, not just a
  privacy tradeoff. This must be communicated honestly in-product, not glossed over.
- **Rung 1 quality gap is real and only loosely characterized for health tasks.** The
  ¼–⅓ figure comes from a code-repair benchmark, not health narration. Helix's own
  eval set (once it exists, per ADR-018/019) is needed before this ladder can carry a
  health-specific quality number instead of a borrowed directional one.
- **Auto-detection adds a failure surface.** Probing for a Rung-1 local node (timeout,
  false negative on a slow-to-respond Seed, false positive on a stale cached address) is
  new logic that did not exist before; a broken probe could silently strand a user on
  Rung 0 when a local node is actually available, or (worse, if the allowlist check has a
  bug) treat a non-local address as local.
- **Egress allowlist correctness is safety-critical.** A bug in the allowlist (missing a
  CGNAT range, mishandling a `.local` name that actually resolves publicly, a DNS-rebinding
  gap) directly breaks the one invariant this ADR exists to guarantee.

### Mitigations

| Risk | Mitigation |
|---|---|
| No cloud backend / no callers yet | Track as explicit implementation debt; this ADR gates on ADR-019's existing consent/PII/audit machinery being wired before `ConnectedBackend` ships |
| Rung 0 has no generative narration | UI states plainly which rung is active and what that means ("structured findings only — no local LLM node detected") |
| Rung 1 quality gap unmeasured for health tasks | Do not claim health-specific quality parity; carry the coding-benchmark figure as a caveat only until a Helix eval set exists (ADR-018/019 dependency) |
| Rung-1 probe failure surface | Fail closed to Rung 0 (never to Connected) on any probe ambiguity; log probe outcome for diagnosis |
| Egress allowlist bugs | Allowlist logic isolated in one function with its own unit test suite (loopback, RFC1918, CGNAT, `.local`, and known-bad public addresses as explicit negative cases); code review requirement before any change to the allowlist |

---

## Open Questions

1. **Egress-gate implementation specifics.** Exact allowlist ranges, `.local`/mDNS
   resolution-then-recheck order, and DNS-rebinding defenses need a concrete design before
   `SealedBackend` ships — this ADR states the invariant, not the final regex/parser.

2. **Rung-1 auto-detection protocol.** Probe order and timeout between a paired Cognitum
   Seed's MCP surface (ADR-056) and a user-declared local endpoint; how false negatives
   are surfaced to the user versus silently retried.

3. **Minimum viable Rung-1 model.** Given ADR-150's honest ¼–⅓ gap on 7–14B local models,
   is there a floor model size/quant below which Rung 1 should not be offered at all
   (falling back to Rung 0 instead of shipping a narration quality users will distrust)?

4. **Connected-mode consent granularity.** Per-call-type versus standing consent for Rung 2
   is ADR-019 Open Question 4's territory; this ADR should inherit whatever that resolves
   to rather than deciding it independently.

5. **`ruv-fann` vs. Tiny Dancer FastGRNN naming.** This ADR routes Rung 0's micro-model
   decision through the confirmed FastGRNN/Tiny Dancer integration (`neural_router.rs`);
   whether a distinct `ruv-fann` crate is also intended for Rung 0 was not confirmed in
   this scan and is graded **[C]** pending verification — do not build against that name
   without re-checking.

6. **Helix-specific quality benchmark.** When Helix's own health-narration eval set exists
   (ADR-018 gating condition), replace the borrowed coding-benchmark quality-gap figure
   with a measured Helix number for Rung 1 vs Rung 2.

---

## References

- `crates/helix-llm/src/lib.rs`, `crates/helix-llm/Cargo.toml` — `LlmBackend` trait,
  `LocalLlmBackend` (ruvLLM default / Ollama fallback), number-guard, 5 passing unit
  tests; confirmed zero external callers in the workspace. **[A]**
- Helix ADR-005, ADR-006, ADR-007 — deterministic, grounded pipeline the model never
  bypasses. **[A]** (local ADR)
- Helix ADR-013 — on-device-preferred inference, consent-gated cloud escalation,
  architectural (not contractual) privacy guarantee. **[A]** (local ADR)
- Helix ADR-019 — three-tier learned (Tiny Dancer FastGRNN) cost/privacy router,
  Darwin-immutable quality-bar floor, consent + PII-gate for cloud escalation. **[A]**
  (local ADR)
- Helix ADR-026 — `helix-llm` on-device narrator, number-guard, LocalLlmBackend
  ruvLLM/ollama presets. **[A]** (local ADR)
- Helix ADR-048 — AIMDS/AIDefence at every inbound/outbound LLM surface. **[A]** (local ADR)
- Helix ADR-056 — Cognitum Seed REST+MCP surface (`cognitum.local:8443`), confirmed real
  and offline-capable; candidate Rung-1 discovery target. **[A]** (local ADR)
- `cognitum-seed/scripts/cognitum/promo-video-script.md` ("KEY STATS FOR LOWER THIRDS") —
  Pi Zero 2 W, max 3 concurrent cogs, ~388 KB avg binary, ~1.7 MB RAM per cog, 100% local,
  Rust. **[A]**
- `cognitum-seed/docs/seed/ADR-094-pi-zero-sparse-llm-cog.md` — Pi Zero 2 W hardware
  envelope (512 MB RAM), model-viability matrix (Phi-3-mini 3.8B Q4 / Llama-3.2-1B Q4 OOM;
  SmolLM2-135M / Qwen2.5-0.5B Q4 marginally viable as a single dedicated cog). **[A]**
- `cognitum-seed/docs/seed/ADR-096-pi-zero-sparse-llm-throughput-optimization.md` —
  measured live-hardware result: SmolLM2-135M Q4_K_M at ~0.08–0.14 tok/s baseline,
  3–10 tok/s optimization target. **[A]**
- `cognitum-seed/src/cognitum-agent/src/neural_router.rs` — Tiny Dancer FastGRNN routing,
  confirmed real and wired on the Seed agent. **[A]**
- `agent-harness-generator/docs/adrs/ADR-150-tailscale-local-frontier-concurrent-
  benchmarks.md` — local-frontier-over-Tailscale pattern (OpenAI-compatible, zero egress);
  measured local 7B/14B resolve-rate at ~¼–⅓ of hosted frontier on SWE-bench (coding
  benchmark, cited here as a directional signal, not a health-specific number). **[B,
  Proposed]**
- `agentic-flow/docs/adr/ADR-073-metaharness-router-cost-optimal-model-routing.md` —
  `ModelRouter` across anthropic/openrouter/onnx/gemini/ollama; proposed cost-optimal,
  quality-bar-gated layer via `@metaharness/router`/`@ruvector/tiny-dancer`. **[B,
  Proposed]**
- `open-claude-code/v2/src/optimize/router.mjs` — shipped, zero-dependency cost-cascade
  router; `DEFAULT_LADDER` (haiku-4-5 → sonnet-4-6 → opus-4-6), deterministic complexity
  heuristic. **[A]**

---

*This document provides architectural guidance, not legal, regulatory, or medical advice.
Helix is a decision-support tool, not a diagnostic authority. The model, at every rung of
this ladder, narrates already-grounded facts; it never diagnoses, recommends, or invents.*
