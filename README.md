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

See [`scripts/README.md`](scripts/README.md), [`data/external/README.md`](data/external/README.md), and [`data/benchmarks/README.md`](data/benchmarks/README.md). Add rows to [`data/benchmarks/windows.csv`](data/benchmarks/windows.csv) for each published `docs/benchmarks/<window_id>/` run.

## Commands (0.1.0)

| Command | Role |
|---------|------|
| `transfer-audit` | Fetch Transfer logs, dedup, supply invariant, QA artifacts |
| `cross-chain-summary` | Roll up one `run_id` into `cross_chain_summary.{md,json}` |
| `resolve-window` | UTC interval → per-chain block bounds |
| `metadata` | IERC-20 metadata + pinned `totalSupply` probes |

**Resume:** same `--run-id` and `--window` args continue from `checkpoint/`; `--fresh` clears checkpoints. Default `--chunk-size` is 500 blocks per `eth_getLogs` call (5 retries on transient RPC errors).

**Experimental** (`cargo build --features experimental`): `fetch`, `report`, `control-audit`, `control-report`.

## Interpretation

- **Supply invariant FAIL** means the accounting identity did not hold under this tool’s definitions — not automatically fraud or depeg.
- **Cross-chain tables** compare per-deployment metrics on one schema; summing `totalSupply` across chains is not circulating supply (bridged inventory double-counts).
