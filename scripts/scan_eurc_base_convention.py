#!/usr/bin/env python3
"""
EURC Base mint/burn convention scan.

Scans the canonical May 13-20 2026 window (Base blocks 45920527 → 46222926)
in 25k-block chunks for zero-address mint/burn Transfer events.

Also checks for non-zero-address events to detect bridge/issuer patterns.

Output: .local/research/eurc_base_mint_burn_convention.md
"""
from __future__ import annotations

import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

EURC_BASE_CONTRACT = "0x60a3e35cc302bfa44cb288bc5a4f316fdb1adb42"
TRANSFER_TOPIC = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
ZERO_PAD = "0x0000000000000000000000000000000000000000000000000000000000000000"

FROM_BLOCK = 45920527
TO_BLOCK   = 46222926
CHUNK_SIZE = 5_000

OUT_MD = Path(".local/research/eurc_base_mint_burn_convention.md")


def load_env() -> None:
    env_path = Path(__file__).resolve().parent.parent / ".env"
    if env_path.is_file():
        for line in env_path.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, _, v = line.partition("=")
                os.environ.setdefault(k.strip(), v.strip())


def rpc(url: str, method: str, params: list) -> dict:
    import urllib.request
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read())


def get_logs_chunk(url: str, contract: str, from_b: int, to_b: int) -> list:
    res = rpc(url, "eth_getLogs", [{
        "address": contract,
        "topics":  [TRANSFER_TOPIC],
        "fromBlock": hex(from_b),
        "toBlock":   hex(to_b),
    }])
    if "error" in res:
        raise RuntimeError(f"eth_getLogs error at blocks {from_b}-{to_b}: {res['error']}")
    return res.get("result", [])


def decode_amount(data: str) -> int:
    if data.startswith("0x"):
        data = data[2:]
    # First 32-byte word is the uint256 amount for ERC-20 Transfer
    return int(data[:64], 16) if len(data) >= 64 else 0


def main() -> None:
    load_env()
    url = os.environ.get("ALCHEMY_BASE_URL")
    if not url:
        print("error: ALCHEMY_BASE_URL not set", file=sys.stderr)
        sys.exit(1)

    print(f"EURC Base convention scan")
    print(f"Contract: {EURC_BASE_CONTRACT}")
    print(f"Window:   blocks {FROM_BLOCK:,} → {TO_BLOCK:,} ({TO_BLOCK - FROM_BLOCK:,} blocks)")
    print(f"Chunk:    {CHUNK_SIZE:,} blocks per request")
    print()

    all_logs: list[dict] = []
    chunks = range(FROM_BLOCK, TO_BLOCK + 1, CHUNK_SIZE)
    total_chunks = (TO_BLOCK - FROM_BLOCK) // CHUNK_SIZE + 1

    for i, chunk_start in enumerate(chunks):
        chunk_end = min(chunk_start + CHUNK_SIZE - 1, TO_BLOCK)
        logs = get_logs_chunk(url, EURC_BASE_CONTRACT, chunk_start, chunk_end)
        all_logs.extend(logs)
        pct = (i + 1) / total_chunks * 100
        print(f"  chunk {i+1}/{total_chunks}: blocks {chunk_start:,}-{chunk_end:,} → {len(logs)} logs  [{pct:.0f}%]")

    print(f"\nTotal Transfer events in window: {len(all_logs):,}")

    # Classify
    mints = [l for l in all_logs if l["topics"][1] == ZERO_PAD]  # from=0x0
    burns = [l for l in all_logs if l["topics"][2] == ZERO_PAD]  # to=0x0

    # Non-zero-address large transfers (potential bridge issuance)
    # Look for transfers from/to a repeated address that isn't zero
    from_counts: dict[str, int] = {}
    to_counts: dict[str, int] = {}
    for l in all_logs:
        frm = l["topics"][1]
        to  = l["topics"][2]
        if frm != ZERO_PAD:
            from_counts[frm] = from_counts.get(frm, 0) + 1
        if to != ZERO_PAD:
            to_counts[to] = to_counts.get(to, 0) + 1

    # Top senders/recipients (heuristic for bridge/issuer pattern)
    top_senders = sorted(from_counts.items(), key=lambda x: -x[1])[:5]
    top_recipients = sorted(to_counts.items(), key=lambda x: -x[1])[:5]

    # Mint amount total
    total_minted = sum(decode_amount(l["data"]) for l in mints) / 1e6
    total_burned = sum(decode_amount(l["data"]) for l in burns) / 1e6

    # Determine verdict
    if mints or burns:
        verdict = "confirmed_zero_address_convention"
    else:
        # Check if the volume is too low to be meaningful
        if len(all_logs) < 100:
            verdict = "no_mint_burn_seen_expand_scan"
        else:
            # High volume but no zero-address events — likely bridge mechanism
            verdict = "nonstandard_or_bridge_mechanism_suspected"

    print(f"\nMint events (from=0x0): {len(mints)}")
    print(f"Burn events (to=0x0):  {len(burns)}")
    print(f"Verdict: {verdict}")

    # Write markdown report
    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    lines: list[str] = []
    lines.append(f"# EURC Base mint/burn convention scan\n")
    lines.append(f"generated: {now}  ")
    lines.append(f"contract: `{EURC_BASE_CONTRACT}`  ")
    lines.append(f"window: Base blocks {FROM_BLOCK:,} → {TO_BLOCK:,} (2026-05-13 → 2026-05-20)\n")
    lines.append(f"---\n")
    lines.append(f"## Verdict\n")
    lines.append(f"**{verdict}**\n")
    lines.append(f"---\n")
    lines.append(f"## Transfer event summary\n")
    lines.append(f"| Metric | Value |")
    lines.append(f"|--------|-------|")
    lines.append(f"| Total Transfer events | {len(all_logs):,} |")
    lines.append(f"| Zero-address mint events (from=0x0) | {len(mints)} |")
    lines.append(f"| Zero-address burn events (to=0x0) | {len(burns)} |")
    lines.append(f"| Total minted (EURC, 6 dec) | {total_minted:,.2f} |")
    lines.append(f"| Total burned (EURC, 6 dec) | {total_burned:,.2f} |")
    lines.append(f"| Blocks scanned | {TO_BLOCK - FROM_BLOCK:,} |")
    lines.append(f"| Chunk size | {CHUNK_SIZE:,} |")
    lines.append("")

    lines.append(f"## Top 5 senders (non-zero)\n")
    lines.append(f"| Address (padded) | Transfer count |")
    lines.append(f"|------------------|---------------|")
    for addr, cnt in top_senders:
        short = "0x" + addr[-40:] if len(addr) > 42 else addr
        lines.append(f"| `{short}` | {cnt} |")
    lines.append("")

    lines.append(f"## Top 5 recipients (non-zero)\n")
    lines.append(f"| Address (padded) | Transfer count |")
    lines.append(f"|------------------|---------------|")
    for addr, cnt in top_recipients:
        short = "0x" + addr[-40:] if len(addr) > 42 else addr
        lines.append(f"| `{short}` | {cnt} |")
    lines.append("")

    lines.append(f"## Interpretation\n")
    if verdict == "confirmed_zero_address_convention":
        lines.append(
            f"Zero-address Transfer events were found in the canonical window. "
            f"EURC Base uses the same mint/burn pattern as USDC and XSGD. "
            f"A full supply-invariant audit can proceed using the existing framework."
        )
    elif verdict == "nonstandard_or_bridge_mechanism_suspected":
        lines.append(
            f"No zero-address Transfer events found despite {len(all_logs):,} total transfer events "
            f"in a {TO_BLOCK - FROM_BLOCK:,}-block window. This strongly suggests EURC on Base "
            f"does not use the zero-address mint/burn pattern. Likely mechanisms:"
        )
        lines.append(f"")
        lines.append(f"1. Bridge-based issuance: mints occur via a bridge contract that does not emit "
                     f"standard Transfer(from=0x0) events, or uses a different event schema.")
        lines.append(f"2. Custodial issuance model: supply changes are recorded differently on Base "
                     f"compared to Ethereum.")
        lines.append(f"")
        lines.append(f"**Do not run full audit on EURC Base without schema investigation.** "
                     f"The supply invariant check relies on zero-address counting and will produce "
                     f"incorrect results if the convention is nonstandard.")
    else:
        lines.append(
            f"No mint or burn events found, and transfer volume is low. "
            f"Try a longer block range or a different window with known issuance activity."
        )

    if mints:
        lines.append(f"\n## Sample mint events\n")
        for m in mints[:5]:
            lines.append(f"- block {int(m['blockNumber'], 16):,}, tx {m['transactionHash'][:18]}..., "
                         f"amount {decode_amount(m['data']) / 1e6:,.2f} EURC")

    if burns:
        lines.append(f"\n## Sample burn events\n")
        for b in burns[:5]:
            lines.append(f"- block {int(b['blockNumber'], 16):,}, tx {b['transactionHash'][:18]}..., "
                         f"amount {decode_amount(b['data']) / 1e6:,.2f} EURC")

    OUT_MD.parent.mkdir(parents=True, exist_ok=True)
    OUT_MD.write_text("\n".join(lines) + "\n")
    print(f"\nReport written to: {OUT_MD}")


if __name__ == "__main__":
    main()
