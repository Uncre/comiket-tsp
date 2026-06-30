"""Write the two CSV artifacts the Rust solver consumes.

* ``block_layouts.csv`` — one row per island (the extended schema in ``src/layout.rs``).
* ``hall_distances.csv`` — the square inter-cluster distance matrix (``src/io.rs``).
"""

from __future__ import annotations

import csv
from pathlib import Path

from .model import (
    BLOCK_CSV_HEADER,
    BlockRecord,
    Cluster,
    EventConfig,
    hall_distance,
)


def _fmt(value: float) -> str:
    """Format a float without a trailing ``.0`` noise on whole numbers."""
    if value == int(value):
        return str(int(value))
    return repr(value)


def _block_row(rec: BlockRecord) -> list[str]:
    return [
        rec.id,
        rec.building,
        str(rec.hall),
        _fmt(rec.anchor_x),
        _fmt(rec.anchor_y),
        rec.along,
        rec.cross,
        str(rec.n_max),
        "" if rec.face0_len is None else str(rec.face0_len),
        _fmt(rec.pitch),
        _fmt(rec.island_width),
        rec.kind,
        rec.cluster,
        "" if rec.along_deg is None else _fmt(rec.along_deg),
        "" if rec.cross_deg is None else _fmt(rec.cross_deg),
        ";".join(f"{_fmt(along)}@{kind}" for along, kind in rec.crossings),
        str(rec.number_base),
    ]


def write_block_layouts(path: Path, records: list[BlockRecord]) -> None:
    """Write ``block_layouts.csv`` (UTF-8, no BOM, ``\\n`` line endings)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.writer(fh)
        writer.writerow(BLOCK_CSV_HEADER)
        for rec in records:
            writer.writerow(_block_row(rec))


def write_hall_distances(path: Path, event: EventConfig) -> None:
    """Write ``hall_distances.csv`` as a square matrix (corner blank, labels on both axes).

    Unknown pairs are left blank; the diagonal is ``0``. The Rust reader treats the table
    as symmetric, so an unfilled lower triangle is fine.
    """
    clusters: tuple[Cluster, ...] = event.clusters
    ids = [c.id for c in clusters]
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.writer(fh)
        writer.writerow(["cluster", *ids])
        for a in ids:
            row = [a]
            for b in ids:
                d = hall_distance(event.hall_distances, a, b)
                row.append("" if d is None else _fmt(d))
            writer.writerow(row)
