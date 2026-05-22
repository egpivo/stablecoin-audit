# XSGD supply audit

**Run id:** `xsgd_7d_20260513_20260520_polygon`

**Generated:** 2026-05-21T05:41:06.065772+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **polygon:** blocks `86794681` → `87140281` (resolved end Some(87140281))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### polygon

- **Resolved to block:** 87140281
- **Contract:** `0xDC3326e71D45186F113a2F448984CA0e8D201995`
- **Transfer events (deduped):** 2784
- **Active senders / recipients (window):** 348 / 360
- **Mints / burns / plain transfers:** 2 / 0 / 2782
- **Sum mints / burns (raw):** 180900000000 / 0
- **totalSupply @ start−1:** 1850279.380000  _( on-chain )_
- **totalSupply @ end:** 2031179.380000
- **On-chain Δ / net mint / discrepancy (raw int):** 180900000000 / 180900000000 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
