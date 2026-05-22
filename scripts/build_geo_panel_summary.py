#!/usr/bin/env python3
"""
Build data/benchmarks/cross_asset_geo_panel_summary.csv from published and
in-progress supply_audit.csv files.

Sources (priority order for new runs):
  1. docs/benchmarks/<run_id>/supply_audit.csv  (published)
  2. out/<asset>/runs/<run_id>/supply_audit.csv  (completed, not yet published)

Usage:
  python3 scripts/build_geo_panel_summary.py
"""
from __future__ import annotations

import csv
import math
from pathlib import Path

DOCS = Path("docs/benchmarks")
OUT  = Path("out")
OUT_CSV = Path("data/benchmarks/cross_asset_geo_panel_summary.csv")

DECIMALS = 6  # all assets use 6-decimal tokens

# Asset metadata
ASSET_META = {
    "USDC": dict(
        asset="USDC",
        peg_currency="USD",
        geography="United States",
        issuer="Circle Internet Financial",
        convention_status="confirmed_zero_address_convention",
    ),
    "XSGD": dict(
        asset="XSGD",
        peg_currency="SGD",
        geography="Singapore",
        issuer="StraitsX (Xfers Pte. Ltd.)",
        convention_status="confirmed_zero_address_convention",
    ),
    "EURC": dict(
        asset="EURC",
        peg_currency="EUR",
        geography="European Union",
        issuer="Circle Internet Financial",
        convention_status="confirmed_zero_address_convention",  # confirmed on Ethereum and Base
    ),
}

# Chain convention overrides (empty — Base now confirmed)
CONVENTION_NOTES: dict[tuple[str, str], str] = {}

# Window → UTC dates (resolved from block timestamps)
WINDOW_UTC = {
    "usdc_7d_20241117_20241124":             ("2024-11-17T00:00:00Z", "2024-11-24T00:00:00Z"),
    "usdc_7d_20260218_20260225":             ("2026-02-18T00:00:00Z", "2026-02-25T00:00:00Z"),
    "usdc_7d_20260501_20260508":             ("2026-05-01T00:00:00Z", "2026-05-08T00:00:00Z"),
    "usdc_7d_20260507_20260514":             ("2026-05-07T00:00:00Z", "2026-05-14T00:00:00Z"),
    "usdc_7d_20260512_20260519":             ("2026-05-12T00:00:00Z", "2026-05-19T00:00:00Z"),
    "usdc_7d_20260514_20260521":             ("2026-05-14T00:00:00Z", "2026-05-21T00:00:00Z"),
    "usdc_7d_20260513_20260520_ethereum":    ("2026-05-13T00:00:00Z", "2026-05-20T00:00:00Z"),
    "usdc_7d_20260513_20260520_base":        ("2026-05-13T00:00:00Z", "2026-05-20T00:00:00Z"),
    "usdc_7d_20260513_20260520_arbitrum":    ("2026-05-13T00:00:00Z", "2026-05-20T00:00:00Z"),
    "eurc_7d_20260513_20260520_ethereum":    ("2026-05-13T00:00:00Z", "2026-05-20T00:00:00Z"),
    "eurc_7d_20260513_20260520_base":        ("2026-05-13T00:00:00Z", "2026-05-20T00:00:00Z"),
    "xsgd_7d_20260513_20260520":             ("2026-05-13T00:00:00Z", "2026-05-20T00:00:00Z"),
    "xsgd_7d_20260513_20260520_polygon":     ("2026-05-13T00:00:00Z", "2026-05-20T00:00:00Z"),
}

# Multi-chain run IDs map to a canonical base window for UTC lookup
WINDOW_UTC_ALIAS = {
    "usdc_7d_20241117_20241124_ethereum":  "usdc_7d_20241117_20241124",
    "usdc_7d_20241117_20241124_base":      "usdc_7d_20241117_20241124",
    "usdc_7d_20241117_20241124_arbitrum":  "usdc_7d_20241117_20241124",
    "usdc_7d_20260218_20260225_ethereum":  "usdc_7d_20260218_20260225",
    "usdc_7d_20260218_20260225_base":      "usdc_7d_20260218_20260225",
    "usdc_7d_20260218_20260225_arbitrum":  "usdc_7d_20260218_20260225",
    "usdc_7d_20260501_20260508_ethereum":  "usdc_7d_20260501_20260508",
    "usdc_7d_20260501_20260508_base":      "usdc_7d_20260501_20260508",
    "usdc_7d_20260501_20260508_arbitrum":  "usdc_7d_20260501_20260508",
    "usdc_7d_20260507_20260514_ethereum":  "usdc_7d_20260507_20260514",
    "usdc_7d_20260507_20260514_base":      "usdc_7d_20260507_20260514",
    "usdc_7d_20260507_20260514_arbitrum":  "usdc_7d_20260507_20260514",
    "usdc_7d_20260512_20260519_ethereum":  "usdc_7d_20260512_20260519",
    "usdc_7d_20260512_20260519_base":      "usdc_7d_20260512_20260519",
    "usdc_7d_20260512_20260519_arbitrum":  "usdc_7d_20260512_20260519",
    "usdc_7d_20260514_20260521_ethereum":  "usdc_7d_20260514_20260521",
    "usdc_7d_20260514_20260521_base":      "usdc_7d_20260514_20260521",
    "usdc_7d_20260514_20260521_arbitrum":  "usdc_7d_20260514_20260521",
}


def detect_asset(run_id: str) -> str | None:
    rid = run_id.lower()
    if rid.startswith("usdc"): return "USDC"
    if rid.startswith("xsgd"): return "XSGD"
    if rid.startswith("eurc"): return "EURC"
    return None


def find_supply_audits() -> list[tuple[str, str, Path]]:
    """Return (run_id, asset, path) for every available supply_audit.csv."""
    found: dict[tuple[str, str], Path] = {}

    # docs/benchmarks/<run_id>/supply_audit.csv
    for p in sorted(DOCS.glob("*/supply_audit.csv")):
        run_id = p.parent.name
        asset  = detect_asset(run_id)
        if asset:
            found[(run_id, asset)] = p

    # out/<asset>/runs/<run_id>/supply_audit.csv (may override if more recent)
    for p in sorted(OUT.glob("*/runs/*/supply_audit.csv")):
        run_id = p.parent.name
        asset  = p.parent.parent.parent.name.upper()
        if asset in ASSET_META and (run_id, asset) not in found:
            found[(run_id, asset)] = p

    return [(rid, ast, path) for (rid, ast), path in sorted(found.items())]


def read_supply_audit(path: Path) -> list[dict]:
    rows = []
    with open(path, newline="") as f:
        for r in csv.DictReader(f):
            rows.append(r)
    return rows


def build_row(run_id: str, asset: str, row: dict) -> dict:
    meta = ASSET_META[asset]
    chain = row["chain"]
    dec = 10 ** DECIMALS

    transfer_count  = int(row["transfer_event_count"])
    active_senders  = int(row["active_senders"])
    active_recips   = int(row["active_recipients"])
    mint_count      = int(row["mint_count"])
    burn_count      = int(row["burn_count"])

    sum_mints_raw   = int(row["sum_mints_raw"])
    sum_burns_raw   = int(row["sum_burns_raw"])
    net_mint_raw    = int(row["net_mint_raw"])

    net_supply_delta = net_mint_raw / dec
    gross_churn      = (sum_mints_raw + sum_burns_raw) / dec
    if abs(net_supply_delta) > 0:
        gtn = round(gross_churn / abs(net_supply_delta), 4)
    else:
        gtn = None  # avoid divide-by-zero

    inv_pass = row.get("supply_invariant_pass", "").strip().lower()
    invariant_status = "PASS" if inv_pass in ("true", "1", "pass") else "FAIL"

    # Window UTC
    utc_key = WINDOW_UTC_ALIAS.get(run_id, run_id)
    from_utc, to_utc = WINDOW_UTC.get(utc_key, ("", ""))

    # Convention override for specific (asset, chain) pairs
    conv = CONVENTION_NOTES.get((asset, chain), meta["convention_status"])

    notes_parts = []
    if gtn is None:
        notes_parts.append("net_supply_delta=0; GN ratio undefined")
        gtn_str = ""
    else:
        gtn_str = f"{gtn:.4f}"

    return {
        "asset":                  asset,
        "peg_currency":           meta["peg_currency"],
        "geography":              meta["geography"],
        "issuer":                 meta["issuer"],
        "chain":                  chain,
        "window_id":              run_id,
        "from_utc":               from_utc,
        "to_utc":                 to_utc,
        "transfer_event_count":   transfer_count,
        "active_senders":         active_senders,
        "active_recipients":      active_recips,
        "mint_count":             mint_count,
        "burn_count":             burn_count,
        "net_supply_delta":       round(net_supply_delta, 6),
        "gross_churn":            round(gross_churn, 6),
        "gross_to_net_ratio":     gtn_str,
        "supply_invariant_status": invariant_status,
        "audit_status":           "complete",
        "convention_status":      conv,
        "notes":                  "; ".join(notes_parts),
    }


FIELDNAMES = [
    "asset", "peg_currency", "geography", "issuer", "chain",
    "window_id", "from_utc", "to_utc",
    "transfer_event_count", "active_senders", "active_recipients",
    "mint_count", "burn_count",
    "net_supply_delta", "gross_churn", "gross_to_net_ratio",
    "supply_invariant_status", "audit_status", "convention_status", "notes",
]


def main() -> None:
    sources = find_supply_audits()
    print(f"Found {len(sources)} supply_audit.csv files")

    out_rows: list[dict] = []
    seen: set[tuple[str, str]] = set()

    for run_id, asset, path in sources:
        audit_rows = read_supply_audit(path)
        for row in audit_rows:
            chain = row["chain"]
            key = (run_id, chain)
            if key in seen:
                print(f"  SKIP duplicate: {run_id} / {chain}")
                continue
            seen.add(key)
            panel_row = build_row(run_id, asset, row)
            out_rows.append(panel_row)
            print(f"  + {asset:4s} {chain:8s} {run_id}  inv={panel_row['supply_invariant_status']}")

    # Sort: asset, chain, window_id
    out_rows.sort(key=lambda r: (r["asset"], r["chain"], r["window_id"]))

    OUT_CSV.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT_CSV, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=FIELDNAMES)
        w.writeheader()
        w.writerows(out_rows)

    print(f"\nWrote {len(out_rows)} rows → {OUT_CSV}")


if __name__ == "__main__":
    main()
