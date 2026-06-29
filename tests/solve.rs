//! Solver tests: known-optimum tiny instances, a grid within tolerance,
//! determinism, and fixed-start handling. All randomness uses a fixed seed and the
//! iteration cap (not wall-clock) so results are reproducible.

use comiket_tsp::{solve, DistanceMatrix, SolveConfig};

/// Points of a 2×k unit grid: bottom row `0..k`, then top row `k..2k`.
fn grid_2xk(k: usize) -> Vec<(f64, f64)> {
    let bottom = (0..k).map(|x| (x as f64, 0.0));
    let top = (0..k).map(|x| (x as f64, 1.0));
    bottom.chain(top).collect()
}

fn euclid(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn matrix(points: &[(f64, f64)]) -> DistanceMatrix {
    DistanceMatrix::from_fn(points.len(), |i, j| euclid(points[i], points[j]))
}

/// Deterministic config: time disabled, so `max_iters` is the only stopping rule.
fn cfg() -> SolveConfig {
    SolveConfig {
        seed: 1,
        restarts: 8,
        time_ms: 0,
        max_iters: 800,
        closed: false,
        start: None,
    }
}

fn is_permutation(order: &[usize], n: usize) -> bool {
    if order.len() != n {
        return false;
    }
    let mut seen = vec![false; n];
    for &x in order {
        if x >= n || seen[x] {
            return false;
        }
        seen[x] = true;
    }
    true
}

#[test]
fn closed_tour_reaches_grid_optimum() {
    // A 2×4 grid has a Hamiltonian cycle of eight unit edges; nothing beats 8.0.
    let pts = grid_2xk(4);
    let m = matrix(&pts);
    let out = solve(
        &m,
        &SolveConfig {
            closed: true,
            ..cfg()
        },
    );
    assert!(is_permutation(&out.order, 8));
    assert!((out.cost - 8.0).abs() < 1e-6, "closed cost {}", out.cost);
}

#[test]
fn open_path_reaches_grid_optimum() {
    // The open path drops one unit edge from the optimal cycle: 7.0.
    let pts = grid_2xk(4);
    let m = matrix(&pts);
    let out = solve(&m, &cfg());
    assert!(is_permutation(&out.order, 8));
    assert!((out.cost - 7.0).abs() < 1e-6, "open cost {}", out.cost);
}

#[test]
fn tiny_square_known_optimum() {
    let pts = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    let m = matrix(&pts);

    let closed = solve(
        &m,
        &SolveConfig {
            closed: true,
            ..cfg()
        },
    );
    assert!((closed.cost - 4.0).abs() < 1e-6, "closed {}", closed.cost);

    let open = solve(&m, &cfg());
    assert!((open.cost - 3.0).abs() < 1e-6, "open {}", open.cost);
}

#[test]
fn deterministic_same_seed_same_route() {
    let pts = grid_2xk(5); // 10 nodes
    let m = matrix(&pts);
    let a = solve(&m, &cfg());
    let b = solve(&m, &cfg());
    assert_eq!(a.order, b.order, "route must be byte-identical");
    assert_eq!(a.cost, b.cost);
}

#[test]
fn fixed_start_is_first_node() {
    let pts = grid_2xk(4);
    let m = matrix(&pts);

    let closed = solve(
        &m,
        &SolveConfig {
            closed: true,
            start: Some(3),
            ..cfg()
        },
    );
    assert_eq!(closed.order[0], 3);
    assert!(is_permutation(&closed.order, 8));
    // Fixing the start cannot beat the unconstrained cycle optimum.
    assert!((closed.cost - 8.0).abs() < 1e-6, "closed {}", closed.cost);

    let open = solve(
        &m,
        &SolveConfig {
            start: Some(2),
            ..cfg()
        },
    );
    assert_eq!(open.order[0], 2);
    assert!(is_permutation(&open.order, 8));
}
