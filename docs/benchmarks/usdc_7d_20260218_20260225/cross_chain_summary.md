# Cross-chain window summary — USDC

**Generated:** 2026-05-20T04:40:32.652662+00:00

**Source transfer-audit run:** `usdc_7d_20260218_20260225` (read `qa_report.json` + `supply_audit.csv` from this run only)

**Transfer-audit QA:** 2026-05-20T00:44:15.801494+00:00 (provenance block generated_at: 2026-05-20T00:44:15.801494+00:00)

**Window:** per-chain native block spans (min from_block in bundle: `24479995`). See each row for `from_block` → resolved end; heights are not comparable across chains.

> Per-chain totalSupply(end) sums are not circulating supply across chains (bridged inventory double-counts). Use this table for same-window, per-deployment accounting only.

> This run used per-chain native block spans (`--window`). Rows are comparable under one schema; block numbers and window lengths are not assumed equal across chains. The signed delta sum is an arithmetic aggregate of per-chain windows.

**Sum of per-chain on-chain supply deltas (signed I256, same string form as transfer-audit):** `1343001595323569`

| Chain | Resolved end | Transfers | Senders | Recipients | Mints | Burns | totalSupply@end (decimal) | On-chain Δ (signed) | metadata | hist_supply | supply_inv | decode | no_dup | prov_stamp |
|-------|---------------:|----------:|--------:|-----------:|------:|------:|--------------------------:|--------------------:|----------|------------|------------|--------|--------|------------|
| arbitrum | 435639345 | 7940173 | 293411 | 336845 | 15155 | 15738 | 6091094085.120981 | -60730685432527 | PASS | PASS | PASS | PASS | PASS | PASS |
| base | 42594126 | 23666689 | 726629 | 944079 | 17839 | 10979 | 4189261427.662630 | 42738845734700 | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | 24530203 | 3551800 | 569718 | 743173 | 17371 | 10384 | 52811093515.542557 | 1360993435021396 | PASS | PASS | PASS | PASS | PASS | PASS |

---

_v0.1: chain-level on-chain accounting comparison in the declared window(s). Not bridge netting, reserve attestation, peg or purchasing-power analysis, or holder census._
