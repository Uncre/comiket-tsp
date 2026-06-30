"""Geometry model for the Comiket venue-layout generator.

This module is the *engine*: it holds the dataclasses that describe a venue in
high-level constants and the pure functions that turn those constants into

* :class:`BlockRecord` rows for ``block_layouts.csv`` (what the Rust solver reads), and
* per-island local cell grids for the Excel ``島配置図``.

The placement "law" extracted from the C107 map (see ``config.py`` for the data and
``README``/plan for prose) is encoded here:

1. A hall is one or more :class:`Row` runs of islands laid along a *prefix axis*.
2. Each island is a serpentine double-row (up face 0, back down face 1); a *wall*
   run is single-faced with linear numbering (壁サークル).
3. Islands sit at a constant ``island_spacing`` along the prefix axis.
4. A long prefix may be split by a cross-corridor — modelled as a face *crossing*
   partway along the island (同じ接頭辞を区切る通路).
5. A row may be rotated by ``angle_deg`` for the diagonal halls (東7/8); West's
   コの字 is just three rows pointing different cardinal ways.

Keeping the serpentine maths here in lock-step with ``src/layout.rs`` means the Excel
picture and the solver agree on where every space is.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field

# --- direction helpers -------------------------------------------------------

#: Unit vectors for the cardinal directions, x = East, y = North (matches Rust `Dir`).
CARDINAL: dict[str, tuple[float, float]] = {
    "N": (0.0, 1.0),
    "S": (0.0, -1.0),
    "E": (1.0, 0.0),
    "W": (-1.0, 0.0),
}


def deg_unit(deg: float) -> tuple[float, float]:
    """Unit vector for an angle in degrees, CCW from East."""
    r = math.radians(deg)
    return (math.cos(r), math.sin(r))


def rot90_ccw(v: tuple[float, float]) -> tuple[float, float]:
    """Rotate a vector 90° counter-clockwise."""
    return (-v[1], v[0])


def nearest_cardinal(v: tuple[float, float]) -> str:
    """The cardinal letter whose unit vector best matches ``v`` (for the CSV fallback)."""
    return max(CARDINAL, key=lambda c: v[0] * CARDINAL[c][0] + v[1] * CARDINAL[c][1])


# --- high-level venue description (the editable constants live in config.py) --


@dataclass(frozen=True)
class Cluster:
    """A node of the inter-hall distance matrix, e.g. East 4-6 grouped as ``E4-6``."""

    id: str
    label: str
    building: str  # "East" | "West" | "South"
    halls: tuple[int, ...]


@dataclass(frozen=True)
class Corridor:
    """A cross-aisle that splits one island, authored as serpentine-number breaks.

    ``face0_after`` is the face-0 number the corridor follows (the gap sits between it
    and the next number); ``face1_after`` is the matching face-1 break. The two need not
    land at the same depth — a 中央通路 may be skewed — but a 細通路 typically aligns.

    ``kind`` is ``"major"`` for a 太通路 (中央通路 / 大通路: wider, cheaper to walk) or
    ``"minor"`` for a 細通路 / 横通路 / block boundary.
    """

    face0_after: int
    face1_after: int
    kind: str = "minor"  # "major" | "minor"


@dataclass(frozen=True)
class WallSegment:
    """One straight run of a 壁サー along a single wall (折れ線の一辺).

    The numbers ``start..end`` march from ``anchor`` along ``along`` (a cardinal letter,
    or any angle via ``angle_deg``), facing ``cross`` into the hall. ``span`` is the
    physical length in metres over which the run is spread *等間隔* — leaving gaps when
    there are fewer circles than metres; ``None`` means one circle per metre.
    """

    start: int
    end: int
    anchor: tuple[float, float]
    along: str = "N"
    cross: str = "W"
    angle_deg: float | None = None
    span: float | None = None


@dataclass(frozen=True)
class Island:
    """One block prefix: a serpentine island, or one segment of a single-faced wall run."""

    prefix: str
    n_max: int
    face0_len: int | None = None
    kind: str = "island"  # "island" | "wall"
    #: Cross-aisles splitting this island (同じ接頭辞を区切る通路), authored as number breaks.
    corridors: tuple[Corridor, ...] = ()
    #: Serpentine-number offset added to every space (for a wall segment that does not
    #: start at 1, so a multi-segment 壁サー keeps continuous numbering). ``0`` for islands.
    number_base: int = 0
    #: Per-island pitch override (metres between desks); ``None`` uses the event pitch.
    #: Used by wall segments spread 等間隔 over a physical span.
    pitch: float | None = None


@dataclass(frozen=True)
class Row:
    """A straight run of islands placed along the *prefix axis*.

    ``along`` is the depth direction, ``cross`` the face-0→face-1 direction, and
    ``prefix_axis`` the direction successive islands march in. Set ``angle_deg`` to
    rotate the whole run (the depth direction becomes ``angle_deg``; cross and prefix
    axes are derived perpendicular) — that is how the diagonal halls are placed.
    """

    islands: tuple[Island, ...]
    origin: tuple[float, float]  # metres: anchor of the first island (n=1, face0, side a)
    island_spacing: float  # metres between successive island anchors
    along: str = "S"
    cross: str = "E"
    prefix_axis: str = "E"
    angle_deg: float | None = None
    label: str = ""  # shown above the run in the Excel detail sheet
    #: Prefixes after which a 縦大通路 (wide vertical aisle) opens; the next island is
    #: pushed further along the prefix axis by ``wide_extra`` extra metres.
    wide_after: tuple[str, ...] = ()
    wide_extra: float = 0.0


@dataclass(frozen=True)
class Hall:
    """One physical hall (e.g. 東4) made of one or more :class:`Row` runs."""

    building: str
    hall: int
    cluster: str
    rows: tuple[Row, ...]
    label: str = ""  # display name, e.g. "東4"


@dataclass(frozen=True)
class EventConfig:
    """Everything needed to generate one event (C107, C108, …)."""

    event: str
    halls: tuple[Hall, ...]
    clusters: tuple[Cluster, ...]
    #: Sparse, symmetric inter-cluster distances in metres, keyed by ``(id_a, id_b)``.
    hall_distances: dict[tuple[str, str], float]
    pitch_m: float = 1.0
    island_width_m: float = 1.5
    cell_size_m: float = 1.0
    #: If set, islands whose longest face is at least this many metres get an automatic
    #: mid corridor (demonstrates the 同じ接頭辞を区切る通路 handling). ``None`` disables it.
    corridor_split_len: float | None = None


# --- derived records ---------------------------------------------------------


@dataclass
class BlockRecord:
    """One row of ``block_layouts.csv`` (the extended schema read by ``src/io.rs``)."""

    id: str
    building: str
    hall: int
    anchor_x: float
    anchor_y: float
    along: str
    cross: str
    n_max: int
    face0_len: int | None
    pitch: float
    island_width: float
    kind: str
    cluster: str
    along_deg: float | None
    cross_deg: float | None
    #: Face-crossing points as ``(along_metres, kind)`` pairs, ``kind`` in {major, minor}.
    crossings: tuple[tuple[float, str], ...]
    #: Serpentine-number offset (wall segments); ``0`` for islands.
    number_base: int = 0


@dataclass
class LocalCell:
    """One placed circle within an island, in local grid terms (for the Excel sheet)."""

    number: int
    side: str  # "a" | "b"
    face: int  # 0 | 1
    depth: int  # 1-based distance from the near (aisle) end


#: Column header for ``block_layouts.csv`` — order matches ``BlockRow`` in ``src/layout.rs``.
BLOCK_CSV_HEADER = [
    "id",
    "building",
    "hall",
    "anchor_x",
    "anchor_y",
    "along",
    "cross",
    "n_max",
    "face0_len",
    "pitch",
    "island_width",
    "kind",
    "cluster",
    "along_deg",
    "cross_deg",
    "crossings",
    "number_base",
]


def _round(value: float) -> float:
    """Tidy a coordinate so the CSV stays readable (sub-millimetre is noise here)."""
    return round(value, 3)


def face0_len_resolved(island: Island) -> int:
    """Desks on face 0: the override, or ``n_max // 2`` (``n_max`` for a wall)."""
    if island.kind == "wall":
        return island.n_max
    return island.face0_len if island.face0_len is not None else island.n_max // 2


def island_len_m(island: Island, pitch: float) -> float:
    """Along-axis length (metres) of the island's longest face."""
    if island.kind == "wall":
        longest = island.n_max
    else:
        f0 = face0_len_resolved(island)
        f1 = island.n_max - f0
        longest = max(f0, f1)
    return max(longest - 1, 0) * pitch


def _row_orientation(row: Row) -> dict:
    """Resolve a row's along/cross/prefix unit vectors and CSV angle overrides."""
    if row.angle_deg is not None:
        along_u = deg_unit(row.angle_deg)
        cross_u = rot90_ccw(along_u)
        prefix_u = rot90_ccw(along_u)
        return {
            "along_u": along_u,
            "prefix_u": prefix_u,
            "along_card": nearest_cardinal(along_u),
            "cross_card": nearest_cardinal(cross_u),
            "along_deg": round(row.angle_deg % 360, 3),
            "cross_deg": round((row.angle_deg + 90) % 360, 3),
        }
    return {
        "along_u": CARDINAL[row.along],
        "prefix_u": CARDINAL[row.prefix_axis],
        "along_card": row.along,
        "cross_card": row.cross,
        "along_deg": None,
        "cross_deg": None,
    }


def corridor_geometry(island: Island, corridor: Corridor, pitch: float) -> dict:
    """Resolve one corridor's along-positions (metres) on each face and the crossing point.

    Depth is 1-based from the near (start, 右下) end, so number ``n`` on face 0 sits at
    ``along = (n - 1) * pitch``; on face 1, ``n`` sits at ``along = (n_max - n) * pitch``.
    A break "after ``k``" is the midpoint of the gap between ``k`` and ``k + 1``.
    """
    a0 = (corridor.face0_after - 0.5) * pitch
    a1 = (island.n_max - corridor.face1_after - 0.5) * pitch
    return {
        "face0_along": _round(a0),
        "face1_along": _round(a1),
        # Crossing point: where the two faces connect (their corridor midpoint).
        "cross_along": _round((a0 + a1) / 2.0),
        "kind": corridor.kind,
    }


def _resolve_crossings(island: Island, pitch: float, split_len: float | None) -> tuple[tuple[float, str], ...]:
    """Crossing points as ``(along, kind)`` pairs from the island's authored corridors.

    Falls back to one auto minor corridor at the midpoint for a long island with none
    authored (keeps the legacy 同じ接頭辞を区切る通路 demo behaviour for bare configs).
    """
    if island.corridors:
        return tuple(
            (corridor_geometry(island, c, pitch)["cross_along"], c.kind) for c in island.corridors
        )
    if island.kind == "wall" or split_len is None:
        return ()
    length = island_len_m(island, pitch)
    if length >= split_len:
        return ((_round(length / 2.0), "minor"),)
    return ()


def hall_block_records(hall: Hall, event: EventConfig) -> list[BlockRecord]:
    """Expand a hall's rows into one :class:`BlockRecord` per island."""
    records: list[BlockRecord] = []
    b_letter = {"East": "E", "West": "W", "South": "S"}[hall.building]
    for row in hall.rows:
        o = _row_orientation(row)
        ox, oy = row.origin
        px, py = o["prefix_u"]
        wide_after = set(row.wide_after)
        offset = 0.0  # cumulative metres along the prefix axis (widened by 縦大通路)
        for i, island in enumerate(row.islands):
            anchor_x = _round(ox + offset * px)
            anchor_y = _round(oy + offset * py)
            offset += row.island_spacing
            if island.prefix in wide_after:
                offset += row.wide_extra
            pitch = island.pitch if island.pitch is not None else event.pitch_m
            records.append(
                BlockRecord(
                    id=f"{b_letter}{hall.hall}-{island.prefix}",
                    building=hall.building,
                    hall=hall.hall,
                    anchor_x=anchor_x,
                    anchor_y=anchor_y,
                    along=o["along_card"],
                    cross=o["cross_card"],
                    n_max=island.n_max,
                    face0_len=island.face0_len,
                    pitch=_round(pitch),
                    island_width=event.island_width_m,
                    kind=island.kind,
                    cluster=hall.cluster,
                    along_deg=o["along_deg"],
                    cross_deg=o["cross_deg"],
                    crossings=_resolve_crossings(island, pitch, event.corridor_split_len),
                    number_base=island.number_base,
                )
            )
    return records


def event_block_records(event: EventConfig) -> list[BlockRecord]:
    """All block records for an event, in hall order."""
    records: list[BlockRecord] = []
    for hall in event.halls:
        records.extend(hall_block_records(hall, event))
    return records


def expand_island_local(island: Island) -> list[LocalCell]:
    """Every circle in an island as a :class:`LocalCell` (mirrors ``Block::expand``).

    ``LocalCell.number`` is the *global* serpentine number (``number_base`` applied), so a
    wall segment shows its true range; ``depth`` stays local to the segment.
    """
    half = face0_len_resolved(island)
    cells: list[LocalCell] = []
    for n in range(1, island.n_max + 1):
        if island.kind == "wall" or n <= half:
            face, depth = 0, n
        else:
            face, depth = 1, island.n_max - n + 1
        for side in ("a", "b"):
            cells.append(LocalCell(number=island.number_base + n, side=side, face=face, depth=depth))
    return cells


def hall_distance(distances: dict[tuple[str, str], float], a: str, b: str) -> float | None:
    """Symmetric lookup; ``0`` on the diagonal, ``None`` when the pair is unknown."""
    if a == b:
        return 0.0
    if (a, b) in distances:
        return distances[(a, b)]
    if (b, a) in distances:
        return distances[(b, a)]
    return None
