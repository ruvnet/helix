# ADR-055: External Medical-Literature Verifier (Live Citation Grounding)

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Extends**: ADR-006 (evidence tiering — Tier-3 definition unchanged), ADR-008 (verifier/critic — Pass 2 evidence-tier check)
**Related**: ADR-005 (grounding), ADR-033 (dashboard recommendations), ADR-052 (proof panel — surfaces the check)

---

## Context

ADR-006 defines Tier 3 as "peer-reviewed literature," cited with author/journal/year/
DOI. ADR-008's Verifier Pass 2 checks that a Tier-3 claim *cites* a publication and
that the tier label is structurally appropriate — but it does not itself independently
retrieve and confirm that the cited work actually exists, says what the claim says, and
has not been retracted. An LLM can format a fake citation perfectly; structural
well-formedness is not existence. The "proactive daily specialist" push wants
recommendations to cite living literature, not just structurally-valid-looking
citations.

**Grounding check (mandatory, via the ruvnet brain):** a named pattern for exactly this
already exists in the ruvnet ecosystem — `agentic-flow/examples/nova-medicina`
(author: ruv/ruvnet), described as "AI-powered medical analysis system with
anti-hallucination safeguards," whose `Verifier` class targets multi-source validation
against `pubmed`, `cochrane`, `uptodate`, and whose `Analyzer` documents "cross-
reference with PubMed, Cochrane, UpToDate."

**However**, on inspection every method across `verifier.js`, `analyzer.js`, and
`provider-search.js` is an unimplemented stub: `// TODO: Implement multi-source
verification`, `// TODO: Query PubMed, Cochrane Library`, returning
`{ verified: false, confidence: 0.0, sources: [], citations: [] }` unconditionally.
**[C] inferred / design-intent only** — nova-medicina is a named pattern and scaffold,
not a working verification system. Helix cannot adopt its code; it can, honestly,
adopt the *shape* (named-source multi-source check, confidence threshold,
contradiction-detection hook) as a naming and design reference.

A real, separately-grounded mechanism for live literature retrieval does exist in this
environment: the PubMed E-utilities API (reachable via a standard MCP PubMed
connector) provides live search, article metadata, and full-text retrieval — this,
not nova-medicina's stub, is the actually-workable integration point.

## Decision

**Add a live literature-grounding sub-step to the Verifier's existing Pass 2 (ADR-008):
Tier-3 claims must be checked against a real bibliographic source before being marked
Verified, not merely structurally validated.**

1. **Additive to ADR-006/008, not a new tier.** Tier 3's definition is unchanged; this
   ADR specifies *how* the Verifier confirms a Tier-3 citation is real rather than
   model-fabricated.
2. **Reference integration: PubMed E-utilities**, as the MVP live lookup — confirms
   the cited work exists, is not retracted, and its abstract is directionally
   consistent with the claim. Cochrane/UpToDate-class sources are later additions,
   pending licensing review (Open Questions).
3. **Name the pattern honestly.** Internal references to "the nova-medicina pattern"
   describe the *shape* adopted from the named ruvnet example — never a dependency on
   or reuse of its (stub) code. Helix's existing `helix-verifier` crate implements
   this fresh.
4. **Currency re-check.** A previously literature-grounded Tier-3 claim whose source
   is later retracted or superseded is caught on a periodic re-verification pass, the
   same staleness pattern ADR-006 already applies to Tier-1 data, applied here to
   literature currency.
5. **Failure is abstention, never silent downgrade.** If a citation cannot be
   confirmed (network unavailable, DOI not found, source contradicts the claim), the
   claim is dropped or downgraded to Tier-4-with-disclosure per ADR-006's existing
   abstention design — never presented as a passed Tier-3 check that didn't actually
   run.
6. **Surfaced in the Proof Panel (ADR-052).** A Tier-3 citation that passed live
   literature-grounding shows a distinct "checked against PubMed" affirmation, visibly
   different from a citation that only passed structural formatting checks.

## Alternatives Considered

- **Trust the model's parametric knowledge of the literature, no live retrieval.**
  Rejected: this is exactly the "Correctness is not Faithfulness" failure mode ADR-005
  cites — a model can cite a plausible but non-existent or misremembered study with
  high confidence.
- **Adopt nova-medicina's code directly as the verification backend.** Rejected: it is
  a confirmed unimplemented stub; shipping it would mean a component that always
  returns `verified: false, confidence: 0.0` — worse than no feature, because it looks
  functional while silently failing every check.
- **Build a full custom biomedical literature index in-house before launch.**
  Deferred, not rejected: a live PubMed lookup is a lower-effort, real, immediately
  gradeable MVP. A curated in-house Tier-2/3 knowledge base (ADR-006 Open Question 3)
  remains a future upgrade, not a blocker for this decision.

## Consequences

### Positive
- Tier-3 citations become independently checkable rather than trusted on formatting
  or model memory alone — closes a real, previously-open gap.
- Honestly names a known ruvnet pattern without overclaiming its (unimplemented) code.

### Negative
- Adds a network dependency (PubMed API) to Tier-3 verification, in tension with
  Helix's local-first/on-device posture (ADR-013) — one of the few places Helix must
  reach an external service.
- Adds latency to Tier-3-claim verification; retraction re-checks require a
  background job, not just point-in-time verification.

### Mitigations
| Risk | Mitigation |
|---|---|
| Network dependency vs. local-first | Scoped narrowly to citation verification only, not general retrieval — ADR-005's "retrieval from the user's own vault" rule stays intact for the primary answering path |
| Latency | Cache verified citations locally with a re-check TTL; don't re-hit the network per repeat citation |
| Lookup failure | Treated as abstention (degrade to Tier-4-with-disclosure), never a blocked response |

## Open Questions

1. Which sources beyond PubMed (Cochrane, UpToDate licensing) are viable at Helix's
   budget/scale?
2. Re-verification cadence for previously-checked citations.
3. Does "checked against PubMed" expire alongside the citation's own currency window,
   or is it permanent once confirmed?
4. Legal review of citing a licensed clinical reference (UpToDate) vs. open
   bibliographic sources only for MVP.

## References

- `agentic-flow/examples/nova-medicina/src/{verifier,analyzer,index,provider-search}.js`
  — confirmed named pattern, confirmed unimplemented stub **[C — design intent only]**
- Helix ADR-005, ADR-006, ADR-008, ADR-033, ADR-052

---

> Architectural/product guidance, not legal or medical advice. Literature-grounding
> strengthens citation integrity; it does not make Helix a source of clinical guidance
> and does not alter ADR-010's wellness/SaMD boundary.
