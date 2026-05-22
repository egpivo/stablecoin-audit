#!/usr/bin/env python3
"""Fetch daily USDC/USDT OHLC from Binance public API.

No API key required. Covers 2024-11-01 → today.
Run once before generating geo-policy figures.

  python3 scripts/fetch_usdc_price.py

Output columns: date_utc, price_usd (close), high_usd, low_usd, source
"""
from __future__ import annotations

import csv
import json
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
OUT  = REPO / "data" / "external" / "usdc_price_daily.csv"

FROM_DT = datetime(2024, 11, 1, tzinfo=timezone.utc)
TO_DT   = datetime.now(tz=timezone.utc)

SOURCE = "binance.com/api/v3/klines USDCUSDT 1d ohlc"
UA     = "stablecoin-audit/0.1 (research)"

# Binance klines: [open_time_ms, open, high, low, close, volume, close_time_ms, ...]
BINANCE_URL = (
    "https://api.binance.com/api/v3/klines"
    "?symbol=USDCUSDT&interval=1d&limit=1000"
    "&startTime={start_ms}&endTime={end_ms}"
)


def fetch_binance(from_dt: datetime, to_dt: datetime) -> list[tuple[str, float, float, float]]:
    start_ms = int(from_dt.timestamp() * 1000)
    end_ms   = int(to_dt.timestamp()   * 1000)
    url = BINANCE_URL.format(start_ms=start_ms, end_ms=end_ms)
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=30) as resp:
        klines = json.load(resp)

    # kline format: [open_time_ms, open, high, low, close, volume, close_time_ms, ...]
    rows = []
    for k in klines:
        dt = datetime.fromtimestamp(k[0] / 1000, tz=timezone.utc)
        rows.append((dt.strftime("%Y-%m-%d"), float(k[4]), float(k[2]), float(k[3])))
    rows.sort()
    return rows


def main() -> int:
    print(f"Fetching USDC/USDT daily close: {FROM_DT.date()} → {TO_DT.date()} …")
    rows = fetch_binance(FROM_DT, TO_DT)
    if not rows:
        print("error: no data returned", file=sys.stderr)
        return 1
    print(f"  {len(rows)} daily rows")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["date_utc", "price_usd", "high_usd", "low_usd", "source"])
        for date_str, close, high, low in rows:
            w.writerow([date_str, f"{close:.6f}", f"{high:.6f}", f"{low:.6f}", SOURCE])

    print(f"Wrote {OUT}  ({len(rows)} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
