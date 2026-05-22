# EURC — transfer-audit summary

**Run id:** `eurc_7d_20260513_20260520_ethereum`

**Generated:** 2026-05-21T07:01:20.306573+00:00

**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). Block heights are chain-native and not numerically comparable across chains.

## Chain overview

| Chain | Chain ID | Contract | from → requested to | resolved end |
|-------|---------:|----------|--------------------:|-------------:|
| ethereum | 1 | `0x1aBaEA1f7C830bD89Acc67eC4af516284b1bC33c` | 25082486 → 25132717 | 25132717 |

## Supply (window)

| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |
|-------|----------------------|-------------------|---------------------|
| ethereum | 275588974.310000 | 276192653.840000 | 603679530000 |

## Mint / burn / transfers (deduped)

| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |
|-------|----------:|------:|------:|------:|---------------:|
| ethereum | 12519 | 113 | 74 | 12332 | 603679530000 |

## QA gates

| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |
|-------|----------|-------------|--------|--------|------------|------------------|
| ethereum | PASS | PASS | PASS | PASS | PASS | PASS |

---

> **Scope:** On-chain accounting in the declared block window(s) only. This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, or holder/identity attribution.

> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates can be read side-by-side for the same asset symbol.
