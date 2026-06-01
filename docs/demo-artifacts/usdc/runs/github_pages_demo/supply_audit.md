# USDC supply audit

**Run id:** `article_ui_demo`

**Generated:** 2026-06-01T11:38:57.375048+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **ethereum:** blocks `24000000` → `24000100` (resolved end Some(24000100))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### ethereum

- **Resolved to block:** 24000100
- **Contract:** `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`
- **Transfer events (deduped):** 4209
- **Active senders / recipients (window):** 1495 / 1972
- **Mints / burns / plain transfers:** 58 / 16 / 4135
- **Sum mints / burns (raw):** 3418175374101 / 72346729566
- **totalSupply @ start−1:** 52593428966.802640  _( on-chain )_
- **totalSupply @ end:** 52596774795.447175
- **On-chain Δ / net mint / discrepancy (raw int):** 3345828644535 / 3345828644535 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
