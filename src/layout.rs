//! Venue layout: the [`Block`] island table and serpentine expansion into [`Space`]s.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::space::{Building, Side, SpaceId};
use crate::CometError;

/// Lateral offset (metres) of the `b` half-table from the `a` half-table.
const SIDE_B_OFFSET: f64 = 0.45;

/// A cardinal direction with a unit vector in the global frame (x = East, y = North).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Dir {
    /// North (+y).
    N,
    /// South (−y).
    S,
    /// East (+x).
    E,
    /// West (−x).
    W,
}

impl Dir {
    /// Unit vector `(dx, dy)` for this direction.
    pub fn unit(self) -> (f64, f64) {
        match self {
            Dir::N => (0.0, 1.0),
            Dir::S => (0.0, -1.0),
            Dir::E => (1.0, 0.0),
            Dir::W => (-1.0, 0.0),
        }
    }
}

/// Unit vector for an axis: an explicit angle (degrees CCW from East) wins, else the
/// cardinal [`Dir`]. The angle override is what lets diagonal halls (東7/8) be placed.
fn axis_unit(dir: Dir, deg: Option<f64>) -> (f64, f64) {
    match deg {
        Some(d) => {
            let r = d.to_radians();
            (r.cos(), r.sin())
        }
        None => dir.unit(),
    }
}

/// What sort of run a block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockKind {
    /// A standard double-faced island: serpentine up face 0, back down face 1.
    #[default]
    Island,
    /// A 壁サークル run along a wall: a single face whose numbers increase linearly.
    Wall,
}

/// Default hall-cluster id for a `(building, hall)`, matching the inter-hall distance
/// matrix nodes: East 1-3 / 4-6 / 7-8 are grouped, West 1-2 grouped, South per-hall.
pub fn default_cluster(building: Building, hall: u8) -> String {
    match (building, hall) {
        (Building::East, 1..=3) => "E1-3".to_string(),
        (Building::East, 4..=6) => "E4-6".to_string(),
        (Building::East, 7..=8) => "E7-8".to_string(),
        (Building::West, 1..=2) => "W1-2".to_string(),
        (Building::South, h) => format!("S{h}"),
        (b, h) => format!("{}{}", b.as_char(), h),
    }
}

/// A point along an island where the two faces can be crossed: an island end, or a
/// cross-aisle (同じ接頭辞を区切る通路). `major` marks a 太通路 (中央通路 / 大通路) — wider
/// and slightly cheaper to walk through than a 細通路.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crossing {
    /// Along-axis position (metres from the near end) of the crossing.
    pub along: f64,
    /// Whether this is a 太通路 (major); `false` for a 細通路 / turnaround (minor).
    #[serde(default)]
    pub major: bool,
}

/// Parse a `;`-separated list of crossings. Each item is `along` or `along@kind`, where
/// `kind` is `major`/`minor` (anything but `major` ⇒ minor; a bare number ⇒ minor). The
/// bare-number form keeps legacy `block_layouts.csv` files parsing unchanged.
fn parse_crossings(s: &str) -> Option<Vec<Crossing>> {
    s.split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (num, kind) = p.split_once('@').unwrap_or((p, ""));
            let along = num.trim().parse::<f64>().ok()?;
            Some(Crossing {
                along,
                major: kind.trim().eq_ignore_ascii_case("major"),
            })
        })
        .collect()
}

/// One island ("島"), authored once per event as a row of `block_layouts.csv`.
#[derive(Debug, Clone)]
pub struct Block {
    /// Island id such as `E4-ア` (building letter + hall, then `-`, then block letter).
    pub id: String,
    /// Block letter extracted from [`Block::id`] (e.g. `ア`).
    pub block: String,
    /// Exhibition building.
    pub building: Building,
    /// Hall number.
    pub hall: u8,
    /// Global metres of the `n = 1`, face-0, side-`a` desk.
    pub anchor: (f64, f64),
    /// Cardinal direction of increasing depth along the island.
    pub along: Dir,
    /// Cardinal direction from face 0 to face 1.
    pub cross: Dir,
    /// Optional angle (degrees CCW from East) overriding [`Block::along`]; enables
    /// diagonal halls (東7/8). `None` ⇒ use the cardinal direction.
    pub along_deg: Option<f64>,
    /// Optional angle (degrees CCW from East) overriding [`Block::cross`].
    pub cross_deg: Option<f64>,
    /// Highest serpentine number in this island.
    pub n_max: u16,
    /// Override for the number of desks on face 0 (default `n_max / 2`). Ignored for
    /// [`BlockKind::Wall`].
    pub face0_len: Option<u16>,
    /// Desk spacing along the island, in metres (≈ 1.0).
    pub pitch: f64,
    /// Distance between the two faces, in metres (≈ 1.5).
    pub island_width: f64,
    /// Whether this is a normal island or a single-faced wall run (壁サークル).
    pub kind: BlockKind,
    /// Serpentine-number offset added to every space, so a wall split into several
    /// segments keeps continuous numbering (segment 2 starting at, say, 23 not 1). `0`
    /// for ordinary islands and single-segment walls.
    pub number_base: u16,
    /// Hall-cluster id (e.g. `E4-6`) used by the inter-hall distance matrix.
    pub cluster: String,
    /// Extra mid-island cross-aisles (the corridors that split a single prefix). The
    /// island ends are always implied; see [`Block::resolved_crossings`].
    pub crossings: Vec<Crossing>,
}

/// Raw `block_layouts.csv` row, deserialized verbatim before validation.
///
/// The first eleven columns are the original schema; the trailing five are optional
/// (`#[serde(default)]`) so older files — and rows that omit them — still parse.
#[derive(Debug, Deserialize)]
pub(crate) struct BlockRow {
    id: String,
    building: Building,
    hall: u8,
    anchor_x: f64,
    anchor_y: f64,
    along: Dir,
    cross: Dir,
    n_max: u16,
    #[serde(default)]
    face0_len: Option<u16>,
    pitch: f64,
    island_width: f64,
    #[serde(default)]
    kind: Option<BlockKind>,
    #[serde(default)]
    cluster: Option<String>,
    #[serde(default)]
    along_deg: Option<f64>,
    #[serde(default)]
    cross_deg: Option<f64>,
    #[serde(default)]
    crossings: Option<String>,
    #[serde(default)]
    number_base: u16,
}

impl TryFrom<BlockRow> for Block {
    type Error = CometError;

    fn try_from(row: BlockRow) -> Result<Self, Self::Error> {
        let reject = |reason: &str| CometError::Block {
            id: row.id.clone(),
            reason: reason.to_string(),
        };
        let block = row
            .id
            .split_once('-')
            .map(|(_, b)| b)
            .filter(|b| !b.is_empty())
            .ok_or_else(|| reject("id must look like '<building><hall>-<block>'"))?
            .to_string();
        if row.n_max == 0 {
            return Err(reject("n_max must be at least 1"));
        }
        if matches!(row.face0_len, Some(f0) if f0 > row.n_max) {
            return Err(reject("face0_len must not exceed n_max"));
        }
        if row.pitch <= 0.0 || row.island_width <= 0.0 {
            return Err(reject("pitch and island_width must be positive"));
        }
        let crossings = match &row.crossings {
            Some(s) if !s.trim().is_empty() => parse_crossings(s)
                .ok_or_else(|| reject("crossings must be ';'-separated numbers"))?,
            _ => Vec::new(),
        };
        let cluster = row
            .cluster
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| default_cluster(row.building, row.hall));
        Ok(Block {
            id: row.id,
            block,
            building: row.building,
            hall: row.hall,
            anchor: (row.anchor_x, row.anchor_y),
            along: row.along,
            cross: row.cross,
            along_deg: row.along_deg,
            cross_deg: row.cross_deg,
            n_max: row.n_max,
            face0_len: row.face0_len,
            pitch: row.pitch,
            island_width: row.island_width,
            kind: row.kind.unwrap_or_default(),
            number_base: row.number_base,
            cluster,
            crossings,
        })
    }
}

impl Block {
    /// Number of desks on face 0 (the override, or `n_max / 2`).
    pub fn face0_len(&self) -> u16 {
        self.face0_len.unwrap_or(self.n_max / 2)
    }

    /// Along-axis distance (metres) from the near end to the far end of the island —
    /// the length of the longer face. Used by the same-island distance model.
    pub fn island_len(&self) -> f64 {
        let longest = match self.kind {
            // A wall run is one face, so its length spans all `n_max` desks.
            BlockKind::Wall => self.n_max,
            BlockKind::Island => {
                let f0 = self.face0_len();
                let f1 = self.n_max - f0;
                f0.max(f1)
            }
        };
        f64::from(longest.saturating_sub(1)) * self.pitch
    }

    /// Map a serpentine number to its `(face, depth)`. Islands run up face 0 then back
    /// down face 1; wall runs stay on face 0 with depth increasing linearly.
    fn face_depth(&self, n: u16) -> (u8, u16) {
        match self.kind {
            BlockKind::Wall => (0, n),
            BlockKind::Island => {
                let half = self.face0_len();
                if n <= half {
                    (0, n)
                } else {
                    (1, self.n_max - n + 1)
                }
            }
        }
    }

    /// Face-crossing points for this island: the two ends plus any authored mid-island
    /// corridors. The near end (the main aisle, depth 0) is a 大通路 (major); the far end
    /// (the 折り返し turnaround) is treated as minor. The same list is stored on every
    /// space in the island.
    fn resolved_crossings(&self, island_len: f64) -> Vec<Crossing> {
        let mut c = Vec::with_capacity(self.crossings.len() + 2);
        c.push(Crossing {
            along: 0.0,
            major: true,
        });
        c.push(Crossing {
            along: island_len,
            major: false,
        });
        c.extend(self.crossings.iter().copied());
        c
    }

    /// Expand this island into all `2 * n_max` spaces (each number has an `a`/`b` half).
    ///
    /// Numbers run serpentine: up face 0 to the far end, then back down face 1 (see
    /// `IMPLEMENTATION_PLAN.md` §2.3). `n = 1`, side `a` lands exactly on the anchor.
    /// Wall runs ([`BlockKind::Wall`]) keep every number on face 0.
    pub fn expand(&self) -> Vec<Space> {
        let island_len = self.island_len();
        let (along_x, along_y) = axis_unit(self.along, self.along_deg);
        let (cross_x, cross_y) = axis_unit(self.cross, self.cross_deg);
        let (anchor_x, anchor_y) = self.anchor;
        let crossings = self.resolved_crossings(island_len);

        let mut spaces = Vec::with_capacity(self.n_max as usize * 2);
        for n in 1..=self.n_max {
            let (face, depth) = self.face_depth(n);
            let along = f64::from(depth - 1) * self.pitch;
            let number = self.number_base + n;
            for side in [Side::A, Side::B] {
                let cross = f64::from(face) * self.island_width + side_offset(side);
                let id = SpaceId {
                    building: self.building,
                    hall: self.hall,
                    block: self.block.clone(),
                    number,
                    side,
                };
                spaces.push(Space {
                    id: id.to_string(),
                    building: self.building,
                    hall: self.hall,
                    block: self.block.clone(),
                    number,
                    side,
                    x: anchor_x + along * along_x + cross * cross_x,
                    y: anchor_y + along * along_y + cross * cross_y,
                    along,
                    cross,
                    face,
                    island_len,
                    cluster: self.cluster.clone(),
                    crossings: crossings.clone(),
                });
            }
        }
        spaces
    }
}

/// Lateral offset of a side from the island's face line.
fn side_offset(side: Side) -> f64 {
    match side {
        Side::A => 0.0,
        Side::B => SIDE_B_OFFSET,
    }
}

/// Expand every block into a single flat list of spaces.
pub fn expand_blocks(blocks: &[Block]) -> Vec<Space> {
    blocks.iter().flat_map(Block::expand).collect()
}

/// One fully-placed space: identity, decomposed fields, and geometry.
///
/// `x`/`y` are global metres (for cross-island Manhattan distance); `along`/`cross`
/// are local island metres and `island_len` the island's far-end along-position
/// (for the same-island around-the-end distance model).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    /// Canonical id string, e.g. `E4-ア-31a`.
    pub id: String,
    /// Exhibition building.
    pub building: Building,
    /// Hall number.
    pub hall: u8,
    /// Island/block letter.
    pub block: String,
    /// Serpentine number.
    pub number: u16,
    /// Table half.
    pub side: Side,
    /// Global x (metres, East-positive).
    pub x: f64,
    /// Global y (metres, North-positive).
    pub y: f64,
    /// Local along-axis position within the island (metres from the near end).
    pub along: f64,
    /// Local cross-axis position within the island (metres from face 0, side `a`).
    pub cross: f64,
    /// Which face (0 or 1) of the island this space is on.
    pub face: u8,
    /// The island's far-end along-position (metres); identical for every space in it.
    pub island_len: f64,
    /// Hall-cluster id (e.g. `E4-6`) for the inter-hall distance matrix.
    #[serde(default)]
    pub cluster: String,
    /// Points where the two faces can be crossed (ends + corridors), with each marked
    /// 太通路/細通路; identical for every space in the island. Empty ⇒ ends-only fallback.
    #[serde(default)]
    pub crossings: Vec<Crossing>,
}

/// The `gen-layout` artifact: an optional event label and every placed space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    /// Event label (e.g. `C107`), if provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// All spaces across all islands.
    pub spaces: Vec<Space>,
}

/// One row of `want_list.csv`: a canonical space id and a free-text circle name.
#[derive(Debug, Clone, Deserialize)]
pub struct WantEntry {
    /// Canonical space id, e.g. `E4-ア-31a`.
    pub space: String,
    /// Free-text circle name for the itinerary.
    pub name: String,
}

/// A want-list joined against a [`Layout`]: matched spaces (in want-list order),
/// their parallel names, and any ids that were not found.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    /// Matched spaces, in want-list order.
    pub spaces: Vec<Space>,
    /// Circle names, parallel to [`Resolved::spaces`].
    pub names: Vec<String>,
    /// Want-list ids absent from the layout (reported as warnings, then skipped).
    pub missing: Vec<String>,
}

impl Resolved {
    /// Index of the matched space with the given canonical id, if present.
    pub fn position(&self, id: &str) -> Option<usize> {
        self.spaces.iter().position(|s| s.id == id)
    }

    /// Number of matched spaces.
    pub fn len(&self) -> usize {
        self.spaces.len()
    }

    /// Whether nothing matched.
    pub fn is_empty(&self) -> bool {
        self.spaces.is_empty()
    }
}

/// Join a want-list against a layout, preserving want-list order and collecting misses.
pub fn resolve_wants(layout: &Layout, wants: &[WantEntry]) -> Resolved {
    let index: HashMap<&str, &Space> = layout.spaces.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut resolved = Resolved::default();
    for want in wants {
        if let Some(&space) = index.get(want.space.as_str()) {
            resolved.spaces.push(space.clone());
            resolved.names.push(want.name.clone());
        } else {
            resolved.missing.push(want.space.clone());
        }
    }
    resolved
}
