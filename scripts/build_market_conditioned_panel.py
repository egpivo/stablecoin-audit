#!/usr/bin/env python3
"""Join window sentiment with per-chain supply_audit metrics (association study panel)."""
from __future__ import annotations

import argparse
import csv
import sys
from pathlib import Path
from typing import Any

from _market_sentiment_common import (
    gross_to_net_ratio,
    read_windows_csv,
    repo_root,
    supply_invariant_status,
    write_csv_dicts,
)

USDC_DECIMALS = 6

PANEL_FIELDS = [
    "window_id",
    "asset",
    "from_utc",
    "to_utc",
    "mean_fng",
    "min_fng",
    "max_fng",
    "dominant_regime",
    "analysis_regime",
    "chain",
    "transfer_event_count",
    "transfer_share",
    "onchain_delta_usdc",
    "net_supply_per_1000_transfers",
    "gross_to_net_ratio",
    "supply_invariant_status",
    "accounting_pass_rate",
]


def load_sentiment_by_window(path: Path) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    with path.open(newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            out[row["window_id"]] = row
    return out


def load_supply_audit(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--windows",
        type=Path,
        default=repo_root() / "data/benchmarks/windows.csv",
    )
    p.add_argument(
        "--sentiment",
        type=Path,
        default=repo_root() / "data/external/window_sentiment_summary.csv",
    )
    p.add_argument(
        "--out",
        type=Path,
        default=repo_root() / "data/benchmarks/market_conditioned_audit.csv",
    )
    args = p.parse_args()

    if not args.sentiment.is_file():
        print(
            f"error: missing {args.sentiment} (run scripts/join_window_sentiment.py)",
            file=sys.stderr,
        )
        return 1

    sentiment = load_sentiment_by_window(args.sentiment)
    panel: list[dict[str, Any]] = []

    for w in read_windows_csv(args.windows):
        sent = sentiment.get(w.window_id)
        if not sent:
            print(f"warning: no sentiment for {w.window_id}", file=sys.stderr)
            continue

        audit_path = w.benchmark_dir / "supply_audit.csv"
        if not audit_path.is_file():
            print(f"warning: missing {audit_path}", file=sys.stderr)
            continue

        chains = load_supply_audit(audit_path)
        total_transfers = sum(int(c["transfer_event_count"]) for c in chains)
        pass_n = sum(1 for c in chains if supply_invariant_status(c) == "PASS")
        pass_rate = f"{pass_n / len(chains):.4f}" if chains else ""

        for c in chains:
            tc = int(c["transfer_event_count"])
            share = f"{tc / total_transfers:.6f}" if total_transfers else ""
            delta_raw = c["onchain_delta_raw"]
            delta_usdc = int(delta_raw) / (10**USDC_DECIMALS)
            per_1k = ""
            if tc > 0:
                per_1k = f"{delta_usdc / (tc / 1000):.4f}"

            panel.append(
                {
                    "window_id": w.window_id,
                    "asset": w.asset,
                    "from_utc": w.from_utc,
                    "to_utc": w.to_utc,
                    "mean_fng": sent["mean_fng"],
                    "min_fng": sent["min_fng"],
                    "max_fng": sent["max_fng"],
                    "dominant_regime": sent["dominant_regime"],
                    "analysis_regime": sent["analysis_regime"],
                    "chain": c["chain"],
                    "transfer_event_count": tc,
                    "transfer_share": share,
                    "onchain_delta_usdc": f"{delta_usdc:.4f}",
                    "net_supply_per_1000_transfers": per_1k,
                    "gross_to_net_ratio": gross_to_net_ratio(
                        c["sum_mints_raw"], c["sum_burns_raw"], delta_raw
                    ),
                    "supply_invariant_status": supply_invariant_status(c),
                    "accounting_pass_rate": pass_rate,
                }
            )

    if not panel:
        print("error: empty panel", file=sys.stderr)
        return 1

    write_csv_dicts(args.out, PANEL_FIELDS, panel)
    print(f"wrote {args.out} ({len(panel)} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
