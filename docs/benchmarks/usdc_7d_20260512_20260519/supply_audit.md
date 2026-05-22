# USDC supply audit

**Run id:** `usdc_7d_20260512_20260519`

**Generated:** 2026-05-19T15:00:15.756199+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **arbitrum:** blocks `461870006` → `464280529` (resolved end Some(464280529))
- **base:** blocks `45877327` → `46179726` (resolved end Some(46179726))
- **ethereum:** blocks `25075306` → `25125536` (resolved end Some(25125536))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### arbitrum

- **Resolved to block:** 464280529
- **Contract:** `0xaf88d065e77c8cC2239327C5EDb3A432268e5831`
- **Transfer events (deduped):** 6477823
- **Active senders / recipients (window):** 353705 / 340657
- **Mints / burns / plain transfers:** 15646 / 11955 / 6450222
- **Sum mints / burns (raw):** 533175599434336 / 491640728120652
- **totalSupply @ start−1:** 5836906102.101342  _( on-chain )_
- **totalSupply @ end:** 5878440973.415026
- **On-chain Δ / net mint / discrepancy (raw int):** 41534871313684 / 41534871313684 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### base

- **Resolved to block:** 46179726
- **Contract:** `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`
- **Transfer events (deduped):** 19850762
- **Active senders / recipients (window):** 907087 / 1103899
- **Mints / burns / plain transfers:** 21557 / 17502 / 19811703
- **Sum mints / burns (raw):** 413420218328740 / 545461878796461
- **totalSupply @ start−1:** 4399414674.696996  _( on-chain )_
- **totalSupply @ end:** 4267373014.229275
- **On-chain Δ / net mint / discrepancy (raw int):** -132041660467721 / -132041660467721 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### ethereum

- **Resolved to block:** 25125536
- **Contract:** `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`
- **Transfer events (deduped):** 4232919
- **Active senders / recipients (window):** 593860 / 713111
- **Mints / burns / plain transfers:** 14375 / 11314 / 4207230
- **Sum mints / burns (raw):** 4356713867622560 / 4611507020199469
- **totalSupply @ start−1:** 54422948307.650423  _( on-chain )_
- **totalSupply @ end:** 54168155155.073514
- **On-chain Δ / net mint / discrepancy (raw int):** -254793152576909 / -254793152576909 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
