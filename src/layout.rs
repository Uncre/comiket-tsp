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

/// One island ("島"), authored once per event as a row of `block_layout.csv`.
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
    /// Unit direction of increasing depth along the island.
    pub along: Dir,
    /// Unit direction from face 0 to face 1.
    pub cross: Dir,
    /// Highest serpentine number in this island.
    pub n_max: u16,
    /// Override for the number of desks on face 0 (default `n_max / 2`).
    pub face0_len: Option<u16>,
    /// Desk spacing along the island, in metres (≈ 1.0).
    pub pitch: f64,
    /// Distance between the two faces, in metres (≈ 1.5).
    pub island_width: f64,
}

/// Raw `block_layout.csv` row, deserialized verbatim before validation.
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
    face0_len: Option<u16>,
    pitch: f64,
    island_width: f64,
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
        Ok(Block {
            id: row.id,
            block,
            building: row.building,
            hall: row.hall,
            anchor: (row.anchor_x, row.anchor_y),
            along: row.along,
            cross: row.cross,
            n_max: row.n_max,
            face0_len: row.face0_len,
            pitch: row.pitch,
            island_width: row.island_width,
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
        let f0 = self.face0_len();
        let f1 = self.n_max - f0;
        f64::from(f0.max(f1).saturating_sub(1)) * self.pitch
    }

    /// Expand this island into all `2 * n_max` spaces (each number has an `a`/`b` half).
    ///
    /// Numbers run serpentine: up face 0 to the far end, then back down face 1 (see
    /// `IMPLEMENTATION_PLAN.md` §2.3). `n = 1`, side `a` lands exactly on the anchor.
    pub fn expand(&self) -> Vec<Space> {
        let half = self.face0_len();
        let island_len = self.island_len();
        let (along_x, along_y) = self.along.unit();
        let (cross_x, cross_y) = self.cross.unit();
        let (anchor_x, anchor_y) = self.anchor;

        let mut spaces = Vec::with_capacity(self.n_max as usize * 2);
        for n in 1..=self.n_max {
            let (face, depth) = if n <= half {
                (0u8, n)
            } else {
                (1u8, self.n_max - n + 1)
            };
            let along = f64::from(depth - 1) * self.pitch;
            for side in [Side::A, Side::B] {
                let cross = f64::from(face) * self.island_width + side_offset(side);
                let id = SpaceId {
                    building: self.building,
                    hall: self.hall,
                    block: self.block.clone(),
                    number: n,
                    side,
                };
                spaces.push(Space {
                    id: id.to_string(),
                    building: self.building,
                    hall: self.hall,
                    block: self.block.clone(),
                    number: n,
                    side,
                    x: anchor_x + along * along_x + cross * cross_x,
                    y: anchor_y + along * along_y + cross * cross_y,
                    along,
                    cross,
                    face,
                    island_len,
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
    let index: HashMap<&str, &Space> =
        layout.spaces.iter().map(|s| (s.id.as_str(), s)).collect();
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
