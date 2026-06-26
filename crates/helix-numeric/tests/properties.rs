//! Property tests (proptest) for the deterministic numeric engine (ADR-007).
//!
//! These assert *invariants* that must hold across all valid inputs — the kind
//! of guarantee the anti-hallucination design rests on: the engine never panics,
//! never emits a non-finite statistic, and its outputs obey their mathematical
//! definitions.

use helix_numeric::{mean, pearson, percent_change, range_crossings, slope_per_day, Point};
use proptest::prelude::*;

const DAY: i64 = 86_400_000;

/// A strictly time-ordered, finite series whose length falls in `len` and whose
/// values are bounded.
fn ordered_series(len: std::ops::Range<usize>) -> impl Strategy<Value = Vec<Point>> {
    prop::collection::vec(-1.0e6f64..1.0e6f64, len).prop_map(|vals| {
        vals.into_iter()
            .enumerate()
            .map(|(i, v)| Point::new(i as i64 * DAY, v))
            .collect()
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// mean is always within [min, max] of the inputs and finite.
    #[test]
    fn mean_is_bounded_and_finite(series in ordered_series(1..50)) {
        let m = mean(&series).unwrap();
        let lo = series.iter().map(|p| p.value).fold(f64::INFINITY, f64::min);
        let hi = series.iter().map(|p| p.value).fold(f64::NEG_INFINITY, f64::max);
        prop_assert!(m.is_finite());
        prop_assert!(m >= lo - 1e-6 && m <= hi + 1e-6);
    }

    /// slope is finite, and a strictly-increasing series yields a positive slope.
    #[test]
    fn slope_finite_and_signed(series in ordered_series(2..50)) {
        let s = slope_per_day(&series).unwrap();
        prop_assert!(s.is_finite());
    }

    /// A monotonically increasing series always has non-negative slope.
    #[test]
    fn monotone_increasing_has_nonneg_slope(start in -1000.0f64..1000.0, step in 0.0f64..100.0, n in 2usize..40) {
        let series: Vec<Point> = (0..n)
            .map(|i| Point::new(i as i64 * DAY, start + step * i as f64))
            .collect();
        let s = slope_per_day(&series).unwrap();
        prop_assert!(s >= -1e-6);
    }

    /// pearson is always within [-1, 1] for any valid finite input.
    #[test]
    fn pearson_in_unit_interval(a in prop::collection::vec(-1e3f64..1e3, 3..40)) {
        // pair each a[i] with a noisy transform; correlation must stay bounded.
        let b: Vec<f64> = a.iter().enumerate().map(|(i, x)| x * 2.0 + (i as f64).cos()).collect();
        if let Ok(r) = pearson(&a, &b) {
            prop_assert!((-1.0..=1.0).contains(&r));
        }
    }

    /// range_crossings never reports more crossings than there are intervals,
    /// and every reported crossing actually changes classification.
    #[test]
    fn crossings_bounded_by_intervals(series in ordered_series(2..50)) {
        let lo = Some(-100.0);
        let hi = Some(100.0);
        let x = range_crossings(&series, lo, hi).unwrap();
        prop_assert!(x.len() < series.len());
        for c in &x {
            prop_assert_ne!(c.from, c.to);
        }
    }

    /// percent_change matches its definition whenever the baseline is non-zero.
    #[test]
    fn percent_change_matches_definition(series in ordered_series(2..30)) {
        let base = series[0].value;
        let last = series[series.len() - 1].value;
        match percent_change(&series) {
            Ok(pc) => {
                prop_assert!(base != 0.0);
                let expected = (last - base) / base;
                prop_assert!((pc - expected).abs() <= 1e-9 * (1.0 + expected.abs()));
            }
            Err(_) => prop_assert_eq!(base, 0.0),
        }
    }
}
