//! Criterion benchmark for the solver on a synthetic grid instance.

use std::hint::black_box;

use comiket_tsp::{solve, DistanceMatrix, SolveConfig};
use criterion::{criterion_group, criterion_main, Criterion};

/// Euclidean distance matrix over a 2×k unit grid (2k nodes).
fn grid_matrix(k: usize) -> DistanceMatrix {
    let points: Vec<(f64, f64)> = (0..k)
        .map(|x| (x as f64, 0.0))
        .chain((0..k).map(|x| (x as f64, 1.0)))
        .collect();
    DistanceMatrix::from_fn(points.len(), |i, j| {
        let (a, b) = (points[i], points[j]);
        ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
    })
}

fn bench_solve(c: &mut Criterion) {
    let matrix = grid_matrix(20); // 40 nodes
    let config = SolveConfig {
        seed: 42,
        restarts: 4,
        time_ms: 0,
        max_iters: 200,
        closed: false,
        start: None,
    };
    c.bench_function("solve_grid_2x20_open", |b| {
        b.iter(|| solve(black_box(&matrix), black_box(&config)))
    });
}

criterion_group!(benches, bench_solve);
criterion_main!(benches);
