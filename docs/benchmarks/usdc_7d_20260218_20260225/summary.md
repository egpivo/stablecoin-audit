# USDC — transfer-audit summary

**Run id:** `usdc_7d_20260218_20260225`

**Generated:** 2026-05-20T00:44:15.801494+00:00

**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). Block heights are chain-native and not numerically comparable across chains.

## Chain overview

| Chain | Chain ID | Contract | from → requested to | resolved end |
|-------|---------:|----------|--------------------:|-------------:|
| arbitrum | 42161 | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` | 433213243 → 435639345 | 435639345 |
| base | 8453 | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` | 42291727 → 42594126 | 42594126 |
| ethereum | 1 | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | 24479995 → 24530203 | 24530203 |

## Supply (window)

| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |
|-------|----------------------|-------------------|---------------------|
| arbitrum | 6151824770.553508 | 6091094085.120981 | -60730685432527 |
| base | 4146522581.927930 | 4189261427.662630 | 42738845734700 |
| ethereum | 51450100080.521161 | 52811093515.542557 | 1360993435021396 |

## Mint / burn / transfers (deduped)

| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |
|-------|----------:|------:|------:|------:|---------------:|
| arbitrum | 7940173 | 15155 | 15738 | 7909280 | -60730685432527 |
| base | 23666689 | 17839 | 10979 | 23637871 | 42738845734700 |
| ethereum | 3551800 | 17371 | 10384 | 3524045 | 1360993435021396 |

## QA gates

| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |
|-------|----------|-------------|--------|--------|------------|------------------|
| arbitrum | PASS | PASS | PASS | PASS | PASS | PASS |
| base | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | PASS | PASS | PASS | PASS | PASS | PASS |

---

> **Scope:** On-chain accounting in the declared block window(s) only. This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, or holder/identity attribution.

> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates can be read side-by-side for the same asset symbol.
