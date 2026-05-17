# Benchmark manifests (multi-window research)

| File | Role |
|------|------|
| `windows.csv` | Catalog of completed audit windows → `benchmark_dir` with `supply_audit.csv` |
| `market_conditioned_audit.csv` | Panel: window sentiment × per-chain audit metrics |

Add a row per published run under `docs/benchmarks/<window_id>/`, then re-run:

```bash
python3 scripts/join_window_sentiment.py
python3 scripts/build_market_conditioned_panel.py
```

Target for regime comparisons: **≥ 3 windows** spanning different F&G bins (see `.local/research/market-conditioned-stablecoin-audit-plan.md`).
