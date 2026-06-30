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

1. **`gen-layout`** — 島テーブル（`block_layouts.csv`、1行＝1島）を、全スペースの
   グローバル座標＋島内ローカル座標に展開し、`spaces.json` に書き出す。**まれに**実行し、以後は
   使い回す。`block_layouts.csv` と `hall_distances.csv` は下記 Python ツールで生成できます。
2. **`solve`** — want-list とレイアウト成果物を読み込み、欲しいスペースだけの「体感距離」行列を
   作り、最近傍法＋2-opt/Or-opt 局所探索＋反復局所探索（ILS、並列リスタート）でルートを求める。
   **頻繁に**実行でき、軽量。`--hall-distances` を渡すとホールクラスタ間の距離が CSV で上書きされます。

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
  --blocks data/block_layouts.csv \
  --out artifacts/spaces.json --event C107

# ステージ2：ルートを計画（頻繁・軽量）
cargo run --release -- solve \
  --spaces artifacts/spaces.json \
  --want data/want_list.csv \
  --hall-distances data/hall_distances.csv \
  --out route.csv \
  --seed 42 --restarts 16
```

主な `solve` オプション：`--hall-distances <csv>`（ホールクラスタ間の体感距離行列。指定すると
クラスタ跨ぎは行列値が**正**となり、棟／ホールの固定ペナルティを置き換える。未指定なら従来の
ペナルティモデル）、`--closed`（周回して始点に戻る）、`--start <SpaceId>`（最初に回るスペースを
固定。want-list に含まれている必要あり）、`--gamma`／`--pen-building`／`--pen-hall`／`--pen-block`
（距離モデルの調整）、`--time-ms`／`--max-iters`（探索の予算）。**同じシードでバイト単位まで同一の
結果**が欲しい場合は `--time-ms 0` とし、反復回数のみを停止条件にします。

[`data/`](data/) に小さなサンプルがあり、両ステージをそのまま試せます。

### 会場レイアウトの生成（Python ツール）

[`tools/layout/`](tools/layout/) の生成ツールが、PDF 配置図から抽出した**会場定数**（島の
プレフィックス順・島長・原点・クラスタ・ホール間距離行列）を入力に、次を一括生成します。

- `data/block_layouts.csv` — Rust の `gen-layout` が読む拡張スキーマ
- `data/hall_distances.csv` — クラスタ×クラスタの距離行列（`solve --hall-distances` 用）
- `out/layout_<event>.xlsx` — 通路・サークルをセルで表した**島配置図**（ホール別の詳細図、
  島単位の一覧図、距離行列、ブロック表）

```bash
pip install -r tools/layout/requirements.txt
python -m tools.layout.generate --event C107
```

会場定数は [`tools/layout/config.py`](tools/layout/config.py) に dataclass で記述されており、
C108 以降は同ファイルをコピー編集して `EVENTS` に登録するだけで対応できます。拡張スキーマは
**壁サークル**（`kind=wall`、単面）・**同一接頭辞を区切る通路**（`crossings`）・**東7/8 の斜め**
（`along_deg`/`cross_deg`）・**西館コの字**（向きの異なる複数 Row）・**クラスタ**（`cluster`）を
表現できます。

### 距離モデル（要点）

- **クラスタ跨ぎ**（`--hall-distances` 指定時）：`hall_distances.csv` のクラスタ間距離が**正**。
  棟／ホールの固定ペナルティを置き換えます（西館コの字・東7/8 斜めなど、座標 Manhattan では
  測りづらい館間移動をデータで与えられる）。
- **クラスタ内・島跨ぎ**：グローバル Manhattan 距離 ＋ 段階ペナルティ（ホール ＞ 隣接島）を `gamma`
  で非線形に圧縮。これにより「近い島から片付ける」戦略が自然に出ます。
- **同一島内**：机を突き抜けられないため、反対面へは**最寄りの交差点（島端＋通路）を回る**距離で
  正確に評価（Manhattan の過小評価も番号差の過大評価も回避）。`crossings` により、同一接頭辞を
  区切る通路で面を渡る近道も考慮されます。

### 出力

`route.csv` は訪問順に各スペース・レッグ毎／累積の体感コストを列挙。CLI は棟・ホール別に
グループ化した人間向け itinerary と総コストも表示します。

---

## English

See [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) for the full design and
[`AGENTS.md`](AGENTS.md) for conventions.

### Two decoupled stages

The stages are split so the want-list can change without rebuilding the venue:

1. **`gen-layout`** — expand a block table (`block_layouts.csv`, one row per island)
   into global plus local coordinates for every space, written to `spaces.json`. Run
   rarely; reused across want-lists. `block_layouts.csv` and `hall_distances.csv` can be
   produced by the Python tool below.
2. **`solve`** — read a want-list and the layout artifact, build a perceived-distance
   matrix over only the wanted spaces, and run nearest-neighbour + 2-opt/Or-opt local
   search + iterated local search (parallel restarts) to produce a route. Run often,
   cheap. Pass `--hall-distances` to override inter-cluster distances from a CSV.

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
  --blocks data/block_layouts.csv \
  --out artifacts/spaces.json --event C107

# Stage 2: plan a route (run often, cheap)
cargo run --release -- solve \
  --spaces artifacts/spaces.json \
  --want data/want_list.csv \
  --hall-distances data/hall_distances.csv \
  --out route.csv \
  --seed 42 --restarts 16
```

Useful `solve` flags: `--hall-distances <csv>` (inter-cluster distance matrix; when
given, cross-cluster legs use the matrix value, **replacing** the building/hall
penalties; omit it for the legacy penalty model), `--closed` (round trip),
`--start <SpaceId>` (fix the first stop; must be in the want-list),
`--gamma`/`--pen-building`/`--pen-hall`/`--pen-block` (tune the distance model),
`--time-ms`/`--max-iters` (search budget). For byte-identical results from a seed, use
`--time-ms 0` so the iteration cap is the only stopping rule.

A small sample lives in [`data/`](data/) so both stages run out of the box.

### Generating venue layouts (Python tool)

The generator in [`tools/layout/`](tools/layout/) turns **venue constants** extracted
from the map PDF (island prefix order, island lengths, origins, clusters, the inter-hall
distance matrix) into:

- `data/block_layouts.csv` — the extended schema read by `gen-layout`,
- `data/hall_distances.csv` — the cluster×cluster matrix (for `solve --hall-distances`),
- `out/layout_<event>.xlsx` — an **island map** drawn with corridors and circles as
  cells (per-hall detail sheets, an island overview, the distance matrix, a block table).

```bash
pip install -r tools/layout/requirements.txt
python -m tools.layout.generate --event C107
```

The constants live as dataclasses in [`tools/layout/config.py`](tools/layout/config.py);
for C108+ copy that file, edit the data, and register it in `EVENTS`. The schema models
**wall circles** (`kind=wall`, single-faced), **corridors splitting a prefix**
(`crossings`), the **diagonal halls** East 7/8 (`along_deg`/`cross_deg`), West's
**コの字** (rows pointing different cardinal ways), and **clusters** (`cluster`).

### Distance model (in brief)

- **Cross-cluster** legs (with `--hall-distances`) use the matrix distance between hall
  clusters as authoritative, replacing the building/hall penalties — letting you supply
  the real inter-hall walks (West's コの字, the East 7/8 diagonal) that raw coordinate
  Manhattan can't capture.
- **Same-cluster, cross-island** legs use global Manhattan distance plus a tiered penalty
  (hall > adjacent island), squashed nonlinearly by `gamma`, so nearer islands get
  cleared first.
- **Same-island** legs round the nearest crossing point (island end **or** a mid-island
  corridor) for opposite faces — you can't cut through the tables — avoiding both
  Manhattan's underestimate and a number-difference overestimate.

### Output

`route.csv` lists each stop in visit order with per-leg and cumulative perceived cost;
the CLI also prints a human itinerary grouped by building and hall, plus the total cost.
