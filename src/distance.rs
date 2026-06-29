//! Perceived-distance model and the precomputed [`DistanceMatrix`].
//!
//! Cross-island legs use global Manhattan distance plus a tiered structural penalty
//! (building ≫ hall ≫ adjacent island); same-island legs use an around-the-nearer-end
//! model over local coordinates. The sum is then squashed by `gamma` so long legs and
//! building/hall crossings dominate — see `IMPLEMENTATION_PLAN.md` §3.
//!
//! Note: the `powf` squash breaks the triangle inequality, so metric-based bounds
//! (e.g. Christofides) do not apply. The 2-opt / Or-opt / ILS search used here works
//! on arbitrary cost matrices, so this is fine.

use std::ops::Index;

use crate::layout::Space;

/// Tunable knobs for [`perceived`]: the squash exponent and the three crossing penalties.
#[derive(Debug, Clone, Copy)]
pub struct DistanceParams {
    /// Nonlinear squash exponent applied to `(base + penalty)`.
    pub gamma: f64,
    /// Penalty (perceived metres) added when crossing between buildings.
    pub pen_building: f64,
    /// Penalty added when crossing between halls of the same building.
    pub pen_hall: f64,
    /// Penalty added when crossing to an adjacent island in the same hall.
    pub pen_block: f64,
}

impl Default for DistanceParams {
    fn default() -> Self {
        Self {
            gamma: 1.25,
            pen_building: 250.0,
            pen_hall: 40.0,
            pen_block: 5.0,
        }
    }
}

/// True when both spaces sit on the same physical island (building, hall, and block).
fn same_island(a: &Space, b: &Space) -> bool {
    a.building == b.building && a.hall == b.hall && a.block == b.block
}

/// Global Manhattan distance between two spaces (metres).
fn manhattan(a: &Space, b: &Space) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// Walking distance (metres) between two spaces on the **same** island.
///
/// You cannot cut through the tables, so opposite faces are reached by rounding
/// whichever island end is nearer. Same-face legs run straight down the aisle.
pub fn intra_island(a: &Space, b: &Space) -> f64 {
    let dcross = (a.cross - b.cross).abs();
    if a.face == b.face {
        (a.along - b.along).abs() + dcross
    } else {
        let l = a.island_len;
        let around_near = a.along + b.along;
        let around_far = (l - a.along) + (l - b.along);
        around_near.min(around_far) + dcross
    }
}

/// Perceived walking cost between two spaces under `params`.
pub fn perceived(a: &Space, b: &Space, params: &DistanceParams) -> f64 {
    let base = if same_island(a, b) {
        intra_island(a, b)
    } else {
        manhattan(a, b)
    };
    let penalty = if a.building != b.building {
        params.pen_building
    } else if a.hall != b.hall {
        params.pen_hall
    } else if a.block != b.block {
        params.pen_block
    } else {
        0.0
    };
    (base + penalty).powf(params.gamma)
}

/// A dense `N × N` matrix of perceived costs, precomputed once and reused.
///
/// Stored row-major in full (not just a triangle) so a future asymmetric model
/// (one-way aisles) is a data change, not a structural one. Currently symmetric.
#[derive(Debug, Clone)]
pub struct DistanceMatrix {
    n: usize,
    data: Vec<f64>,
}

impl DistanceMatrix {
    /// Build the matrix of `perceived` costs over `spaces`, in the given order.
    pub fn build(spaces: &[Space], params: &DistanceParams) -> Self {
        let n = spaces.len();
        let mut data = vec![0.0; n * n];
        for (i, a) in spaces.iter().enumerate() {
            for (j, b) in spaces.iter().enumerate().skip(i + 1) {
                let d = perceived(a, b, params);
                data[i * n + j] = d;
                data[j * n + i] = d;
            }
        }
        Self { n, data }
    }

    /// Build an `n × n` matrix from an arbitrary cost function `f(i, j)`.
    ///
    /// Useful for loading external instances (e.g. TSPLIB) or tests; the main path
    /// uses [`DistanceMatrix::build`].
    pub fn from_fn(n: usize, mut f: impl FnMut(usize, usize) -> f64) -> Self {
        let mut data = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                data[i * n + j] = f(i, j);
            }
        }
        Self { n, data }
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Whether the matrix has no nodes.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
}

impl Index<(usize, usize)> for DistanceMatrix {
    type Output = f64;

    fn index(&self, (i, j): (usize, usize)) -> &f64 {
        &self.data[i * self.n + j]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{Building, Side};

    /// Build a bare [`Space`] with only the fields the distance model reads.
    #[allow(clippy::too_many_arguments)]
    fn sp(
        building: Building,
        hall: u8,
        block: &str,
        x: f64,
        y: f64,
        along: f64,
        cross: f64,
        face: u8,
        island_len: f64,
    ) -> Space {
        Space {
            id: String::new(),
            building,
            hall,
            block: block.to_string(),
            number: 1,
            side: Side::A,
            x,
            y,
            along,
            cross,
            face,
            island_len,
        }
    }

    #[test]
    fn penalty_tiers_are_ordered() {
        let p = DistanceParams::default();
        // All four pairs share a base distance of 1.0, isolating the penalty tier.
        let a = sp(Building::East, 4, "ア", 0.0, 0.0, 0.0, 0.0, 0, 23.0);
        let cross_building = sp(Building::West, 4, "ア", 1.0, 0.0, 0.0, 0.0, 0, 23.0);
        let cross_hall = sp(Building::East, 5, "ア", 1.0, 0.0, 0.0, 0.0, 0, 23.0);
        let cross_block = sp(Building::East, 4, "カ", 1.0, 0.0, 0.0, 0.0, 0, 23.0);
        let same_isl = sp(Building::East, 4, "ア", 9.0, 9.0, 1.0, 0.0, 0, 23.0);

        let d_building = perceived(&a, &cross_building, &p);
        let d_hall = perceived(&a, &cross_hall, &p);
        let d_block = perceived(&a, &cross_block, &p);
        let d_intra = perceived(&a, &same_isl, &p);

        assert!(d_building > d_hall, "{d_building} !> {d_hall}");
        assert!(d_hall > d_block, "{d_hall} !> {d_block}");
        assert!(d_block > d_intra, "{d_block} !> {d_intra}");
    }

    #[test]
    fn matrix_is_symmetric_with_zero_diagonal() {
        let p = DistanceParams::default();
        let spaces = vec![
            sp(Building::East, 4, "ア", 0.0, 0.0, 0.0, 0.0, 0, 23.0),
            sp(Building::East, 4, "ア", 0.0, -1.0, 1.0, 0.0, 0, 23.0),
            sp(Building::West, 1, "あ", -200.0, 0.0, 0.0, 0.0, 0, 23.0),
            sp(Building::East, 5, "ハ", 10.0, -60.0, 0.0, 0.0, 0, 9.0),
        ];
        let m = DistanceMatrix::build(&spaces, &p);
        assert_eq!(m.len(), 4);
        assert!(!m.is_empty());
        for i in 0..m.len() {
            assert_eq!(m[(i, i)], 0.0);
            for j in 0..m.len() {
                assert_eq!(m[(i, j)], m[(j, i)], "asymmetry at ({i},{j})");
            }
        }
    }

    #[test]
    fn intra_island_rounds_the_nearer_end() {
        let l = 23.0;
        let near0 = sp(Building::East, 4, "ア", 0.0, 0.0, 0.0, 0.0, 0, l);
        let near1 = sp(Building::East, 4, "ア", 0.0, 0.0, 0.0, 1.5, 1, l);
        let far0 = sp(Building::East, 4, "ア", 0.0, 0.0, 23.0, 0.0, 0, l);
        let far1 = sp(Building::East, 4, "ア", 0.0, 0.0, 23.0, 1.5, 1, l);

        // Opposite faces, both at the near end → round the near end (just the cross gap).
        assert_eq!(intra_island(&near0, &near1), 1.5);
        // Opposite faces, both at the far end → round the far end (again just the gap).
        assert_eq!(intra_island(&far0, &far1), 1.5);
        // Opposite faces at opposite ends → no shortcut: walk the island length + gap.
        assert_eq!(intra_island(&near0, &far1), 24.5);
        // Same face → straight down the aisle.
        let mid0 = sp(Building::East, 4, "ア", 0.0, 0.0, 5.0, 0.0, 0, l);
        assert_eq!(intra_island(&near0, &mid0), 5.0);
    }
}
