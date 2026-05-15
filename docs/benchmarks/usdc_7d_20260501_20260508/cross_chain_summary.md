# Cross-chain window summary — USDC

**Generated:** 2026-05-15T13:14:19.914985+00:00

**Source transfer-audit run:** `usdc_7d_20260501_20260508` (read `qa_report.json` + `supply_audit.csv` from this run only)

**Transfer-audit QA:** 2026-05-15T08:03:31.695921+00:00 (provenance block generated_at: 2026-05-15T08:03:31.695921+00:00)

**Window:** per-chain native block spans (min from_block in bundle: `24996368`). See each row for `from_block` → resolved end; heights are not comparable across chains.

> Per-chain totalSupply(end) sums are not circulating supply across chains (bridged inventory double-counts). Use this table for same-window, per-deployment accounting only.

> This run used per-chain native block spans (`--window`). Rows are comparable under one schema; block numbers and window lengths are not assumed equal across chains. The signed delta sum is an arithmetic aggregate of per-chain windows.

**Sum of per-chain on-chain supply deltas (signed `I256`, same string form as transfer-audit):** `1394620127319062`

| Chain | Resolved end | Transfers | Senders | Recipients | Mints | Burns | totalSupply@end (decimal) | On-chain Δ (signed) | metadata | hist_supply | supply_inv | decode | no_dup | prov_stamp |
|-------|---------------:|----------:|--------:|-----------:|------:|------:|--------------------------:|--------------------:|----------|------------|------------|--------|--------|------------|
| arbitrum | 460491249 | 4866248 | 287266 | 290038 | 18899 | 13075 | 5742193181.040544 | 216193007299893 | PASS | PASS | PASS | PASS | PASS | PASS |
| base | 45704526 | 17208596 | 925834 | 1118818 | 44948 | 17379 | 4416565673.317707 | -40678928030160 | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | 25046605 | 3130346 | 485752 | 621495 | 15289 | 11357 | 55382617442.148830 | 1219106048049329 | PASS | PASS | PASS | PASS | PASS | PASS |

---

_v0.1: chain-level on-chain accounting comparison in the declared window(s). Not bridge netting, reserve attestation, peg or purchasing-power analysis, or holder census._
