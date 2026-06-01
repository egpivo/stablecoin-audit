# Documentation

Index for CLI, benchmarks, product specs, and research. [Root README](../README.md) has architecture + quick start.

## Scope and interpretation

**In scope:** `Transfer`-derived mint/burn vs `totalSupply` change at pinned boundaries; optional cross-chain rollup per `run_id`.

**Not claimed:** reserves, peg, liquidity, oracles, bridge backing, holder census, intent.

- **FAIL** ≠ fraud or depeg — identity failed under this tool’s definitions.
- Cross-chain tables ≠ global circulating supply (bridges double-count).

[`product/audit_semantics_v0.md`](product/audit_semantics_v0.md) · [`product/claim_registry_v0.md`](product/claim_registry_v0.md)

## CLI

| Command | Role |
|---------|------|
| `transfer-audit` | Logs → dedup → supply invariant → QA artifacts |
| `cross-chain-summary` | One `run_id` → `cross_chain_summary.{md,json}` |
| `resolve-window` | RFC3339 `--from`/`--to` → per-chain `--window` |
| `metadata` | Token metadata + pinned `totalSupply` |
| `stablecoin-map-package` | Map CSVs from benchmarks (+ optional network sources) |

Resume: same `--run-id` + `--window` uses `checkpoint/`; `--fresh` clears. `--chunk-size` default 500; 5 RPC retries.

Experimental (`--features experimental`): `fetch`, `report`, `control-audit`, `control-report`.

**Outputs:** `out/<asset>/runs/<run_id>/` · gates: `qa_report.json` → `chains[].gates` · large CSVs gitignored.

**UTC windows:** `resolve-window` then `transfer-audit` with printed `--window` args — see [`../scripts/README.md`](../scripts/README.md).

**Map package:** `cargo run -- stablecoin-map-package` (`--skip-network` for offline). CSVs under `data/benchmarks/`.

## Adding an asset

`configs/tokens/<asset>.<chain>.yml` + RPC in `.env` → `transfer-audit` / `cross-chain-summary --asset <SYMBOL>`.

## Benchmarks

Published runs: [`benchmarks/`](benchmarks/) (`supply_audit.md`, `qa_report.json`, summaries).

Reference: [`benchmarks/usdc_7d_20260501_20260508/`](benchmarks/usdc_7d_20260501_20260508/) (PASS all three chains). Full transfer CSVs stay in `out/`.

## Product

| Doc | |
|-----|--|
| [`product/backend_architecture_v0.md`](product/backend_architecture_v0.md) | Stack, layers, roadmap |
| [`product/artifact_manifest_schema_v0.md`](product/artifact_manifest_schema_v0.md) | Manifest contract |
| [`product/evidence_browser_v0.md`](product/evidence_browser_v0.md) | UI behavior |
| [`product/audit_product_pipeline_v0.md`](product/audit_product_pipeline_v0.md) | Audit → evidence flow |

API: `cargo run --features api -- serve --artifact-root out/` → `/ui/`.

## GitHub Pages

`python3 scripts/export_github_pages_demo.py` → deploy `/docs` on `main`. [`GITHUB_PAGES.md`](GITHUB_PAGES.md)

## Research and blog

Fear & Greed join (optional, no Rust changes): [`../scripts/README.md`](../scripts/README.md), [`../data/benchmarks/README.md`](../data/benchmarks/README.md).

Blog claim map: [`evidence/blog_evidence_links_v1.md`](evidence/blog_evidence_links_v1.md). Panel CSVs: [`../data/benchmarks/`](../data/benchmarks/).

## Other

[`EVENT_CHANNEL_TAXONOMY.md`](EVENT_CHANNEL_TAXONOMY.md) · [`findings/`](findings/)
