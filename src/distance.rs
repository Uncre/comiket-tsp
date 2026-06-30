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

use std::collections::HashMap;
use std::ops::Index;

use crate::layout::Space;

/// Walking distances (perceived metres) between hall clusters such as `E4-6` and `W1-2`,
/// loaded from `hall_distances.csv`. Used as the authoritative cross-cluster base cost,
/// replacing the flat building/hall penalties when present.
#[derive(Debug, Clone, Default)]
pub struct HallDistances {
    map: HashMap<(String, String), f64>,
}

impl HallDistances {
    /// An empty table (the solver then falls back to the legacy penalty model).
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a symmetric distance between two clusters.
    pub fn insert(&mut self, a: &str, b: &str, metres: f64) {
        self.map.insert((a.to_string(), b.to_string()), metres);
        self.map.insert((b.to_string(), a.to_string()), metres);
    }

    /// Look up the distance between two clusters (0 for the same cluster), if known.
    pub fn get(&self, a: &str, b: &str) -> Option<f64> {
        if a == b {
            return Some(0.0);
        }
        self.map.get(&(a.to_string(), b.to_string())).copied()
    }

    /// Whether no distances have been recorded.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

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
    /// Extra perceived metres charged for crossing faces through a 細通路 (minor aisle)
    /// or the far turnaround, but not through a 太通路 (中央通路 / 大通路). Makes the
    /// solver prefer routing through the wider, cheaper aisles. `0.0` disables it.
    pub pen_corridor: f64,
}

impl Default for DistanceParams {
    fn default() -> Self {
        Self {
            gamma: 1.25,
            pen_building: 250.0,
            pen_hall: 40.0,
            pen_block: 5.0,
            pen_corridor: 1.0,
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
/// whichever face-crossing point (island end or cross-aisle) gives the least perceived
/// cost: the along detour to reach it plus `pen_corridor` if it is a 細通路 / turnaround
/// (a 太通路 is free). Same-face legs run straight down the aisle.
pub fn intra_island(a: &Space, b: &Space, pen_corridor: f64) -> f64 {
    let dcross = (a.cross - b.cross).abs();
    match chosen_crossing(a, b, pen_corridor) {
        // Same face: straight down the aisle.
        None => (a.along - b.along).abs() + dcross,
        // Opposite faces: round the chosen crossing (its along-detour) then cross the gap.
        Some((_, detour)) => detour + dcross,
    }
}

/// The along position of the face-crossing the cost model uses for an opposite-face leg
/// on the same island, or `None` when the two spaces share a face (straight down the
/// aisle, no crossing). Shares [`chosen_crossing`] with [`intra_island`] so the route
/// polyline always runs through the exact crossing the cost was charged for.
pub fn intra_island_crossing(a: &Space, b: &Space, pen_corridor: f64) -> Option<f64> {
    chosen_crossing(a, b, pen_corridor).map(|(along, _)| along)
}

/// The cheapest face-crossing for an opposite-face same-island leg: `(along, detour)`,
/// where `detour` is the along-distance cost (with corridor penalty, excluding the cross
/// gap). `None` when the spaces share a face. The single source of truth behind both
/// [`intra_island`] (cost) and [`intra_island_crossing`] (geometry).
fn chosen_crossing(a: &Space, b: &Space, pen_corridor: f64) -> Option<(f64, f64)> {
    if a.face == b.face {
        return None;
    }
    if a.crossings.is_empty() {
        // Legacy fallback: the two island ends only (both treated as cheap aisles).
        let l = a.island_len;
        let near = a.along + b.along; // cross at along = 0
        let far = (l - a.along) + (l - b.along); // cross at along = l
        return Some(if near <= far { (0.0, near) } else { (l, far) });
    }
    a.crossings
        .iter()
        .map(|c| {
            let pen = if c.major { 0.0 } else { pen_corridor };
            let cost = (a.along - c.along).abs() + (b.along - c.along).abs() + pen;
            (c.along, cost)
        })
        .min_by(|x, y| x.1.total_cmp(&y.1))
}

/// Perceived walking cost between two spaces under `params`.
///
/// When `hall` is supplied and the two spaces sit in different hall clusters, the
/// cluster-to-cluster distance from the matrix is authoritative (it replaces the flat
/// building/hall penalties). Same-cluster legs use global Manhattan plus the small
/// hall/block penalties; same-island legs use the around-the-end model. With `hall =
/// None` the original penalty-only model applies, so legacy inputs are unchanged.
pub fn perceived(
    a: &Space,
    b: &Space,
    params: &DistanceParams,
    hall: Option<&HallDistances>,
) -> f64 {
    if same_island(a, b) {
        return intra_island(a, b, params.pen_corridor).powf(params.gamma);
    }
    if let Some(h) = hall {
        if a.cluster != b.cluster {
            if let Some(d) = h.get(&a.cluster, &b.cluster) {
                return d.powf(params.gamma);
            }
        }
    }
    let base = manhattan(a, b);
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
    ///
    /// Pass `hall` to use the authoritative inter-cluster distances; pass `None` for the
    /// legacy penalty-only model.
    pub fn build(spaces: &[Space], params: &DistanceParams, hall: Option<&HallDistances>) -> Self {
        let n = spaces.len();
        let mut data = vec![0.0; n * n];
        for (i, a) in spaces.iter().enumerate() {
            for (j, b) in spaces.iter().enumerate().skip(i + 1) {
                let d = perceived(a, b, params, hall);
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
    use crate::layout::{default_cluster, Crossing};
    use crate::space::{Building, Side};

    /// Build a bare [`Space`] with only the fields the distance model reads. The cluster
    /// is derived from `(building, hall)`; crossings are left empty (ends-only fallback).
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
            along_unit: Some((1.0, 0.0)),
            cross_unit: Some((0.0, 1.0)),
            cluster: default_cluster(building, hall),
            crossings: Vec::new(),
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

        let d_building = perceived(&a, &cross_building, &p, None);
        let d_hall = perceived(&a, &cross_hall, &p, None);
        let d_block = perceived(&a, &cross_block, &p, None);
        let d_intra = perceived(&a, &same_isl, &p, None);

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
        let m = DistanceMatrix::build(&spaces, &p, None);
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
        assert_eq!(intra_island(&near0, &near1, 0.0), 1.5);
        // Opposite faces, both at the far end → round the far end (again just the gap).
        assert_eq!(intra_island(&far0, &far1, 0.0), 1.5);
        // Opposite faces at opposite ends → no shortcut: walk the island length + gap.
        assert_eq!(intra_island(&near0, &far1, 0.0), 24.5);
        // Same face → straight down the aisle.
        let mid0 = sp(Building::East, 4, "ア", 0.0, 0.0, 5.0, 0.0, 0, l);
        assert_eq!(intra_island(&near0, &mid0, 0.0), 5.0);
    }

    #[test]
    fn intra_island_rounds_a_mid_corridor() {
        // A corridor lets you cross faces at along = 12, much nearer than either end.
        let l = 23.0;
        let mut a = sp(Building::East, 4, "ア", 0.0, 0.0, 11.0, 0.0, 0, l);
        let mut b = sp(Building::East, 4, "ア", 0.0, 0.0, 13.0, 1.5, 1, l);
        let cs = vec![
            Crossing {
                along: 0.0,
                major: true,
            },
            Crossing {
                along: l,
                major: false,
            },
            Crossing {
                along: 12.0,
                major: true,
            },
        ];
        a.crossings = cs.clone();
        b.crossings = cs;
        // Cross at the major aisle at 12: |11-12| + |13-12| = 2, plus the 1.5 face gap.
        assert_eq!(intra_island(&a, &b, 1.0), 3.5);
    }

    #[test]
    fn minor_corridor_costs_more_than_major() {
        // Same geometry, but the only mid aisle is a 細通路 → the penalty applies.
        let l = 23.0;
        let mut a = sp(Building::East, 4, "ア", 0.0, 0.0, 11.0, 0.0, 0, l);
        let mut b = sp(Building::East, 4, "ア", 0.0, 0.0, 13.0, 1.5, 1, l);
        let major = vec![
            Crossing {
                along: 0.0,
                major: true,
            },
            Crossing {
                along: l,
                major: false,
            },
            Crossing {
                along: 12.0,
                major: true,
            },
        ];
        let minor = vec![
            Crossing {
                along: 0.0,
                major: false,
            },
            Crossing {
                along: l,
                major: false,
            },
            Crossing {
                along: 12.0,
                major: false,
            },
        ];
        a.crossings = major.clone();
        b.crossings = major;
        let via_major = intra_island(&a, &b, 1.0);
        a.crossings = minor.clone();
        b.crossings = minor;
        let via_minor = intra_island(&a, &b, 1.0);
        assert!(via_minor > via_major, "{via_minor} !> {via_major}");
        assert_eq!(via_minor - via_major, 1.0);
    }

    #[test]
    fn cross_cluster_uses_matrix_and_replaces_penalties() {
        let p = DistanceParams::default();
        let mut hd = HallDistances::new();
        hd.insert("E4-6", "W1-2", 600.0);

        let a = sp(Building::East, 4, "ア", 0.0, 0.0, 0.0, 0.0, 0, 23.0);
        let b = sp(Building::West, 1, "あ", -200.0, 0.0, 0.0, 0.0, 0, 23.0);

        // With the matrix, the cross-cluster cost is the matrix value (squashed), not the
        // Manhattan-plus-building-penalty the legacy model would charge.
        assert_eq!(perceived(&a, &b, &p, Some(&hd)), 600.0_f64.powf(p.gamma));
        // Without the matrix, the legacy penalty model applies and differs.
        assert_ne!(
            perceived(&a, &b, &p, Some(&hd)),
            perceived(&a, &b, &p, None)
        );
    }

    #[test]
    fn same_cluster_falls_back_to_manhattan() {
        let p = DistanceParams::default();
        let hd = HallDistances::new(); // empty, but clusters match anyway
                                       // East 4 and East 5 share cluster E4-6, so the matrix is bypassed.
        let a = sp(Building::East, 4, "ア", 0.0, 0.0, 0.0, 0.0, 0, 23.0);
        let b = sp(Building::East, 5, "ハ", 10.0, 0.0, 0.0, 0.0, 0, 9.0);
        assert_eq!(
            perceived(&a, &b, &p, Some(&hd)),
            perceived(&a, &b, &p, None)
        );
    }
}
