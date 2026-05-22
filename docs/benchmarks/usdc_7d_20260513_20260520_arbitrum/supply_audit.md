# USDC supply audit

**Run id:** `usdc_7d_20260513_20260520_arbitrum`

**Generated:** 2026-05-21T06:56:54.362950+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **arbitrum:** blocks `462214505` → `464624919` (resolved end Some(464624919))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### arbitrum

- **Resolved to block:** 464624919
- **Contract:** `0xaf88d065e77c8cC2239327C5EDb3A432268e5831`
- **Transfer events (deduped):** 6530670
- **Active senders / recipients (window):** 350263 / 337937
- **Mints / burns / plain transfers:** 15514 / 14453 / 6500703
- **Sum mints / burns (raw):** 113454974907023 / 165654360630663
- **totalSupply @ start−1:** 5809683743.959007  _( on-chain )_
- **totalSupply @ end:** 5830319371.813860
- **On-chain Δ / net mint / discrepancy (raw int):** 20635627854853 / -52199385723640 / -72835013578493

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | FAIL |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
