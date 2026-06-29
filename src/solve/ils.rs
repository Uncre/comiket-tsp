//! Iterated local search: double-bridge perturbation and best-acceptance.

use std::time::{Duration, Instant};

use rand::rngs::StdRng;
use rand::RngExt;

use super::{cycle_cost, two_opt, Cost, SolveConfig, EPS};

/// Run ILS from an already locally-optimised `tour`, leaving the best tour found
/// in place. Stops after `config.max_iters` iterations, or earlier if a non-zero
/// `config.time_ms` budget elapses.
pub(crate) fn run(
    cost: &Cost,
    tour: &mut Vec<usize>,
    lock: usize,
    rng: &mut StdRng,
    config: &SolveConfig,
) {
    let mut best = tour.clone();
    let mut best_cost = cycle_cost(cost, &best);

    let deadline =
        (config.time_ms > 0).then(|| Instant::now() + Duration::from_millis(config.time_ms));

    for _ in 0..config.max_iters {
        if deadline.is_some_and(|dl| Instant::now() >= dl) {
            break;
        }
        let mut candidate = best.clone();
        double_bridge(&mut candidate, lock, rng);
        two_opt::local_search(cost, &mut candidate, lock);
        let candidate_cost = cycle_cost(cost, &candidate);
        if candidate_cost + EPS < best_cost {
            best_cost = candidate_cost;
            best = candidate;
        }
    }

    *tour = best;
}

/// Apply a double-bridge (4-opt) kick to the movable region `tour[lock..]`.
///
/// Splits it into `A|B|C|D` at three random cuts and reconnects as `A|C|B|D` — a
/// perturbation 2-opt cannot undo in one step, which is what lets ILS escape local
/// optima. Degenerate cuts are skipped (the iteration simply finds no improvement).
fn double_bridge(tour: &mut [usize], lock: usize, rng: &mut StdRng) {
    let len = tour.len();
    let m = len - lock;
    if m < 4 {
        return;
    }
    let mut cuts = [
        rng.random_range(1..m),
        rng.random_range(1..m),
        rng.random_range(1..m),
    ];
    cuts.sort_unstable();
    let [a, b, c] = cuts;
    if a == b || b == c {
        return;
    }

    let movable = tour[lock..].to_vec();
    let mut rebuilt = Vec::with_capacity(m);
    rebuilt.extend_from_slice(&movable[0..a]);
    rebuilt.extend_from_slice(&movable[b..c]);
    rebuilt.extend_from_slice(&movable[a..b]);
    rebuilt.extend_from_slice(&movable[c..m]);
    tour[lock..].copy_from_slice(&rebuilt);
}
