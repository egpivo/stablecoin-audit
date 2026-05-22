# Cross-chain window summary — USDC

**Generated:** 2026-05-20T10:19:16.090800+00:00

**Source transfer-audit run:** `usdc_7d_20260507_20260514` (read `qa_report.json` + `supply_audit.csv` from this run only)

**Transfer-audit QA:** 2026-05-20T07:10:57.220330+00:00 (provenance block generated_at: 2026-05-20T07:10:57.220330+00:00)

**Window:** per-chain native block spans (min from_block in bundle: `25039433`). See each row for `from_block` → resolved end; heights are not comparable across chains.

> Per-chain totalSupply(end) sums are not circulating supply across chains (bridged inventory double-counts). Use this table for same-window, per-deployment accounting only.

> This run used per-chain native block spans (`--window`). Rows are comparable under one schema; block numbers and window lengths are not assumed equal across chains. The signed delta sum is an arithmetic aggregate of per-chain windows.

**Sum of per-chain on-chain supply deltas (signed I256, same string form as transfer-audit):** `-932301549860176`

| Chain | Resolved end | Transfers | Senders | Recipients | Mints | Burns | totalSupply@end (decimal) | On-chain Δ (signed) | metadata | hist_supply | supply_inv | decode | no_dup | prov_stamp |
|-------|---------------:|----------:|--------:|-----------:|------:|------:|--------------------------:|--------------------:|----------|------------|------------|--------|--------|------------|
| arbitrum | 462559767 | 5541815 | 338067 | 350343 | 17368 | 13857 | 5803330039.920722 | 66094027180042 | PASS | PASS | PASS | PASS | PASS | PASS |
| base | 45963726 | 17927213 | 932023 | 1136186 | 24205 | 17684 | 4307641235.568829 | -134428255547575 | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | 25089644 | 3472521 | 465587 | 574799 | 14854 | 11436 | 54128632972.670902 | -863967321492643 | PASS | PASS | PASS | PASS | PASS | PASS |

---

_v0.1: chain-level on-chain accounting comparison in the declared window(s). Not bridge netting, reserve attestation, peg or purchasing-power analysis, or holder census._
