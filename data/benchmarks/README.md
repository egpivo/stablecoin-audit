# Benchmark manifests (multi-window research)

| File | Role |
|------|------|
| `windows.csv` | Catalog of audit windows → `benchmark_dir` (published or pending) |
| `market_conditioned_audit.csv` | Panel: window sentiment × per-chain audit metrics |
| `RUN_ADDITIONAL_WINDOWS.md` | Copy-paste `resolve-window` / `transfer-audit` for greed + extreme-fear weeks |

**Registered windows (2026-05-16):**

| `window_id` | F&G 7d mean (approx.) | Audit artifacts |
|-------------|----------------------:|-----------------|
| `usdc_7d_20241117_20241124` | 87.9 (Extreme Greed) | Pending |
| `usdc_7d_20260218_20260225` | 7.7 (Extreme Fear) | Pending |
| `usdc_7d_20260501_20260508` | 42.1 (Fear) | Published |

Add a row per new run under `docs/benchmarks/<window_id>/`, then re-run:

```bash
python3 scripts/join_window_sentiment.py
python3 scripts/build_market_conditioned_panel.py
```

Target for regime comparisons: **≥ 3 windows** spanning different F&G bins (see `.local/research/market-conditioned-stablecoin-audit-plan.md`).
