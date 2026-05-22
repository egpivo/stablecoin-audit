# External market data (research extension)

## Crypto Fear & Greed Index

| File | Role |
|------|------|
| `fear_greed_daily.csv` | Daily index snapshot (`date_utc`, `value`, `value_classification`, `source`) |
| `fear_greed_daily.meta.json` | Fetch metadata (`fetched_at_utc`, row count, date range) |
| `window_sentiment_summary.csv` | Per audit-window aggregates (`mean_fng`, `dominant_regime`, …) |

**Source:** [Alternative.me Crypto Fear & Greed Index](https://alternative.me/crypto/fear-and-greed-index/) via `https://api.alternative.me/fng/`.

**Refresh:**

```bash
python3 scripts/fetch_fear_greed.py
python3 scripts/join_window_sentiment.py
python3 scripts/build_market_conditioned_panel.py
```

## Claims boundary

- **Association only** — no causality, trading signal, or stablecoin safety score.
- F&G is a **coarse market regime proxy**, not pure sentiment.
- On-chain metrics remain **deployment-local** (see main repo README).

**Window dates:** `from_utc` inclusive, `to_utc` **exclusive** when joining daily F&G to audit windows.
