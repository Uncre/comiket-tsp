//! Nearest-neighbour tour construction (with optional multi-start).

use super::Cost;

/// Greedy nearest-neighbour ordering of the `n_real` real nodes, beginning at `start`.
pub(crate) fn nearest_neighbor(cost: &Cost, n_real: usize, start: usize) -> Vec<usize> {
    let mut visited = vec![false; n_real];
    let mut order = Vec::with_capacity(n_real);
    let mut cur = start;
    visited[cur] = true;
    order.push(cur);
    for _ in 1..n_real {
        let mut best = usize::MAX;
        let mut best_d = f64::INFINITY;
        for (cand, &seen) in visited.iter().enumerate() {
            if !seen {
                let d = cost.d(cur, cand);
                if d < best_d {
                    best_d = d;
                    best = cand;
                }
            }
        }
        visited[best] = true;
        order.push(best);
        cur = best;
    }
    order
}

/// Try every node as the NN start and keep the ordering with the lowest path cost.
pub(crate) fn best_nearest_neighbor(cost: &Cost, n_real: usize) -> Vec<usize> {
    (0..n_real)
        .map(|s| nearest_neighbor(cost, n_real, s))
        .min_by(|a, b| path_cost(cost, a).total_cmp(&path_cost(cost, b)))
        .unwrap_or_default()
}

/// Sum of consecutive-edge costs along an open ordering (no wrap-around).
fn path_cost(cost: &Cost, order: &[usize]) -> f64 {
    order.windows(2).map(|w| cost.d(w[0], w[1])).sum()
}
