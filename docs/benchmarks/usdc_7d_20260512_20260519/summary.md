# USDC — transfer-audit summary

**Run id:** `usdc_7d_20260512_20260519`

**Generated:** 2026-05-19T15:00:15.756199+00:00

**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). Block heights are chain-native and not numerically comparable across chains.

## Chain overview

| Chain | Chain ID | Contract | from → requested to | resolved end |
|-------|---------:|----------|--------------------:|-------------:|
| arbitrum | 42161 | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` | 461870006 → 464280529 | 464280529 |
| base | 8453 | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` | 45877327 → 46179726 | 46179726 |
| ethereum | 1 | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | 25075306 → 25125536 | 25125536 |

## Supply (window)

| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |
|-------|----------------------|-------------------|---------------------|
| arbitrum | 5836906102.101342 | 5878440973.415026 | 41534871313684 |
| base | 4399414674.696996 | 4267373014.229275 | -132041660467721 |
| ethereum | 54422948307.650423 | 54168155155.073514 | -254793152576909 |

## Mint / burn / transfers (deduped)

| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |
|-------|----------:|------:|------:|------:|---------------:|
| arbitrum | 6477823 | 15646 | 11955 | 6450222 | 41534871313684 |
| base | 19850762 | 21557 | 17502 | 19811703 | -132041660467721 |
| ethereum | 4232919 | 14375 | 11314 | 4207230 | -254793152576909 |

## QA gates

| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |
|-------|----------|-------------|--------|--------|------------|------------------|
| arbitrum | PASS | PASS | PASS | PASS | PASS | PASS |
| base | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | PASS | PASS | PASS | PASS | PASS | PASS |

---

> **Scope:** On-chain accounting in the declared block window(s) only. This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, or holder/identity attribution.

> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates can be read side-by-side for the same asset symbol.
