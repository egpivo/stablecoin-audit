# USDC 7-day benchmark (`usdc_7d_20260501_20260508`)

UTC wall-clock window **2026-05-01T00:00:00Z → 2026-05-08T00:00:00Z** on Ethereum, Base, and Arbitrum (per-chain block spans from `resolve-window`).

| Chain | from_block | resolved end |
|-------|------------|--------------|
| arbitrum | 458085624 | 460491249 |
| base | 45402127 | 45704526 |
| ethereum | 24996368 | 25046605 |

**Results:** all chains **supply_invariant PASS** (see `cross_chain_summary.md`).

Full run outputs (including ~6GB `decoded_transfers.csv`) stay under `out/usdc/runs/<run_id>/` locally and are **not** committed. To reproduce:

```bash
cargo run -- resolve-window --chains ethereum,base,arbitrum \
  --from 2026-05-01T00:00:00Z --to 2026-05-08T00:00:00Z
# paste transfer-audit command from stdout with --run-id usdc_7d_20260501_20260508
cargo run -- cross-chain-summary --asset USDC --run-id usdc_7d_20260501_20260508
```
