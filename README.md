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
- **Resume:** checkpoints under `runs/<run_id>/checkpoint/`. After each successful `eth_getLogs` chunk (~`--chunk-size` blocks, default 500), progress is written to `fetch_progress_<chain>.json` and decoded rows append to `fetch_partial_<chain>.csv`; when a chain finishes, `transfers_<chain>.csv` + `chain_<chain>.json` are written and in-flight fetch files are removed. Re-run the **same** command with the same `--run-id` and `--window` args to resume mid-chain or skip completed chains. Use **`--fresh`** to discard all checkpoints. `eth_getLogs` retries up to 5 times per chunk on transient RPC errors.

### Expected files (under `out/usdc/runs/<run_id>/` when `--asset USDC`)

| File | Role |
|------|------|
| `decoded_transfers.csv` | Deduped Transfer events (window); often large; **gitignored** under `out/` |
| `supply_audit.csv` / `supply_audit.md` | Per-chain counts, boundaries, invariant |
| `qa_report.json` | QA gate strings + provenance + `run_id` |
| `provenance.json` | Per-chain windows + block header timestamps (RFC 3339) |
| `summary.md` | Human-readable run summary |
| `cross_chain_summary.md` / `.json` | After `cross-chain-summary` on the same `run_id` |

Inspect **`qa_report.json`** → `chains[].gates` for PASS/FAIL only.

---

## Example A — Engineering smoke test

**Purpose:** confirm RPC connectivity, log fetch, decode, deduplication, boundary `totalSupply`, and supply-invariant math on **small, fixed block spans**. This is a pipeline check, not a calendar-window benchmark and not a market or stability finding.

In a small real-data smoke test across Ethereum, Base, and Arbitrum, all three USDC deployments passed the supply invariant gate. That result validates the audit pipeline and provenance outputs under the declared windows. It is **not** a reserve, liquidity, purchasing-power, or holder analysis.

```bash
cargo run -- transfer-audit --asset USDC --run-id smoke_001 \
  --window ethereum:24000000:24001000 \
  --window base:30000000:30001000 \
  --window arbitrum:330000000:330001000

cargo run -- cross-chain-summary --asset USDC --run-id smoke_001
```

Outputs: `out/usdc/runs/smoke_001/`. Older copies under `out/usdc/` (without `runs/<run_id>/`) are legacy layout only.

---

## Example B — UTC-aligned 7-day benchmark (USDC 2026-05-01 → 2026-05-08)

**Purpose:** audit native USDC on Ethereum, Base, and Arbitrum over the **same UTC wall-clock interval**, with **per-chain block windows** from `resolve-window`. This is the first checked-in v0.1 benchmark suitable for README or article citation, subject to the scope limits below.

**Published artifacts:** [`docs/benchmarks/usdc_7d_20260501_20260508/`](docs/benchmarks/usdc_7d_20260501_20260508/) (summary tables, QA JSON, provenance). Full `decoded_transfers.csv` (~6GB) remains local under `out/` and is reproducible from the commands in that README; it is not committed.

**Result (this run):** supply invariant **PASS** on all three chains. See `cross_chain_summary.md` in the benchmark directory for per-chain transfer counts and gate strings.

**Flow:**

```bash
# 1) UTC → per-chain blocks (copy-paste transfer-audit line from stdout)
cargo run -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-05-01T00:00:00Z \
  --to 2026-05-08T00:00:00Z

# 2) Audit (windows below match the completed benchmark run)
cargo run --release -- transfer-audit \
  --asset USDC \
  --run-id usdc_7d_20260501_20260508 \
  --window arbitrum:458085624:460491249 \
  --window base:45402127:45704526 \
  --window ethereum:24996368:25046605

# 3) Rollup
cargo run -- cross-chain-summary --asset USDC --run-id usdc_7d_20260501_20260508
```

Do **not** re-run this benchmark unless you need reproduction or a new `run_id`. Header times are 1s granularity; resolved end blocks are the last block at or before `--to` on each chain.

---

## `resolve-window` (reference)

For multi-chain work you usually want the **same UTC interval** on each chain, but block numbers differ. **`resolve-window`** only calls `eth_blockNumber` and `eth_getBlockByNumber` (no logs, no supply math): binary search finds the smallest block whose header time is **≥ `--from`**, and the largest whose header time is **≤ `--to`**, per chain.

```bash
cargo run -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from <RFC3339> \
  --to <RFC3339>
```

Stdout includes a copy-paste `transfer-audit ... --window chain:start:end ...` command.

### Shared numeric window (only when intentional)

```bash
cargo run -- transfer-audit \
  --asset USDC --chains ethereum,base,arbitrum \
  --from-block <n> --to-block <n|latest>
```

---

## Conservative interpretation

- **Supply invariant:** for each deployment and block window, the tool compares mint/burn aggregates to the change in `totalSupply` at pinned boundary blocks. A FAIL means the identity did not hold under the tool’s definitions—not automatically “fraud” or “depeg.”
- **Chain-level comparison:** tables compare **per-deployment** on-chain metrics using one schema. Summing `totalSupply` across chains is **not** circulating supply (bridged inventory double-counts).
- **Not claimed:** reserve adequacy, purchasing power, holder population, AML, intent, or issuer solvency.

## Roadmap (not in v0.1)

- **v0.2 (planned):** issuer **control-surface** audit (`control-audit` / `control-report`, experimental today). v0.1 does not include this in the default build or benchmark claims.

## Experimental commands (`--features experimental`)

- **`fetch`** — chunked transfers + issuer control-topic logs, CSVs, `fetch_report.json`, `risk_flags.md`
- **`report`** — bundle report from `fetch_report.json` (legacy path)
- **`control-audit` / `control-report`** — control-surface tooling (v0.2 direction; not part of v0.1 presentation)

See `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md`.
