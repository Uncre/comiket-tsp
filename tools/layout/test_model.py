"""Parity & sanity checks for the layout engine.

Run directly (no pytest needed):

    python -m tools.layout.test_model

These lock the Python serpentine/wall expansion to the Rust model in ``src/layout.rs``
(see the matching cases in ``tests/layout.rs``) and exercise the generator end to end.
"""

from __future__ import annotations

from .config import C107
from .model import (
    Corridor,
    Hall,
    Island,
    Row,
    corridor_geometry,
    event_block_records,
    expand_island_local,
    hall_block_records,
    island_len_m,
)


def _by(cells, number, side):
    return next(c for c in cells if c.number == number and c.side == side)


def test_island_serpentine() -> None:
    isl = Island("ア", n_max=48)
    cells = expand_island_local(isl)
    assert len(cells) == 96  # 48 numbers x 2 sides
    assert island_len_m(isl, 1.0) == 23.0
    assert (_by(cells, 1, "a").face, _by(cells, 1, "a").depth) == (0, 1)
    assert (_by(cells, 24, "a").face, _by(cells, 24, "a").depth) == (0, 24)
    assert (_by(cells, 25, "a").face, _by(cells, 25, "a").depth) == (1, 24)
    assert (_by(cells, 48, "a").face, _by(cells, 48, "a").depth) == (1, 1)


def test_wall_is_single_faced() -> None:
    wall = Island("A", n_max=4, kind="wall")
    cells = expand_island_local(wall)
    assert {c.face for c in cells} == {0}
    assert island_len_m(wall, 1.0) == 3.0
    assert (_by(cells, 4, "a").depth) == 4


def test_diagonal_and_crossings_in_records() -> None:
    hall = Hall(
        "East", 8, "E7-8",
        rows=(
            Row((Island("W", n_max=36),), origin=(0.0, 0.0), island_spacing=4.0, angle_deg=215.0),
            Row((Island("ア", n_max=48),), origin=(50.0, 0.0), island_spacing=3.0),
        ),
    )
    recs = {r.id: r for r in hall_block_records(hall, C107)}
    diag = recs["E8-W"]
    assert diag.along_deg == 215.0 and diag.cross_deg == 305.0
    # The 48-desk island is long enough (>= 20 m) to earn an auto minor mid corridor.
    big = recs["E8-ア"]
    assert big.crossings == ((11.5, "minor"),)


def test_corridor_geometry_aligns_thin_and_marks_major() -> None:
    # East 4/5/6 pattern 1 (n_max=66): central 16→49 (major), thin 9→57 / 25→41.
    isl = Island("ウ", n_max=66, corridors=(
        Corridor(16, 49, "major"), Corridor(25, 41), Corridor(9, 57),
    ))
    central = corridor_geometry(isl, isl.corridors[0], 1.0)
    upper = corridor_geometry(isl, isl.corridors[1], 1.0)
    lower = corridor_geometry(isl, isl.corridors[2], 1.0)
    # Thin corridors align on both faces; the central one may be skewed but stays mid.
    assert upper["face0_along"] == upper["face1_along"] == 24.5
    assert lower["face0_along"] == lower["face1_along"] == 8.5
    assert central["kind"] == "major" and central["cross_along"] == 16.0
    # The CSV crossings carry the (along, kind) pairs in authored order.
    recs = {r.id: r for r in hall_block_records(
        Hall("East", 6, "E4-6", rows=(Row((isl,), origin=(0.0, 0.0), island_spacing=3.0),)),
        C107,
    )}
    assert recs["E6-ウ"].crossings == ((16.0, "major"), (24.5, "minor"), (8.5, "minor"))


def test_c107_generates_records() -> None:
    recs = event_block_records(C107)
    assert len(recs) > 100
    ids = {r.id for r in recs}
    assert "E7-A" in ids  # wall run present
    assert any(r.kind == "wall" for r in recs)
    assert any(r.along_deg is not None for r in recs)  # diagonal present
    clusters = {r.cluster for r in recs}
    assert {"E4-6", "E7-8", "W1-2", "S1", "S2"} <= clusters


def main() -> int:
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    for t in tests:
        t()
        print(f"ok  {t.__name__}")
    print(f"\n{len(tests)} passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
