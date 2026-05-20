# Cross-chain window summary — USDC

**Generated:** 2026-05-19T18:36:22.715420+00:00

**Source transfer-audit run:** `usdc_7d_20260512_20260519` (read `qa_report.json` + `supply_audit.csv` from this run only)

**Transfer-audit QA:** 2026-05-19T15:00:15.756199+00:00 (provenance block generated_at: 2026-05-19T15:00:15.756199+00:00)

**Window:** per-chain native block spans (min from_block in bundle: `25075306`). See each row for `from_block` → resolved end; heights are not comparable across chains.

> Per-chain totalSupply(end) sums are not circulating supply across chains (bridged inventory double-counts). Use this table for same-window, per-deployment accounting only.

> This run used per-chain native block spans (`--window`). Rows are comparable under one schema; block numbers and window lengths are not assumed equal across chains. The signed delta sum is an arithmetic aggregate of per-chain windows.

**Sum of per-chain on-chain supply deltas (signed I256, same string form as transfer-audit):** `-345299941730946`

| Chain | Resolved end | Transfers | Senders | Recipients | Mints | Burns | totalSupply@end (decimal) | On-chain Δ (signed) | metadata | hist_supply | supply_inv | decode | no_dup | prov_stamp |
|-------|---------------:|----------:|--------:|-----------:|------:|------:|--------------------------:|--------------------:|----------|------------|------------|--------|--------|------------|
| arbitrum | 464280529 | 6477823 | 353705 | 340657 | 15646 | 11955 | 5878440973.415026 | 41534871313684 | PASS | PASS | PASS | PASS | PASS | PASS |
| base | 46179726 | 19850762 | 907087 | 1103899 | 21557 | 17502 | 4267373014.229275 | -132041660467721 | PASS | PASS | PASS | PASS | PASS | PASS |
| ethereum | 25125536 | 4232919 | 593860 | 713111 | 14375 | 11314 | 54168155155.073514 | -254793152576909 | PASS | PASS | PASS | PASS | PASS | PASS |

---

_v0.1: chain-level on-chain accounting comparison in the declared window(s). Not bridge netting, reserve attestation, peg or purchasing-power analysis, or holder census._
