//! `comiket-tsp` command-line entry point: argument parsing and the I/O boundary.
//!
//! All geometric and randomized logic lives in the `comiket_tsp` library; this file
//! only parses arguments, calls into the library, prints results, and reports errors
//! via `anyhow`.

use std::path::PathBuf;
use std::str::FromStr;

use clap::{Args, Parser, Subcommand};

use comiket_tsp::layout::Resolved;
use comiket_tsp::{
    io, layout, solve, Building, CometError, DistanceMatrix, DistanceParams, Layout, SolveConfig,
    SolveOutcome, SpaceId,
};

/// Plan an efficient walking route around a set of Comiket circles, modeled as a TSP.
#[derive(Debug, Parser)]
#[command(name = "comiket-tsp", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Expand a hand-authored block table into per-space coordinates (run rarely).
    GenLayout(GenLayoutArgs),
    /// Plan a route over a want-list using a layout artifact (run often).
    Solve(SolveArgs),
}

/// Arguments for `gen-layout`.
#[derive(Debug, Args)]
struct GenLayoutArgs {
    /// Hand-authored island table (`block_layout.csv`).
    #[arg(long)]
    blocks: PathBuf,
    /// Output layout artifact (`spaces.json`).
    #[arg(long)]
    out: PathBuf,
    /// Optional event label recorded in the artifact (e.g. `C107`).
    #[arg(long)]
    event: Option<String>,
}

/// Arguments for `solve`.
#[derive(Debug, Args)]
struct SolveArgs {
    /// Layout artifact produced by `gen-layout`.
    #[arg(long)]
    spaces: PathBuf,
    /// Want-list CSV (`space,name`).
    #[arg(long)]
    want: PathBuf,
    /// Output route CSV.
    #[arg(long)]
    out: PathBuf,
    /// RNG seed; same seed + inputs ⇒ identical route (with the time limit disabled).
    #[arg(long, default_value_t = 42)]
    seed: u64,
    /// Number of parallel ILS restarts.
    #[arg(long, default_value_t = 16)]
    restarts: usize,
    /// Wall-clock budget per restart in milliseconds (`0` disables the time limit).
    #[arg(long = "time-ms", default_value_t = 800)]
    time_ms: u64,
    /// Hard cap on ILS iterations per restart (the deterministic stopping criterion).
    #[arg(long = "max-iters", default_value_t = 100_000)]
    max_iters: usize,
    /// Fix the first stop to this space id (must be present in the want-list).
    #[arg(long)]
    start: Option<String>,
    /// Return to the start (closed tour). Default: open path.
    #[arg(long)]
    closed: bool,
    /// Nonlinear squash exponent.
    #[arg(long, default_value_t = 1.25)]
    gamma: f64,
    /// Penalty (perceived metres) for crossing buildings.
    #[arg(long = "pen-building", default_value_t = 250.0)]
    pen_building: f64,
    /// Penalty for crossing halls of the same building.
    #[arg(long = "pen-hall", default_value_t = 40.0)]
    pen_hall: f64,
    /// Penalty for crossing to an adjacent island.
    #[arg(long = "pen-block", default_value_t = 5.0)]
    pen_block: f64,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::GenLayout(args) => run_gen_layout(args),
        Command::Solve(args) => run_solve(args),
    }
}

fn run_gen_layout(args: GenLayoutArgs) -> anyhow::Result<()> {
    let blocks = io::read_blocks(&args.blocks)?;
    let spaces = layout::expand_blocks(&blocks);
    let n_spaces = spaces.len();
    let layout = Layout {
        event: args.event,
        spaces,
    };
    io::write_layout(&args.out, &layout)?;
    println!(
        "Wrote {n_spaces} spaces from {} blocks to {}",
        blocks.len(),
        args.out.display()
    );
    Ok(())
}

fn run_solve(args: SolveArgs) -> anyhow::Result<()> {
    let layout = io::read_layout(&args.spaces)?;
    let wants = io::read_want_list(&args.want)?;
    let resolved = layout::resolve_wants(&layout, &wants);

    for missing in &resolved.missing {
        eprintln!("Warning: want-list space not found in layout: {missing}");
    }
    if resolved.is_empty() {
        anyhow::bail!("No want-list spaces matched the layout; nothing to solve.");
    }

    let params = DistanceParams {
        gamma: args.gamma,
        pen_building: args.pen_building,
        pen_hall: args.pen_hall,
        pen_block: args.pen_block,
    };
    let matrix = DistanceMatrix::build(&resolved.spaces, &params);

    let start = match &args.start {
        Some(s) => Some(resolve_start(&resolved, s)?),
        None => None,
    };
    let config = SolveConfig {
        seed: args.seed,
        restarts: args.restarts,
        time_ms: args.time_ms,
        max_iters: args.max_iters,
        closed: args.closed,
        start,
    };
    let outcome = solve(&matrix, &config);

    let legs = io::route_legs(&resolved, &outcome, &matrix, args.closed);
    io::write_route(&args.out, &legs)?;

    print_itinerary(&resolved, &outcome, args.closed);
    println!(
        "Total perceived cost: {:.1} over {} stops{} -> {}",
        outcome.cost,
        outcome.order.len(),
        if args.closed { " (closed)" } else { "" },
        args.out.display()
    );
    if !resolved.missing.is_empty() {
        println!(
            "Skipped {} unmatched want-list entr{}.",
            resolved.missing.len(),
            if resolved.missing.len() == 1 {
                "y"
            } else {
                "ies"
            }
        );
    }
    Ok(())
}

/// Resolve a `--start` space id (canonicalised, lenient on case) to its index among
/// the matched want-list spaces.
fn resolve_start(resolved: &Resolved, start: &str) -> anyhow::Result<usize> {
    let canonical = SpaceId::from_str(start)?.to_string();
    resolved
        .position(&canonical)
        .ok_or_else(|| CometError::StartNotFound(canonical).into())
}

/// Print the route grouped by building and hall.
fn print_itinerary(resolved: &Resolved, outcome: &SolveOutcome, closed: bool) {
    println!("Itinerary:");
    let mut group: Option<(Building, u8)> = None;
    for (k, &idx) in outcome.order.iter().enumerate() {
        let space = &resolved.spaces[idx];
        let here = (space.building, space.hall);
        if group != Some(here) {
            println!("  [{:?} hall {}]", space.building, space.hall);
            group = Some(here);
        }
        println!("    {:>3}. {:<12} {}", k + 1, space.id, resolved.names[idx]);
    }
    if closed {
        if let Some(&first) = outcome.order.first() {
            println!("    ↩  return to {}", resolved.spaces[first].id);
        }
    }
}
