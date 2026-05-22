---
title: Blog Evidence Links v1
article: "Local-Currency Stablecoins Still Ride Dollar Liquidity Rails"
repo: https://github.com/egpivo/stablecoin-audit
commit: f4b7b26d24c7a407059383a9b5e6cbab22af6474
snapshot_utc: 2026-05-21T10:40:19Z
written: 2026-05-22
---

# Blog Evidence Links v1

Evidence-link map for "Local-Currency Stablecoins Still Ride Dollar Liquidity Rails."
Purpose: identify exactly which repo artifacts support each claim, the strongest link target for each, and placement within the post.

---

## Quick reference

### Key artifacts

| Artifact | What it supports | Key columns / rows | Committed? | Caveat |
|----------|-----------------|-------------------|------------|--------|
| `data/benchmarks/cross_asset_geo_panel_summary.csv` | Accounting panel — transfer counts, mint/burn, GTN, invariant status for all 7 canonical pairs | `transfer_event_count`, `gross_churn`, `net_supply_delta`, `supply_invariant_status`; filter `window_id` contains `20260513_20260520` | Yes | ERC-20 Transfer events only; `decoded_transfers.csv` is gitignored |
| `data/benchmarks/stablecoin_pair_dependence_summary.csv` | USDC, WETH, and EUR-stable share of observed DEX pool TVL for XSGD and EURC | `usdc_share`, `eur_stable_share`, `total_liquidity_usd`; rows where `asset` ∈ {XSGD, EURC} | **Pending** | DexScreener ≤30 pools per token; not CEX, OTC, or off-chain |
| `data/benchmarks/stablecoin_route_dependence.csv` | Deepest observed 1-hop liquidity path to USDC for each local-currency asset-chain | `hop_count`, `contains_usdc`, `route_liquidity_proxy_usd` | **Pending** | Pool TVL proxy only — not observed swap routing |
| `data/benchmarks/stablecoin_liquidity_pairs.csv` | Raw pool-level DexScreener data (up to 30 pools per token; snapshot 2026-05-21T10:40:19Z) | `pool_address`, `counterpart_token`, `liquidity_usd` | **Pending** | Single timestamp; earlier or later snapshots will differ |
| `docs/benchmarks/*/supply_audit.md` | Human-readable per-run supply invariant results with QA gate table | `supply_invariant_pass`, mint/burn counts, discrepancy | Yes (EURC × 2, XSGD × 2, USDC Ethereum; USDC Base + Arbitrum pending) | Verifies ERC-20 ledger only — not reserves, peg, or solvency |
| `data/benchmarks/rail_movement_summary.csv` | USDC price deviation (bps) vs. net supply movement across six audited windows | `max_abs_price_deviation_bps`, `sum_abs_net_delta_m_usdc` | Yes | Background context; no causal claim |
| `scripts/discover_liquidity_surface.py` | Generates all three liquidity CSVs via DexScreener API | — | **Pending** | Snapshot results will differ at a future date |

### Core evidence (reader check)

| Finding | Primary source | How to verify | Caveat |
|---------|---------------|--------------|--------|
| XSGD Polygon observed DEX liquidity is ~100% USDC-paired | `stablecoin_pair_dependence_summary.csv` — `usdc_share=0.9996` | Filter `asset=XSGD, chain=polygon`; read `usdc_share` | ≤30 pools; not complete market; precise value is 0.9996 |
| XSGD Base observed DEX liquidity is 69% USDC-paired | `stablecoin_pair_dependence_summary.csv` — `usdc_share=0.6911` | Filter `asset=XSGD, chain=base` | 5 pools observed |
| EURC Base observed DEX liquidity is 66% USDC-paired | `stablecoin_pair_dependence_summary.csv` — `usdc_share=0.6587` | Filter `asset=EURC, chain=base` | 28 pools observed |
| EURC Ethereum is 52% USDC-paired and 43% EUR-stable including EURCV | `stablecoin_pair_dependence_summary.csv` — `usdc_share=0.5198`, `eur_stable_share=0.431` | Filter `asset=EURC, chain=ethereum` | `eur_stable_share` includes EURCV (Morpho yield vault); excluding it → ~26% |
| Six of seven canonical asset-chain pairs pass the accounting floor | `cross_asset_geo_panel_summary.csv` — six rows have `supply_invariant_status=PASS` | Filter `window_id` contains `20260513_20260520`; read `supply_invariant_status` | USDC Arbitrum = FAIL — schema gap, not token fault (see next row) |
| USDC Arbitrum FAIL is a schema gap, not a token fault | `docs/benchmarks/usdc_7d_20260513_20260520_arbitrum/supply_audit.csv` — `supply_invariant_pass=false`, `onchain_delta_raw=+20635627854853`, `discrepancy_raw=-72835013578493` | Compare `onchain_delta_raw` vs `net_mint_raw`; read discrepancy | Bridge/gateway mints not captured by zero-address convention; Transfer data internally consistent |
| USDC Base has 20.5M transfers vs XSGD Polygon 2,784 | `cross_asset_geo_panel_summary.csv` rows 15 and 25 | Filter by `asset` and `chain`; compare `transfer_event_count` | ERC-20 Transfer events only; does not imply real-world usage ranking |
| DexScreener snapshot is not complete market coverage | `stablecoin_pair_dependence_summary.csv` `pool_count` column — XSGD/base: 5 pools; EURC/ethereum: 22 pools | Read `pool_count` per row | API returns ≤30 pools per token; CEX, OTC, off-chain not captured |

### Recommended blog links

| # | Anchor text | Path | Placement | Verifies |
|---|-------------|------|-----------|---------|
| 1 | "canonical window audit panel" | `data/benchmarks/cross_asset_geo_panel_summary.csv` | After accounting floor table | All 7 pairs — transfer counts, GTN, invariant status |
| 2 | "pair-dependence data" | `data/benchmarks/stablecoin_pair_dependence_summary.csv` *(pending)* | After Fig. B | USDC/WETH/EUR-stable share by asset-chain |
| 3 | "XSGD supply audits" | `docs/benchmarks/xsgd_7d_20260513_20260520/supply_audit.md` + `…_polygon/supply_audit.md` | After accounting floor table | PASS + zero burns on both chains |
| 4 | "Arbitrum supply audit (schema gap)" | `docs/benchmarks/usdc_7d_20260513_20260520_arbitrum/supply_audit.md` *(pending)* | After Arbitrum footnote | FAIL with `onchain_delta` vs Transfer-event net explained |
| 5 | "EURC Ethereum supply audit" | `docs/benchmarks/eurc_7d_20260513_20260520_ethereum/supply_audit.md` | After audit surface section | PASS, GTN=56×, 113 mints / 74 burns |

---

## 1. Evidence link policy

### File suitability for blog links

| Tier | File types | Suitability |
|------|-----------|-------------|
| **Good** | `docs/benchmarks/*/supply_audit.md` — human-readable, already in repo | Yes — direct link |
| **Good** | `docs/benchmarks/*/supply_audit.csv` — structured, committed | Yes — direct link or as table anchor |
| **Good** | `data/benchmarks/cross_asset_geo_panel_summary.csv` — canonical panel, tracked | Yes — primary panel link |
| **Good** | `scripts/discover_liquidity_surface.py` — reproducibility | Yes, once committed |
| **Okay** | `data/benchmarks/stablecoin_pair_dependence_summary.csv` — key liquidity data | Yes, once committed |
| **Okay** | `data/benchmarks/stablecoin_route_dependence.csv` — route data | Yes, once committed |
| **Okay** | `data/benchmarks/stablecoin_liquidity_pairs.csv` — raw pool data | Appendix only — too granular for main post |
| **Not good** | `out/*/decoded_transfers.csv` — multi-GB, gitignored | Never link |
| **Not good** | `.local/findings/*.md` — private working memos | Do not link — move key conclusions to `docs/findings/` if public |
| **Not good** | `.env`, raw RPC logs, checkpoint files | Never link |

### GitHub link format

Use commit-pinned links for stability:

```
https://github.com/egpivo/stablecoin-audit/blob/<sha>/<path>
```

Commit SHA: `f4b7b26d24c7a407059383a9b5e6cbab22af6474` (short: `f4b7b26`)

**Commit status note:** As of 2026-05-22, the following files are untracked and cannot have stable GitHub links until pushed:
- `data/benchmarks/stablecoin_pair_dependence_summary.csv`
- `data/benchmarks/stablecoin_route_dependence.csv`
- `data/benchmarks/stablecoin_liquidity_pairs.csv`
- `data/benchmarks/geo_stablecoin_surface_summary.csv`
- `docs/benchmarks/usdc_7d_20260513_20260520_base/`
- `docs/benchmarks/usdc_7d_20260513_20260520_arbitrum/`
- `scripts/discover_liquidity_surface.py`

All committed `docs/benchmarks/` runs for EURC and XSGD canonical window are linked below using the current SHA.

---

## 2. Claim-to-evidence matrix

**Column guide:** "Exact rows" uses 1-based line numbering (row 1 = header, row 2 = first data row).

| Claim ID | Blog claim | Strength | Primary artifact | Exact rows / columns | Suggested GitHub link | Include? | Placement | Caveat |
|----------|-----------|----------|-----------------|---------------------|----------------------|----------|-----------|--------|
| C1 | Peg denomination and liquidity substrate are different layers | Supported | Structural framing — supported by C5–C10 data | — | `stablecoin_pair_dependence_summary.csv` (aggregate) | Yes — opening | Opening section | No data needed; data in C5–C10 |
| C2 | XSGD and EURC are auditable local-currency stablecoins | Supported | `docs/benchmarks/xsgd_7d_20260513_20260520/supply_audit.md`, `eurc_7d_20260513_20260520_ethereum/supply_audit.md` | PASS gate row in each `.md` | [XSGD Base supply_audit.md](https://github.com/egpivo/stablecoin-audit/blob/f4b7b26d24c7a407059383a9b5e6cbab22af6474/docs/benchmarks/xsgd_7d_20260513_20260520/supply_audit.md) | Yes | After audit table | supply_invariant_pass=true verifies ledger only — not reserves, peg, or safety |
| C3 | Six of seven canonical asset-chain pairs pass the accounting floor | Supported | `data/benchmarks/cross_asset_geo_panel_summary.csv` rows 2,3,8,15,22,24,25 (`supply_invariant_status`); row 8 = FAIL | Rows 2,3,8,15,22,24,25 | [cross_asset_geo_panel_summary.csv](https://github.com/egpivo/stablecoin-audit/blob/f4b7b26d24c7a407059383a9b5e6cbab22af6474/data/benchmarks/cross_asset_geo_panel_summary.csv) | Yes | After accounting table | `decoded_transfers.csv` confirms raw counts but is gitignored |
| C4 | USDC Arbitrum is a schema gap, not a token fault | Supported | `docs/benchmarks/usdc_7d_20260513_20260520_arbitrum/supply_audit.csv` — `supply_invariant_pass=false`, `onchain_delta_raw=+20635627854853`, `discrepancy_raw=-72835013578493` | Row 2, columns `supply_invariant_pass`, `onchain_delta_raw`, `discrepancy_raw` | Commit pending | Yes | After audit table footnote | Must not imply reserve anomaly or double-spend; discrepancy is schema boundary |
| C5 | XSGD Polygon observed DEX liquidity is ~100% USDC-paired | Supported | `data/benchmarks/stablecoin_pair_dependence_summary.csv` row 8 — `usdc_share=0.9996` | Row 8, column `usdc_share` | Commit pending | Yes | Fig. B caption / pair table | ≤30 pools via DexScreener; not complete market; not CEX or OTC; usdc_share=0.9996, not exactly 1.0 |
| C6 | XSGD Base observed DEX liquidity is 69% USDC-paired | Supported | `data/benchmarks/stablecoin_pair_dependence_summary.csv` row 7 — `usdc_share=0.6911` | Row 7, column `usdc_share` | Commit pending | Yes | Fig. B caption / pair table | 5 pools observed; same DexScreener caveat |
| C7 | EURC Base observed DEX liquidity is 66% USDC-paired | Supported | `data/benchmarks/stablecoin_pair_dependence_summary.csv` row 2 — `usdc_share=0.6587` | Row 2, column `usdc_share` | Commit pending | Yes | Fig. B caption / pair table | 28 pools observed; ~34% WETH/other |
| C8 | EURC Ethereum is 52% USDC-paired and 43% EUR-stable including EURCV | Supported | `data/benchmarks/stablecoin_pair_dependence_summary.csv` row 3 — `usdc_share=0.5198`, `eur_stable_share=0.431` | Row 3, columns `usdc_share`, `eur_stable_share` | Commit pending | Yes — with EURCV caveat | Fig. B caption / pair table | `eur_stable_share` uses DexScreener symbol heuristic; EURCV (Morpho vault) classified as EUR-stable |
| C9 | Excluding EURCV lowers EURC Ethereum EUR-stable share to ~26% | Partially supported | Manual pool classification in `liquidity_surface_qa_v1.md` line 175 | EURC/EURCV pool ($2.13M) reclassified as vault/wrapper | Not linkable — local memo | Yes — inline caveat ("26–43% range") | Pair table footnote | EURCV = Morpho MetaMorpho yield vault; QA memo not yet committed |
| C10 | All four local-currency pairs have a direct 1-hop USDC pool as deepest observed pool | Supported | `data/benchmarks/stablecoin_route_dependence.csv` rows 2,4,12,14 — `hop_count=1`, `contains_usdc=true` | Rows 2,4,12,14 | Commit pending | Yes | Route table inline | `route_liquidity_proxy_usd` = pool TVL proxy; not verified swap routing |
| C11 | Route dependence is a liquidity-graph approximation, not verified swap routing | Supported | `data/benchmarks/stablecoin_route_dependence.csv` — `notes` column reads "direct pool" | All rows, column `notes` | Commit pending | Yes — explicit caveat | "What this does not claim" | Methodological boundary |
| C12 | USDC Ethereum processed ~8.5B gross against −202M net in canonical window | Supported | `data/benchmarks/cross_asset_geo_panel_summary.csv` row 22 — `gross_churn=8498638915.31`, `net_supply_delta=-202024380.01` | Row 22 | [cross_asset_geo_panel_summary.csv](https://github.com/egpivo/stablecoin-audit/blob/f4b7b26d24c7a407059383a9b5e6cbab22af6474/data/benchmarks/cross_asset_geo_panel_summary.csv) | Yes | USDC baseline | Window-local; no cross-chain netting; no causal attribution |
| C13 | USDC Base processed ~954M gross against −76M net in canonical window | Supported | `data/benchmarks/cross_asset_geo_panel_summary.csv` row 15 — `gross_churn=954448137.44`, `net_supply_delta=-75875970.18` | Row 15 | [cross_asset_geo_panel_summary.csv](https://github.com/egpivo/stablecoin-audit/blob/f4b7b26d24c7a407059383a9b5e6cbab22af6474/data/benchmarks/cross_asset_geo_panel_summary.csv) | Yes | USDC baseline | Same caveats as C12 |
| C14 | USDC provides the thick dollar-rail baseline | Supported | Structural — rows 22,15 (USDC) vs rows 2,3,24,25 (EURC+XSGD) in `cross_asset_geo_panel_summary.csv` | `gross_churn`, `transfer_event_count` | [cross_asset_geo_panel_summary.csv](https://github.com/egpivo/stablecoin-audit/blob/f4b7b26d24c7a407059383a9b5e6cbab22af6474/data/benchmarks/cross_asset_geo_panel_summary.csv) | No — implied by C12/C13 | USDC baseline | "Thick" = observable transfer volume and gross churn; not economic centrality |
| C15 | Accounting surface differs by ~4 orders of magnitude | Supported | `cross_asset_geo_panel_summary.csv` rows 15 vs 25 — USDC Base 20,512,028 vs XSGD Polygon 2,784 | `transfer_event_count` | [cross_asset_geo_panel_summary.csv](https://github.com/egpivo/stablecoin-audit/blob/f4b7b26d24c7a407059383a9b5e6cbab22af6474/data/benchmarks/cross_asset_geo_panel_summary.csv) | Yes — Fig. C caption | Audit surface | ERC-20 Transfer events only; CEX and OTC not included |
| C16 | USDC Base 20.5M transfers; XSGD Polygon 2,784; EURC Ethereum 12,519 | Supported | `cross_asset_geo_panel_summary.csv` rows 15,25,3 | `transfer_event_count` | [cross_asset_geo_panel_summary.csv](https://github.com/egpivo/stablecoin-audit/blob/f4b7b26d24c7a407059383a9b5e6cbab22af6474/data/benchmarks/cross_asset_geo_panel_summary.csv) | Yes | Audit surface / Fig. C | Deduped, zero decode errors |
| C17 | XSGD has zero burns on both Base and Polygon in canonical window | Supported | `cross_asset_geo_panel_summary.csv` rows 24,25 — `burn_count=0`; confirmed in `xsgd_7d_*/supply_audit.md` | Rows 24,25 column `burn_count` | [XSGD Base supply_audit.md](https://github.com/egpivo/stablecoin-audit/blob/f4b7b26d24c7a407059383a9b5e6cbab22af6474/docs/benchmarks/xsgd_7d_20260513_20260520/supply_audit.md) | Yes | Audit surface | One-window; does not verify full redemption trail |
| C18 | EURC Ethereum has 56× gross-to-net ratio | Supported | `cross_asset_geo_panel_summary.csv` row 3 — `gross_to_net_ratio=56.4509` | Row 3 | [cross_asset_geo_panel_summary.csv](https://github.com/egpivo/stablecoin-audit/blob/f4b7b26d24c7a407059383a9b5e6cbab22af6474/data/benchmarks/cross_asset_geo_panel_summary.csv) | Yes | Audit surface | Does not distinguish institutional round-trips from episodic issuance |
| C19 | DexScreener snapshot is not complete market coverage | Supported | `stablecoin_pair_dependence_summary.csv` `pool_count` column; max 30 per token; XSGD/base has 5 pools | `pool_count` | Commit pending | Yes — explicit caveat | "What this does not claim" | CEX, OTC, off-chain not captured |
| C20 | Post does not support U.S. policy causality, adoption ranking, reserve adequacy, peg durability, or swap routing | Supported | Article-level disclaimers; no repo artifact asserts these claims | "What this does not claim" section | Not applicable | Yes — section heading | "What this does not claim" | Wording in article is correct; no change needed |

---

## 3. Artifact inventory for blog post

| Artifact | Purpose | Committed? | Use in main post? | Link in blog? | Suggested anchor text | Notes |
|----------|---------|------------|------------------|---------------|----------------------|-------|
| `data/benchmarks/cross_asset_geo_panel_summary.csv` | Canonical panel — transfer counts, mint/burn, GTN, invariant status for all 7 pairs | Modified/tracked | Yes — primary quantitative source | Yes | "canonical window audit panel" | C3,C12–C18; 7 canonical window rows |
| `data/benchmarks/stablecoin_pair_dependence_summary.csv` | USDC/WETH/EUR-stable share by asset-chain from DexScreener | **Untracked** | Yes — hero figure source | Yes, after commit | "pair-dependence data" | C5–C9; commit before publishing |
| `data/benchmarks/stablecoin_route_dependence.csv` | 1-hop liquidity path to USDC/WETH by observed TVL | **Untracked** | Yes — route table source | Appendix only | "route-dependence approximation" | C10–C11; commit before publishing |
| `data/benchmarks/stablecoin_liquidity_pairs.csv` | Raw pool-level DexScreener data (up to 30 pools per token) | **Untracked** | No — too granular | Appendix only, optional | "raw pool-level data" | Underpins pair_dependence; useful for QA |
| `data/benchmarks/geo_stablecoin_surface_summary.csv` | Joined audit + liquidity surface per asset-chain | **Untracked** | No — redundant | No | — | Combined view; not required separately |
| `data/benchmarks/rail_movement_summary.csv` | Price deviation vs net supply movement across all windows | Committed | Yes — background fig source | No — fig is self-contained | — | Background fig; 6-window summary |
| `scripts/discover_liquidity_surface.py` | Generates all liquidity CSVs from DexScreener | **Untracked** | No | Appendix only | "liquidity snapshot script" | Commit before publishing |
| `.local/blog/generate_geo_liquidity_figures.py` | Figure generation — figA, figB, figC | Not committed | No | Appendix only, once committed | "figure generation script" | Move to `scripts/` before committing |
| `docs/benchmarks/eurc_7d_20260513_20260520_ethereum/supply_audit.md` | EURC Ethereum audit — human-readable | Committed | Yes | Yes | "EURC Ethereum audit" | C2, C18; GTN=56.45 verifiable |
| `docs/benchmarks/eurc_7d_20260513_20260520_base/supply_audit.md` | EURC Base audit | Committed | Yes | Optional | "EURC Base audit" | C2, C7 |
| `docs/benchmarks/xsgd_7d_20260513_20260520/supply_audit.md` | XSGD Base audit | Committed | Yes | Yes | "XSGD Base audit" | C2, C17; burn_count=0 |
| `docs/benchmarks/xsgd_7d_20260513_20260520_polygon/supply_audit.md` | XSGD Polygon audit | Committed | Yes | Yes | "XSGD Polygon audit" | C2, C17; burn_count=0 |
| `docs/benchmarks/usdc_7d_20260513_20260520_ethereum/supply_audit.md` | USDC Ethereum audit | Committed | Yes | Optional | "USDC Ethereum audit" | C3, C12 |
| `docs/benchmarks/usdc_7d_20260513_20260520_base/supply_audit.md` | USDC Base audit | **Untracked** | Yes | After commit | "USDC Base audit" | C3, C13 |
| `docs/benchmarks/usdc_7d_20260513_20260520_arbitrum/supply_audit.md` | USDC Arbitrum — schema FAIL | **Untracked** | Yes | After commit | "USDC Arbitrum audit (schema gap)" | C4; supply_invariant_pass=false; onchain_delta=+20.6M |

---

## 4. Recommended links for the blog

Five main-text links; two for appendix. Effective once all pending files are committed.

| # | Anchor text | Path | What it verifies | Placement |
|---|-------------|------|-----------------|-----------|
| 1 | "canonical window audit panel" | `data/benchmarks/cross_asset_geo_panel_summary.csv` | C3, C12–C18 | After accounting floor table |
| 2 | "pair-dependence data" | `data/benchmarks/stablecoin_pair_dependence_summary.csv` *(commit pending)* | C5–C8 | After Fig. B |
| 3 | "XSGD supply audits" | `docs/benchmarks/xsgd_7d_20260513_20260520/supply_audit.md` + `…_polygon/supply_audit.md` | C2, C17 | After accounting floor table |
| 4 | "Arbitrum supply audit (schema gap)" | `docs/benchmarks/usdc_7d_20260513_20260520_arbitrum/supply_audit.md` *(commit pending)* | C4 | After Arbitrum footnote |
| 5 | "EURC Ethereum supply audit" | `docs/benchmarks/eurc_7d_20260513_20260520_ethereum/supply_audit.md` | C2, C18 | After audit surface section |

**Appendix:**

| # | Anchor text | Path | What it verifies |
|---|-------------|------|-----------------|
| A1 | "liquidity snapshot script" | `scripts/discover_liquidity_surface.py` *(commit pending)* | DexScreener snapshot reproducibility |
| A2 | "route-dependence approximation" | `data/benchmarks/stablecoin_route_dependence.csv` *(commit pending)* | C10–C11 — 1-hop USDC paths |

---

## 5. Figure evidence mapping

### Background fig — Price deviation vs. net supply movement

| Field | Value |
|-------|-------|
| File | `figures/geo_policy_fig4_rail_movement.png` |
| Script | `.local/blog/generate_geo_policy_figures.py` |
| Source CSVs | `data/benchmarks/rail_movement_summary.csv`, `data/external/usdc_price_daily.csv` |
| Columns | `max_abs_price_deviation_bps`, `sum_abs_net_delta_m_usdc`, `short_window_label` |
| Claims | C12, C13 (contextually); no causality claim |
| Include? | Yes — "Background fig." in USDC baseline section |
| Caption caveat | "Does not establish a causal link between price and rail activity." |
| Reproducibility | `python3 .local/blog/generate_geo_policy_figures.py` |

### Fig. A — USDC baseline: gross mint/burn flows

| Field | Value |
|-------|-------|
| File | `figures/figA_usdc_mint_burn_decomposition.png` |
| Script | `.local/blog/generate_geo_liquidity_figures.py` |
| Source CSV | `data/benchmarks/cross_asset_geo_panel_summary.csv` |
| Columns | `gross_churn` (split via sum_mints/burns), `net_supply_delta`, `chain`, `supply_invariant_status` |
| Exact rows | Row 8 (USDC Arbitrum, FAIL), row 15 (USDC Base), row 22 (USDC Ethereum) |
| Claims | C12, C13, C4 |
| Include? | Yes — USDC baseline section |
| Caption caveat | "Arbitrum net supply shown as `totalSupply` delta (+20.6M); Transfer-event-derived net (−52.2M) not used." |

### Fig. B — Local pegs, dollar-mediated liquidity (hero)

| Field | Value |
|-------|-------|
| File | `figures/figB_pair_dependence.png` |
| Script | `.local/blog/generate_geo_liquidity_figures.py` |
| Source CSV | `data/benchmarks/stablecoin_pair_dependence_summary.csv` *(commit pending)* |
| Columns | `usdc_share`, `weth_share`, `eur_stable_share`, `other_share`, `total_liquidity_usd` |
| Exact rows | Rows 2 (EURC/base), 3 (EURC/eth), 7 (XSGD/base), 8 (XSGD/polygon) |
| Claims | C1, C5, C6, C7, C8 |
| Include? | Yes — hero figure |
| Caption caveat | "DexScreener ≤30 pools per token; OTC and CEX not captured. Not observed swap routing." |

### Fig. C — Same schema, different surface thickness

| Field | Value |
|-------|-------|
| File | `figures/figC_audit_surface.png` |
| Script | `.local/blog/generate_geo_liquidity_figures.py` |
| Source CSV | `data/benchmarks/cross_asset_geo_panel_summary.csv` |
| Columns | `transfer_event_count`, `mint_count`, `burn_count`, `asset`, `chain`, `supply_invariant_status` |
| Exact rows | Rows 2,3,8,15,22,24,25 |
| Claims | C15, C16, C17, C18 |
| Include? | Yes — audit surface section |
| Caption caveat | "USDC Arbitrum (faded) fails zero-address schema — schema boundary, not token fault. Log scale required." |

---

## 6. Evidence quality grading

| Evidence group | Grade | Rationale |
|---------------|-------|-----------|
| Accounting floor / supply invariant | **A** | On-chain RPC, zero decode errors, zero duplicates. Deterministic formula. Committed `supply_audit.csv` files are verifiable against contract addresses and block ranges. |
| Pair-dependence / USDC liquidity share | **B** | DexScreener snapshot, ≤30 pools, single timestamp. DEX surface only. EURCV classification is ambiguous. Commit pending. |
| Route-dependence / 1-hop pool | **B** | Same DexScreener limitations. TVL proxy only — not observed trade execution. Commit pending. |
| USDC gross mint/burn baseline | **A** | Derived from committed `cross_asset_geo_panel_summary.csv`. Gross-to-net ratios computed from same source. |
| Fear & Greed / policy labels | **C** | Not used in this article. No causal claim made. |
| DexScreener raw pool data | **B** | `stablecoin_liquidity_pairs.csv` — raw pool level, ≤30 pools per token, single snapshot. |
| XSGD / EURC price stability | **D** | No historical price data for XSGD or EURC in this artifact set. Do not assert. |

---

## 7. Blog inclusion recommendation

**A. Main blog (5 links):** panel CSV → pair-dependence CSV → XSGD supply audits → Arbitrum supply audit → EURC Ethereum supply audit (see Section 4).

**B. Appendix only:** `discover_liquidity_surface.py`, `stablecoin_route_dependence.csv`, `stablecoin_liquidity_pairs.csv`.

**C. Do not link:** `out/*/decoded_transfers.csv` (gitignored, multi-GB), `.local/findings/*.md` (private memos), `geo_stablecoin_surface_summary.csv` (redundant), `market_conditioned_audit.csv` (Fear & Greed — risks implying causality).

**D. Claims with correct wording already in article:** C8/C9 ("26–43% depending on vault classification"), C10 ("liquidity-graph approximation"), C19 (DexScreener caveat).

**E. Publishing blocker:** C5–C11 pair-dependence and route claims rely on untracked CSVs. Commit `stablecoin_pair_dependence_summary.csv`, `stablecoin_route_dependence.csv`, `stablecoin_liquidity_pairs.csv`, `discover_liquidity_surface.py`, and both USDC canonical `docs/benchmarks/` directories before publishing.

---

## 8. Cleanup recommendations

1. **Commit untracked liquidity artifacts** (high priority, before publishing) — pair_dependence, route_dependence, liquidity_pairs CSVs + `discover_liquidity_surface.py` + both USDC canonical benchmark directories.
2. **Commit USDC canonical benchmark docs** (high) — `docs/benchmarks/usdc_7d_20260513_20260520_base/` and `usdc_7d_20260513_20260520_arbitrum/`.
3. **Move figure generation script** (medium) — `.local/blog/generate_geo_liquidity_figures.py` → `scripts/` or `analysis/`.
4. **Add DexScreener snapshot README** (medium) — note point-in-time snapshot, ≤30 pool API cap, token → pool mapping method.
5. **Move EURCV classification note** (low) — `.local/findings/liquidity_surface_qa_v1.md` → `docs/findings/liquidity_surface_qa.md` so readers can verify the 26% figure.
6. **Canonical window explanation** (low) — add to `data/benchmarks/README.md`: what "canonical window" means, why May 13–20.

---

*`docs/evidence/`. 2026-05-22.*
