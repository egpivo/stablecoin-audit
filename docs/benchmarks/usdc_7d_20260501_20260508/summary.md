# USDC — transfer-audit summary

**Run id:** `usdc_7d_20260501_20260508`

**Generated:** 2026-05-15T08:03:31.695921+00:00

**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). Block heights are chain-native and not numerically comparable across chains.

## Chain overview

| Chain | Chain ID | Contract | from → requested to | resolved end |
|-------|---------:|----------|--------------------:|-------------:|
| arbitrum | 42161 | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` | 458085624 → 460491249 | 460491249 |
| base | 8453 | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` | 45402127 → 45704526 | 45704526 |
| ethereum | 1 | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | 24996368 → 25046605 | 25046605 |

## Supply (window)

| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |
|-------|----------------------|-------------------|---------------------|
| arbitrum | 5526000173.740651 | 5742193181.040544 | 216193007299893 |
| base | 4457244601.347867 | 4416565673.317707 | -40678928030160 |
| ethereum | 54163511394.099501 | 55382617442.148830 | 1219106048049329 |

## Mint / burn / transfers (deduped)

| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |
|-------|----------:|------:|------:|------:|---------------:|
| arbitrum | 4866248 | 18899 | 13075 | 4834274 | 216193007299893 |
| base | 17208596 | 44948 | 17379 | 17146269 | -40678928030160 |
| ethereum | 3130346 | 15289 | 11357 | 3103700 | 1219106048049329 |

## QA gates

| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |
|-------|----------|-------------|--------|--------|------------|------------------|
| arbitrum | PASS | PASS | PASS | PASS | PASS | PASS |
| base | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | PASS | PASS | PASS | PASS | PASS | PASS |

---

> **Scope:** On-chain accounting in the declared block window(s) only. This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, or holder/identity attribution.

> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates can be read side-by-side for the same asset symbol.
