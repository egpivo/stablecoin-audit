# UTC-aligned 7-day benchmark — USDC (2026-05-01 → 2026-05-08)

**Run id:** `usdc_7d_20260501_20260508`
**Asset:** native USDC on Ethereum, Base, and Arbitrum
**Wall-clock interval:** `2026-05-01T00:00:00Z` through `2026-05-08T00:00:00Z` (per-chain block heights differ; resolved via `resolve-window`)

This directory holds the **checked-in summary artifacts** for one completed v0.1 run. The run validates that the audit pipeline can ingest real chain data, stamp provenance, and evaluate the supply invariant gate on three deployments under one schema. It is **not** a reserve attestation, liquidity study, or market conclusion.

## Commands used

**1. Resolve UTC bounds to per-chain block windows** (RPC: `eth_blockNumber`, `eth_getBlockByNumber` only):

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-05-01T00:00:00Z \
  --to 2026-05-08T00:00:00Z
```

**2. Transfer audit** (RPC: metadata, historical `totalSupply`, chunked `eth_getLogs`, decode, dedup, invariant):

```bash
cargo run --release -- transfer-audit \
  --asset USDC \
  --run-id usdc_7d_20260501_20260508 \
  --window arbitrum:458085624:460491249 \
  --window base:45402127:45704526 \
  --window ethereum:24996368:25046605
```

**3. Cross-chain rollup** (reads only this run’s `qa_report.json` + `supply_audit.csv`):

```bash
cargo run --release -- cross-chain-summary \
  --asset USDC \
  --run-id usdc_7d_20260501_20260508
```

Note: `resolve-window` output should be re-run if chain head or RPC provider changes; block numbers above match the run that produced the artifacts in this folder.

## Resolved per-chain block windows

| Chain | `from_block` | `to_block` (resolved) | Window start (header) | Window end (header) |
|-------|-------------:|----------------------:|-----------------------|---------------------|
| arbitrum | 458085624 | 460491249 | 2026-05-01T00:00:00Z | 2026-05-08T00:00:00Z |
| base | 45402127 | 45704526 | 2026-05-01T00:00:01Z | 2026-05-07T23:59:59Z |
| ethereum | 24996368 | 25046605 | 2026-05-01T00:00:11Z | 2026-05-07T23:59:59Z |

Source: `provenance.json` in this directory (`transfer-audit-provenance-v1`).

## Output artifacts

### Committed here (`docs/benchmarks/usdc_7d_20260501_20260508/`)

| File | Role |
|------|------|
| `cross_chain_summary.md` / `cross_chain_summary.json` | Per-chain table, QA gate strings, signed delta sum |
| `supply_audit.csv` / `supply_audit.md` | Per-chain counts, boundary supply, invariant fields |
| `qa_report.json` | Machine-readable gates + provenance |
| `provenance.json` | Per-chain windows and header timestamps |
| `summary.md` | Human-readable transfer-audit summary |

### Local only (`out/usdc/runs/usdc_7d_20260501_20260508/`)

| File | Role |
|------|------|
| `decoded_transfers.csv` | Full deduped Transfer rows (~6GB for this run) |
| `checkpoint/` | Resume state if a run is interrupted (optional) |

`decoded_transfers.csv` is **not** committed. The repository `.gitignore` excludes `out/`. Reproduce it by re-running step 2 with the same `--run-id` and `--window` arguments (and RPC access configured in token config / `.env`).

## Key QA result

On this run, **all three chains reported `supply_invariant_pass: PASS`**, together with PASS on metadata, historical supply, decode, no-duplicate logs, and provenance stamp (see `cross_chain_summary.md` and `qa_report.json`).

| Chain | Transfers (deduped) | Supply invariant |
|-------|------------------:|----------------|
| arbitrum | 4,866,248 | PASS |
| base | 17,208,596 | PASS |
| ethereum | 3,130,346 | PASS |

A PASS means mint/burn aggregates match the change in `totalSupply` at the pinned boundary blocks **under this tool’s definitions**. It does not, by itself, establish issuer solvency or cross-chain token conservation.

## What this benchmark does not claim

This run is **not**:

- a **reserve audit** or attestation of off-chain backing
- a **liquidity** or market-depth analysis
- a **purchasing-power** or FX / peg study
- a **holder census** or wallet-identification exercise
- a **payment-volume** or economic-activity estimate
- a **cross-chain circulating-supply** proof (summing `totalSupply` across deployments double-counts bridged inventory)

For interpretation rules, see the root [README.md](../../../README.md) (*Conservative interpretation*).

## Reproduction

To reproduce the full run directory including `decoded_transfers.csv`:

1. Configure RPC URLs (see token YAML and `.env`; `.env` is gitignored).
2. Run the three commands in [Commands used](#commands-used).
3. Expect long RPC time on Arbitrum/Base log volume; use checkpoint resume (same `--run-id`, no `--fresh`) if interrupted.

No need to re-run for repository readers who only need the published tables in this folder.
