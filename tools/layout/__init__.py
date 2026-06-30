"""Comiket venue-layout generator.

Turns the per-event constants in :mod:`tools.layout.config` into the ``block_layouts.csv``
and ``hall_distances.csv`` the Rust solver reads, plus a human-readable ``島配置図`` Excel
workbook. Entry point: ``python -m tools.layout.generate --event C107``.
"""

from .config import EVENTS
from .model import EventConfig, event_block_records

__all__ = ["EVENTS", "EventConfig", "event_block_records"]
