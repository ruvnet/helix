//! # helix-numeric — ADR-007: Deterministic Numeric/Trend Engine
//!
//! LLMs miscompute over time series; in Helix they are **architecturally
//! prohibited** from doing arithmetic on health data. This crate owns every
//! quantitative operation — slopes, deltas, percent-change, reference-range
//! crossings, correlations, and change-point detection — and hands the results
//! to the analyst as *facts* (ADR-005) the analyst may only narrate, never
//! recompute.
//!
//! Every function is pure and deterministic, returns a [`Result`] that refuses
//! to produce a statistic below a documented **minimum sample size**, and never
//! reads the clock. That makes the whole engine reproducible and exhaustively
//! testable — the property the anti-hallucination design depends on.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Milliseconds since the Unix epoch (caller-supplied; the engine never reads
/// the wall clock).
pub type EpochMillis = i64;

/// One observation in a series.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub t: EpochMillis,
    pub value: f64,
}

impl Point {
    pub const fn new(t: EpochMillis, value: f64) -> Self {
        Self { t, value }
    }
}

/// Why a computation refused to run. Returning an error here is a *feature*: it
/// is how the engine declines to manufacture a statistic the data cannot
/// support, which the analyst then surfaces as an ADR-006 abstention rather than
/// a guess.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NumericError {
    #[error("need at least {needed} points, got {got}")]
    TooFewPoints { needed: usize, got: usize },
    #[error("series timestamps are not strictly increasing at index {0}")]
    Unordered(usize),
    #[error("input contained a non-finite value (NaN or infinity)")]
    NonFinite,
    #[error("all timestamps are identical; slope/correlation is undefined")]
    ZeroTimeSpan,
}

fn check_finite(xs: impl IntoIterator<Item = f64>) -> Result<(), NumericError> {
    for x in xs {
        if !x.is_finite() {
            return Err(NumericError::NonFinite);
        }
    }
    Ok(())
}

/// Validates a series is finite and strictly time-ordered. Most engine
/// functions call this first so callers get one consistent contract.
pub fn validate(series: &[Point]) -> Result<(), NumericError> {
    check_finite(series.iter().flat_map(|p| [p.value, p.t as f64]))?;
    for w in series.windows(2) {
        if w[1].t <= w[0].t {
            // index of the offending (later) point
            let idx = series.iter().position(|p| p.t == w[1].t).unwrap_or(1);
            return Err(NumericError::Unordered(idx));
        }
    }
    Ok(())
}

/// Arithmetic mean of the values. Minimum sample size: 1.
pub fn mean(series: &[Point]) -> Result<f64, NumericError> {
    if series.is_empty() {
        return Err(NumericError::TooFewPoints { needed: 1, got: 0 });
    }
    check_finite(series.iter().map(|p| p.value))?;
    let sum: f64 = series.iter().map(|p| p.value).sum();
    Ok(sum / series.len() as f64)
}

/// Absolute change from the first to the last observation. Minimum: 2 points.
pub fn delta(series: &[Point]) -> Result<f64, NumericError> {
    if series.len() < 2 {
        return Err(NumericError::TooFewPoints {
            needed: 2,
            got: series.len(),
        });
    }
    validate(series)?;
    Ok(series[series.len() - 1].value - series[0].value)
}

/// Percent change from first to last observation, as a fraction (0.10 == +10%).
/// Returns `None`-equivalent error semantics via [`NumericError::NonFinite`]
/// when the baseline is zero (percent change is undefined). Minimum: 2 points.
pub fn percent_change(series: &[Point]) -> Result<f64, NumericError> {
    let d = delta(series)?;
    let base = series[0].value;
    if base == 0.0 {
        return Err(NumericError::NonFinite);
    }
    Ok(d / base)
}

/// Ordinary least-squares slope of value vs. time, in **units per day**.
/// Positive == trending up. Minimum: 2 points; errors if all timestamps equal.
pub fn slope_per_day(series: &[Point]) -> Result<f64, NumericError> {
    if series.len() < 2 {
        return Err(NumericError::TooFewPoints {
            needed: 2,
            got: series.len(),
        });
    }
    validate(series)?;
    // Work in days relative to the first timestamp to keep magnitudes sane.
    const MS_PER_DAY: f64 = 86_400_000.0;
    let t0 = series[0].t as f64;
    let xs: Vec<f64> = series.iter().map(|p| (p.t as f64 - t0) / MS_PER_DAY).collect();
    let ys: Vec<f64> = series.iter().map(|p| p.value).collect();
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for (x, y) in xs.iter().zip(ys.iter()) {
        num += (x - mean_x) * (y - mean_y);
        den += (x - mean_x) * (x - mean_x);
    }
    if den == 0.0 {
        return Err(NumericError::ZeroTimeSpan);
    }
    Ok(num / den)
}

/// Where a value sits against a reference interval (mirrors
/// `helix-provenance::RangePosition`; duplicated to keep this crate dependency
/// free at the leaf).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangePosition {
    Below,
    Within,
    Above,
}

/// A point at which the series crossed a reference bound between consecutive
/// observations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RangeCrossing {
    /// Timestamp of the observation *after* the crossing.
    pub at: EpochMillis,
    pub from: RangePosition,
    pub to: RangePosition,
}

fn classify(value: f64, low: Option<f64>, high: Option<f64>) -> RangePosition {
    if let Some(l) = low {
        if value < l {
            return RangePosition::Below;
        }
    }
    if let Some(h) = high {
        if value > h {
            return RangePosition::Above;
        }
    }
    RangePosition::Within
}

/// Detects every reference-range crossing in the series. Minimum: 2 points.
pub fn range_crossings(
    series: &[Point],
    low: Option<f64>,
    high: Option<f64>,
) -> Result<Vec<RangeCrossing>, NumericError> {
    if series.len() < 2 {
        return Err(NumericError::TooFewPoints {
            needed: 2,
            got: series.len(),
        });
    }
    validate(series)?;
    let mut out = Vec::new();
    for w in series.windows(2) {
        let from = classify(w[0].value, low, high);
        let to = classify(w[1].value, low, high);
        if from != to {
            out.push(RangeCrossing {
                at: w[1].t,
                from,
                to,
            });
        }
    }
    Ok(out)
}

/// Pearson correlation between two equal-length, index-aligned series of
/// values. Minimum: 3 pairs (below that, correlation is not meaningful).
/// Returns a value in `[-1.0, 1.0]`.
pub fn pearson(a: &[f64], b: &[f64]) -> Result<f64, NumericError> {
    if a.len() != b.len() {
        return Err(NumericError::TooFewPoints {
            needed: a.len().max(b.len()),
            got: a.len().min(b.len()),
        });
    }
    if a.len() < 3 {
        return Err(NumericError::TooFewPoints {
            needed: 3,
            got: a.len(),
        });
    }
    check_finite(a.iter().chain(b.iter()).copied())?;
    let n = a.len() as f64;
    let ma = a.iter().sum::<f64>() / n;
    let mb = b.iter().sum::<f64>() / n;
    let mut cov = 0.0;
    let mut va = 0.0;
    let mut vb = 0.0;
    for (x, y) in a.iter().zip(b.iter()) {
        cov += (x - ma) * (y - mb);
        va += (x - ma) * (x - ma);
        vb += (y - mb) * (y - mb);
    }
    if va == 0.0 || vb == 0.0 {
        return Err(NumericError::ZeroTimeSpan);
    }
    Ok((cov / (va.sqrt() * vb.sqrt())).clamp(-1.0, 1.0))
}

/// A detected change-point: the index/timestamp where the running mean shifted
/// most sharply, per a simple CUSUM-style scan.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChangePoint {
    pub index: usize,
    pub at: EpochMillis,
    /// Mean of the segment before vs. after the split.
    pub mean_before: f64,
    pub mean_after: f64,
}

/// Detects the single most significant change-point via the maximum absolute
/// CUSUM deviation from the global mean. Minimum: 4 points (need ≥2 on each
/// side of a split). Returns `Ok(None)` when no interior split improves on a
/// flat series.
pub fn change_point(series: &[Point]) -> Result<Option<ChangePoint>, NumericError> {
    const MIN: usize = 4;
    if series.len() < MIN {
        return Err(NumericError::TooFewPoints {
            needed: MIN,
            got: series.len(),
        });
    }
    validate(series)?;
    let n = series.len();
    let global_mean = mean(series)?;
    // Cumulative sum of deviations; the extremum is the likeliest split.
    let mut cusum = 0.0;
    let mut best_idx = 0usize;
    let mut best_abs = 0.0;
    for (i, p) in series.iter().enumerate().take(n - 1) {
        cusum += p.value - global_mean;
        if cusum.abs() > best_abs {
            best_abs = cusum.abs();
            best_idx = i;
        }
    }
    // Require at least 2 points on each side.
    if best_idx < 1 || best_idx > n - 3 {
        return Ok(None);
    }
    let split = best_idx + 1;
    let before = &series[..split];
    let after = &series[split..];
    Ok(Some(ChangePoint {
        index: split,
        at: series[split].t,
        mean_before: mean(before)?,
        mean_after: mean(after)?,
    }))
}

/// A direction label for narrating a trend without the analyst doing math.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendDirection {
    Rising,
    Falling,
    Flat,
}

/// Classifies a slope into a direction using a caller-supplied dead-band
/// (units/day below which the trend is reported as flat — prevents over-reading
/// noise as a trend).
pub fn trend_direction(slope_per_day: f64, flat_band: f64) -> TrendDirection {
    if slope_per_day > flat_band {
        TrendDirection::Rising
    } else if slope_per_day < -flat_band {
        TrendDirection::Falling
    } else {
        TrendDirection::Flat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400_000;

    fn series(vals: &[(i64, f64)]) -> Vec<Point> {
        vals.iter().map(|&(t, v)| Point::new(t * DAY, v)).collect()
    }

    #[test]
    fn mean_basic() {
        let s = series(&[(0, 10.0), (1, 20.0), (2, 30.0)]);
        assert_eq!(mean(&s).unwrap(), 20.0);
    }

    #[test]
    fn delta_and_percent_change() {
        let s = series(&[(0, 50.0), (3, 40.0)]);
        assert_eq!(delta(&s).unwrap(), -10.0);
        assert!((percent_change(&s).unwrap() - (-0.2)).abs() < 1e-12);
    }

    #[test]
    fn percent_change_zero_baseline_is_error() {
        let s = series(&[(0, 0.0), (1, 5.0)]);
        assert_eq!(percent_change(&s), Err(NumericError::NonFinite));
    }

    #[test]
    fn slope_is_units_per_day() {
        // +2 per day, exactly.
        let s = series(&[(0, 0.0), (1, 2.0), (2, 4.0), (3, 6.0)]);
        assert!((slope_per_day(&s).unwrap() - 2.0).abs() < 1e-9);
        assert_eq!(trend_direction(2.0, 0.1), TrendDirection::Rising);
    }

    #[test]
    fn too_few_points_refuses() {
        let s = series(&[(0, 1.0)]);
        assert_eq!(
            slope_per_day(&s),
            Err(NumericError::TooFewPoints { needed: 2, got: 1 })
        );
    }

    #[test]
    fn rejects_unordered_series() {
        let s = vec![Point::new(2 * DAY, 1.0), Point::new(DAY, 2.0)];
        assert!(matches!(validate(&s), Err(NumericError::Unordered(_))));
    }

    #[test]
    fn rejects_non_finite() {
        let s = vec![Point::new(0, f64::NAN), Point::new(DAY, 1.0)];
        assert_eq!(mean(&s), Err(NumericError::NonFinite));
    }

    #[test]
    fn detects_reference_range_crossing() {
        // ferritin dropping from in-range to below 30.
        let s = series(&[(0, 45.0), (30, 33.0), (60, 28.0)]);
        let x = range_crossings(&s, Some(30.0), Some(400.0)).unwrap();
        assert_eq!(x.len(), 1);
        assert_eq!(x[0].from, RangePosition::Within);
        assert_eq!(x[0].to, RangePosition::Below);
        assert_eq!(x[0].at, 60 * DAY);
    }

    #[test]
    fn pearson_perfect_positive() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [2.0, 4.0, 6.0, 8.0];
        assert!((pearson(&a, &b).unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn pearson_needs_three_pairs() {
        assert_eq!(
            pearson(&[1.0, 2.0], &[1.0, 2.0]),
            Err(NumericError::TooFewPoints { needed: 3, got: 2 })
        );
    }

    #[test]
    fn change_point_finds_level_shift() {
        // flat at 10 then flat at 20.
        let s = series(&[
            (0, 10.0),
            (1, 10.0),
            (2, 10.0),
            (3, 20.0),
            (4, 20.0),
            (5, 20.0),
        ]);
        let cp = change_point(&s).unwrap().expect("a change-point");
        assert!((cp.mean_before - 10.0).abs() < 1e-9);
        assert!((cp.mean_after - 20.0).abs() < 1e-9);
    }

    #[test]
    fn change_point_needs_four_points() {
        let s = series(&[(0, 1.0), (1, 2.0), (2, 3.0)]);
        assert_eq!(
            change_point(&s),
            Err(NumericError::TooFewPoints { needed: 4, got: 3 })
        );
    }
}
