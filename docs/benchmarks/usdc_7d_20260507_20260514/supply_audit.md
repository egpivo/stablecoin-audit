# USDC supply audit

**Run id:** `usdc_7d_20260507_20260514`

**Generated:** 2026-05-20T07:10:57.220330+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **arbitrum:** blocks `460146890` → `462559767` (resolved end Some(462559767))
- **base:** blocks `45661327` → `45963726` (resolved end Some(45963726))
- **ethereum:** blocks `25039433` → `25089644` (resolved end Some(25089644))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### arbitrum

- **Resolved to block:** 462559767
- **Contract:** `0xaf88d065e77c8cC2239327C5EDb3A432268e5831`
- **Transfer events (deduped):** 5541815
- **Active senders / recipients (window):** 338067 / 350343
- **Mints / burns / plain transfers:** 17368 / 13857 / 5510590
- **Sum mints / burns (raw):** 522149861064753 / 456055833884711
- **totalSupply @ start−1:** 5737236012.740680  _( on-chain )_
- **totalSupply @ end:** 5803330039.920722
- **On-chain Δ / net mint / discrepancy (raw int):** 66094027180042 / 66094027180042 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### base

- **Resolved to block:** 45963726
- **Contract:** `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`
- **Transfer events (deduped):** 17927213
- **Active senders / recipients (window):** 932023 / 1136186
- **Mints / burns / plain transfers:** 24205 / 17684 / 17885324
- **Sum mints / burns (raw):** 374677084879432 / 509105340427007
- **totalSupply @ start−1:** 4442069491.116404  _( on-chain )_
- **totalSupply @ end:** 4307641235.568829
- **On-chain Δ / net mint / discrepancy (raw int):** -134428255547575 / -134428255547575 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### ethereum

- **Resolved to block:** 25089644
- **Contract:** `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`
- **Transfer events (deduped):** 3472521
- **Active senders / recipients (window):** 465587 / 574799
- **Mints / burns / plain transfers:** 14854 / 11436 / 3446231
- **Sum mints / burns (raw):** 3565710284902464 / 4429677606395107
- **totalSupply @ start−1:** 54992600294.163545  _( on-chain )_
- **totalSupply @ end:** 54128632972.670902
- **On-chain Δ / net mint / discrepancy (raw int):** -863967321492643 / -863967321492643 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
