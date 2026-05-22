# USDC supply audit

**Run id:** `usdc_7d_20260218_20260225`

**Generated:** 2026-05-20T00:44:15.801494+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **arbitrum:** blocks `433213243` → `435639345` (resolved end Some(435639345))
- **base:** blocks `42291727` → `42594126` (resolved end Some(42594126))
- **ethereum:** blocks `24479995` → `24530203` (resolved end Some(24530203))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### arbitrum

- **Resolved to block:** 435639345
- **Contract:** `0xaf88d065e77c8cC2239327C5EDb3A432268e5831`
- **Transfer events (deduped):** 7940173
- **Active senders / recipients (window):** 293411 / 336845
- **Mints / burns / plain transfers:** 15155 / 15738 / 7909280
- **Sum mints / burns (raw):** 508244151486612 / 568974836919139
- **totalSupply @ start−1:** 6151824770.553508  _( on-chain )_
- **totalSupply @ end:** 6091094085.120981
- **On-chain Δ / net mint / discrepancy (raw int):** -60730685432527 / -60730685432527 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### base

- **Resolved to block:** 42594126
- **Contract:** `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`
- **Transfer events (deduped):** 23666689
- **Active senders / recipients (window):** 726629 / 944079
- **Mints / burns / plain transfers:** 17839 / 10979 / 23637871
- **Sum mints / burns (raw):** 417227109590607 / 374488263855907
- **totalSupply @ start−1:** 4146522581.927930  _( on-chain )_
- **totalSupply @ end:** 4189261427.662630
- **On-chain Δ / net mint / discrepancy (raw int):** 42738845734700 / 42738845734700 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

### ethereum

- **Resolved to block:** 24530203
- **Contract:** `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`
- **Transfer events (deduped):** 3551800
- **Active senders / recipients (window):** 569718 / 743173
- **Mints / burns / plain transfers:** 17371 / 10384 / 3524045
- **Sum mints / burns (raw):** 4151929812777919 / 2790936377756523
- **totalSupply @ start−1:** 51450100080.521161  _( on-chain )_
- **totalSupply @ end:** 52811093515.542557
- **On-chain Δ / net mint / discrepancy (raw int):** 1360993435021396 / 1360993435021396 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
