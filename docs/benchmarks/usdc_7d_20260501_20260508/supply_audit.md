# USDC supply audit

**Run id:** `usdc_7d_20260501_20260508`

**Generated:** 2026-05-15T08:03:31.695921+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **arbitrum:** blocks `458085624` → `460491249` (resolved end Some(460491249))
- **base:** blocks `45402127` → `45704526` (resolved end Some(45704526))
- **ethereum:** blocks `24996368` → `25046605` (resolved end Some(25046605))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### arbitrum

- **Resolved to block:** 460491249
- **Contract:** `0xaf88d065e77c8cC2239327C5EDb3A432268e5831`
- **Transfer events (deduped):** 4866248
- **Active senders / recipients (window):** 287266 / 290038
- **Mints / burns / plain transfers:** 18899 / 13075 / 4834274
- **Sum mints / burns (raw):** 625735399308676 / 409542392008783
- **totalSupply @ start−1:** 5526000173.740651  _( on-chain )_
- **totalSupply @ end:** 5742193181.040544
- **On-chain Δ / net mint / discrepancy (raw int):** 216193007299893 / 216193007299893 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### base

- **Resolved to block:** 45704526
- **Contract:** `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`
- **Transfer events (deduped):** 17208596
- **Active senders / recipients (window):** 925834 / 1118818
- **Mints / burns / plain transfers:** 44948 / 17379 / 17146269
- **Sum mints / burns (raw):** 409839663285135 / 450518591315295
- **totalSupply @ start−1:** 4457244601.347867  _( on-chain )_
- **totalSupply @ end:** 4416565673.317707
- **On-chain Δ / net mint / discrepancy (raw int):** -40678928030160 / -40678928030160 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### ethereum

- **Resolved to block:** 25046605
- **Contract:** `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`
- **Transfer events (deduped):** 3130346
- **Active senders / recipients (window):** 485752 / 621495
- **Mints / burns / plain transfers:** 15289 / 11357 / 3103700
- **Sum mints / burns (raw):** 4744575419715288 / 3525469371665959
- **totalSupply @ start−1:** 54163511394.099501  _( on-chain )_
- **totalSupply @ end:** 55382617442.148830
- **On-chain Δ / net mint / discrepancy (raw int):** 1219106048049329 / 1219106048049329 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
