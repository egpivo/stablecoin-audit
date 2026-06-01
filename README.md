# stablecoin-audit

[![CI](https://github.com/egpivo/stablecoin-audit/actions/workflows/ci.yml/badge.svg)](https://github.com/egpivo/stablecoin-audit/actions/workflows/ci.yml)
[![codecov](https://codecov.io/github/egpivo/stablecoin-audit/graph/badge.svg?token=mN0h7zLOtR)](https://codecov.io/github/egpivo/stablecoin-audit)

**0.1.0** — For each EVM deployment and declared block window, the CLI tests whether ERC-20 mint/burn aggregates match the change in `totalSupply` at pinned boundaries, then rolls per-chain results into one comparison schema. Runs are keyed by `--asset` and `configs/tokens/<asset>.<chain>.yml`; USDC on Ethereum, Base, and Arbitrum is the first published benchmark, not a hard-coded sole token.

**Not claimed under this tool:** reserves, peg, purchasing power, liquidity, oracles, bridge backing, holder census, or intent.

**Another asset:** add YAML per chain, RPC env vars (`.env.example`), then `transfer-audit` / `cross-chain-summary` with `--asset <SYMBOL>`. 0.1.0 core paths use standard `Transfer` + `totalSupply`; experimental control-topic decoding is issuer-specific today.

## Quick start (USDC)

```bash
cargo build
# .env: ALCHEMY_ETHEREUM_URL, ALCHEMY_BASE_URL, ALCHEMY_ARBITRUM_URL (see .env.example)

cargo run -- transfer-audit --asset USDC --run-id smoke_001 \
  --window ethereum:24000000:24001000 \
  --window base:30000000:30001000 \
  --window arbitrum:330000000:330001000

cargo run -- cross-chain-summary --asset USDC --run-id smoke_001
```

Outputs land under `out/<asset>/runs/<run_id>/`. Gate strings live in `qa_report.json` → `chains[].gates` (PASS/FAIL). Large `decoded_transfers.csv` files stay under `out/` (gitignored).

**UTC wall-clock window:** `resolve-window` maps `--from` / `--to` (RFC 3339) to per-chain `--window` args via block headers only — then run `transfer-audit` with those windows.

```bash
cargo run -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-05-01T00:00:00Z \
  --to 2026-05-08T00:00:00Z
```

## Reference benchmark (USDC)

Published tables and QA for one USDC run (2026-05-01 → 2026-05-08 UTC): [`docs/benchmarks/usdc_7d_20260501_20260508/`](docs/benchmarks/usdc_7d_20260501_20260508/). Supply invariant **PASS** on all three chains in that window. Future assets can publish the same layout under `docs/benchmarks/<asset>_…/`. Full `decoded_transfers.csv` files stay local under `out/`, not in git.

## Research extension: market-conditioned join (optional)

Python scripts join published benchmark windows with the [Crypto Fear & Greed Index](https://alternative.me/crypto/fear-and-greed-index/) as a **market regime proxy** (association only—not causality, not a safety score). Does **not** change `transfer-audit` Rust code.

```bash
python3 scripts/fetch_fear_greed.py
python3 scripts/join_window_sentiment.py
python3 scripts/build_market_conditioned_panel.py
```

See [`scripts/README.md`](scripts/README.md), [`data/external/README.md`](data/external/README.md), and [`data/benchmarks/README.md`](data/benchmarks/README.md). Two additional greed/fear weeks are pre-registered in [`data/benchmarks/windows.csv`](data/benchmarks/windows.csv)—run instructions in [`data/benchmarks/RUN_ADDITIONAL_WINDOWS.md`](data/benchmarks/RUN_ADDITIONAL_WINDOWS.md).

## Commands (0.1.0)

| Command | Role |
|---------|------|
| `transfer-audit` | Fetch Transfer logs, dedup, supply invariant, QA artifacts |
| `cross-chain-summary` | Roll up one `run_id` into `cross_chain_summary.{md,json}` |
| `resolve-window` | UTC interval → per-chain block bounds |
| `metadata` | IERC-20 metadata + pinned `totalSupply` probes |
| `stablecoin-map-package` | Build stablecoin-map CSV evidence package from benchmark data, DefiLlama, and Artemis |

**Resume:** same `--run-id` and `--window` args continue from `checkpoint/`; `--fresh` clears checkpoints. Default `--chunk-size` is 500 blocks per `eth_getLogs` call (5 retries on transient RPC errors).

**Experimental** (`cargo build --features experimental`): `fetch`, `report`, `control-audit`, `control-report`.

## Interpretation

- **Supply invariant FAIL** means the accounting identity did not hold under this tool’s definitions — not automatically fraud or depeg.
- **Cross-chain tables** compare per-deployment metrics on one schema; summing `totalSupply` across chains is not circulating supply (bridged inventory double-counts).

## Blog evidence

This repo supports stablecoin-liquidity articles with two layers: supply-invariant audit artifacts and DexScreener liquidity snapshots.

| Artifact | What it shows |
|----------|--------------|
| [`data/benchmarks/cross_asset_geo_panel_summary.csv`](data/benchmarks/cross_asset_geo_panel_summary.csv) | Transfer counts, mint/burn, gross-to-net ratio, and invariant status for all seven canonical asset-chain pairs |
| [`data/benchmarks/rail_movement_summary.csv`](data/benchmarks/rail_movement_summary.csv) | USDC price deviation (bps) vs. net supply movement across six audited windows |
| [`docs/benchmarks/xsgd_7d_20260513_20260520/supply_audit.md`](docs/benchmarks/xsgd_7d_20260513_20260520/supply_audit.md) | XSGD Base canonical window — supply invariant PASS, zero burns |
| [`docs/benchmarks/eurc_7d_20260513_20260520_ethereum/supply_audit.md`](docs/benchmarks/eurc_7d_20260513_20260520_ethereum/supply_audit.md) | EURC Ethereum canonical window — supply invariant PASS, 56× gross-to-net ratio |
| [`docs/evidence/blog_evidence_links_v1.md`](docs/evidence/blog_evidence_links_v1.md) | Full claim-to-artifact map (C1–C20) with exact rows, figure evidence, quality grades, and recommended blog links |

Stablecoin-map package CSVs can be regenerated from Rust:

```bash
cargo run -- stablecoin-map-package
# local-only dependency CSVs, no DefiLlama/Artemis calls:
cargo run -- stablecoin-map-package --skip-network
```

Generated map-package outputs land in `data/benchmarks/`: `global_stablecoin_inventory_v1.csv`, `stablecoin_transfer_volume_selected_rails_v1.csv`, `stablecoin_dependency_summary.csv`, and `stablecoin_dependency_edges.csv`.

## Product architecture (toolkit, not dashboard)

This repo is a **reproducible evidence toolkit** (Rust CLI + filesystem artifacts), not a hosted risk dashboard. Layered design and roadmap: [`docs/product/backend_architecture_v0.md`](docs/product/backend_architecture_v0.md). Manifest schema: [`docs/product/artifact_manifest_schema_v0.md`](docs/product/artifact_manifest_schema_v0.md). Read-only evidence API (v0.3 skeleton): `cargo run --features api -- serve --artifact-root out/`.

## Evidence browser

Minimal local UI for inspecting completed runs — claim boundaries, artifacts, and package actions. Served by the API at `/ui/`; no separate frontend build.

```bash
# After at least one successful transfer-audit run under out/
cargo run --features api -- serve --artifact-root out/
open http://127.0.0.1:8080/ui/
```

Details: [`docs/product/evidence_browser_v0.md`](docs/product/evidence_browser_v0.md).

### GitHub Pages demo (read-only)

A **static public demo** uses dummy run id `github_pages_demo` and recorded artifacts under `docs/demo-artifacts/` (no RPC, no new audits). Regenerate after changing the UI or demo bundle:

```bash
python3 scripts/export_github_pages_demo.py
# requires a completed run under out/usdc/runs/ (default source: article_ui_demo)
```

Enable **Settings → Pages → Deploy from branch `main`, folder `/docs`**. Open `https://<org>.github.io/stablecoin-audit/ui/`. See [`docs/GITHUB_PAGES.md`](docs/GITHUB_PAGES.md).
