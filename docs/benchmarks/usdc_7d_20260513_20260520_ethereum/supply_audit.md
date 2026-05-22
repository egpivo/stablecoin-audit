# USDC supply audit

**Run id:** `usdc_7d_20260513_20260520_ethereum`

**Generated:** 2026-05-21T06:56:49.274322+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **ethereum:** blocks `25082486` → `25132717` (resolved end Some(25132717))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### ethereum

- **Resolved to block:** 25132717
- **Contract:** `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`
- **Transfer events (deduped):** 4718360
- **Active senders / recipients (window):** 643144 / 759414
- **Mints / burns / plain transfers:** 14267 / 11432 / 4692661
- **Sum mints / burns (raw):** 4148307267646039 / 4350331647660492
- **totalSupply @ start−1:** 54279532107.012549  _( on-chain )_
- **totalSupply @ end:** 54077507726.998096
- **On-chain Δ / net mint / discrepancy (raw int):** -202024380014453 / -202024380014453 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
