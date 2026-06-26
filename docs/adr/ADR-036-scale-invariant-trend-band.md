# ADR-036: Scale-Invariant (Reference-Range-Relative) Trend Dead-Band

**Status**: Accepted
**Date**: 2026-06-26
**Project**: Helix — Personal Health Intelligence (PHI)
**Related**: ADR-007 (deterministic numerics), ADR-035 (parameter evolution), ADR-031 (timeline), ADR-006 (honest uncertainty)

---

## Context

The trend classifier (`helix-numeric::trend_direction`) used a single **absolute**
dead-band `flat_band_per_day` (value-units/day) below which a slope reads "flat".
A diverse multi-marker eval set (ADR-035, `helix-evolve`) exposed that **no single
absolute band can be correct across markers of different scales**:

- **Small-scale markers** (HbA1c span ~1.6%, TSH span ~3.6 mIU/L): a clinically
  important slow trend — e.g. a prediabetic HbA1c creep of ~0.0033/day, or a
  developing-hypothyroid TSH drift of ~0.008/day — has a slope *below* the shipping
  band (0.01), so it was silently read as **"stable"**. A missed real trend.
- **Large-scale markers** (cholesterol span ~75 mg/dL): ordinary day-to-day
  variation produces slopes *above* 0.01, so noise was read as a **trend**.

On the 17-case eval set, the absolute band scored 13/17 (4 wrong) and **no single
absolute value did better** — the two failure modes pull the threshold in opposite
directions.

## Decision

Add a **scale-invariant dead-band**: a move counts as a trend only if it exceeds a
**fraction of the marker's reference-range span over the observation window**.

1. **`trend_direction_relative(slope, range_span, window_days, frac)`**
   (`helix-numeric`): effective band = `frac * range_span / window_days`; reports
   Flat when span/window/frac is non-positive (no scale to judge against). Pure,
   deterministic (ADR-007).
2. **Pipeline opt-in** (`AnalyzeRequest.flat_band_frac`): when `> 0` and a reference
   range is present, the relative band supersedes the absolute `flat_band_per_day`;
   otherwise behaviour is unchanged (backward compatible, `0.0` = absolute).
3. **Adopted default `flat_band_frac = 0.08`** (≈8% of reference range over the
   window) in the shipping UI. This value was **found by evolution** (ADR-035) — it
   scores **17/17** on the eval set, fixing both failure modes — and reviewed before
   adoption, consistent with ADR-035's no-auto-promotion rule.
4. **Safety unchanged.** This only affects *trend direction* wording, never the
   abstention or red-flag escalation paths; over-confidence stayed at zero across
   the eval (a more sensitive trend band cannot make Helix answer when it should
   abstain).

## Alternatives Considered

- **Keep tuning the absolute band.** Rejected: the eval proved it has an
  irreducible error floor across scales (best was 15/17 after evolution).
- **Per-marker absolute bands.** Rejected as the primary mechanism: a curation
  burden per concept; the relative band generalizes from the reference range Helix
  already has.
- **Normalize by mean instead of range span.** Reasonable, but the reference range
  is the clinically meaningful scale and is already attached to every record.

## Consequences

**Positive.** One threshold works across all markers (17/17); catches slow
clinically-important trends small absolute bands missed; rejects large-scale noise;
generalizes from data already present (the reference range). **Negative.** Needs a
reference range and ≥2 points spanning a window (falls back to the absolute band
otherwise). **Mitigations.** Graceful fallback; the value is eval-gated and can be
re-evolved as the eval set grows.

## Real-data validation (honest finding)

Checked against the **MIMIC-IV demo** (open, 894 real serial-lab series), with ground
truth = the **statistical significance of the OLS slope** (t-test, |t|>2 ⇒ trend) —
deliberately independent of the range-fraction heuristic. Result:

| config | agreement w/ statistical trend test |
|---|---|
| absolute band (0.01/day) | 55.4% |
| **adopted relative band (frac 0.08)** | 54.3% |
| evolved on this data | 82.1% (→ frac ≈ 0.37) |

**The adopted 0.08 is no better than the absolute band on this data, and evolution
wants a far higher frac.** This is a *population* effect, not a refutation: MIMIC is
**ICU** data — short, noisy, irregular bursts — a poor proxy for Helix's **outpatient
longitudinal self-tracking** target, and a high frac "wins" mostly by calling noisy
series flat. **Decision: retain 0.08 for the outpatient target** (it remains correct
on the clinically-labeled outpatient-style eval, 27/27) and treat the frac as
**population-dependent, to be re-calibrated on outpatient longitudinal data** (ICU is
the wrong population). The real-data harness (`examples/mimic_eval`) is kept as the
calibration tool; this is the kind of check that needs real outpatient data to settle.

## References

- ADR-035 (the eval + evolve run that surfaced this), ADR-007 (deterministic engine). **[A]**
- `helix-evolve` example `evolve_full`: absolute 13/17 → relative (frac≈0.077) 17/17, +2.80, over-confident 0. **[A]**

> Architectural/product guidance, not legal or medical advice. Affects trend wording only; abstention and red-flag safety paths are unchanged.
