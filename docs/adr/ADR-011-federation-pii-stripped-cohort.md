# ADR-011: Federation for Opt-In, PII-Stripped Cohort Intelligence

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001 (User-owned vault), ADR-002 (Ruflo meta-harness), ADR-003 (RuVector), ADR-010 (Wellness/SaMD boundary), ADR-013 (On-device inference)

---

> **Regulatory and privacy disclaimer**: This ADR provides architectural guidance for a
> privacy-preserving federation layer. It does not constitute legal, regulatory, or clinical
> advice. "PII-stripped" and "anonymized" are engineering terms describing controls; their
> legal sufficiency under HIPAA's Expert Determination or Safe Harbor de-identification
> methods, under GDPR's anonymization standard (Recital 26), or under state genetic privacy
> laws must be verified with qualified privacy counsel before any cohort intelligence
> feature is enabled for users.

---

## Context

### The value of population context — and its privacy cost

Personal health data gains explanatory power when placed in population context. A ferritin
level of 28 ng/mL has richer meaning if the analyst can say "among people your age, sex,
and activity level who corrected ferritin below 30 ng/mL, 70% reported improved energy
within 8 weeks of supplementation." Without population context, Helix's analyst can only
compare a user's value to a published reference range — which is itself a population
aggregate, but a coarse and often poorly matched one.

This population context is one of the strongest arguments for a connected-data model. It
is also the primary source of privacy risk in health intelligence products.

The risk is not hypothetical. De-identification failures in genomic and health datasets
are well-documented in the scientific literature. A 1997 study (Sweeney) showed that 87%
of Americans could be uniquely identified from their zip code, birth date, and sex — all
fields that appear in standard health datasets. Health data is substantially more
identifying: a handful of biomarkers can uniquely identify an individual in a large cohort,
and genomic data is uniquely identifying by definition. Even "anonymized" aggregate
statistics — if sufficiently granular — can reveal individual information through
re-identification attacks, linkage attacks, and differencing attacks.

### Why raw-data federation is not an option

**[A]** ADR-001 establishes that the canonical copy of user data lives in an encrypted,
user-owned vault that Helix the company cannot read. The federation architecture must be
consistent with this constraint: raw health records cannot leave the user's vault because
Helix cannot decrypt them server-side.

Beyond the architectural constraint, raw-record sharing for population intelligence creates
exactly the consent and trust failure that the 23andMe event illustrated at scale. Users
sharing raw records for "research" with a third party (or a Helix-operated aggregation
service) are subject to all the risks of a centralized data aggregation: breach, policy
change, sale, and — most insidiously — re-identification from "anonymized" exports that
were not actually anonymous.

### The gap that federation fills

Helix's on-device intelligence (ADR-013) can reason over a user's personal longitudinal
data and retrieve from literature; it cannot provide population-contextualized comparisons
from real-world outcomes without access to aggregated signals from other consenting users.
This is the gap federation fills: a way to get the value of population context without
any raw records leaving any user's vault.

### Prior art and established approaches

**[B]** Privacy-preserving federated intelligence for health data draws on three established
technical approaches, each with well-studied tradeoff profiles:

**Federated learning (FL)**: Each device trains a local model on local data; only gradient
updates (not raw data) are aggregated. The aggregated model can then be distributed back to
devices. FL has been deployed for health applications (e.g., brain tumor segmentation, drug
discovery) with demonstrated privacy properties. FL does not share raw data; it shares
parameter updates, which themselves can leak information if not further protected.

**Differential privacy (DP)**: A mathematical framework (Dwork et al., 2006) that bounds
the information an aggregate computation reveals about any single individual by adding
calibrated noise to outputs. The privacy budget is parameterized by ε (epsilon): smaller ε
means stronger privacy (less information per query) but more noise in the aggregate. DP
provides formal, composable privacy guarantees.

**K-anonymity / l-diversity / t-closeness**: Statistical anonymization techniques that
ensure no individual in a released dataset is uniquely identifiable from a specified set of
quasi-identifiers. K-anonymity requires every record to be indistinguishable from at least
k-1 others across all quasi-identifiers. L-diversity strengthens this by requiring
diversity in sensitive attribute values within each equivalence class. T-closeness further
requires the distribution of sensitive values within a class to resemble the overall
population distribution.

**[B]** HIPAA's de-identification standard (45 CFR §164.514) defines two paths:
- **Expert Determination**: a qualified statistician certifies that re-identification risk
  is very small.
- **Safe Harbor**: 18 specific identifiers are removed and the covered entity has no
  knowledge that the remaining information could re-identify an individual.

For Helix, where the context is DTC (not HIPAA-covered entity) but the data includes
genetic information, the goal is to exceed HIPAA's Safe Harbor standard, not merely meet it.

---

## Decision

Helix implements opt-in, PII-stripped, differentially-private cohort intelligence via
**Ruflo Federation**, in a way that is consistent with the vault architecture of ADR-001.
Raw health records, genomic data, and identifiers never leave any user's device as part
of the federation pipeline.

### Consent model: opt-in, granular, revocable

Federation is **off by default**. Users are enrolled in cohort intelligence only via an
explicit, informed consent flow that:

1. Explains in plain language what data leaves the device (never raw records — only
   privacy-budget-bounded aggregated statistics) and what is returned (population-contextualized
   insights, never individual records of other users).
2. Specifies the data domains covered by consent: the user can opt in to biomarker
   aggregation but not genomic aggregation, or sleep data but not medication data.
3. Is revocable at any time from settings; revocation causes all contributed aggregate
   statistics associated with that user's participation to be marked for deletion in the
   next purge cycle (within 30 days).
4. Is renewed when the scope of federation expands (new data domains, new aggregation
   partners).

No dark patterns. Consent for cohort intelligence is a separate opt-in from consent for
individual feature use. A user who does not opt into federation receives the full benefit
of their own personal intelligence; cohort insights are additive, not gating.

### PII-stripping pipeline (Ruflo AIDefence gate)

Before any data token participates in cohort computation, it passes through a multi-layer
PII-stripping pipeline implemented in Ruflo's AIDefence agent:

**Layer 1 — Identifier suppression**:
Remove or hash: name, email, phone, address, exact date of birth (replace with 5-year
age band), exact zip/postal code (retain only region-level), device ID, account ID,
IP address, any free-text fields, clinical note text, provider names.

**Layer 2 — Quasi-identifier generalization**:
Generalize any quasi-identifier that, in combination, could re-identify: age (5-year bands),
geographic region (no finer than county/region), body metrics (height/weight rounded to
nearest 5 units), rare condition flags.

**Layer 3 — Sensitive attribute handling**:
Genomic data is processed on-device only and never exits the vault as part of federation,
even in aggregate form, without a separate, explicit genomic-cohort consent that is clearly
distinct from general health cohort consent.

**Layer 4 — K-anonymity floor**:
No aggregate statistic is released if fewer than k=50 users in the cohort contributed
to it for genetic or rare-condition data; k=10 for common biomarker domains. This prevents
small-cell disclosure of rare phenotype combinations. If a cohort query fails the k floor,
the response is suppressed rather than rounded.

**Layer 5 — Differential privacy noise injection**:
After PII stripping and k-anonymity enforcement, DP noise is added to numeric aggregates
before transmission:
- For common biomarker aggregates (mean ferritin in a cohort): ε ≤ 1.0 (total budget
  per user per 30-day rolling window)
- For health-outcome signals (energy improvement after intervention): ε ≤ 0.5
- For genetic-adjacent phenotypes or rare conditions: ε ≤ 0.1 (if the domain is in scope
  at all — default is excluded from federation)

The DP composition budget is tracked per user; once the rolling-window budget is consumed,
no further contributions are made until the budget resets. This prevents re-identification
via repeated queries.

**Layer 6 — AIDefence final scan**:
Ruflo's AIDefence agent performs a final pass on the outbound payload to detect any
residual PII patterns (named-entity recognition, regex for common identifier formats,
contextual PII detection).

### Federation architecture

The federation model uses a **centrally aggregated, edge-computed** pattern rather than
federated gradient learning (which is complex to deploy correctly and still requires
gradient protection):

```
Each user device (on-device compute):
  1. Decrypts relevant vault shard into RAM (never writes cleartext to disk)
  2. Computes local aggregate statistics (mean, std, percentile of biomarker X
     in user's data over time window T)
  3. Passes aggregate through PII-stripping pipeline (Layers 1–6 above)
  4. Transmits DP-noised, PII-stripped aggregate to Ruflo Federation endpoint

Federation endpoint (Ruflo server):
  1. Receives only DP-noised statistics (never raw records)
  2. Aggregates across consenting users (weighted mean, with k-floor enforcement)
  3. Stores only the population-level aggregate (not per-user contributions)
  4. Returns population context to querying devices on request

Return to device:
  1. Device receives population-level aggregate
  2. Analyst interprets result in context of user's own data
  3. Population context is presented as approximate ("people with profiles similar
     to yours..." — not as individual records of other users)
```

The server-side component sees only noise-added aggregates. It cannot reconstruct any
individual's health record.

### Data minimization and retention

- Per-user contribution records are not retained server-side. The server retains only
  population aggregates.
- Population aggregates are versioned and time-bounded; a rolling 90-day window is the
  default.
- Users who revoke consent trigger a deletion request for their contributions; since the
  server retains only aggregates (not per-user contributions), the practical effect is that
  post-revocation contributions stop and the next population aggregate re-computation
  excludes the revoked user.
- Genomic-derived data: excluded from standard federation by default. Any genomic cohort
  feature requires separate ADR-level review, separate consent, and legal review of
  applicable state genetic privacy laws in affected user jurisdictions.

### GDPR and state law alignment

**[A/C]** Under GDPR Article 9, health data requires explicit consent or another Art. 9(2)
basis. Helix's opt-in federation consent flow provides the explicit consent required. Data
that is genuinely anonymized (no longer re-identifiable per Recital 26) falls outside GDPR
entirely; however, the GDPR standard for anonymization is stricter than HIPAA's Safe Harbor,
and DP noise alone may not satisfy the standard. Legal review of the pipeline against GDPR
Art. 9 and Recital 26 is required before enabling federation for EEA users.

**[B]** For state genetic privacy laws, the default exclusion of genetic data from the
federation pipeline is the conservative position. If genomic cohort intelligence is added
as a future feature, per-state analysis of Illinois GIPA, Montana GIPA, Texas Genomic Act,
and any other applicable state laws is required before enabling it in those jurisdictions.

### Connection to related ADRs

- ADR-001: Raw vault data never leaves the device. Federation operates on derived,
  DP-noised aggregates only.
- ADR-002 (Ruflo): AIDefence (PII gate), audit hooks, behavioral trust scoring, and
  HIPAA/GDPR-mode audit trails are Ruflo capabilities used directly by the federation layer.
- ADR-010: Any cohort-level insight presented to users must satisfy the wellness/SaMD
  gates. Population-contextualized recommendations use the same evidence tiering (ADR-006)
  and same "not a diagnosis" framing required elsewhere.
- ADR-013: On-device computation of local aggregates is a prerequisite for the federation
  model described here.

---

## Alternatives Considered

### Alternative A: Raw-record federation (user shares records with consent)

Users opt in to sharing their full (though consent-gated) health records with a federated
research dataset for population-level analytics.

**Rejected** because: (1) inconsistent with ADR-001's vault model (raw records leaving the
device contradicts the "user holds keys" architecture); (2) re-identification risk from
raw health records in a cohort dataset is high even with nominal anonymization; (3) the
consent burden for raw-record sharing in health data is qualitatively different and
creates legal complexity that the DP-aggregate approach avoids.

### Alternative B: No federation (purely personal intelligence)

Helix provides only personal intelligence derived from the user's own data plus published
literature. No population context.

**Not adopted** but deferred to Phase 5 (per spec §8). Population context is a
meaningful product value. Federation is Phase 5 precisely because it requires a user base
large enough for meaningful DP-protected aggregates (k ≥ 50 floors require meaningful
cohort sizes). For Phase 0–4, the product is entirely local-first with no federation.

### Alternative C: Federated learning (gradient aggregation)

Use classical federated learning (FedAvg or similar): each device trains local model
updates; only gradients are shared and aggregated; the updated global model is pushed back.

**Not adopted for Phase 5** because: (1) gradient leakage attacks (e.g., gradient
inversion) can partially reconstruct training data from gradients — DP guarantees must
still be applied to gradients; (2) implementing a full FL training loop correctly for
health domains requires significant infrastructure (secure aggregation, Byzantine-robust
aggregation, poisoning defense); (3) for the Helix use case (population-contextualized
biomarker comparisons), aggregate statistics with DP are sufficient and much simpler.
FL is preserved as a future option for model personalization if on-device fine-tuning
requires it.

### Alternative D: Third-party research data aggregation partner

Contract with an existing health data aggregation platform (e.g., HealthVerity, Ciox,
Symphony Health) to provide population benchmarks without Helix building its own federation.

**Not adopted** because: (1) using a third-party data aggregator requires sending user
data to that aggregator, which conflicts with ADR-001 and the user's expectation that their
data is not shared; (2) population data from aggregators is typically claims- or EHR-derived
and would not match Helix's data domains (wearable signals, ambient sensing, functional
medicine biomarkers); (3) creates a new vendor dependency on a third party's privacy
practices.

---

## Consequences

### Positive

- Network-effect intelligence with genuine privacy preservation: cohort context improves
  the analyst's output without any user's raw data leaving their device.
- DP provides formal, mathematical privacy guarantees — not just contractual promises.
- The k-anonymity floor prevents rare-phenotype disclosure, protecting users with uncommon
  conditions.
- Opt-in model means users who do not trust federation still receive full personal
  intelligence value.
- Consistent with GDPR data minimization principles: the server never processes more data
  than the aggregate statistic.

### Negative

- DP noise reduces the accuracy of population aggregates; at small cohort sizes, the noise
  may make aggregates uninformative. This is an inherent tradeoff: stronger privacy = more
  noise = less signal. At Phase 5 scale, cohort sizes should be sufficient to absorb noise
  at ε ≤ 1.0 for common biomarkers.
- k ≥ 50 floor for genetic-adjacent data means cohort features for rare conditions are
  effectively unavailable until user base is very large.
- On-device aggregate computation requires meaningful compute on the user's device for
  the local statistics pass. Must be scheduled during idle/charging periods to avoid
  impact on battery and performance.
- The legal sufficiency of the pipeline under GDPR Recital 26 (anonymization) and HIPAA
  Expert Determination is not guaranteed by the technical design alone — it requires
  independent privacy counsel verification.

### Mitigations

- Privacy engineering review of the PII-stripping pipeline before Phase 5 launch,
  specifically targeting GDPR Recital 26 and HIPAA Expert Determination sufficiency.
- Adaptive ε budget per data domain: common biomarkers get more budget (ε = 1.0);
  sensitive domains get less (ε = 0.1 or excluded).
- Population aggregate quality metrics published in-app to users: "This comparison is
  based on N users" so users understand the precision of the cohort context.
- Genetic data excluded from federation by default and governed by a separate ADR
  (future) that incorporates per-state genetic privacy law analysis.

---

## Open Questions

1. **Minimum viable cohort size for DP at ε ≤ 0.5**: At Helix's initial user scale (Phase
   5 is likely 10K–100K users), what ε budget is achievable for rare-phenotype cohorts while
   maintaining useful signal? Engage a differential privacy specialist (e.g., OpenDP library
   contributors) to run privacy budget simulations before feature design.

2. **Legal anonymization sufficiency**: Does the six-layer pipeline produce output that
   qualifies as "anonymized" under GDPR Recital 26 and "de-identified" under HIPAA
   §164.514? This is a legal question requiring privacy counsel with quantitative expertise.

3. **Regulatory classification of cohort intelligence**: If Helix uses cohort signals to
   make population-contextualized health recommendations to individual users, does that push
   the recommendation toward SaMD territory (see ADR-010)? The wellness gate analysis must
   be extended to federation-sourced insights.

4. **Ruflo Federation's audit trail**: Ruflo's federation module should log each aggregation
   event (timestamp, data domain, cohort size, epsilon consumed, aggregate value) in an
   audit trail that the user can inspect. Is this audit trail implemented in Ruflo's current
   federation module, or does it need to be built?

5. **Consent for minor users**: If a parent uses Helix to track their child's health data
   and opts into federation, what are the consent implications? This likely requires a
   separate consent architecture for minors, which should be addressed before any user
   onboarding that could include minors.

---

## References

| # | Source | Evidence | URL |
|---|--------|----------|-----|
| 1 | GDPR Article 9: Special categories of personal data | [A] | https://gdpr-info.eu/art-9-gdpr/ |
| 2 | ICO: What are the rules on special category data? | [A] | https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/lawful-basis/special-category-data/what-are-the-rules-on-special-category-data/ |
| 3 | GDPR Article 9 text with commentary (GDPR-Text.com) | [A] | https://gdpr-text.com/read/article-9/ |
| 4 | EU GDPR secondary use of health and genetic data for research: Oxford Academic | [B] | https://academic.oup.com/idpl/advance-article/doi/10.1093/idpl/ipag001/8443007 |
| 5 | HHS.gov: HIPAA de-identification guidance (45 CFR §164.514) | [A] | https://www.hhs.gov/hipaa/for-professionals/privacy/laws-regulations/index.html |
| 6 | Orrick: Navigating privacy gaps and new legal requirements for genetic data, 2025 | [B] | https://www.orrick.com/en/Insights/2025/08/Navigating-Privacy-Gaps-and-New-Legal-Requirements-for-Companies-Processing-Genetic-Data |
| 7 | Inside Privacy: Multiple states enact genetic privacy legislation 2025 | [B] | https://www.insideprivacy.com/health-privacy/multiple-states-enact-genetic-privacy-legislation-in-a-busy-start-to-2025/ |
| 8 | Inside Privacy: States introduce new genetic privacy bills 2026 | [B] | https://www.insideprivacy.com/health-privacy/several-states-introduce-new-genetic-privacy-bills-in-early-2026/ |
| 9 | Global Policy Watch: Utah and South Dakota enact genetic privacy laws, 2026 | [B] | https://www.globalpolicywatch.com/2026/04/utah-and-south-dakota-enact-genetic-privacy-laws-as-other-states-advance-bills/ |
| 10 | AccountableHQ: Is genetic testing protected by HIPAA? | [B] | https://www.accountablehq.com/post/is-genetic-testing-protected-by-hipaa-privacy-rights-disclosures-and-compliance-guide |
| 11 | Exabeam: GDPR Article 9 — Special Personal Data Categories | [B] | https://www.exabeam.com/explainers/gdpr-compliance/gdpr-article-9-special-personal-data-categories-and-how-to-protect-them/ |
| 12 | 23andMe state AG coalition lawsuit, May 2025 (Stateline / Courthouse News) | [A] | https://stateline.org/2025/05/02/23andme-users-genetic-data-is-at-risk-state-ags-warn/ |
