# Cross-chain window summary — USDC

**Generated:** 2026-05-20T00:22:19.971712+00:00

**Source transfer-audit run:** `usdc_7d_20241117_20241124` (read `qa_report.json` + `supply_audit.csv` from this run only)

**Transfer-audit QA:** 2026-05-19T18:36:23.252810+00:00 (provenance block generated_at: 2026-05-19T18:36:23.252810+00:00)

**Window:** per-chain native block spans (min from_block in bundle: `21203704`). See each row for `from_block` → resolved end; heights are not comparable across chains.

> Per-chain totalSupply(end) sums are not circulating supply across chains (bridged inventory double-counts). Use this table for same-window, per-deployment accounting only.

> This run used per-chain native block spans (`--window`). Rows are comparable under one schema; block numbers and window lengths are not assumed equal across chains. The signed delta sum is an arithmetic aggregate of per-chain windows.

**Sum of per-chain on-chain supply deltas (signed I256, same string form as transfer-audit):** `1312842921347363`

| Chain | Resolved end | Transfers | Senders | Recipients | Mints | Burns | totalSupply@end (decimal) | On-chain Δ (signed) | metadata | hist_supply | supply_inv | decode | no_dup | prov_stamp |
|-------|---------------:|----------:|--------:|-----------:|------:|------:|--------------------------:|--------------------:|----------|------------|------------|--------|--------|------------|
| arbitrum | 277637793 | 4142603 | 110305 | 129724 | 7062 | 5958 | 1885762051.310776 | 82689239942930 | PASS | PASS | PASS | PASS | PASS | PASS |
| base | 22808526 | 5571685 | 218519 | 268817 | 8983 | 6415 | 3245920954.010536 | -40481623889742 | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | 21253879 | 748862 | 150205 | 167721 | 4750 | 5100 | 28595782977.431447 | 1270635305294175 | PASS | PASS | PASS | PASS | PASS | PASS |

---

_v0.1: chain-level on-chain accounting comparison in the declared window(s). Not bridge netting, reserve attestation, peg or purchasing-power analysis, or holder census._
