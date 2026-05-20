# USDC supply audit

**Run id:** `usdc_7d_20260514_20260521`

**Generated:** 2026-05-19T12:32:36.420555+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **arbitrum:** blocks `462559764` → `464438118` (resolved end Some(464438118))
- **base:** blocks `45963727` → `46199501` (resolved end Some(46199501))
- **ethereum:** blocks `25089645` → `25128822` (resolved end Some(25128822))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### arbitrum

- **Resolved to block:** 464438118
- **Contract:** `0xaf88d065e77c8cC2239327C5EDb3A432268e5831`
- **Transfer events (deduped):** 5078139
- **Active senders / recipients (window):** 289023 / 277549
- **Mints / burns / plain transfers:** 11581 / 9063 / 5057495
- **Sum mints / burns (raw):** 422784380580231 / 346144742099862
- **totalSupply @ start−1:** 5803330039.920722  _( on-chain )_
- **totalSupply @ end:** 5879969678.401091
- **On-chain Δ / net mint / discrepancy (raw int):** 76639638480369 / 76639638480369 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### base

- **Resolved to block:** 46199501
- **Contract:** `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`
- **Transfer events (deduped):** 15679507
- **Active senders / recipients (window):** 741703 / 897329
- **Mints / burns / plain transfers:** 15604 / 13061 / 15650842
- **Sum mints / burns (raw):** 310918610325619 / 352779671646855
- **totalSupply @ start−1:** 4307641235.568829  _( on-chain )_
- **totalSupply @ end:** 4265780174.247593
- **On-chain Δ / net mint / discrepancy (raw int):** -41861061321236 / -41861061321236 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### ethereum

- **Resolved to block:** 25128822
- **Contract:** `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`
- **Transfer events (deduped):** 3692900
- **Active senders / recipients (window):** 520521 / 614786
- **Mints / burns / plain transfers:** 10525 / 8506 / 3673869
- **Sum mints / burns (raw):** 3061684036431389 / 3029935778117704
- **totalSupply @ start−1:** 54128632972.670902  _( on-chain )_
- **totalSupply @ end:** 54160381230.984587
- **On-chain Δ / net mint / discrepancy (raw int):** 31748258313685 / 31748258313685 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
