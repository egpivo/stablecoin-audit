# UTC-aligned 7-day benchmark — USDC (2026-02-18 → 2026-02-25) — **pending**

**Run id:** `usdc_7d_20260218_20260225`
**Asset:** native USDC on Ethereum, Base, and Arbitrum
**Wall-clock interval:** `2026-02-18T00:00:00Z` → `2026-02-25T00:00:00Z` (`to` exclusive)

## Why this window (Fear & Greed stratification)

Pre-registered for the market-conditioned research extension. Over this span, daily Crypto Fear & Greed averaged about **7.7** (min 5, max 9), dominant label **Extreme Fear** (`analysis_regime`: fear). Contrasts with the greed week `usdc_7d_20241117_20241124` and the published fear-week benchmark `usdc_7d_20260501_20260508` (higher mean F&G, same fear bin).

F&G is a **market regime proxy** only—association studies, not causality or safety scores.

## Status

**Artifacts not yet committed.** Run `transfer-audit` + `cross-chain-summary`, then publish—see [`data/benchmarks/RUN_ADDITIONAL_WINDOWS.md`](../../../data/benchmarks/RUN_ADDITIONAL_WINDOWS.md).

## Commands

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-02-18T00:00:00Z \
  --to 2026-02-25T00:00:00Z

cargo run --release -- transfer-audit \
  --asset USDC \
  --run-id usdc_7d_20260218_20260225 \
  --window arbitrum:FROM:TO \
  --window base:FROM:TO \
  --window ethereum:FROM:TO

cargo run --release -- cross-chain-summary \
  --asset USDC \
  --run-id usdc_7d_20260218_20260225

./scripts/publish_benchmark.sh usdc_7d_20260218_20260225
```

After publish, expect the same file layout as [`usdc_7d_20260501_20260508`](../usdc_7d_20260501_20260508/README.md).
