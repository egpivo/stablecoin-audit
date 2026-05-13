# Architecture

`stablecoin-audit` is a CLI tool that runs reproducible, windowed audits of stablecoin supply and issuer control events across EVM chains. It is structured in seven layers.

## Layer 1 — Config / Registry

- **Location**: `configs/tokens/<asset>.<chain>.yml`
- One YAML file per (asset, chain) pair.
- Each file contains the contract address, decimals, issuer, deployment block, expected interfaces, and the name of the environment variable that holds the RPC URL.
- Loaded per-chain by `config::load_single_token_config(asset, chain)` inside the fetch loop. A missing or malformed YAML for one chain is captured as a per-chain error; other chains continue.
- The registry is intentionally file-based so diffs are auditable in git.

## Layer 2 — Fetch

- **Location**: `src/rpc/`
- Builds an HTTP provider from `RootProvider<Http<reqwest::Client>>` using `alloy`.
- **Chain identity check (hard per-chain precondition)**: calls `eth_chainId` and compares against `config.chain_id` before any contract call. A mismatch or RPC failure sets all gates to `[FAIL]` for that chain, skips remaining calls, and causes the command to exit nonzero after writing the partial report. This prevents silently auditing the wrong chain due to a miswired `.env` URL.
- **End block resolution (hard per-chain precondition)**: resolves `--to-block latest` via `get_block_number()`. Failure is treated the same way — chain result is written with an error, remaining chains continue, command exits nonzero at the end.
- All other `eth_call` failures (individual metadata or historical supply calls) are isolated: the error string is captured, the field is set to `None`, and the run continues.
- A partial report is always written before any nonzero exit so successful chain results are not lost.

## Layer 3 — Decode

- **Location**: `src/rpc/metadata.rs` and future `src/rpc/logs.rs`
- The `sol!` macro generates typed call builders and return structs from Solidity interface definitions.
- `U256` totals are converted to decimal strings via `report::format_token_amount`.
- Future milestones will decode `Transfer` log topics and data, classifying each as mint / burn / transfer based on the zero address.

## Layer 4 — Reconstruction

- Not active in Milestone 1.
- Planned: accumulate mints and burns over the block window, reconstruct the running supply, and compare against on-chain `totalSupply` snapshots.

## Layer 5 — QA Gates

- **Location**: `src/rpc/metadata.rs` (inline), future `src/qa.rs`
- Gates are boolean flags stored on each `ChainMetadata` record.
- `metadata_call_pass`: all four ERC-20 view calls succeeded.
- `historical_supply_pass`: both boundary `totalSupply` values are present. The `start_block - 1` value may be a synthetic pre-deployment zero (see AUDIT_GATES.md Gate 2 for provenance rules); the end-block value must come from an on-chain call.
- Gates are evaluated per-chain and aggregated into the report.

## Layer 6 — Reports

- **Location**: `src/report/mod.rs`, output in `out/<asset>/`
- JSON output written to `out/<asset>/metadata.json`.
- `serde_json` with `preserve_order` feature keeps field order stable across runs.
- Human-readable summary printed to stdout with comma-formatted token amounts.
- Future milestones add CSV transfer logs and a cross-chain markdown summary.

## Layer 7 — Cross-chain Summary

- Not active in Milestone 1.
- Planned: aggregate `ChainAuditResult` records from all chains into a single `CrossChainSummary` JSON and a markdown table, comparing circulating supply across chains for the same block window.

## Data Flow

```
.env + configs/tokens/*.yml
        |
        v  (per chain — failures captured, run continues)
config::load_single_token_config()
        |
        v
rpc::build_provider()  <-- per chain
        |
        v
IERC20 eth_calls (name / symbol / decimals / totalSupply @ blocks)
        |
        v
ChainMetadata { ..., metadata_call_pass, historical_supply_pass }
        |
        v
MetadataReport { asset, generated_at, chains: Vec<ChainMetadata> }
        |
     /     \
stdout    out/<asset>/metadata.json
```
