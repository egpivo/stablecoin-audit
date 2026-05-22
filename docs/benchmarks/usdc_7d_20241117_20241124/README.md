# UTC-aligned 7-day benchmark — USDC (2024-11-17 → 2024-11-24) — **pending**

**Run id:** `usdc_7d_20241117_20241124`
**Asset:** native USDC on Ethereum, Base, and Arbitrum
**Wall-clock interval:** `2024-11-17T00:00:00Z` → `2024-11-24T00:00:00Z` (`to` exclusive)

## Why this window (Fear & Greed stratification)

Pre-registered for the market-conditioned research extension. Over this span, daily Crypto Fear & Greed averaged about **87.9** (min 82, max 94), dominant label **Extreme Greed** (`analysis_regime`: greed). Contrasts with the published fear-week benchmark `usdc_7d_20260501_20260508` and the pending extreme-fear week `usdc_7d_20260218_20260225`.

F&G is a **market regime proxy** only—association studies, not causality or safety scores.

## Status

**Artifacts not yet committed.** Run `transfer-audit` + `cross-chain-summary`, then publish—see [`data/benchmarks/RUN_ADDITIONAL_WINDOWS.md`](../../../data/benchmarks/RUN_ADDITIONAL_WINDOWS.md).

## Commands

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2024-11-17T00:00:00Z \
  --to 2024-11-24T00:00:00Z

cargo run --release -- transfer-audit --asset USDC \
  --run-id usdc_7d_20241117_20241124 \
  --window arbitrum:275231008:277637793 \
  --window base:22506127:22808526 \
  --window ethereum:21203704:21253879

cargo run --release -- cross-chain-summary --asset USDC \
  --run-id usdc_7d_20241117_20241124

./scripts/publish_benchmark.sh usdc_7d_20241117_20241124
```

After publish, expect the same file layout as [`usdc_7d_20260501_20260508`](../usdc_7d_20260501_20260508/README.md).
