"""Per-event venue constants. **This is the file you edit for C108 and beyond.**

The structure is transcribed from ``island_implemantation.txt`` (the C107 配置図 spec):
each island is a serpentine run with explicit cross-aisles (中央通路 / 細通路 / 横通路 /
ブロック境界) authored as :class:`~tools.layout.model.Corridor` number-breaks, classified
``"major"`` (太通路: wider, cheaper to walk) or ``"minor"`` (細通路). Walls (壁サー) are
single-faced runs; their real L-shaped multi-wall routing is approximated by one linear
run (see the README note).

To make a new event, copy the ``C107`` block, rename it, tweak the prefix lists /
patterns / origins / distances, and register it in :data:`EVENTS`.

Coordinate convention: metres, x = East, y = North. Halls are spread out so their
bounding boxes do not overlap; the authoritative inter-hall walking distances come from
:data:`HALL_DISTANCES`, not from these coordinates. Per the spec the Excel figure is
rotated relative to reality (東456/東78 by 180°, 南12 by 90° CCW); these coordinates are
authored in the real orientation so cross-island Manhattan distances stay sensible.
"""

from __future__ import annotations

from .model import Cluster, Corridor, EventConfig, Hall, Island, Row, WallSegment

# --- pattern tables ----------------------------------------------------------
# A pattern is ``(n_max, corridors)`` where each corridor is ``(face0_after,
# face1_after, kind)``. ``face0_after`` / ``face1_after`` are the serpentine numbers the
# cross-aisle follows on each face (the gap sits between that number and the next).

C = Corridor

#: 東4/5/6 (and 東1/2/3): five sizes, each a 中央通路 (major) plus upper/lower 細通路.
PAT_E456: dict[str, tuple[int, tuple[Corridor, ...]]] = {
    "P1": (66, (C(16, 49, "major"), C(25, 41), C(9, 57))),
    "P2": (64, (C(17, 47, "major"), C(25, 39), C(9, 55))),
    "P3": (62, (C(16, 46, "major"), C(24, 38), C(8, 54))),
    "P4": (54, (C(14, 40, "major"), C(20, 34), C(8, 46))),
    "P5": (48, (C(12, 36, "major"), C(19, 29), C(5, 43))),
}

#: Which 東456 pattern each katakana prefix uses (from the spec's 該当する島 lists).
PREFIX_E456: dict[str, str] = {
    **{p: "P1" for p in "ウ エ キ ク ケ コ サ タ チ ツ テ ト ヌ ネ ヘ ホ マ ミ ム".split()},
    **{p: "P2" for p in "ヤ ユ".split()},
    **{p: "P3" for p in "オ カ シ ソ ナ ニ ノ フ メ モ".split()},
    **{p: "P4" for p in "イ ヨ".split()},
    **{p: "P5" for p in "ス セ ハ ヒ".split()},
}

#: 南1/2: three sizes, each two ブロック境界 cross-aisles (minor).
PAT_SOUTH: dict[str, tuple[int, tuple[Corridor, ...]]] = {
    "P1": (46, (C(9, 37), C(16, 30))),
    "P2": (44, (C(9, 35), C(16, 28))),
    "P3": (42, (C(9, 33), C(16, 26))),
}
PREFIX_SOUTH: dict[str, str] = {
    **{p: "P1" for p in "b c d j k l m n h i".split()},
    **{p: "P2" for p in "o p q r s t".split()},
    **{p: "P3" for p in "e f g".split()},
}

#: 西 上半分: three sizes (P3 adds a 下横通路 minor below its 中央通路).
PAT_WEST_TOP: dict[str, tuple[int, tuple[Corridor, ...]]] = {
    "P1": (28, (C(6, 22, "major"),)),
    "P2": (26, (C(5, 21, "major"),)),
    "P3": (52, (C(18, 34, "major"), C(11, 41))),
}
#: 西 下半分: a four-block コの字 wing — one serpentine with three minor block boundaries.
PAT_WEST_BOTTOM: tuple[int, tuple[Corridor, ...]] = (52, (C(6, 46), C(13, 39), C(19, 33)))


def _pat_island(prefix: str, spec: tuple[int, tuple[Corridor, ...]]) -> Island:
    n_max, corridors = spec
    return Island(prefix=prefix, n_max=n_max, corridors=corridors)


def _by_pattern(prefixes: str, table: dict, lookup: dict[str, str]) -> tuple[Island, ...]:
    """Build islands for ``prefixes`` (space-separated) via a prefix→pattern map."""
    return tuple(_pat_island(p, table[lookup[p]]) for p in prefixes.split())


def _same_pattern(prefixes: str, spec: tuple[int, tuple[Corridor, ...]]) -> tuple[Island, ...]:
    """Build islands that all share one pattern."""
    return tuple(_pat_island(p, spec) for p in prefixes.split())


#: Terse alias for authoring wall segments below.
WS = WallSegment


def wall_rows(prefix: str, segments: tuple[WallSegment, ...]) -> tuple[Row, ...]:
    """Turn a 壁サー's straight runs into one single-island wall :class:`Row` each.

    Numbering is continuous across segments via ``number_base``; each run spreads its
    circles 等間隔 over its physical ``span`` (so a sparse wall leaves gaps). The folds
    between runs (right→top→left, etc.) come from each segment's own ``along`` direction.
    """
    rows: list[Row] = []
    for seg in segments:
        count = seg.end - seg.start + 1
        pitch = (seg.span / count) if seg.span is not None else None
        island = Island(prefix=prefix, n_max=count, kind="wall",
                        number_base=seg.start - 1, pitch=pitch)
        rows.append(Row((island,), origin=seg.anchor, island_spacing=0.0,
                        along=seg.along, cross=seg.cross, angle_deg=seg.angle_deg,
                        label=f"壁{prefix} {seg.start}–{seg.end}"))
    return tuple(rows)


# --- 壁サー (folded multi-segment perimeters; numbers run continuously) ------------------
# Coordinates trace the correct shape (right/top/left walls, diagonals, コの字 perimeters)
# around each hall's island band; absolute lengths are approximate but the directions,
# number ranges, continuity, and 等間隔 spreading follow island_implemantation.txt.

WALL_A_E456 = (  # ア: 東6 right wall up, top wall across 東6→5→4, 東4 left wall down
    WS(1, 22, anchor=(300.0, -33.0), along="N", cross="W", span=40.0),
    WS(23, 73, anchor=(295.0, 8.0), along="W", cross="S", span=290.0),
    WS(74, 95, anchor=(2.0, 5.0), along="S", cross="E", span=42.0),
)
WALL_A_E7 = (  # A: lower-tier diagonal up-right, central-aisle top wall, lower-tier left
    WS(1, 18, anchor=(50.0, -65.0), along="N", cross="W", angle_deg=60.0, span=40.0),
    WS(19, 34, anchor=(50.0, -30.0), along="W", cross="N", span=45.0),
    WS(35, 48, anchor=(5.0, -30.0), along="S", cross="E", span=35.0),
)
WALL_V_E8 = (  # V: left wall down then right wall up, beside the 東8 diagonal band
    WS(1, 12, anchor=(75.0, -5.0), along="S", cross="E", span=14.0),
    WS(13, 19, anchor=(72.0, -20.0), along="N", cross="W", span=8.0),
)
WALL_a_S = (  # a: bottom(right) → right wall up → top wall → left wall down → bottom(centre)
    WS(1, 4, anchor=(-40.0, -226.0), along="E", cross="N", span=8.0),
    WS(5, 20, anchor=(-29.0, -224.0), along="N", cross="W", span=26.0),
    WS(21, 44, anchor=(-29.0, -196.0), along="W", cross="S", span=33.0),
    WS(45, 50, anchor=(-64.0, -196.0), along="S", cross="E", span=12.0),
    WS(51, 54, anchor=(-52.0, -226.0), along="E", cross="N", span=8.0),
)
WALL_me_W1 = (  # め: bottom → left wall up → top → な,と spur → 下半分 right wall down
    WS(1, 15, anchor=(-425.0, -112.0), along="W", cross="N", span=40.0),
    WS(16, 39, anchor=(-467.0, -112.0), along="N", cross="E", span=155.0),
    WS(40, 57, anchor=(-467.0, 47.0), along="E", cross="S", span=45.0),
    WS(58, 61, anchor=(-435.0, 47.0), along="W", cross="S", span=10.0),
    WS(66, 73, anchor=(-428.0, -65.0), along="S", cross="W", span=20.0),
)
WALL_a_W2 = (  # あ: mirror of め (W2 is left-right symmetric with W1)
    WS(1, 15, anchor=(-325.0, -112.0), along="E", cross="N", span=40.0),
    WS(16, 39, anchor=(-193.0, -112.0), along="N", cross="W", span=155.0),
    WS(40, 57, anchor=(-193.0, 47.0), along="W", cross="S", span=45.0),
    WS(58, 61, anchor=(-205.0, 47.0), along="E", cross="S", span=10.0),
    WS(66, 73, anchor=(-318.0, -65.0), along="S", cross="E", span=20.0),
)


# --- the six distance-matrix clusters ---------------------------------------

CLUSTERS: tuple[Cluster, ...] = (
    Cluster("E1-3", "東1・2・3", "East", (1, 2, 3)),
    Cluster("E4-6", "東4・5・6", "East", (4, 5, 6)),
    Cluster("E7-8", "東7・8", "East", (7, 8)),
    Cluster("W1-2", "西1・2", "West", (1, 2)),
    Cluster("S1", "南1", "South", (1,)),
    Cluster("S2", "南2", "South", (2,)),
)

HALL_DISTANCES: dict[tuple[str, str], float] = {
    ("E1-3", "E4-6"): 120.0,
    ("E1-3", "E7-8"): 250.0,
    ("E4-6", "E7-8"): 150.0,
    ("E1-3", "W1-2"): 520.0,
    ("E4-6", "W1-2"): 450.0,
    ("E7-8", "W1-2"): 560.0,
    ("E1-3", "S1"): 640.0,
    ("E1-3", "S2"): 700.0,
    ("E4-6", "S1"): 600.0,
    ("E4-6", "S2"): 660.0,
    ("E7-8", "S1"): 700.0,
    ("E7-8", "S2"): 760.0,
    ("W1-2", "S1"): 300.0,
    ("W1-2", "S2"): 350.0,
    ("S1", "S2"): 90.0,
}

# --- East 4 / 5 / 6 : katakana islands with per-pattern corridors -----------------------
# Each hall is an island band; the 壁サー ア wraps the outer wall (modelled as one linear
# wall run across the top). 縦大通路 widen the gap after オ/カ, ス/セ, ナ/ニ, メ/モ.

_E4_ISLANDS = "ヨ ユ ヤ モ メ ム ミ マ ホ ヘ フ ヒ"
_E5_ISLANDS = "ハ ノ ネ ヌ ニ ナ ト テ ツ チ タ ソ セ ス"
_E6_ISLANDS = "シ サ コ ケ ク キ カ オ エ ウ イ"

E4 = Hall(
    "East", 4, "E4-6", label="東4",
    rows=(
        Row(_by_pattern(_E4_ISLANDS, PAT_E456, PREFIX_E456),
            origin=(10.0, 0.0), island_spacing=3.0, label="東4 島",
            wide_after=("モ",), wide_extra=2.0),
        *wall_rows("ア", WALL_A_E456),
    ),
)
E5 = Hall(
    "East", 5, "E4-6", label="東5",
    rows=(Row(_by_pattern(_E5_ISLANDS, PAT_E456, PREFIX_E456),
              origin=(130.0, 0.0), island_spacing=3.0, label="東5 島",
              wide_after=("ニ", "セ"), wide_extra=2.0),),
)
E6 = Hall(
    "East", 6, "E4-6", label="東6",
    rows=(Row(_by_pattern(_E6_ISLANDS, PAT_E456, PREFIX_E456),
              origin=(260.0, 0.0), island_spacing=3.0, label="東6 島",
              wide_after=("カ",), wide_extra=2.0),),
)

# --- East 7 : wall "A" on top, two island bands (central corridor only), the lone N -----
_PAT_E7 = (48, (C(12, 36, "major"),))  # 中央通路 only, per the spec's 東7 patterns
_PAT_E7_N = (60, (C(15, 46, "major"),))  # N: three stacked blocks, approximated as one run

E7 = Hall(
    "East", 7, "E7-8", label="東7",
    rows=(
        Row(_same_pattern("M L K J I H G F E D C B", _PAT_E7),
            origin=(10.0, 0.0), island_spacing=3.0, label="東7 上段"),
        Row(_same_pattern("U T S R Q P O", _PAT_E7) + (_pat_island("N", _PAT_E7_N),),
            origin=(10.0, -60.0), island_spacing=3.0, label="東7 下段"),
        *wall_rows("A", WALL_A_E7),
    ),
)

# --- East 8 : the diagonal band (W X Y Z V) ------------------------------------------
# Each island is really two side-by-side serpentine blocks; approximated here as one
# serpentine of the combined length with an auto mid corridor (see corridor_split_len).
E8 = Hall(
    "East", 8, "E7-8", label="東8",
    rows=(
        Row((Island("W", n_max=68), Island("X", n_max=66), Island("Y", n_max=50),
             Island("Z", n_max=56)),
            origin=(80.0, -10.0), island_spacing=4.0, angle_deg=215.0, label="東8 斜め"),
        *wall_rows("V", WALL_V_E8),
    ),
)

# --- South 1 / 2 : lowercase latin (a..t), three patterns, plus the wall a -------------
S1 = Hall(
    "South", 1, "S1", label="南1",
    rows=(
        Row(_by_pattern("t s r q p o n m l k", PAT_SOUTH, PREFIX_SOUTH),
            origin=(-200.0, -200.0), island_spacing=3.0, label="南1 島"),
    ),
)
S2 = Hall(
    "South", 2, "S2", label="南2",
    rows=(
        Row(_by_pattern("j i h g f e d c b", PAT_SOUTH, PREFIX_SOUTH),
            origin=(-60.0, -200.0), island_spacing=3.0, label="南2 島"),
        *wall_rows("a", WALL_a_S),
    ),
)

# --- West 1 / 2 : hiragana コの字 (top band + bottom four-block wing + side wall) -------
W1 = Hall(
    "West", 1, "W1-2", label="西1",
    rows=(
        Row(_by_pattern("ふ ひ は の ね ぬ に な と て つ", PAT_WEST_TOP, {
                **{p: "P3" for p in "ね の は ひ ふ".split()},
                **{p: "P2" for p in "に ぬ".split()},
                **{p: "P1" for p in "つ て と な".split()},
            }),
            origin=(-460.0, 40.0), island_spacing=3.0,
            along="S", cross="E", prefix_axis="E", label="西1 上辺"),
        Row(_same_pattern("む み ま ほ へ", PAT_WEST_BOTTOM),
            origin=(-460.0, -80.0), island_spacing=3.0,
            along="N", cross="E", prefix_axis="E", label="西1 下辺"),
        *wall_rows("め", WALL_me_W1),
    ),
)
W2 = Hall(
    "West", 2, "W1-2", label="西2",
    rows=(
        Row(_by_pattern("ち た そ せ す し さ こ け く き", PAT_WEST_TOP, {
                **{p: "P3" for p in "き く け こ さ".split()},
                **{p: "P2" for p in "し す".split()},
                **{p: "P1" for p in "せ そ た ち".split()},
            }),
            origin=(-320.0, 40.0), island_spacing=3.0,
            along="S", cross="E", prefix_axis="E", label="西2 上辺"),
        Row(_same_pattern("か お え う い", PAT_WEST_BOTTOM),
            origin=(-320.0, -80.0), island_spacing=3.0,
            along="N", cross="E", prefix_axis="E", label="西2 下辺"),
        *wall_rows("あ", WALL_a_W2),
    ),
)


C107 = EventConfig(
    event="C107",
    halls=(E4, E5, E6, E7, E8, S1, S2, W1, W2),
    clusters=CLUSTERS,
    hall_distances=HALL_DISTANCES,
    pitch_m=1.0,
    island_width_m=1.5,
    cell_size_m=1.0,
    # Islands carry explicit corridors now; auto-split only catches any bare island
    # (currently just the East 8 approximation) whose longest face is >= this many metres.
    corridor_split_len=20.0,
)


#: Registry consulted by the CLI. Add ``"C108": C108`` here for the next event.
EVENTS: dict[str, EventConfig] = {
    "C107": C107,
}
