//! Route search: construction, local search, and the iterated-local-search driver.
//!
//! ## Open paths, closed tours, and a fixed start
//!
//! All local search runs on a **cyclic** tour with a frozen leading prefix of
//! `lock` positions. This single representation covers every mode:
//!
//! - **Closed tour** — the nodes themselves form the cycle. Position 0 is the start
//!   (or an arbitrary anchor); `lock = 1`.
//! - **Open path** — a zero-cost *dummy* node closes the path into a cycle. The two
//!   dummy edges cost nothing, so the cycle cost equals the path cost. The dummy sits
//!   at position 0 (`lock = 1`); with a fixed start it sits at position 0 and the
//!   start at position 1 (`lock = 2`), pinning the start as one path endpoint.
//!
//! Moves never touch the locked prefix, so the start (and the dummy) stay put while
//! everything else is free to move.

pub mod construct;
pub mod ils;
pub mod two_opt;

use rand::rngs::StdRng;
use rand::SeedableRng;
use rayon::prelude::*;

use crate::distance::DistanceMatrix;

/// Improvement threshold: moves must beat the current cost by more than this to be
/// accepted, which keeps floating-point noise from causing infinite cycling.
pub(crate) const EPS: f64 = 1e-7;

/// Edge-cost lookup that treats the optional open-path dummy node as zero-cost.
pub(crate) struct Cost<'a> {
    matrix: &'a DistanceMatrix,
    dummy: Option<usize>,
}

impl Cost<'_> {
    /// Perceived cost of the edge between nodes `a` and `b` (0 if either is the dummy).
    #[inline]
    pub(crate) fn d(&self, a: usize, b: usize) -> f64 {
        if Some(a) == self.dummy || Some(b) == self.dummy {
            0.0
        } else {
            self.matrix[(a, b)]
        }
    }
}

/// Total cost of a cyclic tour (sum of every edge, including the wrap-around).
pub(crate) fn cycle_cost(cost: &Cost, tour: &[usize]) -> f64 {
    let len = tour.len();
    if len < 2 {
        return 0.0;
    }
    let mut total = 0.0;
    for (k, &a) in tour.iter().enumerate() {
        total += cost.d(a, tour[(k + 1) % len]);
    }
    total
}

/// Knobs for [`solve`].
#[derive(Debug, Clone, Copy)]
pub struct SolveConfig {
    /// Base RNG seed; restart `r` derives the child seed `seed ^ r`.
    pub seed: u64,
    /// Number of independent ILS chains run in parallel; the global best wins.
    pub restarts: usize,
    /// Wall-clock budget per restart in milliseconds; `0` disables the time limit.
    pub time_ms: u64,
    /// Hard cap on ILS iterations per restart (the deterministic stopping criterion).
    pub max_iters: usize,
    /// Return to the start (closed tour) instead of an open path.
    pub closed: bool,
    /// Fixed first node (index into the distance matrix), or `None` to let the
    /// solver choose the best start.
    pub start: Option<usize>,
}

impl Default for SolveConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            restarts: 16,
            time_ms: 800,
            max_iters: 100_000,
            closed: false,
            start: None,
        }
    }
}

/// The chosen route: node indices in visit order, plus the total perceived cost.
#[derive(Debug, Clone)]
pub struct SolveOutcome {
    /// Node indices (into the distance matrix) in visit order; the start is first.
    pub order: Vec<usize>,
    /// Total perceived cost (includes the return leg when `closed`).
    pub cost: f64,
}

/// Deterministic child seed for restart `r` (see [`SolveConfig::seed`]).
fn child_seed(base: u64, r: usize) -> u64 {
    base ^ r as u64
}

/// Build the initial cyclic tour for restart `r` and report how many leading
/// positions are locked.
fn initial_tour(
    cost: &Cost,
    n_real: usize,
    config: &SolveConfig,
    rng: &mut StdRng,
    r: usize,
) -> (Vec<usize>, usize) {
    // The ordering of the real nodes, beginning at the required or chosen start.
    let seq = match config.start {
        Some(s) => construct::nearest_neighbor(cost, n_real, s),
        None if r == 0 => construct::best_nearest_neighbor(cost, n_real),
        None => construct::nearest_neighbor(cost, n_real, rand_start(n_real, rng)),
    };

    if config.closed {
        // The nodes are the cycle; position 0 is the (anchored) start.
        (seq, 1)
    } else {
        // Prepend the zero-cost dummy to close the path into a cycle.
        let dummy = n_real;
        let mut tour = Vec::with_capacity(n_real + 1);
        tour.push(dummy);
        tour.extend_from_slice(&seq);
        let lock = if config.start.is_some() { 2 } else { 1 };
        (tour, lock)
    }
}

/// A uniformly random real-node index, used as a diversified NN start.
fn rand_start(n_real: usize, rng: &mut StdRng) -> usize {
    use rand::RngExt;
    rng.random_range(0..n_real)
}

/// Recover the real-node visit order from a finished cyclic tour.
fn extract_order(tour: &[usize], config: &SolveConfig, n_real: usize) -> Vec<usize> {
    if config.closed {
        tour.to_vec()
    } else {
        // Drop the dummy; the path is everything after it, wrapping around.
        let dummy = n_real;
        let pos = tour.iter().position(|&x| x == dummy).unwrap_or(0);
        (1..tour.len())
            .map(|k| tour[(pos + k) % tour.len()])
            .collect()
    }
}

/// Plan a route over `matrix` under `config`.
///
/// Runs `config.restarts` independent ILS chains in parallel (each with a
/// deterministic child seed) and keeps the global best.
pub fn solve(matrix: &DistanceMatrix, config: &SolveConfig) -> SolveOutcome {
    let n_real = matrix.len();
    if n_real <= 1 {
        return SolveOutcome {
            order: (0..n_real).collect(),
            cost: 0.0,
        };
    }

    let dummy = if config.closed { None } else { Some(n_real) };
    let cost = Cost { matrix, dummy };

    // Collect in restart order first, then reduce sequentially, so ties break
    // deterministically regardless of how rayon schedules the work.
    let results: Vec<(f64, Vec<usize>)> = (0..config.restarts.max(1))
        .into_par_iter()
        .map(|r| {
            let mut rng = StdRng::seed_from_u64(child_seed(config.seed, r));
            let (mut tour, lock) = initial_tour(&cost, n_real, config, &mut rng, r);
            two_opt::local_search(&cost, &mut tour, lock);
            ils::run(&cost, &mut tour, lock, &mut rng, config);
            let c = cycle_cost(&cost, &tour);
            (c, tour)
        })
        .collect();

    let best = results
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .expect("at least one restart");

    SolveOutcome {
        order: extract_order(&best.1, config, n_real),
        cost: best.0,
    }
}
