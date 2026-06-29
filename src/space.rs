//! Space identity: [`SpaceId`] and its canonical string form (e.g. `E4-ア-31a`).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::CometError;

/// Which exhibition building a space sits in.
///
/// Serializes to its full name (`"East"`/`"West"`/`"South"`) in the layout
/// artifact, while the canonical id uses the single-letter [`Building::as_char`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Building {
    /// 東 — East building.
    East,
    /// 西 — West building.
    West,
    /// 南 — South building.
    South,
}

impl Building {
    /// The single-letter code used in a canonical id (`E`/`W`/`S`).
    pub fn as_char(self) -> char {
        match self {
            Building::East => 'E',
            Building::West => 'W',
            Building::South => 'S',
        }
    }

    /// Parse a single-letter building code, case-insensitively.
    pub fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_uppercase() {
            'E' => Some(Building::East),
            'W' => Some(Building::West),
            'S' => Some(Building::South),
            _ => None,
        }
    }
}

/// Which half of a shared table a circle occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    /// Left half (`a`).
    A,
    /// Right half (`b`).
    B,
}

impl Side {
    /// The lowercase letter used in a canonical id (`a`/`b`).
    pub fn as_char(self) -> char {
        match self {
            Side::A => 'a',
            Side::B => 'b',
        }
    }

    /// Parse a side letter, case-insensitively.
    pub fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_lowercase() {
            'a' => Some(Side::A),
            'b' => Some(Side::B),
            _ => None,
        }
    }
}

/// Fully-qualified identity of a single space (one circle's half-table).
///
/// Round-trips through the canonical string `"<B><hall>-<block>-<number><side>"`,
/// e.g. `E4-ア-31a`, via [`FromStr`] and [`Display`](fmt::Display). Parsing is
/// lenient about letter case; [`Display`](fmt::Display) always emits the canonical
/// form (uppercase building, lowercase side).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpaceId {
    /// Exhibition building.
    pub building: Building,
    /// Hall number within the building.
    pub hall: u8,
    /// Island/block letter (one grapheme; カタカナ・ひらがな・ラテン all valid).
    pub block: String,
    /// Serpentine index within the island.
    pub number: u16,
    /// Which half of the table.
    pub side: Side,
}

impl fmt::Display for SpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}-{}-{}{}",
            self.building.as_char(),
            self.hall,
            self.block,
            self.number,
            self.side.as_char()
        )
    }
}

impl FromStr for SpaceId {
    type Err = CometError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let reject = |reason: &str| CometError::SpaceParse {
            input: s.to_string(),
            reason: reason.to_string(),
        };

        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return Err(reject(
                "expected '<building><hall>-<block>-<number><side>' (three '-'-separated parts)",
            ));
        }

        // Part 0: the building letter followed by the hall number.
        let mut head = parts[0].chars();
        let building = head
            .next()
            .and_then(Building::from_char)
            .ok_or_else(|| reject("building must start with E, W, or S"))?;
        let hall: u8 = head
            .as_str()
            .parse()
            .map_err(|_| reject("hall must be an integer"))?;

        // Part 1: the block letter, kept verbatim (may be multi-byte).
        let block = parts[1];
        if block.is_empty() {
            return Err(reject("block must not be empty"));
        }

        // Part 2: the serpentine number followed by a single side letter.
        let tail = parts[2];
        let side_char = tail
            .chars()
            .next_back()
            .ok_or_else(|| reject("missing number and side"))?;
        let side = Side::from_char(side_char).ok_or_else(|| reject("side must be a or b"))?;
        let number: u16 = tail[..tail.len() - side_char.len_utf8()]
            .parse()
            .map_err(|_| reject("number must be a u16"))?;

        Ok(SpaceId {
            building,
            hall,
            block: block.to_string(),
            number,
            side,
        })
    }
}
