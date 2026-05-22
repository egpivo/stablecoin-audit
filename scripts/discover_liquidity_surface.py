#!/usr/bin/env python3
"""
Discover DEX pools and compute pair-dependence metrics for XSGD, EURC, and USDC.

Source: DexScreener public API (no API key required).
Snapshot time: script run timestamp; results are point-in-time.

Outputs (all under data/benchmarks/):
  stablecoin_liquidity_pairs.csv          — raw pool inventory
  stablecoin_pair_dependence_summary.csv  — aggregated share metrics
  stablecoin_route_dependence.csv         — primary route to ETH or USD
  geo_stablecoin_surface_summary.csv      — joined with audit surface

Claim boundary: descriptive only. Liquidity data is a point-in-time snapshot.
No causal claims. No adoption ranking.
"""
from __future__ import annotations

import csv
import json
import time
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

# ---------------------------------------------------------------------------
# Token definitions
# ---------------------------------------------------------------------------

TOKENS: list[dict] = [
    dict(asset="XSGD", chain="base",     ds_chain="base",
         address="0x0a4c9cb2778ab3302996a34befcf9a8bc288c33b"),
    dict(asset="XSGD", chain="polygon",  ds_chain="polygon",
         address="0xdc3326e71d45186f113a2f448984ca0e8d201995"),
    dict(asset="EURC", chain="ethereum", ds_chain="ethereum",
         address="0x1abaEA1f7C830bD89Acc67eC4af516284b1bC33c"),
    dict(asset="EURC", chain="base",     ds_chain="base",
         address="0x60a3e35cc302bfa44cb288bc5a4f316fdb1adb42"),
    dict(asset="USDC", chain="ethereum", ds_chain="ethereum",
         address="0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
    dict(asset="USDC", chain="base",     ds_chain="base",
         address="0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
    dict(asset="USDC", chain="arbitrum", ds_chain="arbitrum",
         address="0xaf88d065e77c8cc2239327c5edb3a432268e5831"),
]

# Known USD stablecoins (lowercase addresses)
USD_STABLES: set[str] = {
    # USDC
    "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",   # eth
    "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",   # base
    "0x2791bca1f2de4661ed88a30c99a7a9449aa84174",   # polygon (USDC.e)
    "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359",   # polygon (native)
    "0xaf88d065e77c8cc2239327c5edb3a432268e5831",   # arbitrum (native USDC)
    "0xff970a61a04b1ca14834a43f5de4533ebddb5cc8",   # arbitrum (USDC.e — bridged, legacy)
    "0x0b2c639c533813f4aa9d7837caf62653d097ff85",   # optimism
    # DAI
    "0x6b175474e89094c44da98b954eedeac495271d0f",   # eth
    "0x50c5725949a6f0c72e6c4a641f24049a917db0cb",   # base
    "0x8f3cf7ad23cd3cadbd9735aff958023239c6a063",   # polygon
    "0xda10009cbd5d07dd0cecc66161fc93d7c9000da1",   # arbitrum
    # USDT
    "0xdac17f958d2ee523a2206206994597c13d831ec7",   # eth
    "0xfde4c96c8593536e31f229ea8f37b2ada2699bb2",   # base
    "0xc2132d05d31c914a87c6611c10748aeb04b58e8f",   # polygon
    "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9",   # arbitrum
    # DAI / sDAI
    "0x6b175474e89094c44da98b954eedeac495271d0f",   # eth
    "0x50c5725949a6f0c72e6c4a641f24049a917db0cb",   # base
    "0x8f3cf7ad23cd3cadbd9735aff958023239c6a063",   # polygon
    # FRAX
    "0x853d955acef822db058eb8505911ed77f175b99e",   # eth
    # crvUSD
    "0xf939e0a03fb07f59a73314e73794be0e57ac1b4e",   # eth
    # PYUSD
    "0x6c3ea9036406852006290770bedfcaba0e23a0e8",   # eth
    # GHO
    "0x40d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f",   # eth
    # USDBC (bridged USDC on Base — deprecated but may appear)
    "0xd9aaec86b65d86f6a7b5b1b0c42ffa531710b6ca",   # base
}

USDC_ADDRS: set[str] = {
    "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
    "0x2791bca1f2de4661ed88a30c99a7a9449aa84174",
    "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359",
    "0xaf88d065e77c8cc2239327c5edb3a432268e5831",
    "0xff970a61a04b1ca14834a43f5de4533ebddb5cc8",   # USDC.e arbitrum
    "0x0b2c639c533813f4aa9d7837caf62653d097ff85",
    "0xd9aaec86b65d86f6a7b5b1b0c42ffa531710b6ca",
}

USDT_ADDRS: set[str] = {
    "0xdac17f958d2ee523a2206206994597c13d831ec7",
    "0xfde4c96c8593536e31f229ea8f37b2ada2699bb2",
    "0xc2132d05d31c914a87c6611c10748aeb04b58e8f",
    "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9",
}

WETH_ADDRS: set[str] = {
    # WETH
    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",   # eth mainnet
    "0x4200000000000000000000000000000000000006",   # base / op
    "0x7ceb23fd6bc0add59e62ac25578270cff1b9f619",   # polygon
    "0x82af49447d8a07e3bd95bd0d56f35241523fbab1",   # arbitrum
    "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d",   # gnosis
    # Native ETH as represented by DexScreener (zero-address or EEE-address)
    "0x0000000000000000000000000000000000000000",
    "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
    # Liquid staking tokens (ETH-denominated)
    "0xae7ab96520de3a18e5e111b5eaab095312d7fe84",   # stETH eth
    "0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0",   # wstETH eth
    "0xc1cba3fcea344f92d9239c08c0568f6f2f0ee452",   # wstETH base
    "0x5979d7b546e38e414f7e9822514be443a4800529",   # wstETH arbitrum
    "0x03b54a6e9a984069379fae1a4fc4dbae93b3bccd",   # wstETH polygon
    "0xba5ddd1f9d7f570dc94a51479a000e3bce967196",   # cbETH (coinbase staked ETH)
    "0x2ae3f1ec7f1f5012cfeab0185bfc7aa3cf0dec22",   # cbETH base
}

# EUR-pegged stablecoin detection: use symbol heuristic rather than addresses.
# On-chain addresses for non-USDC tokens are hard to verify without a registry;
# symbol matching on DexScreener data is reliable enough for classification.
EUR_STABLE_SYMBOLS: set[str] = {
    "EURA", "EURCV", "EUROP", "EUR0", "EURe", "EUR",
    "EURQ", "ZCHF", "EURS", "EURt", "jEUR", "agEUR",
    "EURT", "CEUR", "sEUR",
}
# Keep EUR_STABLES as empty set — classification done via symbol below.
EUR_STABLES: set[str] = set()

BTC_ADDRS: set[str] = {
    "0xcbb7c0000ab88b473b1f5afd9ef808440eed33bf",   # cbBTC (Coinbase BTC) base
    "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599",   # WBTC eth
    "0x1bfd67037b42cf73acf2047067bd4f2c47d9bfd6",   # WBTC polygon
    "0x68f180fcce6836688e9084f035309e29bf0a2095",   # WBTC optimism
    "0x03c7054bcb39f7b2e5b2c7acb37583e32d70cfa3",   # WBTC base (bridged)
}

OUT_DIR      = Path("data/benchmarks")
AUDIT_PANEL  = Path("data/benchmarks/cross_asset_geo_panel_summary.csv")
DS_BASE_URL  = "https://api.dexscreener.com/latest/dex/tokens/{address}"

PAIRS_CSV        = OUT_DIR / "stablecoin_liquidity_pairs.csv"
DEPENDENCE_CSV   = OUT_DIR / "stablecoin_pair_dependence_summary.csv"
ROUTE_CSV        = OUT_DIR / "stablecoin_route_dependence.csv"
SURFACE_CSV      = OUT_DIR / "geo_stablecoin_surface_summary.csv"

SNAPSHOT_UTC = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

MIN_LIQUIDITY_USD = 100.0   # filter out dust pools (< $100 TVL)

# ---------------------------------------------------------------------------
# DexScreener fetch
# ---------------------------------------------------------------------------

def fetch_dexscreener(address: str) -> list[dict]:
    url = DS_BASE_URL.format(address=address)
    req = urllib.request.Request(url, headers={"User-Agent": "stablecoin-audit/1.0"})
    try:
        with urllib.request.urlopen(req, timeout=15) as r:
            data = json.loads(r.read())
        return data.get("pairs") or []
    except Exception as e:
        print(f"  WARNING: DexScreener fetch failed for {address}: {e}")
        return []


def safe_float(v) -> float:
    try:
        return float(v) if v is not None else 0.0
    except (TypeError, ValueError):
        return 0.0


def classify_symbol(addr: str, symbol: str) -> str:
    a = addr.lower()
    if a in USDC_ADDRS:                    return "USDC"
    if a in USDT_ADDRS:                    return "USDT"
    if a in USD_STABLES:                   return "USD_STABLE"
    if a in WETH_ADDRS:                    return "WETH_ETH"
    if a in BTC_ADDRS:                     return "BTC"
    if symbol in EUR_STABLE_SYMBOLS:       return "EUR_STABLE"
    # Broad EUR heuristic: symbol starts with EUR or contains EUR (not EURC — that's our asset)
    sym_upper = symbol.upper()
    if sym_upper.startswith("EUR") and sym_upper != "EURC": return "EUR_STABLE"
    return symbol


# ---------------------------------------------------------------------------
# Task 1: pool inventory
# ---------------------------------------------------------------------------

PAIRS_FIELDS = [
    "asset", "chain", "snapshot_utc", "pool_address", "dex",
    "token0", "token1", "token0_symbol", "token1_symbol",
    "reserve_usd", "volume_24h_usd",
    "is_usd_stable_pair", "is_usdc_pair", "is_usdt_pair",
    "is_weth_pair", "is_eur_stable_pair", "is_btc_pair",
    "counterpart_class", "source",
]


def build_pair_rows(token: dict, ds_pairs: list[dict]) -> list[dict]:
    asset    = token["asset"]
    chain    = token["chain"]
    ds_chain = token["ds_chain"]
    own_addr = token["address"].lower()

    rows = []
    seen_pools: set[str] = set()

    for p in ds_pairs:
        if p.get("chainId") != ds_chain:
            continue

        pool_addr = (p.get("pairAddress") or "").lower()
        if not pool_addr or pool_addr in seen_pools:
            continue
        seen_pools.add(pool_addr)

        liq_usd  = safe_float((p.get("liquidity") or {}).get("usd"))
        vol_24h  = safe_float((p.get("volume") or {}).get("h24"))

        if liq_usd < MIN_LIQUIDITY_USD:
            continue

        base_addr  = (p["baseToken"]["address"]).lower()
        quote_addr = (p["quoteToken"]["address"]).lower()
        base_sym   = p["baseToken"]["symbol"]
        quote_sym  = p["quoteToken"]["symbol"]

        # Determine which slot is our asset, which is the counterpart
        if base_addr == own_addr:
            t0, t1 = base_addr, quote_addr
            s0, s1 = base_sym,  quote_sym
            cp_addr, cp_sym = quote_addr, quote_sym
        elif quote_addr == own_addr:
            t0, t1 = quote_addr, base_addr
            s0, s1 = quote_sym,  base_sym
            cp_addr, cp_sym = base_addr, base_sym
        else:
            # Pool doesn't actually contain our token — skip
            continue

        cp_class = classify_symbol(cp_addr, cp_sym)
        is_usd  = cp_addr in USD_STABLES
        is_usdc = cp_addr in USDC_ADDRS
        is_usdt = cp_addr in USDT_ADDRS
        is_weth = cp_addr in WETH_ADDRS
        is_eur  = cp_class == "EUR_STABLE"
        is_btc  = cp_addr.lower() in BTC_ADDRS

        rows.append({
            "asset":              asset,
            "chain":              chain,
            "snapshot_utc":       SNAPSHOT_UTC,
            "pool_address":       pool_addr,
            "dex":                p.get("dexId", ""),
            "token0":             t0,
            "token1":             t1,
            "token0_symbol":      s0,
            "token1_symbol":      s1,
            "reserve_usd":        round(liq_usd, 2),
            "volume_24h_usd":     round(vol_24h, 2),
            "is_usd_stable_pair": "true" if is_usd  else "false",
            "is_usdc_pair":       "true" if is_usdc else "false",
            "is_usdt_pair":       "true" if is_usdt else "false",
            "is_weth_pair":       "true" if is_weth else "false",
            "is_eur_stable_pair": "true" if is_eur  else "false",
            "is_btc_pair":        "true" if is_btc  else "false",
            "counterpart_class":  cp_class,
            "source":             "dexscreener",
        })

    return rows


# ---------------------------------------------------------------------------
# Task 2: pair-dependence summary
# ---------------------------------------------------------------------------

DEPENDENCE_FIELDS = [
    "asset", "chain", "snapshot_utc",
    "pool_count",
    "total_liquidity_usd",
    "usd_stable_liquidity_usd", "usdc_liquidity_usd", "usdt_liquidity_usd",
    "weth_liquidity_usd", "eur_stable_liquidity_usd", "btc_liquidity_usd",
    "other_liquidity_usd",
    "usd_stable_share", "usdc_share", "usdt_share",
    "weth_share", "eur_stable_share", "btc_share", "other_share",
    "top_pair", "top_pair_liquidity_usd", "top_pair_share",
    "notes",
]


def build_dependence_rows(all_pairs: list[dict]) -> list[dict]:
    from collections import defaultdict
    groups: dict[tuple, list[dict]] = defaultdict(list)
    for row in all_pairs:
        groups[(row["asset"], row["chain"])].append(row)

    out = []
    for (asset, chain), rows in sorted(groups.items()):
        total    = sum(r["reserve_usd"] for r in rows)
        usd_liq  = sum(r["reserve_usd"] for r in rows if r["is_usd_stable_pair"] == "true")
        usdc_liq = sum(r["reserve_usd"] for r in rows if r["is_usdc_pair"] == "true")
        usdt_liq = sum(r["reserve_usd"] for r in rows if r["is_usdt_pair"] == "true")
        weth_liq = sum(r["reserve_usd"] for r in rows if r["is_weth_pair"] == "true")
        eur_liq  = sum(r["reserve_usd"] for r in rows if r["is_eur_stable_pair"] == "true")
        btc_liq  = sum(r["reserve_usd"] for r in rows if r["is_btc_pair"] == "true")
        other_liq = total - usd_liq - weth_liq - eur_liq - btc_liq

        def share(v): return round(v / total, 4) if total > 0 else 0.0

        top = max(rows, key=lambda r: r["reserve_usd"])
        top_label = f"{top['token0_symbol']}/{top['token1_symbol']} ({top['dex']})"

        notes_parts = []
        if total == 0:
            notes_parts.append("no pools above threshold")
        if asset == "USDC":
            notes_parts.append("USDC is the reference USD rail; pair-dependence metric not analytically meaningful for this asset")

        out.append({
            "asset":                    asset,
            "chain":                    chain,
            "snapshot_utc":             SNAPSHOT_UTC,
            "pool_count":               len(rows),
            "total_liquidity_usd":      round(total, 2),
            "usd_stable_liquidity_usd": round(usd_liq, 2),
            "usdc_liquidity_usd":       round(usdc_liq, 2),
            "usdt_liquidity_usd":       round(usdt_liq, 2),
            "weth_liquidity_usd":       round(weth_liq, 2),
            "eur_stable_liquidity_usd": round(eur_liq, 2),
            "btc_liquidity_usd":        round(btc_liq, 2),
            "other_liquidity_usd":      round(other_liq, 2),
            "usd_stable_share":         share(usd_liq),
            "usdc_share":               share(usdc_liq),
            "usdt_share":               share(usdt_liq),
            "weth_share":               share(weth_liq),
            "eur_stable_share":         share(eur_liq),
            "btc_share":                share(btc_liq),
            "other_share":              share(other_liq),
            "top_pair":                 top_label,
            "top_pair_liquidity_usd":   round(top["reserve_usd"], 2),
            "top_pair_share":           share(top["reserve_usd"]),
            "notes":                    "; ".join(notes_parts),
        })

    return out


# ---------------------------------------------------------------------------
# Task 3: route dependence
# ---------------------------------------------------------------------------

ROUTE_FIELDS = [
    "asset", "chain", "snapshot_utc",
    "route_target", "best_route", "hop_count",
    "contains_usdc", "contains_usdt", "contains_weth",
    "route_liquidity_proxy_usd",
    "notes",
]

ROUTE_TARGETS = ["USDC", "WETH/ETH"]


def build_route_rows(all_pairs: list[dict]) -> list[dict]:
    from collections import defaultdict
    groups: dict[tuple, list[dict]] = defaultdict(list)
    for row in all_pairs:
        groups[(row["asset"], row["chain"])].append(row)

    out = []
    for (asset, chain), rows in sorted(groups.items()):
        # Direct routes: pools the asset already trades in
        direct_usdc = [r for r in rows if r["is_usdc_pair"] == "true"]
        direct_usdt = [r for r in rows if r["is_usdt_pair"] == "true"]
        direct_weth = [r for r in rows if r["is_weth_pair"] == "true" or r["counterpart_class"] == "WETH_ETH"]

        # Route to USDC
        if direct_usdc:
            best = max(direct_usdc, key=lambda r: r["reserve_usd"])
            liq  = best["reserve_usd"]
            route_str = f"{asset} → USDC ({best['dex']}, ${liq:,.0f})"
            out.append({
                "asset": asset, "chain": chain, "snapshot_utc": SNAPSHOT_UTC,
                "route_target": "USDC",
                "best_route": route_str,
                "hop_count": 1,
                "contains_usdc": "true",
                "contains_usdt": "false",
                "contains_weth": "false",
                "route_liquidity_proxy_usd": round(liq, 2),
                "notes": "direct pool",
            })
        elif direct_weth:
            # 2-hop: asset → WETH → USDC
            best_weth = max(direct_weth, key=lambda r: r["reserve_usd"])
            liq_proxy = best_weth["reserve_usd"]   # conservative: bottleneck is asset/WETH leg
            route_str = f"{asset} → WETH ({best_weth['dex']}) → USDC (2-hop)"
            out.append({
                "asset": asset, "chain": chain, "snapshot_utc": SNAPSHOT_UTC,
                "route_target": "USDC",
                "best_route": route_str,
                "hop_count": 2,
                "contains_usdc": "true",
                "contains_usdt": "false",
                "contains_weth": "true",
                "route_liquidity_proxy_usd": round(liq_proxy, 2),
                "notes": "no direct USDC pool; routed via WETH",
            })
        else:
            out.append({
                "asset": asset, "chain": chain, "snapshot_utc": SNAPSHOT_UTC,
                "route_target": "USDC",
                "best_route": "not_found",
                "hop_count": 0,
                "contains_usdc": "false",
                "contains_usdt": "false",
                "contains_weth": "false",
                "route_liquidity_proxy_usd": 0,
                "notes": "no USDC or WETH pool above threshold",
            })

        # Route to WETH/ETH
        if direct_weth:
            best = max(direct_weth, key=lambda r: r["reserve_usd"])
            liq  = best["reserve_usd"]
            route_str = f"{asset} → WETH ({best['dex']}, ${liq:,.0f})"
            out.append({
                "asset": asset, "chain": chain, "snapshot_utc": SNAPSHOT_UTC,
                "route_target": "WETH/ETH",
                "best_route": route_str,
                "hop_count": 1,
                "contains_usdc": "false",
                "contains_usdt": "false",
                "contains_weth": "true",
                "route_liquidity_proxy_usd": round(liq, 2),
                "notes": "direct pool",
            })
        elif direct_usdc:
            best_usdc = max(direct_usdc, key=lambda r: r["reserve_usd"])
            liq_proxy = best_usdc["reserve_usd"]
            route_str = f"{asset} → USDC ({best_usdc['dex']}) → WETH (2-hop)"
            out.append({
                "asset": asset, "chain": chain, "snapshot_utc": SNAPSHOT_UTC,
                "route_target": "WETH/ETH",
                "best_route": route_str,
                "hop_count": 2,
                "contains_usdc": "true",
                "contains_usdt": "false",
                "contains_weth": "true",
                "route_liquidity_proxy_usd": round(liq_proxy, 2),
                "notes": "no direct WETH pool; routed via USDC",
            })
        else:
            out.append({
                "asset": asset, "chain": chain, "snapshot_utc": SNAPSHOT_UTC,
                "route_target": "WETH/ETH",
                "best_route": "not_found",
                "hop_count": 0,
                "contains_usdc": "false",
                "contains_usdt": "false",
                "contains_weth": "false",
                "route_liquidity_proxy_usd": 0,
                "notes": "no WETH or USDC pool above threshold",
            })

    return out


# ---------------------------------------------------------------------------
# Task 4: combined surface summary
# ---------------------------------------------------------------------------

SURFACE_FIELDS = [
    "asset", "peg_currency", "chain",
    "window_id", "from_utc", "to_utc",
    "transfer_event_count", "mint_count", "burn_count",
    "gross_to_net_ratio", "supply_invariant_status",
    "total_liquidity_usd", "usd_stable_share", "usdc_share",
    "top_pair", "top_pair_share",
    "liquidity_snapshot_utc",
    "interpretation_note",
]

CANONICAL_WINDOW_FROM = "2026-05-13T00:00:00Z"


def build_surface_rows(
    dep_by_key: dict[tuple, dict],
) -> list[dict]:
    if not AUDIT_PANEL.exists():
        print(f"  WARNING: {AUDIT_PANEL} not found — surface summary will omit audit columns")
        return []

    with open(AUDIT_PANEL, newline="") as f:
        audit_rows = list(csv.DictReader(f))

    out = []
    for ar in audit_rows:
        asset = ar["asset"]
        chain = ar["chain"]
        key   = (asset, chain)

        dep = dep_by_key.get(key)

        total_liq     = dep["total_liquidity_usd"]    if dep else ""
        usd_share     = dep["usd_stable_share"]       if dep else ""
        usdc_share    = dep["usdc_share"]              if dep else ""
        top_pair      = dep["top_pair"]                if dep else ""
        top_pair_sh   = dep["top_pair_share"]          if dep else ""
        snap_utc      = dep["snapshot_utc"]            if dep else ""

        # Interpretation note — purely descriptive
        note = ""
        if dep and asset != "USDC":
            us = float(usd_share) if usd_share != "" else 0.0
            uc = float(usdc_share) if usdc_share != "" else 0.0
            tl = float(total_liq) if total_liq != "" else 0.0
            if tl == 0:
                note = "no liquidity pools found above threshold"
            elif us >= 0.80:
                note = (
                    f"observed liquidity is predominantly USD-stablecoin-paired "
                    f"({us*100:.0f}% of ${tl:,.0f} total); "
                    f"direct USDC share {uc*100:.0f}%. "
                    f"Descriptive only; does not imply reserve quality or usage ranking."
                )
            elif us >= 0.40:
                note = (
                    f"USD-stablecoin pools account for {us*100:.0f}% of ${tl:,.0f} observed liquidity; "
                    f"non-USD pools (WETH or local pairs) provide the remainder. "
                    f"Descriptive only."
                )
            else:
                note = (
                    f"USD-stablecoin pools account for {us*100:.0f}% of ${tl:,.0f} observed liquidity; "
                    f"non-USD pools dominate. Descriptive only."
                )

        gtn = ar.get("gross_to_net_ratio", "")

        out.append({
            "asset":                  asset,
            "peg_currency":           ar.get("peg_currency", ""),
            "chain":                  chain,
            "window_id":              ar.get("window_id", ""),
            "from_utc":               ar.get("from_utc", ""),
            "to_utc":                 ar.get("to_utc", ""),
            "transfer_event_count":   ar.get("transfer_event_count", ""),
            "mint_count":             ar.get("mint_count", ""),
            "burn_count":             ar.get("burn_count", ""),
            "gross_to_net_ratio":     gtn,
            "supply_invariant_status": ar.get("supply_invariant_status", ""),
            "total_liquidity_usd":    total_liq,
            "usd_stable_share":       usd_share,
            "usdc_share":             usdc_share,
            "top_pair":               top_pair,
            "top_pair_share":         top_pair_sh,
            "liquidity_snapshot_utc": snap_utc,
            "interpretation_note":    note,
        })

    return out


# ---------------------------------------------------------------------------
# Write helpers
# ---------------------------------------------------------------------------

def write_csv(path: Path, fieldnames: list[str], rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames, extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)
    print(f"  → {path}  ({len(rows)} rows)")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    print(f"Snapshot: {SNAPSHOT_UTC}")
    print(f"Threshold: pools with < ${MIN_LIQUIDITY_USD:.0f} TVL excluded\n")

    # --- Fetch all pools ---
    all_pair_rows: list[dict] = []

    for token in TOKENS:
        asset, chain = token["asset"], token["chain"]
        print(f"Fetching {asset} on {chain} ({token['address'][:10]}…)")
        ds_pairs = fetch_dexscreener(token["address"])
        print(f"  DexScreener returned {len(ds_pairs)} raw pairs")
        rows = build_pair_rows(token, ds_pairs)
        print(f"  Kept {len(rows)} pools (liq ≥ ${MIN_LIQUIDITY_USD:.0f})")
        all_pair_rows.extend(rows)
        time.sleep(0.4)   # be polite to the public API

    print(f"\nTotal pool rows: {len(all_pair_rows)}")

    # --- Task 1: write pairs ---
    print("\n[1] Writing liquidity pairs …")
    write_csv(PAIRS_CSV, PAIRS_FIELDS, all_pair_rows)

    # --- Task 2: dependence summary ---
    print("\n[2] Computing pair-dependence summary …")
    dep_rows = build_dependence_rows(all_pair_rows)
    write_csv(DEPENDENCE_CSV, DEPENDENCE_FIELDS, dep_rows)
    dep_by_key = {(r["asset"], r["chain"]): r for r in dep_rows}

    # Print quick digest
    for r in dep_rows:
        if r["asset"] != "USDC":
            print(
                f"  {r['asset']:4s} {r['chain']:8s}  "
                f"total=${float(r['total_liquidity_usd']):>10,.0f}  "
                f"usd_share={float(r['usd_stable_share'])*100:5.1f}%  "
                f"usdc_share={float(r['usdc_share'])*100:5.1f}%  "
                f"top={r['top_pair']}"
            )

    # --- Task 3: route dependence ---
    print("\n[3] Computing route dependence …")
    route_rows = build_route_rows(all_pair_rows)
    write_csv(ROUTE_CSV, ROUTE_FIELDS, route_rows)

    # --- Task 4: surface summary ---
    print("\n[4] Building geo surface summary …")
    surface_rows = build_surface_rows(dep_by_key)
    write_csv(SURFACE_CSV, SURFACE_FIELDS, surface_rows)

    print("\nDone.")


if __name__ == "__main__":
    main()
