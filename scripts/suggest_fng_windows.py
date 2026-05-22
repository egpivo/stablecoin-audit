#!/usr/bin/env python3
"""Suggest 7-day UTC windows by Fear & Greed regime (for multi-window benchmarks)."""
from __future__ import annotations

import argparse
import csv
from collections import Counter
from datetime import date, timedelta
from pathlib import Path

from _market_sentiment_common import analysis_regime, read_fear_greed_csv, repo_root


def week_stats(rows: list, start: date) -> dict | None:
    end = start + timedelta(days=7)
    win = [r for r in rows if start <= r.date_utc < end]
    if len(win) < 7:
        return None
    vals = [r.value for r in win]
    mean_v = sum(vals) / len(vals)
    dom = Counter(r.value_classification for r in win).most_common(1)[0][0]
    return {
        "from_utc": f"{start.isoformat()}T00:00:00Z",
        "to_utc": f"{end.isoformat()}T00:00:00Z",
        "mean_fng": round(mean_v, 2),
        "min_fng": min(vals),
        "max_fng": max(vals),
        "dominant_regime": dom,
        "analysis_regime": analysis_regime(dom),
    }


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--fng",
        type=Path,
        default=repo_root() / "data/external/fear_greed_daily.csv",
    )
    p.add_argument("--since", default="2024-01-01", help="Earliest window start (YYYY-MM-DD)")
    p.add_argument("--until", default="2026-12-31", help="Latest window start (YYYY-MM-DD)")
    p.add_argument("--top", type=int, default=5, help="Rows per bucket to print")
    args = p.parse_args()

    rows = read_fear_greed_csv(args.fng)
    since = date.fromisoformat(args.since)
    until = date.fromisoformat(args.until)

    by_start: dict[str, dict] = {}
    for r in rows:
        if r.date_utc < since or r.date_utc > until:
            continue
        s = week_stats(rows, r.date_utc)
        if s:
            by_start[s["from_utc"][:10]] = s

    candidates = list(by_start.values())
    fear = sorted([c for c in candidates if c["mean_fng"] < 35], key=lambda x: x["mean_fng"])[: args.top]
    greed = sorted([c for c in candidates if c["mean_fng"] >= 60], key=lambda x: -x["mean_fng"])[: args.top]
    mid = sorted(
        [c for c in candidates if 45 <= c["mean_fng"] <= 55],
        key=lambda x: abs(x["mean_fng"] - 50),
    )[: args.top]

    def print_bucket(title: str, items: list[dict]) -> None:
        print(f"\n## {title}\n")
        for c in items:
            wid = (
                f"usdc_7d_{c['from_utc'][:10].replace('-', '')}_"
                f"{c['to_utc'][:10].replace('-', '')}"
            )
            print(
                f"- `{wid}`  {c['from_utc']} → {c['to_utc']}  "
                f"mean={c['mean_fng']}  {c['dominant_regime']}  ({c['analysis_regime']})"
            )

    print_bucket("Fear (low mean F&G)", fear)
    print_bucket("Greed (high mean F&G)", greed)
    print_bucket("Neutral-ish", mid)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
