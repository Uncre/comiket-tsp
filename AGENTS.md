# AGENTS.md

Guidance for any coding agent working in this repo. Read `IMPLEMENTATION_PLAN.md`
for the full design and the phased task list.

## Project

`comiket-tsp` is a Rust CLI that plans an efficient walking route around a set of
Comiket circles, modeled as a Travelling Salesman Problem.

It has **two independent stages**, deliberately decoupled so the want-list can
change without rebuilding the venue:

1. `gen-layout` — expands a hand-authored *block table* (one row per island) into
   global coordinates for every space, and writes a reusable artifact.
2. `solve` — reads a want-list + the layout artifact, builds a perceived-distance
   matrix over only the wanted spaces, and runs local search to produce a route.

The expensive/manual part (authoring the block table) feeds stage 1 and is reused.
Changing which circles to visit only re-runs stage 2.

## Stack

- Rust, latest stable (set `edition = "2021"` unless 2024 is needed).
- `clap` v4 (`derive` feature) — CLI + subcommands.
- `csv` v1 + `serde` (`derive`) — CSV I/O.
- `serde_json` — the layout artifact (`spaces.json`).
- `rand` v0.9+ — RNG for simulated annealing / iterated local search.
- `rayon` v1 — parallel independent restarts.
- `anyhow` (binary boundary) + `thiserror` (library errors).
- dev: `criterion` (optional benches).

Resolve exact versions with `cargo add`; do not hand-pin patch numbers.

### rand API gotcha (read before writing any RNG code)

rand 0.9/0.10 renamed much of the surface. Do **not** copy rand 0.8 tutorials.
Current API:

- `rand::thread_rng()` → `rand::rng()`
- `rng.gen_range(0..n)` → `rng.random_range(0..n)`
- `rng.gen::<f64>()` → `rng.random::<f64>()`
- `rand::distributions::*` → `rand::distr::*`
- seedable RNG: `use rand::{SeedableRng, rngs::StdRng}; let mut rng = StdRng::seed_from_u64(seed);`

Confirm against `cargo doc -p rand --open` for the resolved version before coding.

## Commands

```bash
cargo build --release
cargo clippy --all-targets -- -D warnings
cargo fmt
cargo test

# stage 1: build venue coordinates (run rarely)
cargo run --release -- gen-layout \
  --blocks data/block_layout.csv \
  --out artifacts/spaces.json

# stage 2: plan a route (run often, cheap)
cargo run --release -- solve \
  --spaces artifacts/spaces.json \
  --want data/want_list.csv \
  --out route.csv \
  --seed 42 --restarts 16
```

## Conventions

- **Separation of concerns**: `space`, `layout`, `distance`, `solve` are pure (no
  file/stdout I/O) so they stay unit-testable. All I/O lives in `io.rs` and
  `main.rs`.
- **Errors**: library modules return `Result<_, CometError>` (thiserror). No
  `unwrap()` / `expect()` / `panic!` on anything derived from user input. `main.rs`
  may use `anyhow` and `?`.
- **Determinism**: every randomized routine takes an explicit `seed: u64`. Same
  seed + same inputs ⇒ byte-identical route. Parallel restarts derive child seeds
  deterministically from the base seed (e.g. `base ^ restart_index`).
- **Distances** are `f64` "perceived meters". Never truncate to integers.
- Public items get `///` docs. A task is done only when `cargo fmt` is applied and
  `cargo clippy --all-targets -- -D warnings` is clean.
- Sentence-case log/CLI messages. Keep `println!` out of library code; return data.

## Data contracts

Schemas are defined in `IMPLEMENTATION_PLAN.md` (§ Data formats). The CSV headers
and the `spaces.json` shape are a contract between the two stages — do not change a
field without updating the plan and the round-trip tests.

## Testing bar

- `space`: round-trip parse for space strings incl. `a`/`b` side and multi-byte
  block letters (カタカナ・ひらがな・ラテン).
- `layout`: serpentine derivation — assert known `(n → depth, face)` pairs for both
  an even and an odd `n_max`, and assert `n = 1` maps to the block anchor.
- `distance`: symmetry `d(i,j) == d(j,i)`; ordering
  `building-cross ≫ hall-cross ≫ block-cross ≫ intra-island`.
- `solve`: a tiny hand-checked instance (≤ 6 nodes) whose optimum is known — assert
  the solver reaches it; a TSPLIB-style grid — assert within tolerance of the known
  optimum. All randomized tests use a fixed seed.

## Definition of done (per task)

Compiles · `clippy -D warnings` clean · `cargo fmt` applied · new logic has tests ·
the relevant `cargo run` example above produces sane output.

> Claude Code note: this repo also works if you symlink `CLAUDE.md -> AGENTS.md`.
> Keep both stages runnable in isolation — never make `solve` implicitly re-run
> `gen-layout`.
