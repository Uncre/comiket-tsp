# comiket-tsp

> 🇯🇵 [日本語](#日本語) ・ 🇬🇧 [English](#english)

A Rust CLI that plans an efficient walking route around a set of Comiket circles,
modeled as a Travelling Salesman Problem.
Comiket のサークル巡回を巡回セールスマン問題（TSP）として解き、効率的な歩行ルートを
計画する Rust 製 CLI です。

---

## 日本語

[`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) に設計全体、[`AGENTS.md`](AGENTS.md) に
規約があります。

### 2つの独立したステージ

want-list（買い物リスト）が変わっても会場データを作り直さずに済むよう、処理を2段に分離して
います。

1. **`gen-layout`** — 手作業で書いた島テーブル（`block_layout.csv`、1行＝1島）を、全スペースの
   グローバル座標＋島内ローカル座標に展開し、`spaces.json` に書き出す。**まれに**実行し、以後は
   使い回す。
2. **`solve`** — want-list とレイアウト成果物を読み込み、欲しいスペースだけの「体感距離」行列を
   作り、最近傍法＋2-opt/Or-opt 局所探索＋反復局所探索（ILS、並列リスタート）でルートを求める。
   **頻繁に**実行でき、軽量。

### ビルドと検査

```bash
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
cargo bench            # 任意：ソルバーのベンチマーク（criterion）
```

### 使い方

```bash
# ステージ1：会場座標を生成（まれに実行）
cargo run --release -- gen-layout \
  --blocks data/block_layout.csv \
  --out artifacts/spaces.json --event C107

# ステージ2：ルートを計画（頻繁・軽量）
cargo run --release -- solve \
  --spaces artifacts/spaces.json \
  --want data/want_list.csv \
  --out route.csv \
  --seed 42 --restarts 16
```

主な `solve` オプション：`--closed`（周回して始点に戻る）、`--start <SpaceId>`（最初に回る
スペースを固定。want-list に含まれている必要あり）、`--gamma`／`--pen-building`／`--pen-hall`／
`--pen-block`（距離モデルの調整）、`--time-ms`／`--max-iters`（探索の予算）。**同じシードで
バイト単位まで同一の結果**が欲しい場合は `--time-ms 0` とし、反復回数のみを停止条件にします。

[`data/`](data/) に小さな合成サンプルがあり、両ステージをそのまま試せます。

### 距離モデル（要点）

- **島跨ぎ**：グローバル Manhattan 距離 ＋ 段階ペナルティ（棟 ≫ ホール ≫ 隣接島）を、`gamma` で
  非線形に圧縮。これにより「1ホール／1棟を片付けてから移動する」現実的な戦略が自然に出ます。
- **同一島内**：机を突き抜けられないため、反対面へは**島の近い方の端を回る**距離で正確に評価
  （Manhattan の過小評価も番号差の過大評価も回避）。

### 出力

`route.csv` は訪問順に各スペース・レッグ毎／累積の体感コストを列挙。CLI は棟・ホール別に
グループ化した人間向け itinerary と総コストも表示します。

---

## English

See [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) for the full design and
[`AGENTS.md`](AGENTS.md) for conventions.

### Two decoupled stages

The stages are split so the want-list can change without rebuilding the venue:

1. **`gen-layout`** — expand a hand-authored block table (`block_layout.csv`, one row
   per island) into global plus local coordinates for every space, written to
   `spaces.json`. Run rarely; reused across want-lists.
2. **`solve`** — read a want-list and the layout artifact, build a perceived-distance
   matrix over only the wanted spaces, and run nearest-neighbour + 2-opt/Or-opt local
   search + iterated local search (parallel restarts) to produce a route. Run often,
   cheap.

### Build & checks

```bash
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
cargo bench            # optional: solver benchmark (criterion)
```

### Usage

```bash
# Stage 1: build venue coordinates (run rarely)
cargo run --release -- gen-layout \
  --blocks data/block_layout.csv \
  --out artifacts/spaces.json --event C107

# Stage 2: plan a route (run often, cheap)
cargo run --release -- solve \
  --spaces artifacts/spaces.json \
  --want data/want_list.csv \
  --out route.csv \
  --seed 42 --restarts 16
```

Useful `solve` flags: `--closed` (round trip), `--start <SpaceId>` (fix the first stop;
must be in the want-list), `--gamma`/`--pen-building`/`--pen-hall`/`--pen-block` (tune
the distance model), `--time-ms`/`--max-iters` (search budget). For byte-identical
results from a seed, use `--time-ms 0` so the iteration cap is the only stopping rule.

A small synthetic sample lives in [`data/`](data/) so both stages run out of the box.

### Distance model (in brief)

- **Cross-island** legs use global Manhattan distance plus a tiered penalty
  (building ≫ hall ≫ adjacent island), squashed nonlinearly by `gamma`. This makes the
  realistic "clear one hall/building before moving on" strategy fall out naturally.
- **Same-island** legs round the nearer island end for opposite faces (you can't cut
  through the tables), avoiding both Manhattan's underestimate and a number-difference
  overestimate.

### Output

`route.csv` lists each stop in visit order with per-leg and cumulative perceived cost;
the CLI also prints a human itinerary grouped by building and hall, plus the total cost.
