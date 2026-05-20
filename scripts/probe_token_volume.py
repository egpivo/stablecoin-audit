#!/usr/bin/env python3
"""
Estimate 7-day Transfer log volume for a token without running a full transfer-audit.

Also checks the zero-address mint/burn convention if --check-mint-convention is passed.

Usage:
  python3 scripts/probe_token_volume.py --chain ethereum --contract 0x1aBaEA1f7C830bD89Acc67eC4af516284b1bC33c
  python3 scripts/probe_token_volume.py --chain ethereum --contract 0x70e8de73ce538da2beed35d14187f6959a8eca96 --check-mint-convention

Requires ALCHEMY_ETHEREUM_URL (or ALCHEMY_BASE_URL / ALCHEMY_ARBITRUM_URL) in .env or environment.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
from pathlib import Path

TRANSFER_TOPIC = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
ZERO_ADDRESS   = "0x0000000000000000000000000000000000000000000000000000000000000000"

RPC_ENV = {
    "ethereum": "ALCHEMY_ETHEREUM_URL",
    "base":     "ALCHEMY_BASE_URL",
    "arbitrum": "ALCHEMY_ARBITRUM_URL",
    "polygon":  "ALCHEMY_POLYGON_URL",
}

BLOCKS_PER_DAY = {
    "ethereum": 7200,
    "base":     43200,
    "arbitrum": 345600,
    "polygon":  43200,
}


def load_env() -> None:
    env_path = Path(__file__).resolve().parent.parent / ".env"
    if env_path.is_file():
        for line in env_path.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, _, v = line.partition("=")
                os.environ.setdefault(k.strip(), v.strip())


def rpc(url: str, method: str, params: list) -> dict:
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read())


def hex_to_int(h: str) -> int:
    return int(h, 16)


def get_latest_block(url: str) -> int:
    res = rpc(url, "eth_blockNumber", [])
    return hex_to_int(res["result"])


def get_logs(url: str, contract: str, from_block: int, to_block: int) -> list:
    res = rpc(url, "eth_getLogs", [{
        "address": contract,
        "topics":  [TRANSFER_TOPIC],
        "fromBlock": hex(from_block),
        "toBlock":   hex(to_block),
    }])
    if "error" in res:
        raise RuntimeError(f"eth_getLogs error: {res['error']}")
    return res.get("result", [])


def main() -> int:
    load_env()
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--chain", required=True, choices=list(RPC_ENV), help="ethereum | base | arbitrum | polygon")
    p.add_argument("--contract", required=True, help="Contract address to probe")
    p.add_argument("--sample-blocks", type=int, default=500,
                   help="Number of recent blocks to sample for volume estimate (default: 500)")
    p.add_argument("--check-mint-convention", action="store_true",
                   help="Check whether mints use zero-address from (Transfer(from=0x0...))")
    args = p.parse_args()

    rpc_env = RPC_ENV[args.chain]
    url = os.environ.get(rpc_env)
    if not url:
        print(f"error: {rpc_env} not set", file=sys.stderr)
        return 1

    contract = args.contract.lower()

    print(f"chain:    {args.chain}")
    print(f"contract: {contract}")
    print()

    latest = get_latest_block(url)
    from_block = latest - args.sample_blocks
    print(f"latest block:  {latest}")
    print(f"sample window: {from_block} → {latest} ({args.sample_blocks} blocks)")

    logs = get_logs(url, contract, from_block, latest)
    print(f"Transfer logs in sample: {len(logs)}")

    if len(logs) == 0:
        print("\nWARNING: zero logs in sample — contract may be inactive, wrong address, or wrong chain")
        return 0

    # Extrapolate to 7 days
    blocks_7d = BLOCKS_PER_DAY[args.chain] * 7
    estimated_7d = int(len(logs) * blocks_7d / args.sample_blocks)
    print(f"Estimated 7-day Transfer count: ~{estimated_7d:,}  (extrapolated from sample)")
    if estimated_7d < 1000:
        print("  → WARNING: very low volume; supply invariant check may be trivial")
    elif estimated_7d < 50_000:
        print("  → NOTE: moderate volume; audit is feasible but window will be fast")
    else:
        print("  → volume sufficient for meaningful audit window")

    if args.check_mint_convention:
        print()
        # Mints: Transfer(from=0x0)
        mints = [l for l in logs if l["topics"][1] == ZERO_ADDRESS]
        burns = [l for l in logs if l["topics"][2] == ZERO_ADDRESS]
        print(f"Zero-address mint pattern (from=0x0): {len(mints)} events in sample")
        print(f"Zero-address burn pattern (to=0x0):   {len(burns)} events in sample")
        if mints or burns:
            print("  → COMPATIBLE: zero-address mint/burn convention confirmed")
        else:
            print("  → UNKNOWN: no mint or burn events in sample window")
            print("     Try a larger --sample-blocks value or a window with known issuance activity")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
