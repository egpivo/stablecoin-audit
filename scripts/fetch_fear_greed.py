#!/usr/bin/env python3
"""Fetch Crypto Fear & Greed Index (daily) into data/external/fear_greed_daily.csv."""
from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

from _market_sentiment_common import (
    SOURCE_DEFAULT,
    parse_fng_api_payload,
    repo_root,
    write_fear_greed_csv,
    write_meta_json,
)

API_URL = "https://api.alternative.me/fng/?limit=0&format=json"


def fetch_payload(url: str = API_URL, timeout: int = 60) -> dict:
    req = urllib.request.Request(url, headers={"User-Agent": "stablecoin-audit/0.1 (research)"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.load(resp)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--out",
        type=Path,
        default=repo_root() / "data/external/fear_greed_daily.csv",
        help="Output CSV path",
    )
    p.add_argument("--url", default=API_URL, help="Alternative.me F&G API URL")
    p.add_argument("--source", default=SOURCE_DEFAULT, help="source column value")
    p.add_argument("--fixture", type=Path, help="Read JSON fixture instead of HTTP (tests/offline)")
    args = p.parse_args()

    if args.fixture:
        payload = json.loads(args.fixture.read_text(encoding="utf-8"))
    else:
        try:
            payload = fetch_payload(args.url)
        except urllib.error.URLError as e:
            print(f"error: fetch failed: {e}", file=sys.stderr)
            return 1

    rows = parse_fng_api_payload(payload, source=args.source)
    if not rows:
        print("error: no rows parsed", file=sys.stderr)
        return 1

    write_fear_greed_csv(args.out, rows)
    meta_path = args.out.with_suffix(".meta.json")
    write_meta_json(
        meta_path,
        {
            "fetched_at_utc": datetime.now(timezone.utc).isoformat(),
            "source_url": args.url if not args.fixture else str(args.fixture),
            "row_count": len(rows),
            "date_min": rows[0].date_utc.isoformat(),
            "date_max": rows[-1].date_utc.isoformat(),
        },
    )
    print(f"wrote {args.out} ({len(rows)} rows)")
    print(f"wrote {meta_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
