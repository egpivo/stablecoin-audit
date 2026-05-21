#!/usr/bin/env python3
"""Fetch CCTP V1 DepositForBurn events for all six USDC audit windows.

Queries Ethereum, Base, and Arbitrum TokenMessenger contracts using the
same from_block / to_block boundaries as the supply audit.

Goal: determine what fraction of each window's gross mint/burn activity
on each chain can be attributed to CCTP cross-chain routing — in particular,
whether the Ethereum May 14–21 gross-to-net outlier (191.87×) is explained
by large outbound CCTP burns.

This script makes descriptive accounting claims only. It does not model
causality, trading signals, or risk rankings.

Usage:
  python3 scripts/fetch_cctp_flows.py

Output:
  data/benchmarks/cctp_flow_raw.csv     — one row per DepositForBurn event
  data/benchmarks/cctp_flow_summary.csv — aggregated per window × chain × destination

Requires ALCHEMY_ETHEREUM_URL, ALCHEMY_BASE_URL, ALCHEMY_ARBITRUM_URL in .env

CCTP V1 contract addresses (Circle, public):
  Ethereum:  0xBd3fa81B58Ba92a82136038B25aDec7066af3155
  Base:      0x1682Ae6375C4E4A97e4B583BC394c861A46D8962
  Arbitrum:  0x19330d10D9Cc8751218eaf51E8885D058642E08A

DepositForBurn event topic (keccak256 of ABI signature):
  0x2fa9ca894982930190727e75500a97d8dc500233a5065e0f3126c48fbe0343c0
  Verify: search for the topic on Etherscan under TokenMessenger → Events.

CCTP domain IDs:
  0 = Ethereum, 1 = Avalanche, 2 = Optimism, 3 = Arbitrum, 6 = Base, 7 = Polygon
"""
from __future__ import annotations

import csv
import json
import os
import sys
import time
from pathlib import Path
from typing import Iterator

HERE = Path(__file__).resolve().parent
REPO = HERE.parent

# ── outputs ──────────────────────────────────────────────────────────────────
RAW_OUT     = REPO / "data" / "benchmarks" / "cctp_flow_raw.csv"
SUMMARY_OUT = REPO / "data" / "benchmarks" / "cctp_flow_summary.csv"

# ── CCTP V1 TokenMessenger contract addresses ─────────────────────────────────
CCTP_V1 = {
    "ethereum": "0xBd3fa81B58Ba92a82136038B25aDec7066af3155",
    "base":     "0x1682Ae6375C4E4A97e4B583BC394c861A46D8962",
    "arbitrum": "0x19330d10D9Cc8751218eaf51E8885D058642E08A",
}

# ── CCTP V1 DepositForBurn event topic ───────────────────────────────────────
# keccak256("DepositForBurn(uint64,address,uint256,address,bytes32,uint32,bytes32,bytes32)")
# If this returns 0 logs: verify the topic against a known CCTP tx on Etherscan.
DEPOSIT_FOR_BURN_TOPIC = "0x2fa9ca894982930190727e75500a97d8dc500233a5065e0f3126c48fbe0343c0"

# ── CCTP destination domain → chain name ─────────────────────────────────────
DOMAIN_NAME = {
    0: "ethereum",
    1: "avalanche",
    2: "op-mainnet",
    3: "arbitrum",
    4: "noble",
    5: "solana",
    6: "base",
    7: "polygon-pos",
    8: "sui",
    9: "aptos",
    10: "unichain",
    # verify unknown domains against https://developers.circle.com/stablecoins/cctp-protocol-contract-addresses-and-supported-chains
}

# USDC decimals
USDC_DECIMALS = 6

# ── RPC env key per chain ─────────────────────────────────────────────────────
RPC_ENV = {
    "ethereum": "ALCHEMY_ETHEREUM_URL",
    "base":     "ALCHEMY_BASE_URL",
    "arbitrum": "ALCHEMY_ARBITRUM_URL",
}

# ── chunk sizes (CCTP events are sparse; large chunks are safe) ───────────────
CHUNK_SIZE = {
    "ethereum": 5_000,
    "base":     25_000,
    "arbitrum": 200_000,
}

# ── audit window block ranges (from supply_audit.csv boundary blocks) ─────────
# from_block / to_block match the exact blocks used in the transfer audit.
WINDOW_BLOCKS: dict[str, dict[str, tuple[int, int]]] = {
    "usdc_7d_20241117_20241124": {
        "ethereum": (21203704, 21253879),
        "base":     (22506127, 22808526),
        "arbitrum": (275231008, 277637793),
    },
    "usdc_7d_20260218_20260225": {
        "ethereum": (24479995, 24530203),
        "base":     (42291727, 42594126),
        "arbitrum": (433213243, 435639345),
    },
    "usdc_7d_20260501_20260508": {
        "ethereum": (24996368, 25046605),
        "base":     (45402127, 45704526),
        "arbitrum": (458085624, 460491249),
    },
    "usdc_7d_20260507_20260514": {
        "ethereum": (25039433, 25089644),
        "base":     (45661327, 45963726),
        "arbitrum": (460146890, 462559767),
    },
    "usdc_7d_20260512_20260519": {
        "ethereum": (25075306, 25125536),
        "base":     (45877327, 46179726),
        "arbitrum": (461870006, 464280529),
    },
    "usdc_7d_20260514_20260521": {
        "ethereum": (25089645, 25128822),
        "base":     (45963727, 46199501),
        "arbitrum": (462559764, 464438118),
    },
}

# ── gross mint+burn from supply audit (for coverage ratio) ───────────────────
# Precomputed as gross_to_net_ratio * |onchain_delta_usdc| from market_conditioned_audit.csv
# Used to compute what fraction of gross churn is CCTP outbound.
GROSS_CHURN_M: dict[str, dict[str, float]] = {
    "usdc_7d_20241117_20241124": {
        "ethereum": 3.9296  * 1270.6,   # ratio × |net|
        "base":     13.0929 * 40.5,
        "arbitrum": 4.9111  * 82.7,
    },
    "usdc_7d_20260218_20260225": {
        "ethereum": 5.1013  * 1361.0,
        "base":     18.5245 * 42.7,
        "arbitrum": 17.7376 * 60.7,
    },
    "usdc_7d_20260501_20260508": {
        "ethereum": 6.7837  * 1219.1,
        "base":     21.15   * 40.7,
        "arbitrum": 4.7887  * 216.2,
    },
    "usdc_7d_20260507_20260514": {
        "ethereum": 9.2543  * 864.0,
        "base":     6.5744  * 134.4,
        "arbitrum": 14.8002 * 66.1,
    },
    "usdc_7d_20260512_20260519": {
        "ethereum": 35.198  * 254.8,
        "base":     7.262   * 132.0,
        "arbitrum": 24.6736 * 41.5,
    },
    "usdc_7d_20260514_20260521": {
        "ethereum": 191.8726 * 31.7,
        "base":     15.8548  * 41.9,
        "arbitrum": 10.033   * 76.6,
    },
}


# ── helpers ───────────────────────────────────────────────────────────────────
def load_env() -> None:
    env_path = REPO / ".env"
    if env_path.is_file():
        for line in env_path.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, _, v = line.partition("=")
                os.environ.setdefault(k.strip(), v.strip())


def rpc_call(url: str, method: str, params: list) -> dict:
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    import urllib.request
    req = urllib.request.Request(
        url, data=body,
        headers={"Content-Type": "application/json", "User-Agent": "stablecoin-audit/0.1"},
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read())


def eth_get_logs(url: str, address: str, topic: str, from_block: int, to_block: int) -> list[dict]:
    result = rpc_call(url, "eth_getLogs", [{
        "address":   address,
        "topics":    [topic],
        "fromBlock": hex(from_block),
        "toBlock":   hex(to_block),
    }])
    if "error" in result:
        raise RuntimeError(f"eth_getLogs error: {result['error']}")
    return result.get("result", [])


def chunked_get_logs(
    url: str, address: str, topic: str,
    from_block: int, to_block: int, chunk: int,
    delay: float = 0.15,
) -> Iterator[dict]:
    cur = from_block
    total_chunks = (to_block - from_block) // chunk + 1
    done = 0
    while cur <= to_block:
        hi = min(cur + chunk - 1, to_block)
        logs = eth_get_logs(url, address, topic, cur, hi)
        for log in logs:
            yield log
        done += 1
        if done % 20 == 0:
            print(f"    chunk {done}/{total_chunks} (block {cur}–{hi}, {len(logs)} logs)")
        cur = hi + 1
        if cur <= to_block:
            time.sleep(delay)


def decode_deposit_for_burn(log: dict) -> dict | None:
    """Decode a DepositForBurn log into a structured dict.

    Event layout (V1):
      topics[0] = event signature
      topics[1] = nonce (indexed uint64)
      topics[2] = burnToken (indexed address)
      topics[3] = depositor (indexed address)
      data (non-indexed, 5 × 32 bytes):
        [0]  amount (uint256)
        [1]  mintRecipient (bytes32)
        [2]  destinationDomain (uint32)
        [3]  destinationTokenMessenger (bytes32)
        [4]  destinationCaller (bytes32)
    """
    try:
        topics = log["topics"]
        data_hex = log["data"][2:]  # strip 0x

        nonce      = int(topics[1], 16) if len(topics) > 1 else None
        burn_token = "0x" + topics[2][-40:] if len(topics) > 2 else None
        depositor  = "0x" + topics[3][-40:] if len(topics) > 3 else None

        # 5 × 64 hex chars = 320 chars minimum
        if len(data_hex) < 320:
            return None

        amount_raw      = int(data_hex[0:64],   16)
        dest_domain     = int(data_hex[128:192], 16)  # slot 2, uint32 right-padded

        amount_usdc = amount_raw / (10 ** USDC_DECIMALS)

        return {
            "tx_hash":      log.get("transactionHash", ""),
            "block_number": int(log["blockNumber"], 16),
            "nonce":        nonce,
            "burn_token":   burn_token,
            "depositor":    depositor,
            "amount_usdc":  amount_usdc,
            "dest_domain":  dest_domain,
            "dest_chain":   DOMAIN_NAME.get(dest_domain, f"domain_{dest_domain}"),
        }
    except Exception as e:
        print(f"  decode error: {e} — log: {log.get('transactionHash')}", file=sys.stderr)
        return None


# ── main fetch loop ───────────────────────────────────────────────────────────
def fetch_all() -> list[dict]:
    load_env()
    all_rows: list[dict] = []

    for window_id, chain_blocks in WINDOW_BLOCKS.items():
        for chain, (from_blk, to_blk) in chain_blocks.items():
            rpc_url = os.environ.get(RPC_ENV[chain])
            if not rpc_url:
                print(f"  SKIP {window_id}/{chain}: {RPC_ENV[chain]} not set", file=sys.stderr)
                continue

            contract = CCTP_V1[chain]
            chunk    = CHUNK_SIZE[chain]
            n_blocks = to_blk - from_blk + 1

            print(f"\n{window_id} / {chain}  blocks {from_blk}–{to_blk} ({n_blocks:,})")

            count = 0
            for log in chunked_get_logs(
                rpc_url, contract, DEPOSIT_FOR_BURN_TOPIC,
                from_blk, to_blk, chunk,
            ):
                decoded = decode_deposit_for_burn(log)
                if decoded:
                    decoded["window_id"] = window_id
                    decoded["chain"]     = chain
                    all_rows.append(decoded)
                    count += 1

            print(f"  → {count} DepositForBurn events")

    return all_rows


# ── aggregate summary ─────────────────────────────────────────────────────────
def summarise(rows: list[dict]) -> list[dict]:
    from collections import defaultdict

    # key: (window_id, source_chain, dest_chain)
    agg: dict[tuple, dict] = defaultdict(lambda: {"count": 0, "cctp_burn_usdc": 0.0})

    for r in rows:
        key = (r["window_id"], r["chain"], r["dest_chain"])
        agg[key]["count"]          += 1
        agg[key]["cctp_burn_usdc"] += r["amount_usdc"]

    summary: list[dict] = []
    for (wid, src, dst), vals in sorted(agg.items()):
        gross_churn_m = GROSS_CHURN_M.get(wid, {}).get(src)
        cctp_m        = vals["cctp_burn_usdc"] / 1e6
        coverage      = (cctp_m / gross_churn_m * 100) if gross_churn_m else None
        summary.append({
            "window_id":             wid,
            "source_chain":          src,
            "dest_chain":            dst,
            "cctp_burn_event_count": vals["count"],
            "cctp_burn_usdc":        round(vals["cctp_burn_usdc"], 2),
            "cctp_burn_m_usdc":      round(cctp_m, 4),
            "gross_churn_m_usdc":    round(gross_churn_m, 4) if gross_churn_m else "",
            "cctp_pct_of_gross":     round(coverage, 2) if coverage is not None else "",
        })

    return summary


def write_raw(rows: list[dict]) -> None:
    if not rows:
        print("No raw rows to write.")
        return
    RAW_OUT.parent.mkdir(parents=True, exist_ok=True)
    fields = ["window_id", "chain", "block_number", "tx_hash", "nonce",
              "burn_token", "depositor", "amount_usdc", "dest_domain", "dest_chain"]
    with open(RAW_OUT, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields, extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)
    print(f"\nWrote {RAW_OUT}  ({len(rows)} rows)")


def write_summary(summary: list[dict]) -> None:
    if not summary:
        print("No summary rows to write.")
        return
    fields = ["window_id", "source_chain", "dest_chain",
              "cctp_burn_event_count", "cctp_burn_usdc", "cctp_burn_m_usdc",
              "gross_churn_m_usdc", "cctp_pct_of_gross"]
    with open(SUMMARY_OUT, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields)
        w.writeheader()
        w.writerows(summary)
    print(f"Wrote {SUMMARY_OUT}  ({len(summary)} rows)")


def main() -> int:
    print("CCTP V1 DepositForBurn fetch — all 6 audit windows × 3 chains")
    print("=" * 60)
    rows = fetch_all()
    if not rows:
        print("\nNo events found. Check RPC URLs and event topic.", file=sys.stderr)
        return 1
    write_raw(rows)
    summary = summarise(rows)
    write_summary(summary)

    # quick diagnostic for the outlier window
    print("\n── Ethereum May 14–21 CCTP breakdown ──")
    for r in summary:
        if r["window_id"] == "usdc_7d_20260514_20260521" and r["source_chain"] == "ethereum":
            pct = r["cctp_pct_of_gross"]
            print(
                f"  → {r['dest_chain']:15s}  {r['cctp_burn_m_usdc']:>8.1f}M USDC  "
                f"({pct}% of gross churn)" if pct else
                f"  → {r['dest_chain']:15s}  {r['cctp_burn_m_usdc']:>8.1f}M USDC"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
