# ADR-007: Deterministic Numeric/Trend Engine

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-003, ADR-005, ADR-006, ADR-008, ADR-016, ADR-018

---

## Context

### LLMs cannot do arithmetic reliably over health time-series data

The single most dangerous and least-discussed failure mode of LLMs in health applications
is numeric. A language model trained to predict the next token has no intrinsic arithmetic
engine. It reproduces number-like outputs by pattern-matching on how numbers appear in
training data — not by computing. For simple single-step arithmetic this produces
approximately correct results often enough to feel reliable. For multi-step, multi-value,
time-indexed calculations over a user's personal health data, it fails in ways that are
difficult to detect and potentially harmful.

The empirical record on this is now substantial.

"Mathematical Reasoning in Large Language Models: Assessing Logical and Arithmetic
Errors across Wide Numerical Ranges" (arXiv 2502.08680, 2025) found that LLM arithmetic
failures increase rapidly as operand magnitude or result precision increases, and that
errors are systematic rather than random — models fail in the same ways across the same
numeric patterns [A]. A companion paper "Beyond Accuracy: Diagnosing Algebraic Reasoning
Failures in LLMs Across Nine Complexity Dimensions" (arXiv 2604.06799, 2026) showed that
LLMs rely on superficial pattern-matching rather than arithmetic algorithms, leading to
predictable failure modes in multi-step computations involving intermediate values [A].

"Large Language Model Reasoning Failures" (arXiv 2602.06176, 2026) identified that
failures originate primarily from architectural and representational limits — tokenization
of numbers, positional encoding, training data composition — rather than from prompting
or task framing. These are not fixable by better prompts; they are structural [A].

Critically for Helix: "A Picture is Worth A Thousand Numbers: Enabling LLMs Reason about
Time Series via Visualization" (arXiv 2411.06018, 2024) found that "a key cause of LLM
failures for time series reasoning is the numerical modeling of time-series data, leading
to difficulty of feature extraction." LLMs cannot reliably extract trends, slopes, or
change-points from a table of numeric time-indexed values — even when instructed to do
so carefully [A].

A medical-domain confirmation: "Accuracy Is Not Enough: Reasoning and Reference Reliability
in Orthopaedic Large Language Model Applications" (PMC 12874175, 2026) found that LLMs
in clinical contexts produce incorrect numeric inferences from lab values significantly
more often than they produce incorrect text-level claims — and do so with comparable
confidence, making numeric errors harder to detect than factual errors [A].

The implications for personal health intelligence are direct:

- A 6-month trend in ferritin values (e.g., 48 → 42 → 35 → 28) involves slope estimation,
  percent change calculation, and threshold-crossing detection. An LLM asked to compute
  these from a table of values will produce plausible-looking numbers that may be wrong.
- Correlation between two time-series (e.g., resting HR and sleep efficiency over 90 days)
  requires a calculation the LLM cannot perform correctly from raw data.
- A percent-of-range calculation ("your ferritin is X% below the lower reference limit")
  sounds simple but involves subtraction, division, and reference-range lookup — each step
  a potential error.

None of these are acceptable to get wrong in a health context.

### The standard solution: deterministic code, not LLM arithmetic

The standard engineering response to this problem is well-established: route all numeric
computation to code, not to the language model. The LLM's job is to compose readable
prose around facts; the computation engine's job is to produce those facts. This separation
is common in function-calling frameworks, tool-use agents, and structured output pipelines.

Helix applies this principle in a health-specific form: the Trend/Numeric agent computes
all quantitative findings deterministically, producing a structured numeric facts payload
that the Functional-Medicine Analyst receives as pre-computed inputs. The Analyst never
sees a raw time-series and is never asked to compute a number.

---

## Decision

### Architectural separation: Analyst receives facts, not data

The Functional-Medicine Analyst is prohibited from receiving raw time-series data and
prohibited from performing arithmetic. Every number that appears in an Analyst response
must come from the numeric facts payload produced by the Trend/Numeric agent — not from
the Analyst's own computation.

This is enforced by the pipeline structure (ADR-005 step 2): RuVector retrieval returns
measurement nodes to the Trend/Numeric agent, which processes them and produces a
structured payload. The Analyst receives only this payload. The Analyst may quote numbers
from the payload, contextualize them in prose, and reason about their meaning — but may
not alter them, combine them arithmetically, or produce new numbers not present in the
payload.

The Verifier/Critic agent (ADR-008) checks this constraint: if the Analyst draft contains
a number not present in the numeric facts payload or the ProvRecords, it is flagged as
an unverified claim and removed.

### Computations performed by the Trend/Numeric agent

The following computations are owned exclusively by the Trend/Numeric agent. All
computations are performed in deterministic Rust code (consistent with the ruvnet stack's
Rust mandate). The agent runs on-device or in a local compute context with no LLM
involvement.

**1. Point-in-time values and reference-range contextualization**
- Latest measured value for each requested metric.
- Delta from reference range midpoint and from range boundaries (low / high).
- Percent above or below reference range limits.
- Flag: within range / below range / above range / critically out of range.
- Flag: how many consecutive measurements have been out-of-range in the same direction
  (trend consistency).

**2. Trend computation over time-series**
- Linear slope over a specified window (least-squares regression): value per day or value
  per week depending on measurement frequency.
- Slope direction: increasing / decreasing / flat (bounded by a significance threshold
  to avoid flagging noise as trends).
- Slope significance: coefficient of determination (R²) to indicate how well the linear
  model fits; low R² means the trend claim is weak and the payload flags it as such.
- Percent change: from a specified start date to a specified end date.
- Rolling N-day average: smoothed value to reduce single-measurement noise.
- Exponential weighted moving average (EWMA) for metrics with high day-to-day variance
  (e.g., HRV, sleep efficiency).

**3. Change-point detection**
- Identify statistically significant shifts in a time-series using a simple cumulative
  sum (CUSUM) algorithm. A change-point is a date at which the mean of the series
  shifts significantly.
- Reported fields: change-point date, magnitude of shift, confidence (proportion of
  bootstrap samples that confirm the shift).
- Change-points are reported to the Analyst as: "a significant shift in [metric] occurred
  around [date], changing from approximately [before_mean] to [after_mean]."

**4. Reference-range crossing events**
- Timeline of dates when a metric crossed its reference range boundary (entered or
  exited the normal range).
- For each crossing: direction (entered / left normal range), value at crossing, reference
  range boundary crossed.
- Count of range excursions in a specified window (e.g., "glucose has exceeded 140 mg/dL
  post-meal on 8 of the last 30 days with CGM data").

**5. Correlation computation**
- Pearson and Spearman correlation between two time-series over a specified window,
  after interpolating both to a common time grid.
- Correlation confidence: p-value and effective sample size (accounting for
  autocorrelation).
- Only correlations meeting a minimum p-value threshold (< 0.05) and minimum sample
  size (n ≥ 20 aligned time points) are surfaced; weaker correlations are suppressed
  or labeled "insufficient data for reliable correlation."

**6. Aggregated period statistics**
- Mean, median, standard deviation, and interquartile range over a specified window.
- Min and max values with their dates.
- Coefficient of variation (CV = SD / mean) for metrics where variability is clinically
  meaningful (e.g., HRV, CGM glucose variability, blood pressure variability).

**7. Goal tracking and target gap calculations**
- Distance from a user-set goal value: absolute and percent.
- Projected time to reach goal at current trend slope (with confidence interval derived
  from R²).
- Alert: "at this rate, you will reach your target in approximately [N] weeks" or
  "at this rate, you will not reach your target in the next [N] weeks."

### The numeric facts payload schema

The Trend/Numeric agent produces a structured payload for each response context. Example
structure (JSON-serializable Rust struct):

```
NumericFact {
  metric_id:          String         // LOINC code or Helix internal ID
  metric_name:        String         // human-readable label
  latest_value:       Option<f64>    // most recent measured value
  latest_unit:        String         // UCUM unit
  latest_measured_at: ISO-8601
  reference_range:    { low: f64?, high: f64?, source: String, population: String }
  range_status:       RangeStatus    // InRange | BelowRange | AboveRange | Critical
  pct_below_low:      Option<f64>    // e.g. 6.7 means 6.7% below lower limit
  slope_per_week:     Option<f64>    // linear regression slope
  slope_r_squared:    Option<f64>    // goodness of fit
  slope_direction:    Option<Dir>    // Increasing | Decreasing | Flat
  slope_window_days:  u32            // window over which slope was computed
  pct_change_90d:     Option<f64>    // percent change over last 90 days
  change_point:       Option<ChangePoint> // most recent significant shift
  correlations:       Vec<Correlation>    // significant correlations with other metrics
  n_observations:     u32            // total observations used in this computation
  data_quality_flag:  DataQuality    // Good | PartialOCR | LowSampleSize | HighVariance
}
```

All fields are `Option<T>` where the computation cannot be performed due to insufficient
data. The Analyst prose must reflect the absence of these values honestly (per ADR-006
abstention rules) rather than speculating.

### Temporal grounding: windows and sample-size sufficiency rules

Trend claims carry a minimum sample-size requirement enforced in the Trend/Numeric agent:

| Computation          | Minimum observations | Note                              |
|----------------------|----------------------|-----------------------------------|
| Latest value         | 1                    | No trend; point estimate only     |
| Linear slope / trend | 5                    | Below 5: flag as low-sample-size  |
| Change-point         | 10 per segment       | Fewer: change-point not reported  |
| Correlation          | 20 aligned points    | Fewer: correlation not reported   |
| Coefficient of variation | 7               | Fewer: CV not reported            |

When sample size is below threshold, the payload field is `None` and the `data_quality_flag`
is `LowSampleSize`. The Analyst is prohibited from asserting a trend without a corresponding
`slope_per_week` value in the payload — an absent payload field means no trend claim.

The time window for each computation is specified by the query manifest (ADR-005):
the Analyst's query decomposition step identifies which metrics and which time windows
are needed to answer the question. The Trend/Numeric agent computes within those windows.
Multiple windows may be requested for the same metric (e.g., 30-day slope, 90-day slope,
12-month slope) to detect recency vs. longer-term trends.

### Integration with the 0–100 health score (ADR-016)

The health score computation is entirely within the Trend/Numeric agent — the LLM has no
role in the score calculation. Each subsystem sub-score is computed from the numeric facts
for the metrics in that subsystem, weighted by the recency and completeness of data, and
compared against the Tier-2 reference standard (ADR-006) for each metric. The score
formula is versioned, documented, and validated against a held-out clinical benchmark
dataset.

The score value surfaced in the UI is passed to the Analyst as a single numeric fact in
the payload. The Analyst contextualizes it ("your score increased by 4 points this month")
but does not calculate it.

### Handling CGM and high-frequency continuous data

Continuous glucose monitors and Cognitum Seed ambient data generate high-frequency
time-series (CGM: ~288 samples/day; Seed respiration: ~1 sample/3 seconds). The
Trend/Numeric agent pre-aggregates these into derived metrics before the response context
is assembled:

**CGM**: time-in-range (TIR) per ISPAD/ADA/ATTD consensus targets, glucose variability
index (coefficient of variation), meal-response peaks, overnight glucose profile.

**Cognitum Seed ambient vitals**: overnight median respiration rate, respiration rate
variability (RRV), HR beat regularity, restlessness index (motion events per hour),
sleep-disruption events (defined as N consecutive minutes of anomalous RRV pattern).

These derived metrics are the only form in which ambient data enters the Analyst context.
Raw sensor samples are never passed to the LLM.

### On-device deterministic computation (privacy alignment)

The Trend/Numeric agent is a Rust WASM binary (or native binary for on-device inference
paths, ADR-013). Numeric computation runs entirely on-device in Phase 0–3 (ADR roadmap
§8). No health measurements are sent to a cloud service for numeric computation. This
aligns with the local-first data principle (ADR-001) and eliminates the exposure created
by transmitting time-series health data to a remote compute service.

The Trend/Numeric agent is stateless between invocations (its inputs are the retrieved
RuVector nodes and the query manifest; its outputs are the numeric facts payload). It can
be unit-tested exhaustively, independently of the LLM pipeline.

---

## Alternatives Considered

### Alternative A: Function-calling / tool-use from the LLM itself

Allow the LLM Analyst to call a `compute_trend(metric, window)` function as part of its
reasoning, receiving numeric results that it then incorporates into its response. This is
the "tool use" pattern popularized by GPT-4 function calling.

Rejected because: tool-use puts the LLM in charge of deciding *which* computations to
request and *how to use* the results. In a health context, an LLM that decides to request
a trend but misinterprets the sign (confusing a positive slope with a negative trend) or
selects the wrong time window produces a subtle but dangerous error. The Helix architecture
deliberately separates computation specification (the query manifest, compiled before the
LLM sees any data) from computation execution (the Trend/Numeric agent). The LLM has no
influence over which computations are run or how they are parameterized.

### Alternative B: Python/NumPy numeric computation micro-service

Run numeric computations in a Python service using NumPy, Pandas, or SciPy, called via
HTTP from the Ruflo pipeline.

Rejected because: the ruvnet stack's Rust mandate (CLAUDE.md) applies here. A separate
Python microservice creates an external process boundary that adds latency, a network
dependency, and a maintenance surface. The Rust Trend/Numeric agent can be compiled to
WASM for on-device execution, which the Python microservice cannot. The statistical
algorithms required (linear regression, CUSUM, Pearson/Spearman correlation) are
straightforward to implement in Rust using the `statrs` or `linfa` crates and require
no Python ecosystem dependency.

### Alternative C: Pre-compute all numerics at ingestion time (no on-demand computation)

Compute all trends, slopes, and statistics at ingestion time and store them in RuVector
as derived fact nodes. The Analyst retrieves pre-computed stats rather than invoking
a computation agent at response time.

Rejected because: health Q&A requires on-demand computation over user-specified windows.
A user asking "how has my ferritin changed since I started iron supplementation on April
12th?" requires a computation over a specific date range that cannot be pre-computed.
Pre-computation at ingestion is valuable for the standing health model and the daily
briefing (fixed windows: 7-day, 30-day, 90-day) but is insufficient for conversational
Q&A. The Trend/Numeric agent is invoked on-demand for question-specific windows and uses
pre-computed values from the standing model as a cache when the window matches.

---

## Consequences

### Positive

- **Numeric trustworthiness.** Every number the user sees has been computed by deterministic
  code, not estimated by an LLM. This is verifiable: the same query on the same data
  always produces the same numbers. The numeric layer can be unit-tested independently.
- **Transparent trend data.** Slope, R², change-point date, and sample size are available
  in the payload and can be surfaced in the UI "view source" flow. A user who wants to
  know how confident a trend claim is can see the underlying statistics.
- **LLM remains in its strength domain.** The LLM handles what it is good at: natural
  language composition, contextual reasoning, and explanation. It does not handle what
  it is bad at: arithmetic. This is the correct division of labor.
- **Darwin/eval-compatible.** The numeric facts payload is machine-readable and
  deterministic. Eval test cases can specify expected numeric outputs and verify them
  exactly, enabling the Darwin Mode fitness function (ADR-018) to measure whether a
  configuration change affected numeric accuracy.

### Negative

- **Separate codebase to maintain.** The Trend/Numeric agent is a specialized Rust
  module with health-specific statistical algorithms. It requires testing, maintenance,
  and updating as new data types are added (e.g., adding CGM support requires new
  glycemic variability metrics). This is meaningful engineering work.
- **Query manifest specification burden.** The query decomposition step (ADR-005, step 1)
  must correctly specify which computations are needed for each question type. If the
  manifest is under-specified, the Analyst may not receive the numeric facts it needs and
  will abstain where it could have answered. Over-specification increases compute cost.
  The manifest specification logic is itself LLM-assisted — it must be tested carefully.
- **Correlation latency for large windows.** Computing Pearson/Spearman correlation over
  90-day high-frequency CGM data (25,920 samples) on-device is fast but not instant.
  Target: < 500ms for all correlation computations on standard mobile hardware. Achieved
  via efficient Rust implementations with pre-sorted time indices and SIMD acceleration
  where available.

### Mitigations

- Maintain a test suite for the Trend/Numeric agent with known-correct numeric outputs
  for all computation types; run it in CI (ADR-002 build pipeline).
- Pre-compute common windows (7d, 30d, 90d) at ingestion time and invalidate/recompute
  when new data arrives, so the on-demand agent can use cached results for standard queries.
- Log query manifest generation errors as a distinct failure mode in Ruflo's HIPAA audit
  trail, enabling post-hoc analysis of manifest specification quality.

---

## Open Questions

1. **Slope significance threshold.** The "flat" vs. "increasing/decreasing" classification
   requires a threshold on the absolute slope value (below which a slope is called flat).
   This threshold should be metric-specific — a ferritin slope of 0.1 ng/mL/week may be
   meaningful; a HRV slope of 0.1 ms/week may be noise. Clinical advisory input is needed
   to set appropriate thresholds for the 20–30 most important metrics.

2. **Seasonal and circadian effects.** Some metrics have strong seasonal cycles (vitamin D,
   mood proxies from wearables) or time-of-day patterns (cortisol, HRV, blood pressure).
   Simple linear regression over these metrics may flag a "declining trend" that is
   actually a seasonal trough. Should the Trend/Numeric agent include seasonal decomposition?
   Proposed: yes, using a simple seasonal-trend decomposition via LOESS (STL) for metrics
   with known seasonal periodicity, in Phase 2.

3. **Measurement frequency heterogeneity.** A user who measures ferritin quarterly has
   a different data density than a user who measures it monthly. Slope estimates are less
   reliable for sparse data. Should confidence intervals on slope be reported explicitly
   in the Analyst response? Proposed: yes — "your ferritin is declining at approximately
   2 ng/mL per month (±1.5 ng/mL, based on 4 measurements)" gives the user a sense of
   uncertainty without requiring them to understand statistics.

4. **WASM size constraints.** A full Rust statistical library compiled to WASM for
   on-device use may be large (~5–15 MB depending on which crates are included). Evaluate
   whether the full computation suite is feasible on-device in Phase 0 or whether some
   computations (e.g., correlation) should be on-device-first with cloud fallback for
   performance on older devices.

---

## References

- [A] "Mathematical Reasoning in Large Language Models: Assessing Logical and Arithmetic Errors
  across Wide Numerical Ranges" (arXiv 2502.08680):
  https://arxiv.org/html/2502.08680v1
- [A] "Beyond Accuracy: Diagnosing Algebraic Reasoning Failures in LLMs Across Nine Complexity
  Dimensions" (arXiv 2604.06799):
  https://arxiv.org/html/2604.06799v1
- [A] "Large Language Model Reasoning Failures" (arXiv 2602.06176):
  https://arxiv.org/html/2602.06176v1
- [A] "A Picture is Worth A Thousand Numbers: Enabling LLMs Reason about Time Series via
  Visualization" (arXiv 2411.06018):
  https://arxiv.org/pdf/2411.06018
- [A] "Accuracy Is Not Enough: Reasoning and Reference Reliability in Orthopaedic LLM
  Applications" (PMC 12874175):
  https://www.ncbi.nlm.nih.gov/pmc/articles/PMC12874175/
- [B] "Mathematical Computation and Reasoning Errors by Large Language Models" (arXiv 2508.09932,
  accepted AIME-Con 2025):
  https://arxiv.org/html/2508.09932v2
- [B] "A model of errors in transformers" (arXiv 2601.14175):
  https://arxiv.org/pdf/2601.14175
- [C] ISPAD/ADA/ATTD consensus on time-in-range (TIR) CGM metrics as clinical endpoints:
  Battelino et al., Diabetes Care 2019; https://doi.org/10.2337/dc19-0184

---

*This document provides architectural guidance, not legal, regulatory, or medical advice.
Helix is a decision-support tool, not a diagnostic authority. Numeric outputs from the
Trend/Numeric agent are derived from user-supplied measurements and carry the accuracy
limitations of those source measurements.*
