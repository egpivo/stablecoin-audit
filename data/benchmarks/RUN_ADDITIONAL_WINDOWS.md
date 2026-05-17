# Run and publish additional USDC benchmark windows

Three windows are registered in [`windows.csv`](windows.csv) for **market-conditioned** research (Fear & Greed stratification). Only `usdc_7d_20260501_20260508` is published today; the other two are **pre-registered** UTC spans chosen from `fear_greed_daily.csv`.

| `window_id` | UTC span (`from` inclusive, `to` exclusive) | F&G (7d mean) | Status |
|-------------|-----------------------------------------------|---------------|--------|
| `usdc_7d_20241117_20241124` | 2024-11-17 → 2024-11-24 | ~**87.9** (Extreme Greed) | **Pending** `transfer-audit` |
| `usdc_7d_20260218_20260225` | 2026-02-18 → 2026-02-25 | ~**7.7** (Extreme Fear) | **Pending** `transfer-audit` |
| `usdc_7d_20260501_20260508` | 2026-05-01 → 2026-05-08 | ~**42.1** (Fear) | **Published** |

Re-scan candidates anytime:

```bash
python3 scripts/suggest_fng_windows.py
```

---

## Per-window workflow (greed + extreme-fear examples)

Replace `WINDOW_ID`, `FROM`, and `TO` for each row.

### 1. Resolve blocks

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2024-11-17T00:00:00Z \
  --to 2024-11-24T00:00:00Z
```

Paste the printed `--window chain:from:to` lines into step 2.

### 2. Transfer audit

```bash
cargo run --release -- transfer-audit \
  --asset USDC \
  --run-id usdc_7d_20241117_20241124 \
  --window arbitrum:FROM:TO \
  --window base:FROM:TO \
  --window ethereum:FROM:TO
```

### 3. Cross-chain summary

```bash
cargo run --release -- cross-chain-summary \
  --asset USDC \
  --run-id usdc_7d_20241117_20241124
```

### 4. Publish into `docs/benchmarks/`

Copy from `out/usdc/runs/<run_id>/` into `docs/benchmarks/<run_id>/`:

- `supply_audit.csv`, `supply_audit.md`
- `cross_chain_summary.json`, `cross_chain_summary.md`
- `qa_report.json`, `provenance.json`, `summary.md`

Or use the helper:

```bash
./scripts/publish_benchmark.sh usdc_7d_20241117_20241124
```

### 5. Refresh sentiment join

```bash
python3 scripts/join_window_sentiment.py
python3 scripts/build_market_conditioned_panel.py
```

---

## Concrete commands (copy-paste)

**Greed week (2024-11-17 → 2024-11-24):**

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2024-11-17T00:00:00Z \
  --to 2024-11-24T00:00:00Z

# then transfer-audit + cross-chain-summary with --run-id usdc_7d_20241117_20241124
```

**Extreme-fear week (2026-02-18 → 2026-02-25):**

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-02-18T00:00:00Z \
  --to 2026-02-25T00:00:00Z

# then transfer-audit + cross-chain-summary with --run-id usdc_7d_20260218_20260225
```

---

## Claims reminder

F&G labels stratify **market regime** for association studies only—not causality, safety scores, or trading signals. On-chain metrics stay deployment-local.
