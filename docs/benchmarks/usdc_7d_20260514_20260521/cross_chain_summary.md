# Cross-chain window summary — USDC

**Generated:** 2026-05-19T15:00:15.242644+00:00

**Source transfer-audit run:** `usdc_7d_20260514_20260521` (read `qa_report.json` + `supply_audit.csv` from this run only)

**Transfer-audit QA:** 2026-05-19T12:32:36.420555+00:00 (provenance block generated_at: 2026-05-19T12:32:36.420555+00:00)

**Window:** per-chain native block spans (min from_block in bundle: `25089645`). See each row for `from_block` → resolved end; heights are not comparable across chains.

> Per-chain totalSupply(end) sums are not circulating supply across chains (bridged inventory double-counts). Use this table for same-window, per-deployment accounting only.

> This run used per-chain native block spans (`--window`). Rows are comparable under one schema; block numbers and window lengths are not assumed equal across chains. The signed delta sum is an arithmetic aggregate of per-chain windows.

**Sum of per-chain on-chain supply deltas (signed I256, same string form as transfer-audit):** `66526835472818`

| Chain | Resolved end | Transfers | Senders | Recipients | Mints | Burns | totalSupply@end (decimal) | On-chain Δ (signed) | metadata | hist_supply | supply_inv | decode | no_dup | prov_stamp |
|-------|---------------:|----------:|--------:|-----------:|------:|------:|--------------------------:|--------------------:|----------|------------|------------|--------|--------|------------|
| arbitrum | 464438118 | 5078139 | 289023 | 277549 | 11581 | 9063 | 5879969678.401091 | 76639638480369 | PASS | PASS | PASS | PASS | PASS | PASS |
| base | 46199501 | 15679507 | 741703 | 897329 | 15604 | 13061 | 4265780174.247593 | -41861061321236 | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | 25128822 | 3692900 | 520521 | 614786 | 10525 | 8506 | 54160381230.984587 | 31748258313685 | PASS | PASS | PASS | PASS | PASS | PASS |

---

_v0.1: chain-level on-chain accounting comparison in the declared window(s). Not bridge netting, reserve attestation, peg or purchasing-power analysis, or holder census._
