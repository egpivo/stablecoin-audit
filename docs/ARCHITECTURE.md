# Architecture

`stablecoin-audit` is a CLI for **v0.1 windowed supply-invariant audits** and **chain-level comparison** of the same token symbol on multiple EVM deployments. Optional **`--features experimental`** adds fetch/report and issuer control surfaces. The code is organized in layers.

## Layer 1 — Config / Registry

- **Location**: `configs/tokens/<asset>.<chain>.yml`
- One YAML file per (asset, chain) pair.
- Each file contains the contract address, decimals, issuer, deployment block, expected interfaces, and the name of the environment variable that holds the RPC URL.
- Loaded per-chain by `config::load_single_token_config(asset, chain)` inside the fetch loop. A missing or malformed YAML for one chain is captured as a per-chain error; other chains continue.
- The registry is intentionally file-based so diffs are auditable in git.

## Layer 2 — Fetch

- **Location**: `src/rpc/` (providers, block queries, log fetch helpers)
- **`resolve-window` (v0.1):** `src/rpc/resolve_window.rs` — maps `--from` / `--to` RFC3339 UTC bounds to per-chain block heights via binary search on block header timestamps only (no `eth_getLogs`).
- Builds an HTTP provider from `RootProvider<Http<reqwest::Client>>` using `alloy`.
- **Chain identity check (hard per-chain precondition)**: calls `eth_chainId` and compares against `config.chain_id` before any contract call. A mismatch or RPC failure sets all gates to `[FAIL]` for that chain, skips remaining calls, and causes the command to exit nonzero after writing the partial report. This prevents silently auditing the wrong chain due to a miswired `.env` URL.
- **End block resolution (hard per-chain precondition)**: resolves `--to-block latest` via `get_block_number()`. Failure is treated the same way — chain result is written with an error, remaining chains continue, command exits nonzero at the end.
- All other `eth_call` failures (individual metadata or historical supply calls) are isolated: the error string is captured, the field is set to `None`, and the run continues.
- A partial report is always written before any nonzero exit so successful chain results are not lost.

## Layer 3 — Decode

- **Location**: `src/decode/mod.rs`
- Transfer logs are decoded to typed events; mint/burn/transfer classification uses the zero address rule.
- `U256` totals are converted to decimal strings via `report::format_token_amount` where appropriate.
- Used by **v0.1 `transfer-audit`** and experimental **`fetch`**.

## Layer 4 — Reconstruction

- Not a full holder reconstruction.
- **`transfer-audit`** (v0.1) performs window-scoped supply reconciliation: mint/burn aggregates vs pinned `totalSupply` boundaries; outputs are under `out/<asset>/runs/<run_id>/`.

## Layer 5 — QA Gates

- **Location**: `src/rpc/metadata.rs` (inline), future `src/qa.rs`
- Gates are boolean flags stored on each `ChainMetadata` record.
- `metadata_call_pass`: all four ERC-20 view calls succeeded.
- `historical_supply_pass`: both boundary `totalSupply` values are present. The `start_block - 1` value may be a synthetic pre-deployment zero (see AUDIT_GATES.md Gate 2 for provenance rules); the end-block value must come from an on-chain call.
- Gates are evaluated per-chain and aggregated into the report.

## Layer 6 — Reports

- **Location**: `src/report/mod.rs`
- **Metadata (always):** `out/<asset>/metadata.json` plus stdout.
- **`transfer-audit` (v0.1):** `out/<asset>/runs/<run_id>/` — `decoded_transfers.csv`, `supply_audit.csv`, `supply_audit.md`, `qa_report.json`, `provenance.json` (`transfer-audit-provenance-v1`, includes per-chain block header timestamps), `summary.md`. Optional `--run-id`; default is a UTC timestamp id. Use `--window chain:from:to` (repeatable) for per-chain native block spans, or `--chains` + `--from-block` + `--to-block` for one numeric window on every chain.
- **Experimental `fetch`** (`rpc::fetch_logs`): under `out/<asset>/` — chunked `eth_getLogs` for **Transfer** + **issuer control** topics, `fetch_report.json`, `transfers_<chain>.csv`, `control_events_<chain>.csv`, **`risk_flags.md`**.
- **Experimental `control-audit` / `control-report`:** control-surface and benchmark artifacts (not v0.1 core).

## Layer 7 — Cross-chain Summary

- **v0.1:** `cross-chain-summary --asset <SYM> --run-id <id>` reads **`out/<asset>/runs/<run_id>/qa_report.json`** and **`supply_audit.csv` only** (no silent fallback to other runs). Requires **≥ 2 chains**, per-chain QA vs supply alignment, and either a single global provenance window or `per_chain_spans`. Writes `cross_chain_summary.json` (`schema_version: 2`) and `cross_chain_summary.md` into **that same run directory**. Same scope limits as transfer-audit: **chain-level comparison** of on-chain accounting, not reserves, peg, purchasing power, or holder counts.

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
