# XSGD — transfer-audit summary

**Run id:** `xsgd_7d_20260513_20260520_polygon`

**Generated:** 2026-05-21T05:41:06.065772+00:00

**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). Block heights are chain-native and not numerically comparable across chains.

## Chain overview

| Chain | Chain ID | Contract | from → requested to | resolved end |
|-------|---------:|----------|--------------------:|-------------:|
| polygon | 137 | `0xDC3326e71D45186F113a2F448984CA0e8D201995` | 86794681 → 87140281 | 87140281 |

## Supply (window)

| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |
|-------|----------------------|-------------------|---------------------|
| polygon | 1850279.380000 | 2031179.380000 | 180900000000 |

## Mint / burn / transfers (deduped)

| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |
|-------|----------:|------:|------:|------:|---------------:|
| polygon | 2784 | 2 | 0 | 2782 | 180900000000 |

## QA gates

| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |
|-------|----------|-------------|--------|--------|------------|------------------|
| polygon | PASS | PASS | PASS | PASS | PASS | PASS |

---

> **Scope:** On-chain accounting in the declared block window(s) only. This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, or holder/identity attribution.

> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates can be read side-by-side for the same asset symbol.
