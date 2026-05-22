# USDC — transfer-audit summary

**Run id:** `usdc_7d_20260513_20260520_arbitrum`

**Generated:** 2026-05-21T06:56:54.362950+00:00

**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). Block heights are chain-native and not numerically comparable across chains.

## Chain overview

| Chain | Chain ID | Contract | from → requested to | resolved end |
|-------|---------:|----------|--------------------:|-------------:|
| arbitrum | 42161 | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` | 462214505 → 464624919 | 464624919 |

## Supply (window)

| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |
|-------|----------------------|-------------------|---------------------|
| arbitrum | 5809683743.959007 | 5830319371.813860 | 20635627854853 |

## Mint / burn / transfers (deduped)

| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |
|-------|----------:|------:|------:|------:|---------------:|
| arbitrum | 6530670 | 15514 | 14453 | 6500703 | -52199385723640 |

## QA gates

| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |
|-------|----------|-------------|--------|--------|------------|------------------|
| arbitrum | PASS | PASS | PASS | PASS | FAIL | PASS |

---

> **Scope:** On-chain accounting in the declared block window(s) only. This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, or holder/identity attribution.

> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates can be read side-by-side for the same asset symbol.
