# EURC supply audit

**Run id:** `eurc_7d_20260513_20260520_ethereum`

**Generated:** 2026-05-21T07:01:20.306573+00:00

## Provenance

**Per-chain block spans** — each L2/L1 uses its own block height; numbers are not comparable across chains, but metrics use one schema.

- **ethereum:** blocks `25082486` → `25132717` (resolved end Some(25132717))

> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. They are **not** estimates of total token holders or a full holder reconstruction.

## Per-chain results

### ethereum

- **Resolved to block:** 25132717
- **Contract:** `0x1aBaEA1f7C830bD89Acc67eC4af516284b1bC33c`
- **Transfer events (deduped):** 12519
- **Active senders / recipients (window):** 2319 / 2470
- **Mints / burns / plain transfers:** 113 / 74 / 12332
- **Sum mints / burns (raw):** 17340951360000 / 16737271830000
- **totalSupply @ start−1:** 275588974.310000  _( on-chain )_
- **totalSupply @ end:** 276192653.840000
- **On-chain Δ / net mint / discrepancy (raw int):** 603679530000 / 603679530000 / 0

**QA gates:**

| metadata | historical totalSupply | no dup logs | decode | supply invariant |
|---|---|---|---|---|
| PASS | PASS | PASS | PASS | PASS |

---

_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._
