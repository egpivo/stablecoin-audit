# XSGD — transfer-audit summary

**Run id:** `xsgd_7d_20260513_20260520`

**Generated:** 2026-05-20T05:37:15.140366+00:00

**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). Block heights are chain-native and not numerically comparable across chains.

## Chain overview

| Chain | Chain ID | Contract | from → requested to | resolved end |
|-------|---------:|----------|--------------------:|-------------:|
| base | 8453 | `0x0A4C9cb2778aB3302996A34BeFCF9a8Bc288C33b` | 45920527 → 46222926 | 46222926 |

## Supply (window)

| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |
|-------|----------------------|-------------------|---------------------|
| base | 6617005.000000 | 6924558.000000 | 307553000000 |

## Mint / burn / transfers (deduped)

| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |
|-------|----------:|------:|------:|------:|---------------:|
| base | 15309 | 5 | 0 | 15304 | 307553000000 |

## QA gates

| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |
|-------|----------|-------------|--------|--------|------------|------------------|
| base | PASS | PASS | PASS | PASS | PASS | PASS |

---

> **Scope:** On-chain accounting in the declared block window(s) only. This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, or holder/identity attribution.

> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates can be read side-by-side for the same asset symbol.
