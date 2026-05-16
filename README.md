# stablecoin-audit

[![CI](https://github.com/egpivo/stablecoin-audit/actions/workflows/ci.yml/badge.svg)](https://github.com/egpivo/stablecoin-audit/actions/workflows/ci.yml)
[![codecov](https://codecov.io/github/egpivo/stablecoin-audit/graph/badge.svg?token=mN0h7zLOtR)](https://codecov.io/github/egpivo/stablecoin-audit)

**v0.1** — CLI for **windowed supply-invariant audits** and **cross-chain comparison** of the same stablecoin symbol on multiple EVM deployments. Inside your declared block windows, it checks whether mint/burn aggregates match `totalSupply` at pinned boundaries.

**Not in scope:** reserves, peg, purchasing power, liquidity, oracles, bridge backing, holder census, wallet attribution, or chain safety rankings.

## Quick start

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

## Checked-in benchmark

Published tables and QA for USDC (2026-05-01 → 2026-05-08 UTC): [`docs/benchmarks/usdc_7d_20260501_20260508/`](docs/benchmarks/usdc_7d_20260501_20260508/). Supply invariant **PASS** on Ethereum, Base, and Arbitrum for that run. Full transfer CSVs are reproducible locally, not committed.

## Commands (v0.1)

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
