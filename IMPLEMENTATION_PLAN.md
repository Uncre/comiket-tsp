# comiket-tsp — Implementation Plan

A Rust CLI that plans an efficient walking route around Comiket circles as a TSP.
Two decoupled stages: **layout generation** and **route search**.

---

## 1. Goal & scope

- Input A (manual, reused): a *block table* describing each island's position and
  numbering — `block_layout.csv`.
- Input B (changes often): the circles to visit — `want_list.csv`, authored in Excel.
- Output: an ordered itinerary minimizing total *perceived walking cost*.
- Size: up to ~200 wanted circles. Whole-venue layout is ~100–150 islands.
- Quality target: near-optimal (within a few % of optimal) in well under a second
  for the solve stage. Exactness is not required.

Non-goals (for now): time windows / sell-out priorities, one-way aisle asymmetry,
real-time crowd data. The design leaves room for these (see § 9).

---

## 2. Domain model

### 2.1 Space

A space is one circle's half-table. Notation decomposes into:

```
building : East | West | South     // 東 / 西 / 南
hall     : u8                       // 4, 5, 6, 7, 8, 1, 2 ...
block    : String                   // "ア", "あ", "A", "t" ... (1 letter, multi-byte ok)
number   : u16                      // serpentine index within the island
side     : A | B                    // left/right half of one table
```

`SpaceId` = `(building, hall, block, number, side)`. It must round-trip through a
canonical string (e.g. `E4-ア-31a`) for CSV matching against the want-list.

### 2.2 Block (one island)

Authored once per event. Few fields per row:

```rust
struct Block {
    id:           String,        // "E4-ア"
    building:     Building,
    hall:         u8,
    anchor:       (f64, f64),     // global meters: the n=1, face0, side-a desk
    along:        Dir,            // unit dir of increasing depth (N/S/E/W)
    cross:        Dir,            // unit dir from face0 to face1
    n_max:        u16,            // highest number in this island
    face0_len:    Option<u16>,    // override split for irregular islands (default n_max/2)
    pitch:        f64,            // desk spacing along the island, ≈ 1.0 m
    island_width: f64,           // distance between the two faces, ≈ 1.5 m
}
```

### 2.3 Serpentine coordinate derivation

Numbers run up one face and back down the other (see chat diagram). For number `n`:

```
half  = face0_len.unwrap_or(n_max / 2)
if n <= half {
    face  = 0
    depth = n
} else {
    face  = 1
    depth = n_max - n + 1
}
along_m = (depth - 1) as f64 * pitch
cross_m = face as f64 * island_width + side_offset(side)   // side_offset(a)=0, (b)=~0.45
global  = anchor + along_m * along.unit() + cross_m * cross.unit()
```

Edge cases to handle:
- Odd `n_max` / unequal faces → use `face0_len` override.
- Island-head ("お誕生日席") seats and `2日目はありません` blanks: ignore for 概算;
  they don't shift coordinates materially. Document the assumption.
- `a`/`b` are the same face, ~0.45 m apart laterally.

---

## 3. Distance model

Hybrid of coordinate distance (case 2) + tiered structural penalties (case 1),
then a nonlinear squash so long legs and building/hall crossings dominate.

```rust
fn perceived(a: &Space, b: &Space) -> f64 {
    let mut d = manhattan(a.coord, b.coord);          // base walking meters
    d += if a.building != b.building { 250.0 }         // cross-building: galleria/security
         else if a.hall  != b.hall  { 40.0 }            // cross-hall
         else if a.block != b.block { 5.0 }             // adjacent island (one aisle)
         else { 0.0 };
    d.powf(GAMMA)                                       // GAMMA = 1.25
}
```

Effect: a building jump (~+250) becomes ~990 after the squash, so the solver
naturally clears one hall/building before moving on — the real Comiket strategy.

Notes / constants are tunable; expose `GAMMA` and the three penalties as `solve`
flags (`--gamma`, `--pen-building`, …) with the defaults above.

Caveats to encode as comments/tests:
- `powf` breaks the triangle inequality → metric-based bounds (Christofides) don't
  apply. Fine: 2-opt / Or-opt / SA / ILS work on arbitrary cost matrices.
- Manhattan underestimates the walk-around between the two faces of the *same*
  island. Acceptable for 概算; optionally add a small same-island-opposite-face
  penalty later.
- The matrix is symmetric for now. Keep the type asymmetric-ready (`d[i][j]`) so a
  future one-way-aisle model is a data change, not a code change.

---

## 4. CLI design (clap, derive)

```
comiket-tsp gen-layout --blocks <csv> --out <json>
comiket-tsp solve      --spaces <json> --want <csv> --out <csv>
                       [--seed 42] [--restarts 16] [--time-ms 800]
                       [--start <SpaceId>] [--closed]
                       [--gamma 1.25] [--pen-building 250] [--pen-hall 40] [--pen-block 5]
```

- `--start` fixes the first node (your arrival gate). Default: solver picks best.
- `--closed` returns to the start (round trip). Default: open path.

---

## 5. File tree

```
comiket-tsp/
├── Cargo.toml
├── AGENTS.md
├── IMPLEMENTATION_PLAN.md
├── data/
│   ├── block_layout.csv      # hand-authored island table
│   └── want_list.csv         # exported from Excel
├── artifacts/
│   └── spaces.json           # produced by gen-layout
├── src/
│   ├── main.rs               # clap dispatch, anyhow boundary
│   ├── lib.rs                # re-exports, error type
│   ├── space.rs              # SpaceId, parsing, canonical string
│   ├── layout.rs             # Block, serpentine expansion → Vec<Space>
│   ├── distance.rs           # perceived(), DistanceMatrix
│   ├── io.rs                 # csv/json read+write
│   └── solve/
│       ├── mod.rs            # orchestration: construct → improve → restart
│       ├── construct.rs      # nearest-neighbor (+ multi-start)
│       ├── two_opt.rs        # 2-opt + Or-opt local search
│       └── ils.rs            # double-bridge kicks; optional simulated annealing
└── tests/
    ├── space.rs
    ├── layout.rs
    └── solve.rs
```

### Cargo.toml (starting point)

```toml
[package]
name = "comiket-tsp"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4", features = ["derive"] }
csv = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rand = "0.9"
rayon = "1"
anyhow = "1"
thiserror = "2"

[dev-dependencies]
criterion = "0.5"

[profile.release]
lto = "thin"
codegen-units = 1
```

---

## 6. Algorithm (solve stage)

1. **Build matrix.** Look up each wanted space's coords from the layout; warn on
   any want-list entry not found, then `perceived()` over all pairs → `N×N` `f64`
   (`N ≤ 200`, ~320 KB — precompute once, reuse everywhere).
2. **Construct.** Nearest-neighbor from the start. If `--start` is unset, try every
   node as a start (or a sampled subset) and keep the best NN tour.
3. **Local search.** 2-opt (first-improvement, segment-reverse) interleaved with
   Or-opt (relocate segments of length 1–3). Loop until no improving move — a
   2-opt/Or-opt local optimum. Respect fixed endpoints in open-path mode (don't
   move index 0; don't wrap).
4. **Iterated local search (primary metaheuristic).** Perturb the local optimum
   with a **double-bridge** (4-opt) kick, re-run step 3, accept if better (or with
   an SA-style probability). Repeat until `--time-ms` or an iteration cap. ILS +
   double-bridge is simple and strong for this size.
   - Optional alternative: classic simulated annealing over the same neighborhood,
     geometric cooling. Keep behind the same trait so either can be selected.
5. **Parallel restarts.** Run `--restarts` independent ILS chains via `rayon`,
   each with a deterministic child seed; keep the global best.
6. **Output.** Emit `route.csv`: visit order, space id, circle name, leg cost,
   cumulative cost; print total perceived cost and a short human itinerary
   (grouped by building/hall).

Tour representation: `Vec<usize>` of node indices. Cache tour cost and update it
incrementally on each accepted move (don't recompute from scratch).

Open path vs cycle: implement both. Open path = fixed first node, free end, no
wrap-around edge. Cycle = `--closed`, includes the return edge.

---

## 7. Data formats

### block_layout.csv
```
id,building,hall,anchor_x,anchor_y,along,cross,n_max,face0_len,pitch,island_width
E4-ア,East,4,12.0,3.0,S,E,48,,1.0,1.5
E4-ヨ,East,4,15.0,3.0,S,E,48,,1.0,1.5
```
`along`/`cross` ∈ {N,S,E,W}. Empty `face0_len` ⇒ `n_max/2`.

### want_list.csv (from Excel)
```
space,name
E4-ア-31a,さくらサークル
W1-あ-12b,Example Circle
```
`space` is the canonical id; `name` is free text for the itinerary.

### spaces.json (artifact, gen-layout → solve)
```json
{
  "event": "C107",
  "spaces": [
    { "id": "E4-ア-31a", "building": "East", "hall": 4,
      "block": "ア", "number": 31, "side": "A", "x": 12.0, "y": 33.0 }
  ]
}
```

---

## 8. Phased task list (work top to bottom)

- [ ] **P0 — scaffold.** `cargo new`, add deps, wire `clap` subcommands as stubs,
      set up `lib.rs` error type, CI-ish make targets in README. DoD: both
      subcommands parse args and print "not yet implemented".
- [ ] **P1 — space.** `SpaceId`, canonical string, `FromStr`/`Display`, parse tests
      (a/b, multi-byte blocks).
- [ ] **P2 — layout.** `Block` + CSV load + serpentine expansion to `Vec<Space>`;
      `gen-layout` writes `spaces.json`. Tests: known (n→depth,face), n=1 at anchor,
      even & odd `n_max`.
- [ ] **P3 — distance.** `perceived()` + `DistanceMatrix`; tests for symmetry and
      penalty ordering.
- [ ] **P4 — construct.** NN (+ multi-start). Test against a tiny known instance.
- [ ] **P5 — local search.** 2-opt + Or-opt with incremental cost; fixed-endpoint
      handling. Test: reaches optimum on ≤6-node instance.
- [ ] **P6 — ILS + restarts.** double-bridge kick, acceptance, `rayon` parallel
      restarts, deterministic seeds. Test: within tolerance on a TSPLIB grid.
- [ ] **P7 — solve I/O + itinerary.** read want-list, match to layout (warn on
      misses), write `route.csv`, print grouped itinerary.
- [ ] **P8 — polish.** flags (`--gamma`, penalties, `--start`, `--closed`,
      `--time-ms`), `--help` text, a sample `data/` set, optional criterion bench.

---

## 9. Future hooks (don't build yet, just don't preclude)

- Time windows / sell-out risk → weighted or prize-collecting TSP; the matrix and
  tour-cost function are the only touch points.
- One-way aisles / crowd asymmetry → keep `DistanceMatrix` asymmetric-capable.
- Multiple days → partition want-list by day; solve per day.

---

## 10. References (algorithms & Rust)

TSP in Rust:
- `ibn_battuta` (crates.io / github BIRSAx2/ibn-battuta) — full library: NN, 2-opt,
  3-opt, Lin-Kernighan, SA, GA, ACO, rayon, TSPLIB parser. Best reference for move
  implementations; optional dependency if hand-rolling stalls.
- `localsearch` (lib.rs) — generic local-search framework via an `OptModel` trait,
  rayon-parallel. Use if you'd rather plug into a framework than hand-roll.
- `aprender-tsp` — CLI + library with seeded determinism and TSPLIB/CSV input; good
  reference for CLI ergonomics and benchmarking against known optima.
- `stoksc/tsp-rs` — small, readable 2-opt(+3-opt) reference.
- `onkolahmet/TSP` — writeup of 2-opt with fast/guided local search.

Why hand-roll the core here: the cost matrix is non-metric (the `powf` squash) and
we need fixed-start open paths — both are awkward to express through a generic TSP
crate, and the move code is small. Crib details from the above; own the orchestration.

Benchmarks: TSPLIB instances (berlin52, eil51, att48) for validating the solver
against published optima before trusting it on venue data.

Concepts to implement (keywords for lookup): nearest-neighbor construction, 2-opt
segment reversal, Or-opt (segment relocation), double-bridge 4-opt perturbation,
iterated local search (ILS), simulated annealing with geometric cooling.
