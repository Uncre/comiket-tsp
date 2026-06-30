"""CLI: turn an event's constants into ``block_layouts.csv``, ``hall_distances.csv``
and ``layout_<event>.xlsx``.

    python -m tools.layout.generate --event C107

Run with no install beyond ``openpyxl`` (see ``tools/layout/requirements.txt``).
"""

from __future__ import annotations

import argparse
from pathlib import Path

from .config import EVENTS
from .csvout import write_block_layouts, write_hall_distances
from .excel import build_workbook
from .model import event_block_records


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generate Comiket layout files from constants.")
    parser.add_argument("--event", default="C107", help="event key from config.EVENTS (default: C107)")
    parser.add_argument("--data-dir", default="data", type=Path, help="output dir for the CSVs")
    parser.add_argument("--out-dir", default="out", type=Path, help="output dir for the .xlsx")
    args = parser.parse_args(argv)

    if args.event not in EVENTS:
        parser.error(f"unknown event {args.event!r}; known: {', '.join(sorted(EVENTS))}")
    event = EVENTS[args.event]

    records = event_block_records(event)

    blocks_csv = args.data_dir / "block_layouts.csv"
    dist_csv = args.data_dir / "hall_distances.csv"
    xlsx = args.out_dir / f"layout_{event.event}.xlsx"

    write_block_layouts(blocks_csv, records)
    write_hall_distances(dist_csv, event)
    build_workbook(event, records, xlsx)

    print(f"{event.event}: {len(records)} islands across {len(event.halls)} halls")
    print(f"  wrote {blocks_csv}")
    print(f"  wrote {dist_csv}")
    print(f"  wrote {xlsx}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
