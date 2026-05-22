#!/usr/bin/env python3
"""Annotate windows with nearby regulatory/macro events.

Outputs one row per (window, event) pair where the event falls within
PROXIMITY_DAYS of the window boundary. Position is descriptive only —
no causal claim is made or implied between events and on-chain metrics.
"""
from __future__ import annotations

import argparse
import csv
import sys
from datetime import date, datetime, timezone
from pathlib import Path
from typing import Any

from _market_sentiment_common import iso_to_date_utc, read_windows_csv, repo_root, write_csv_dicts

PROXIMITY_DAYS = 14

FIELDS = [
    "window_id",
    "from_utc",
    "to_utc",
    "event_id",
    "event_date",
    "event_type",
    "event_name",
    "expected_channel",
    "position",
    "days_from_window_start",
    "days_from_window_end",
]


def read_event_context(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def _event_date(ev: dict[str, str]) -> date:
    s = ev["event_date"].strip()
    return datetime.fromisoformat(s).date() if "T" not in s else datetime.fromisoformat(s).astimezone(timezone.utc).date()


def join_window_events(
    windows, events: list[dict[str, str]], proximity_days: int = PROXIMITY_DAYS
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for w in windows:
        w_start = iso_to_date_utc(w.from_utc)
        w_end = iso_to_date_utc(w.to_utc)
        for ev in events:
            ev_date = _event_date(ev)
            days_from_start = (ev_date - w_start).days
            days_from_end = (ev_date - w_end).days

            if ev_date < w_start:
                position = "pre_window"
                distance = (w_start - ev_date).days
            elif ev_date < w_end:
                position = "within"
                distance = 0
            else:
                position = "post_window"
                distance = (ev_date - w_end).days + 1

            if distance > proximity_days:
                continue

            rows.append(
                {
                    "window_id": w.window_id,
                    "from_utc": w.from_utc,
                    "to_utc": w.to_utc,
                    "event_id": ev["event_id"],
                    "event_date": ev["event_date"],
                    "event_type": ev["event_type"],
                    "event_name": ev["event_name"],
                    "expected_channel": ev["expected_channel"],
                    "position": position,
                    "days_from_window_start": days_from_start,
                    "days_from_window_end": days_from_end,
                }
            )
    return rows


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--windows",
        type=Path,
        default=repo_root() / "data/benchmarks/windows.csv",
    )
    p.add_argument(
        "--events",
        type=Path,
        default=repo_root() / "data/external/event_context.csv",
    )
    p.add_argument(
        "--out",
        type=Path,
        default=repo_root() / "data/external/window_event_context.csv",
    )
    p.add_argument(
        "--proximity-days",
        type=int,
        default=PROXIMITY_DAYS,
        help="Max days from window boundary to include an event (default: %(default)s)",
    )
    args = p.parse_args()

    if not args.events.is_file():
        print(f"error: missing {args.events}", file=sys.stderr)
        return 1

    windows = read_windows_csv(args.windows)
    events = read_event_context(args.events)
    rows = join_window_events(windows, events, args.proximity_days)

    if not rows:
        print("warning: no (window, event) pairs within proximity threshold", file=sys.stderr)

    write_csv_dicts(args.out, FIELDS, rows)
    print(f"wrote {args.out} ({len(rows)} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
