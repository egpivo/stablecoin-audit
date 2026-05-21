# USDC — transfer-audit summary

**Run id:** `usdc_7d_20260513_20260520_ethereum`

**Generated:** 2026-05-21T06:56:49.274322+00:00

**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). Block heights are chain-native and not numerically comparable across chains.

## Chain overview

| Chain | Chain ID | Contract | from → requested to | resolved end |
|-------|---------:|----------|--------------------:|-------------:|
| ethereum | 1 | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | 25082486 → 25132717 | 25132717 |

## Supply (window)

| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |
|-------|----------------------|-------------------|---------------------|
| ethereum | 54279532107.012549 | 54077507726.998096 | -202024380014453 |

## Mint / burn / transfers (deduped)

| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |
|-------|----------:|------:|------:|------:|---------------:|
| ethereum | 4718360 | 14267 | 11432 | 4692661 | -202024380014453 |

## QA gates

| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |
|-------|----------|-------------|--------|--------|------------|------------------|
| ethereum | PASS | PASS | PASS | PASS | PASS | PASS |

---

> **Scope:** On-chain accounting in the declared block window(s) only. This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, or holder/identity attribution.

> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates can be read side-by-side for the same asset symbol.
