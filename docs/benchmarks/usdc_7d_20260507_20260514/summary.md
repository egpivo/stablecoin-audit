# USDC — transfer-audit summary

**Run id:** `usdc_7d_20260507_20260514`

**Generated:** 2026-05-20T07:10:57.220330+00:00

**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). Block heights are chain-native and not numerically comparable across chains.

## Chain overview

| Chain | Chain ID | Contract | from → requested to | resolved end |
|-------|---------:|----------|--------------------:|-------------:|
| arbitrum | 42161 | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` | 460146890 → 462559767 | 462559767 |
| base | 8453 | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` | 45661327 → 45963726 | 45963726 |
| ethereum | 1 | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | 25039433 → 25089644 | 25089644 |

## Supply (window)

| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |
|-------|----------------------|-------------------|---------------------|
| arbitrum | 5737236012.740680 | 5803330039.920722 | 66094027180042 |
| base | 4442069491.116404 | 4307641235.568829 | -134428255547575 |
| ethereum | 54992600294.163545 | 54128632972.670902 | -863967321492643 |

## Mint / burn / transfers (deduped)

| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |
|-------|----------:|------:|------:|------:|---------------:|
| arbitrum | 5541815 | 17368 | 13857 | 5510590 | 66094027180042 |
| base | 17927213 | 24205 | 17684 | 17885324 | -134428255547575 |
| ethereum | 3472521 | 14854 | 11436 | 3446231 | -863967321492643 |

## QA gates

| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |
|-------|----------|-------------|--------|--------|------------|------------------|
| arbitrum | PASS | PASS | PASS | PASS | PASS | PASS |
| base | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | PASS | PASS | PASS | PASS | PASS | PASS |

---

> **Scope:** On-chain accounting in the declared block window(s) only. This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, or holder/identity attribution.

> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates can be read side-by-side for the same asset symbol.
