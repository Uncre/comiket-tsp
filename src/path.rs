//! Reconstruct the walked **route polyline** from a solved tour.
//!
//! The solver and distance model deal only in per-leg costs; they never materialise the
//! path between two stops. This module rebuilds, for every leg, a list of global `(x, y)`
//! waypoints so the route can be drawn on a venue map.
//!
//! - **Same-island** legs route through the exact face-crossing the cost model charged
//!   for (via [`crate::distance::intra_island_crossing`]), giving an accurate polyline
//!   `[a, P1, P2, b]` that rounds the chosen aisle rather than cutting through the tables.
//! - **Cross-island / cross-cluster** legs have no geometry in the model (Manhattan plus
//!   penalties, or a single hall-matrix number), so they are emitted as the straight
//!   segment `[a, b]`. The [`LegKind`] tag lets a renderer draw these approximate
//!   connectors differently (e.g. dashed).

use serde::Serialize;

use crate::distance::{intra_island_crossing, DistanceMatrix};
use crate::layout::{Resolved, Space};
use crate::solve::SolveOutcome;

/// How a leg's polyline was derived, so consumers can distinguish the accurate
/// same-island routing from the straight-line cross-island/cluster approximations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegKind {
    /// Both stops on the same physical island; the polyline rounds the chosen crossing.
    IntraIsland,
    /// Different islands in the same hall cluster; straight-segment approximation.
    CrossIsland,
    /// Different hall clusters; straight-segment approximation (no geometry in the model).
    CrossCluster,
}

/// One leg of the route: the walk from one stop to the next, with its cost and polyline.
#[derive(Debug, Clone, Serialize)]
pub struct PathLeg {
    /// 1-based index of the originating stop (leg connects stop `order` to `order + 1`;
    /// for a closed tour the final leg returns from the last stop to the first).
    pub order: usize,
    /// Canonical space id the leg starts at.
    pub from_space: String,
    /// Canonical space id the leg ends at.
    pub to_space: String,
    /// Perceived cost of this leg.
    pub leg_cost: f64,
    /// Running total of perceived cost up to and including this leg.
    pub cumulative_cost: f64,
    /// How the polyline was derived.
    pub kind: LegKind,
    /// Global `(x, y)` waypoints in metres, from `from_space` to `to_space` (inclusive).
    pub waypoints: Vec<(f64, f64)>,
}

/// The full route as polylines: every leg in visit order, plus the totals.
#[derive(Debug, Clone, Serialize)]
pub struct RoutePath {
    /// Event label carried over from the layout artifact, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Whether the tour returns to its start (a final return leg is included).
    pub closed: bool,
    /// Total perceived cost (matches [`SolveOutcome::cost`]).
    pub total_cost: f64,
    /// The legs in visit order.
    pub legs: Vec<PathLeg>,
}

/// Build the route polyline from a solved order.
///
/// Walks the consecutive stop pairs (mirroring [`crate::io::route_legs`]) and, when
/// `closed`, appends the return leg from the last stop back to the first. `pen_corridor`
/// must match the value used to build `matrix` so the polyline runs through the same
/// crossings the cost charged for.
pub fn build_route_path(
    resolved: &Resolved,
    outcome: &SolveOutcome,
    matrix: &DistanceMatrix,
    pen_corridor: f64,
    closed: bool,
    event: Option<String>,
) -> RoutePath {
    let order = &outcome.order;
    let n = order.len();
    // Number of legs: one between each consecutive pair, plus the return leg when closed.
    // A tour of 0 or 1 stops has no travel.
    let n_legs = match (closed, n) {
        (_, 0 | 1) => 0,
        (true, _) => n,
        (false, _) => n - 1,
    };

    let mut legs = Vec::with_capacity(n_legs);
    let mut cumulative = 0.0;
    for i in 0..n_legs {
        let from_idx = order[i];
        let to_idx = order[(i + 1) % n];
        let a = &resolved.spaces[from_idx];
        let b = &resolved.spaces[to_idx];
        let leg_cost = matrix[(from_idx, to_idx)];
        cumulative += leg_cost;
        legs.push(PathLeg {
            order: i + 1,
            from_space: a.id.clone(),
            to_space: b.id.clone(),
            leg_cost,
            cumulative_cost: cumulative,
            kind: leg_kind(a, b),
            waypoints: leg_waypoints(a, b, pen_corridor),
        });
    }

    RoutePath {
        event,
        closed,
        total_cost: outcome.cost,
        legs,
    }
}

/// True when both spaces sit on the same physical island (building, hall, and block).
fn same_island(a: &Space, b: &Space) -> bool {
    a.building == b.building && a.hall == b.hall && a.block == b.block
}

/// Classify a leg the same way the distance model splits its cases.
fn leg_kind(a: &Space, b: &Space) -> LegKind {
    if same_island(a, b) {
        LegKind::IntraIsland
    } else if a.cluster == b.cluster {
        LegKind::CrossIsland
    } else {
        LegKind::CrossCluster
    }
}

/// Global waypoints for one leg, from `a` to `b`.
///
/// Same-island opposite-face legs round the chosen crossing; everything else (same face,
/// cross-island, cross-cluster, or a legacy artifact missing the axis vectors) is the
/// straight segment `[a, b]`.
fn leg_waypoints(a: &Space, b: &Space, pen_corridor: f64) -> Vec<(f64, f64)> {
    let straight = || vec![(a.x, a.y), (b.x, b.y)];
    if !same_island(a, b) {
        return straight();
    }
    match (
        intra_island_crossing(a, b, pen_corridor),
        a.along_unit,
        a.cross_unit,
    ) {
        (Some(c), Some(u), Some(v)) => {
            // a → P1 along a's face to the crossing, P1 → P2 across to b's face,
            // P2 → b along b's face. Derived from local→global: P = anchor + along·U + cross·V.
            let p1 = (a.x + (c - a.along) * u.0, a.y + (c - a.along) * u.1);
            let dcross = b.cross - a.cross;
            let p2 = (p1.0 + dcross * v.0, p1.1 + dcross * v.1);
            dedup(vec![(a.x, a.y), p1, p2, (b.x, b.y)])
        }
        // Same face (no crossing) or missing axis vectors → straight down the aisle.
        _ => straight(),
    }
}

/// Drop consecutive points that coincide (within a small epsilon), so a crossing landing
/// on a stop doesn't emit a zero-length segment.
fn dedup(points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(points.len());
    for p in points {
        let same = out
            .last()
            .is_some_and(|q| (q.0 - p.0).abs() <= 1e-9 && (q.1 - p.1).abs() <= 1e-9);
        if !same {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{default_cluster, Crossing};
    use crate::space::{Building, Side};

    /// A minimal [`Space`] with the geometry the path builder reads. `anchor` is the
    /// global origin and the axes are the identity (along = +x, cross = +y), so global
    /// `(x, y) = (anchor.0 + along, anchor.1 + cross)`.
    #[allow(clippy::too_many_arguments)]
    fn sp(
        id: &str,
        building: Building,
        hall: u8,
        block: &str,
        anchor: (f64, f64),
        along: f64,
        cross: f64,
        face: u8,
        island_len: f64,
        crossings: Vec<Crossing>,
        axes: Option<((f64, f64), (f64, f64))>,
    ) -> Space {
        let (along_unit, cross_unit) = match axes {
            Some((u, v)) => (Some(u), Some(v)),
            None => (None, None),
        };
        Space {
            id: id.to_string(),
            building,
            hall,
            block: block.to_string(),
            number: 1,
            side: Side::A,
            x: anchor.0 + along,
            y: anchor.1 + cross,
            along,
            cross,
            face,
            island_len,
            along_unit,
            cross_unit,
            cluster: default_cluster(building, hall),
            crossings,
        }
    }

    fn ident_axes() -> Option<((f64, f64), (f64, f64))> {
        Some(((1.0, 0.0), (0.0, 1.0)))
    }

    #[test]
    fn opposite_faces_route_through_chosen_crossing() {
        let crossings = vec![
            Crossing {
                along: 0.0,
                major: true,
            },
            Crossing {
                along: 10.0,
                major: false,
            },
        ];
        let a = sp(
            "E4-ア-3a",
            Building::East,
            4,
            "ア",
            (0.0, 0.0),
            2.0,
            0.0,
            0,
            10.0,
            crossings.clone(),
            ident_axes(),
        );
        let b = sp(
            "E4-ア-18a",
            Building::East,
            4,
            "ア",
            (0.0, 0.0),
            8.0,
            1.5,
            1,
            10.0,
            crossings,
            ident_axes(),
        );

        // Near end (along 0) is cheapest, so the crossing the cost picks is at along 0.
        assert_eq!(intra_island_crossing(&a, &b, 1.0), Some(0.0));
        let wp = leg_waypoints(&a, &b, 1.0);
        assert_eq!(
            wp,
            vec![(2.0, 0.0), (0.0, 0.0), (0.0, 1.5), (8.0, 1.5)],
            "polyline must round the chosen crossing"
        );
        // Endpoints coincide with the two booths.
        assert_eq!(*wp.first().unwrap(), (a.x, a.y));
        assert_eq!(*wp.last().unwrap(), (b.x, b.y));
        assert_eq!(leg_kind(&a, &b), LegKind::IntraIsland);
    }

    #[test]
    fn same_face_is_a_straight_segment() {
        let a = sp(
            "E4-ア-3a",
            Building::East,
            4,
            "ア",
            (0.0, 0.0),
            2.0,
            0.0,
            0,
            10.0,
            Vec::new(),
            ident_axes(),
        );
        let b = sp(
            "E4-ア-5a",
            Building::East,
            4,
            "ア",
            (0.0, 0.0),
            6.0,
            0.0,
            0,
            10.0,
            Vec::new(),
            ident_axes(),
        );
        assert_eq!(intra_island_crossing(&a, &b, 1.0), None);
        assert_eq!(leg_waypoints(&a, &b, 1.0), vec![(2.0, 0.0), (6.0, 0.0)]);
    }

    #[test]
    fn missing_axes_falls_back_to_straight() {
        let crossings = vec![Crossing {
            along: 0.0,
            major: true,
        }];
        let a = sp(
            "E4-ア-3a",
            Building::East,
            4,
            "ア",
            (0.0, 0.0),
            2.0,
            0.0,
            0,
            10.0,
            crossings.clone(),
            None,
        );
        let b = sp(
            "E4-ア-18a",
            Building::East,
            4,
            "ア",
            (0.0, 0.0),
            8.0,
            1.5,
            1,
            10.0,
            crossings,
            None,
        );
        // Opposite faces, but no axis vectors → can't place the crossing → straight.
        assert_eq!(leg_waypoints(&a, &b, 1.0), vec![(2.0, 0.0), (8.0, 1.5)]);
    }

    #[test]
    fn cross_island_and_cross_cluster_are_straight_with_right_kind() {
        let a = sp(
            "E4-ア-3a",
            Building::East,
            4,
            "ア",
            (0.0, 0.0),
            0.0,
            0.0,
            0,
            10.0,
            Vec::new(),
            ident_axes(),
        );
        // Same cluster (E4-6), different block → cross-island.
        let same_cluster = sp(
            "E5-カ-1a",
            Building::East,
            5,
            "カ",
            (50.0, 0.0),
            0.0,
            0.0,
            0,
            10.0,
            Vec::new(),
            ident_axes(),
        );
        assert_eq!(leg_kind(&a, &same_cluster), LegKind::CrossIsland);
        assert_eq!(
            leg_waypoints(&a, &same_cluster, 1.0),
            vec![(0.0, 0.0), (50.0, 0.0)]
        );

        // Different cluster (West) → cross-cluster.
        let other_cluster = sp(
            "W1-あ-1a",
            Building::West,
            1,
            "あ",
            (-200.0, 0.0),
            0.0,
            0.0,
            0,
            10.0,
            Vec::new(),
            ident_axes(),
        );
        assert_eq!(leg_kind(&a, &other_cluster), LegKind::CrossCluster);
        assert_eq!(
            leg_waypoints(&a, &other_cluster, 1.0),
            vec![(0.0, 0.0), (-200.0, 0.0)]
        );
    }
}
