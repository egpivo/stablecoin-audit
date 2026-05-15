# stablecoin-audit

**v0.1** is a CLI for **windowed supply-invariant audits** and **chain-level comparison** of the same token symbol on multiple EVM deployments. It checks on-chain accounting consistency (mint/burn vs `totalSupply` boundaries) inside the blocks you declare.

It does **not** prove reserves, peg, purchasing power, liquidity, oracle health, bridged-asset backing, holder counts, wallet attribution, or “which chain is safer.”

## Build

```bash
cargo build              # v0.1: metadata, resolve-window, transfer-audit, cross-chain-summary
cargo build --features experimental   # + fetch, report, control-audit, control-report
```

## Output layout (v0.1)

Each `transfer-audit` run writes to:

`out/<asset>/runs/<run_id>/`

- **`run_id`:** pass `--run-id my-smoke` or omit for an auto-generated UTC stamp (e.g. `20260513T143022_042Z`).
- **`cross-chain-summary`** always takes **`--run-id`** so it never reads stale files from another run.
- **Resume:** checkpoints live under `runs/<run_id>/checkpoint/`. After each successful `eth_getLogs` chunk (~`--chunk-size` blocks, default 500), progress is written to `fetch_progress_<chain>.json` and decoded rows append to `fetch_partial_<chain>.csv`; when a chain finishes, `transfers_<chain>.csv` + `chain_<chain>.json` are written and in-flight fetch files are removed. Re-run the **same** command with the same `--run-id` and `--window` args to resume mid-chain or skip completed chains. Use **`--fresh`** to discard all checkpoints. `eth_getLogs` retries up to 5 times per chunk on transient RPC errors. Smaller chunks improve resume granularity but add RPC round-trips (usually slower overall).

## Smoke test (engineering)

Confirm RPC, decoding, deduplication, boundary `totalSupply`, and the supply invariant—not stablecoin “conclusions.”

```bash
cargo run -- transfer-audit \
  --asset USDC \
  --window ethereum:24000000:24001000 \
  --window base:30000000:30001000 \
  --window arbitrum:330000000:330001000
```

Optional explicit run id:

```bash
cargo run -- transfer-audit --asset USDC --run-id smoke_001 \
  --window ethereum:24000000:24001000 \
  --window base:30000000:30001000 \
  --window arbitrum:330000000:330001000
```

Then:

```bash
cargo run -- cross-chain-summary --asset USDC --run-id smoke_001
```

Shared numeric window on every chain (only when that is what you intend):

```bash
cargo run -- transfer-audit \
  --asset USDC --chains ethereum,base,arbitrum \
  --from-block <n> --to-block <n|latest>
```

### Expected files (under `out/usdc/runs/<run_id>/` when `--asset USDC`)

| File | Role |
|------|------|
| `decoded_transfers.csv` | Deduped Transfer events (window) |
| `supply_audit.csv` / `supply_audit.md` | Per-chain counts, boundaries, invariant |
| `qa_report.json` | QA gate strings + provenance + `run_id` |
| `provenance.json` | `transfer-audit-provenance-v1`: per-chain windows + **block header timestamps** (RFC 3339) for window start/end blocks |
| `summary.md` | Human-readable smoke summary |

Inspect **`qa_report.json`** → `chains[].gates` for PASS/FAIL only.

### Cross-chain rollup (same run)

Requires the **same** `--run-id` as the `transfer-audit` that produced the directory (see `smoke_001` example above).

```bash
cargo run -- cross-chain-summary --asset USDC --run-id <run_id>
```

Writes `cross_chain_summary.json` and `cross_chain_summary.md` **into that run directory** (`out/usdc/runs/<run_id>/`).

## `resolve-window` (UTC → per-chain `--window`)

For multi-chain benchmarks you usually want the **same UTC wall-clock interval** on each chain, but block numbers differ. **`resolve-window`** only calls `eth_blockNumber` and `eth_getBlockByNumber` (no logs, no supply math): binary search finds the smallest block whose header time is **≥ `--from`**, and the largest whose header time is **≤ `--to`**, per chain.

```bash
cargo run -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-05-01T00:00:00Z \
  --to 2026-05-08T00:00:00Z
```

Stdout includes comment lines and a **copy-paste** `cargo run -- transfer-audit ... --window chain:start:end ...` command. Substitute `--run-id`, run the audit, then `cross-chain-summary` with that id.

**Note:** header times are 1s granularity; the resolved end block may be the last block at or before `--to` (e.g. `23:59:59Z` on the prior calendar day if no header lands exactly on `--to`).

## Example: 7-day USDC cross-chain audit

This flow audits native USDC on Ethereum, Base, and Arbitrum over the **same UTC-aligned** 7-day wall-clock window (per-chain block heights still differ).

1. `resolve-window` → paste `transfer-audit` with a chosen `--run-id` (e.g. `usdc_7d_20260501_20260508`).
2. `cross-chain-summary --asset USDC --run-id …` on that run.

The resulting artifacts support reporting **per-chain supply invariant PASS/FAIL**, **transfer / mint / burn counts**, **net_mint**, and **window-active sender/recipient counts**—under one schema. They do **not** measure reserves, holder counts, payment volume, purchasing power, or “which chain is safer.”

## Conservative interpretation

- **Supply invariant:** for each deployment and block window, the tool compares mint/burn aggregates to the change in `totalSupply` at pinned boundary blocks. A FAIL means the identity did not hold under the tool’s definitions—not automatically “fraud” or “depeg.”
- **Chain-level comparison:** tables compare **per-deployment** on-chain metrics using one schema. Summing `totalSupply` across chains is **not** circulating supply (bridged inventory double-counts).
- **Not claimed:** reserve adequacy, purchasing power, holder population, AML, intent, or issuer solvency.

## Experimental commands (`--features experimental`)

- **`fetch`** — chunked transfers + issuer control-topic logs, CSVs, `fetch_report.json`, `risk_flags.md`
- **`report`** — bundle report from `fetch_report.json` (legacy path; may also write `provenance.json` / `summary.md` in `out/<asset>/`)
- **`control-audit` / `control-report`** — issuer control surface (v0.2-style); not part of v0.1 core

See `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md`.
