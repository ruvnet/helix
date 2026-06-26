//! Criterion benchmarks for the deterministic numeric engine (ADR-007 hot path).
//! Establishes baselines for the operations the pipeline runs per answer.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use helix_numeric::{change_point, pearson, range_crossings, slope_per_day, Point};

const DAY: i64 = 86_400_000;

fn make_series(n: usize) -> Vec<Point> {
    (0..n)
        .map(|i| Point::new(i as i64 * DAY, (i as f64 * 0.37).sin() * 10.0 + 50.0))
        .collect()
}

fn bench_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("numeric-engine");
    for &n in &[16usize, 256, 4096] {
        let s = make_series(n);
        let a: Vec<f64> = s.iter().map(|p| p.value).collect();
        let b: Vec<f64> = a.iter().rev().copied().collect();

        group.bench_with_input(BenchmarkId::new("slope_per_day", n), &s, |bch, s| {
            bch.iter(|| slope_per_day(black_box(s)).unwrap())
        });
        group.bench_with_input(BenchmarkId::new("range_crossings", n), &s, |bch, s| {
            bch.iter(|| range_crossings(black_box(s), Some(45.0), Some(55.0)).unwrap())
        });
        group.bench_with_input(BenchmarkId::new("change_point", n), &s, |bch, s| {
            bch.iter(|| change_point(black_box(s)).unwrap())
        });
        group.bench_with_input(BenchmarkId::new("pearson", n), &(a, b), |bch, (a, b)| {
            bch.iter(|| pearson(black_box(a), black_box(b)).unwrap())
        });
    }
    group.finish();
}

criterion_group!(benches, bench_engine);
criterion_main!(benches);
