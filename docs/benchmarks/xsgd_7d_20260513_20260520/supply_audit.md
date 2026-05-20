# XSGD supply audit

**Run id:** `xsgd_7d_20260513_20260520`

**Generated:** 2026-05-20T05:37:15.140366+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **base:** blocks `45920527` → `46222926` (resolved end Some(46222926))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### base

- **Resolved to block:** 46222926
- **Contract:** `0x0A4C9cb2778aB3302996A34BeFCF9a8Bc288C33b`
- **Transfer events (deduped):** 15309
- **Active senders / recipients (window):** 275 / 341
- **Mints / burns / plain transfers:** 5 / 0 / 15304
- **Sum mints / burns (raw):** 307553000000 / 0
- **totalSupply @ start−1:** 6617005.000000  _( on-chain )_
- **totalSupply @ end:** 6924558.000000
- **On-chain Δ / net mint / discrepancy (raw int):** 307553000000 / 307553000000 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
