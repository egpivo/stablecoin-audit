# Research scripts (optional; no Rust core changes)

## Market-conditioned audit (Fear & Greed join)

```bash
# 1) Daily index (network)
python3 scripts/fetch_fear_greed.py

# 2) Window-level sentiment (needs data/benchmarks/windows.csv)
python3 scripts/join_window_sentiment.py

# 3) Panel: sentiment + supply_audit metrics per chain
python3 scripts/build_market_conditioned_panel.py
```

Outputs:

- `data/external/fear_greed_daily.csv`
- `data/external/window_sentiment_summary.csv`
- `data/benchmarks/market_conditioned_audit.csv`

**Suggest F&G windows** (no RPC):

```bash
python3 scripts/suggest_fng_windows.py
```

**Publish** a completed run into `docs/benchmarks/`:

```bash
chmod +x scripts/publish_benchmark.sh   # once
./scripts/publish_benchmark.sh usdc_7d_20241117_20241124
```

See [`data/benchmarks/RUN_ADDITIONAL_WINDOWS.md`](../data/benchmarks/RUN_ADDITIONAL_WINDOWS.md).

Blog figures (separate): `python3 .local/blog/generate_blog_figures.py`
