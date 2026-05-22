#!/usr/bin/env python3
"""Produce data/benchmarks/data_completeness.csv — one row per registered window.

Re-run after each new transfer-audit publish to keep the tracker current.
"""
from __future__ import annotations

import csv
import sys
from pathlib import Path

from _market_sentiment_common import read_windows_csv, repo_root, write_csv_dicts

FIELDS = [
    "window_id",
    "fng_day_count",
    "audit_artifacts_present",
    "gross_fields_available",
    "supply_invariant_status",
    "included_in_churn_analysis",
]


def _load_fng_counts(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    if not path.is_file():
        return out
    with path.open(newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            out[row["window_id"]] = row.get("fng_day_count", "")
    return out


def _load_panel_windows(path: Path) -> set[str]:
    """Window IDs present in market_conditioned_audit.csv with at least one non-empty gross_to_net_ratio."""
    found: set[str] = set()
    if not path.is_file():
        return found
    with path.open(newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            if row.get("gross_to_net_ratio", "").strip():
                found.add(row["window_id"])
    return found


def _assess_supply_audit(path: Path) -> tuple[bool, bool, str]:
    """Return (artifacts_present, gross_fields_available, supply_invariant_status)."""
    if not path.is_file():
        return False, False, "PENDING"

    chains: list[dict[str, str]] = []
    with path.open(newline="", encoding="utf-8") as f:
        chains = list(csv.DictReader(f))

    if not chains:
        return True, False, "PENDING"

    gross_ok = any(c.get("sum_mints_raw", "").strip() for c in chains)

    pass_values = [c.get("supply_invariant_pass", "").strip().lower() for c in chains]
    if all(v in ("true", "1", "yes") for v in pass_values):
        inv_status = "PASS"
    elif any(v in ("false", "0", "no") for v in pass_values):
        inv_status = "FAIL"
    else:
        inv_status = "UNKNOWN"

    return True, gross_ok, inv_status


def main() -> int:
    root = repo_root()
    sentiment_path = root / "data/external/window_sentiment_summary.csv"
    panel_path = root / "data/benchmarks/market_conditioned_audit.csv"
    out_path = root / "data/benchmarks/data_completeness.csv"

    fng_counts = _load_fng_counts(sentiment_path)
    panel_windows = _load_panel_windows(panel_path)
    windows = read_windows_csv(root / "data/benchmarks/windows.csv")

    rows = []
    for w in windows:
        audit_csv = w.benchmark_dir / "supply_audit.csv"
        present, gross_ok, inv_status = _assess_supply_audit(audit_csv)
        rows.append(
            {
                "window_id": w.window_id,
                "fng_day_count": fng_counts.get(w.window_id, ""),
                "audit_artifacts_present": str(present).lower(),
                "gross_fields_available": str(gross_ok).lower(),
                "supply_invariant_status": inv_status,
                "included_in_churn_analysis": str(w.window_id in panel_windows).lower(),
            }
        )

    write_csv_dicts(out_path, FIELDS, rows)
    print(f"wrote {out_path} ({len(rows)} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
