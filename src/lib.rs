//! comiket-tsp: plan an efficient walking route around Comiket circles as a TSP.
//!
//! The crate is split into pure, I/O-free modules — [`space`], [`layout`],
//! [`distance`], and [`solve`] — so they stay unit-testable. All file and stdout
//! I/O is confined to [`io`] and the binary's `main`.

pub mod distance;
pub mod io;
pub mod layout;
pub mod solve;
pub mod space;

pub use distance::{DistanceMatrix, DistanceParams, HallDistances};
pub use layout::{default_cluster, Block, BlockKind, Crossing, Dir, Layout, Space};
pub use solve::{solve, SolveConfig, SolveOutcome};
pub use space::{Building, Side, SpaceId};

use thiserror::Error;

/// Error type for all library operations.
///
/// The binary boundary (`main`) maps these into `anyhow` for reporting; library
/// code never panics or `unwrap`s on anything derived from user input.
#[derive(Debug, Error)]
pub enum CometError {
    /// A space-id string could not be parsed into a [`space::SpaceId`].
    #[error("invalid space id {input:?}: {reason}")]
    SpaceParse {
        /// The offending input string.
        input: String,
        /// Why parsing failed.
        reason: String,
    },

    /// A `block_layouts.csv` row was malformed or internally inconsistent.
    #[error("invalid block {id:?}: {reason}")]
    Block {
        /// The block id (or raw row) at fault.
        id: String,
        /// Why the block was rejected.
        reason: String,
    },

    /// A `hall_distances.csv` cell could not be parsed as a number.
    #[error("invalid hall-distance value {0:?}")]
    HallDistance(String),

    /// A want-list entry referenced a space absent from the layout artifact.
    #[error("want-list space not found in layout: {0}")]
    WantNotFound(String),

    /// The requested `--start` space was not present in the want-list.
    #[error("--start space not in want-list: {0}")]
    StartNotFound(String),

    /// Underlying CSV (de)serialization failure.
    #[error("csv: {0}")]
    Csv(#[from] csv::Error),

    /// Underlying JSON (de)serialization failure.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    /// Underlying filesystem I/O failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience result alias for library functions.
pub type Result<T> = std::result::Result<T, CometError>;
