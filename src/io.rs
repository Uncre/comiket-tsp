//! CSV/JSON reading and writing — the only file I/O in the crate besides `main`.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use serde::Serialize;

use crate::distance::DistanceMatrix;
use crate::layout::{Block, BlockRow, Layout, Resolved, WantEntry};
use crate::solve::SolveOutcome;

/// Read and validate `block_layout.csv` into a list of [`Block`]s.
pub fn read_blocks(path: &Path) -> crate::Result<Vec<Block>> {
    let mut reader = csv::Reader::from_reader(BufReader::new(File::open(path)?));
    let mut blocks = Vec::new();
    for row in reader.deserialize::<BlockRow>() {
        blocks.push(Block::try_from(row?)?);
    }
    Ok(blocks)
}

/// Write a [`Layout`] artifact as pretty JSON, creating parent directories as needed.
pub fn write_layout(path: &Path, layout: &Layout) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let writer = BufWriter::new(File::create(path)?);
    serde_json::to_writer_pretty(writer, layout)?;
    Ok(())
}

/// Read a [`Layout`] artifact back from JSON.
pub fn read_layout(path: &Path) -> crate::Result<Layout> {
    Ok(serde_json::from_reader(BufReader::new(File::open(path)?))?)
}

/// Read `want_list.csv` into a list of [`WantEntry`]s.
pub fn read_want_list(path: &Path) -> crate::Result<Vec<WantEntry>> {
    let mut reader = csv::Reader::from_reader(BufReader::new(File::open(path)?));
    let mut wants = Vec::new();
    for row in reader.deserialize::<WantEntry>() {
        wants.push(row?);
    }
    Ok(wants)
}

/// One row of `route.csv`: visit order, space id, circle name, and costs.
#[derive(Debug, Serialize)]
pub struct RouteLeg {
    /// 1-based visit order.
    pub order: usize,
    /// Canonical space id.
    pub space: String,
    /// Circle name.
    pub name: String,
    /// Perceived cost of the leg from the previous stop (0 for the first).
    pub leg_cost: f64,
    /// Running total of perceived cost up to and including this leg.
    pub cumulative_cost: f64,
}

/// Turn a solved order into route rows, computing per-leg and cumulative cost.
///
/// When `closed`, a final row repeats the start to account for the return leg, so
/// the last `cumulative_cost` equals the closed-tour total.
pub fn route_legs(
    resolved: &Resolved,
    outcome: &SolveOutcome,
    matrix: &DistanceMatrix,
    closed: bool,
) -> Vec<RouteLeg> {
    let order = &outcome.order;
    let mut legs = Vec::with_capacity(order.len() + usize::from(closed));
    let mut cumulative = 0.0;
    let mut prev: Option<usize> = None;
    for (k, &idx) in order.iter().enumerate() {
        let leg = prev.map_or(0.0, |p| matrix[(p, idx)]);
        cumulative += leg;
        legs.push(RouteLeg {
            order: k + 1,
            space: resolved.spaces[idx].id.clone(),
            name: resolved.names[idx].clone(),
            leg_cost: leg,
            cumulative_cost: cumulative,
        });
        prev = Some(idx);
    }
    if closed {
        if let (Some(&first), Some(&last)) = (order.first(), order.last()) {
            let leg = matrix[(last, first)];
            cumulative += leg;
            legs.push(RouteLeg {
                order: order.len() + 1,
                space: resolved.spaces[first].id.clone(),
                name: format!("{} (return)", resolved.names[first]),
                leg_cost: leg,
                cumulative_cost: cumulative,
            });
        }
    }
    legs
}

/// Write route rows to `route.csv`, creating parent directories as needed.
pub fn write_route(path: &Path, legs: &[RouteLeg]) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut writer = csv::Writer::from_writer(BufWriter::new(File::create(path)?));
    for leg in legs {
        writer.serialize(leg)?;
    }
    writer.flush()?;
    Ok(())
}
