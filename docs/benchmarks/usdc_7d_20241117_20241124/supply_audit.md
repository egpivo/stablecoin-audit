# USDC supply audit

**Run id:** `usdc_7d_20241117_20241124`

**Generated:** 2026-05-19T18:36:23.252810+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **arbitrum:** blocks `275231008` → `277637793` (resolved end Some(277637793))
- **base:** blocks `22506127` → `22808526` (resolved end Some(22808526))
- **ethereum:** blocks `21203704` → `21253879` (resolved end Some(21253879))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### arbitrum

- **Resolved to block:** 277637793
- **Contract:** `0xaf88d065e77c8cC2239327C5EDb3A432268e5831`
- **Transfer events (deduped):** 4142603
- **Active senders / recipients (window):** 110305 / 129724
- **Mints / burns / plain transfers:** 7062 / 5958 / 4129583
- **Sum mints / burns (raw):** 244390209238759 / 161700969295829
- **totalSupply @ start−1:** 1803072811.367846  _( on-chain )_
- **totalSupply @ end:** 1885762051.310776
- **On-chain Δ / net mint / discrepancy (raw int):** 82689239942930 / 82689239942930 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### base

- **Resolved to block:** 22808526
- **Contract:** `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`
- **Transfer events (deduped):** 5571685
- **Active senders / recipients (window):** 218519 / 268817
- **Mints / burns / plain transfers:** 8983 / 6415 / 5556287
- **Sum mints / burns (raw):** 244769292856483 / 285250916746225
- **totalSupply @ start−1:** 3286402577.900278  _( on-chain )_
- **totalSupply @ end:** 3245920954.010536
- **On-chain Δ / net mint / discrepancy (raw int):** -40481623889742 / -40481623889742 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### ethereum

- **Resolved to block:** 21253879
- **Contract:** `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`
- **Transfer events (deduped):** 748862
- **Active senders / recipients (window):** 150205 / 167721
- **Mints / burns / plain transfers:** 4750 / 5100 / 739012
- **Sum mints / burns (raw):** 3131847086438594 / 1861211781144419
- **totalSupply @ start−1:** 27325147672.137272  _( on-chain )_
- **totalSupply @ end:** 28595782977.431447
- **On-chain Δ / net mint / discrepancy (raw int):** 1270635305294175 / 1270635305294175 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
