# USDC — transfer-audit summary

**Run id:** `usdc_7d_20241117_20241124`

**Generated:** 2026-05-19T18:36:23.252810+00:00

**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). Block heights are chain-native and not numerically comparable across chains.

## Chain overview

| Chain | Chain ID | Contract | from → requested to | resolved end |
|-------|---------:|----------|--------------------:|-------------:|
| arbitrum | 42161 | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` | 275231008 → 277637793 | 277637793 |
| base | 8453 | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` | 22506127 → 22808526 | 22808526 |
| ethereum | 1 | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | 21203704 → 21253879 | 21253879 |

## Supply (window)

| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |
|-------|----------------------|-------------------|---------------------|
| arbitrum | 1803072811.367846 | 1885762051.310776 | 82689239942930 |
| base | 3286402577.900278 | 3245920954.010536 | -40481623889742 |
| ethereum | 27325147672.137272 | 28595782977.431447 | 1270635305294175 |

## Mint / burn / transfers (deduped)

| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |
|-------|----------:|------:|------:|------:|---------------:|
| arbitrum | 4142603 | 7062 | 5958 | 4129583 | 82689239942930 |
| base | 5571685 | 8983 | 6415 | 5556287 | -40481623889742 |
| ethereum | 748862 | 4750 | 5100 | 739012 | 1270635305294175 |

## QA gates

| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |
|-------|----------|-------------|--------|--------|------------|------------------|
| arbitrum | PASS | PASS | PASS | PASS | PASS | PASS |
| base | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | PASS | PASS | PASS | PASS | PASS | PASS |

---

> **Scope:** On-chain accounting in the declared block window(s) only. This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, or holder/identity attribution.

> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates can be read side-by-side for the same asset symbol.
