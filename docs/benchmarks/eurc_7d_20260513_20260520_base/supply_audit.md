# EURC supply audit

**Run id:** `eurc_7d_20260513_20260520_base`

**Generated:** 2026-05-21T07:14:43.120443+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **base:** blocks `45920527` → `46222926` (resolved end Some(46222926))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### base

- **Resolved to block:** 46222926
- **Contract:** `0x60a3e35cc302bfa44cb288bc5a4f316fdb1adb42`
- **Transfer events (deduped):** 326692
- **Active senders / recipients (window):** 4369 / 5705
- **Mints / burns / plain transfers:** 128 / 57 / 326507
- **Sum mints / burns (raw):** 5041776800000 / 6503236340000
- **totalSupply @ start−1:** 51239145.980000  _( on-chain )_
- **totalSupply @ end:** 49777686.440000
- **On-chain Δ / net mint / discrepancy (raw int):** -1461459540000 / -1461459540000 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
