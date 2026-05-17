#!/usr/bin/env python3
"""Aggregate daily Fear & Greed into per-audit-window sentiment summaries."""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from _market_sentiment_common import (
    read_fear_greed_csv,
    read_windows_csv,
    repo_root,
    summarize_window_fng,
    write_csv_dicts,
)

OUT_FIELDS = [
    "window_id",
    "from_utc",
    "to_utc",
    "fng_day_count",
    "mean_fng",
    "min_fng",
    "max_fng",
    "dominant_regime",
    "analysis_regime",
]


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--windows",
        type=Path,
        default=repo_root() / "data/benchmarks/windows.csv",
    )
    p.add_argument(
        "--fng",
        type=Path,
        default=repo_root() / "data/external/fear_greed_daily.csv",
    )
    p.add_argument(
        "--out",
        type=Path,
        default=repo_root() / "data/external/window_sentiment_summary.csv",
    )
    args = p.parse_args()

    if not args.fng.is_file():
        print(f"error: missing {args.fng} (run scripts/fetch_fear_greed.py)", file=sys.stderr)
        return 1
    if not args.windows.is_file():
        print(f"error: missing {args.windows}", file=sys.stderr)
        return 1

    fng = read_fear_greed_csv(args.fng)
    windows = read_windows_csv(args.windows)
    summaries: list[dict] = []
    for w in windows:
        try:
            summaries.append(summarize_window_fng(w, fng))
        except ValueError as e:
            print(f"warning: {e}", file=sys.stderr)

    if not summaries:
        print("error: no window summaries produced", file=sys.stderr)
        return 1

    write_csv_dicts(args.out, OUT_FIELDS, summaries)
    print(f"wrote {args.out} ({len(summaries)} windows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
