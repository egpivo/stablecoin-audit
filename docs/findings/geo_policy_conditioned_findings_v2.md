# Geo-policy-conditioned stablecoin audit — findings v2

**Generated:** 2026-05-20

## 1. Scope and claim boundary

Stablecoins may be price-stable, but their on-chain rails are not necessarily quiet.

This memo is a descriptive geo-policy-conditioned USDC audit. It is not price analysis, arbitrage analysis, a trading signal, a stablecoin safety score, a reserve-adequacy review, or a depeg-probability model.

Price is the final compression layer. This audit starts one layer earlier: chain-local transfer activity, deployment-local supply delta, and gross mint/burn churn. Fear & Greed values and U.S. policy or macro events label audit windows; they do not explain flows. The evidence in this memo is the mined on-chain audit metrics.

## 2. Data inventory used in this memo

Sources used:

| Source | Use |
|---|---|
| `data/benchmarks/market_conditioned_audit.csv` | Chain-window audit metrics, F&G labels, transfer counts, net supply deltas, gross-to-net ratios |
| `data/external/window_sentiment_summary.csv` | F&G coverage and window-level sentiment summaries |
| `data/external/window_event_context.csv` | Policy and macro event annotations by window |
| `data/benchmarks/data_completeness.csv` | Completeness, gross-field availability, and invariant status |
| `docs/benchmarks/<window_id>/supply_audit.csv` | Supporting audit artifact location where chain-window detail is needed |

Inventory:

| Item | Count / status |
|---|---:|
| USDC windows | 6 |
| Chains per window | 3: Ethereum, Base, Arbitrum |
| Chain-window rows | 18 |
| Supply invariant gates | PASS on all 18 rows |
| Gross fields | Available for all windows |
| F&G coverage | 7/7 days for all 6 windows |
| Event-window pairs | 8 |

## 3. Window context table

| window_id | date range | F&G mean | F&G regime | policy/macro event context | event position | audit status |
|---|---:|---:|---|---|---|---|
| `usdc_7d_20241117_20241124` | 2024-11-17 to 2024-11-24 | 87.86 | Extreme Greed | none | none | PASS |
| `usdc_7d_20260218_20260225` | 2026-02-18 to 2026-02-25 | 7.71 | Extreme Fear | none | none | PASS |
| `usdc_7d_20260501_20260508` | 2026-05-01 to 2026-05-08 | 42.14 | Fear | CLARITY committee advance; Fed transition context after window | post-window | PASS |
| `usdc_7d_20260507_20260514` | 2026-05-07 to 2026-05-14 | 44.14 | Neutral | CLARITY at window end; Fed transition context after window | post-window | PASS |
| `usdc_7d_20260514_20260521` | 2026-05-14 to 2026-05-21 | 30.71 | Fear | CLARITY and Fed transition context inside window | within | PASS |
| `usdc_7d_20260512_20260519` | 2026-05-12 to 2026-05-19 | 36.29 | Fear | CLARITY inside window; Fed transition context at window end | within | PASS |

## 4. Signal movement table

Net deltas below are deployment-local USDC deltas, rounded to millions. The overall direction sums the three audited deployments only; it is not global USDC circulating supply.

| window_id | F&G regime | event context | transfer leader | net supply leader | Ethereum net delta | Base net delta | Arbitrum net delta | overall direction |
|---|---|---|---|---|---:|---:|---:|---|
| `usdc_7d_20241117_20241124` | Extreme Greed | none | Base | Ethereum expansion | +1,270.64M | -40.48M | +82.69M | net expansion |
| `usdc_7d_20260218_20260225` | Extreme Fear | none | Base | Ethereum expansion | +1,360.99M | +42.74M | -60.73M | net expansion |
| `usdc_7d_20260501_20260508` | Fear | post-window policy/macro | Base | Ethereum expansion | +1,219.11M | -40.68M | +216.19M | net expansion |
| `usdc_7d_20260507_20260514` | Neutral | post-window policy/macro | Base | Ethereum contraction | -863.97M | -134.43M | +66.09M | net contraction |
| `usdc_7d_20260514_20260521` | Fear | within-window policy/macro | Base | Arbitrum expansion | +31.75M | -41.86M | +76.64M | net expansion |
| `usdc_7d_20260512_20260519` | Fear | within-window policy/macro | Base | Ethereum contraction | -254.79M | -132.04M | +41.53M | net contraction |

Patterns visible in this panel:

1. Base is the transfer leader in every audited window.
2. Net supply leadership changes across windows: Ethereum expansion dominates several windows, Ethereum contraction dominates two May windows, and Arbitrum is the largest positive expansion in `usdc_7d_20260514_20260521`.
3. Ethereum flips from large deployment-local net expansion in several windows to major contraction in the Neutral pre-CLARITY window.
4. The May windows show dynamic rails: transfer leadership stays stable, but net supply direction and gross churn do not stay static.

## 5. Gross-to-net churn table

Gross-to-net ratio is gross mint plus burn movement relative to absolute net supply delta. High gross-to-net does not mean higher risk. It means net supply delta is hiding larger two-sided mint/burn movement.

| window_id | chain | gross_to_net_ratio | net supply delta | note |
|---|---|---:|---:|---|
| `usdc_7d_20260514_20260521` | Ethereum | 191.87 | +31.75M | Very high churn despite modest positive net delta |
| `usdc_7d_20260512_20260519` | Ethereum | 35.20 | -254.79M | Elevated two-sided movement during net contraction |
| `usdc_7d_20260501_20260508` | Base | 21.15 | -40.68M | High gross churn with relatively small Base contraction |
| `usdc_7d_20260218_20260225` | Base | 18.52 | +42.74M | High gross churn with modest Base expansion |
| `usdc_7d_20260514_20260521` | Base | 15.85 | -41.86M | High gross churn alongside Base contraction |
| `usdc_7d_20260512_20260519` | Arbitrum | 24.67 | +41.53M | Elevated churn with modest Arbitrum expansion |
| `usdc_7d_20260218_20260225` | Arbitrum | 17.74 | -60.73M | Elevated churn during Arbitrum contraction |
| `usdc_7d_20260507_20260514` | Arbitrum | 14.80 | +66.09M | Elevated churn with positive Arbitrum net delta |

## 6. Main descriptive observations

1. The accounting floor held across all 18 chain-window rows: every supply invariant status is PASS and every accounting pass rate is 1.0000.
2. Price stability does not imply quiet on-chain accounting rails. Several windows show large transfer counts, large deployment-local supply deltas, or high gross-to-net churn.
3. Transfer activity and supply movement remain separate signals across windows. Base leads transfer events throughout the panel, while net supply leadership shifts across Ethereum and Arbitrum and sometimes becomes a contraction signal.
4. Gross-to-net churn can spike even when net delta is small. Ethereum in `usdc_7d_20260514_20260521` is the clearest example: a 191.87 ratio with only +31.75M deployment-local net delta.
5. Policy and F&G context label windows, but cannot explain flows by themselves. This memo does not claim that CLARITY, Fed transition context, or Fear & Greed caused mint/burn activity.
6. A reader interested in arbitrage should treat this as pre-price evidence, not a strategy. The memo describes accounting movement before price compression, not execution conditions or trading opportunity.

## 7. What this still does not answer

This memo does not answer:

| Excluded question | Status |
|---|---|
| Reserve adequacy | not measured |
| Redemption health | not measured |
| Liquidity depth | not measured |
| Depeg prediction | not modeled |
| CCTP route reconstruction | not included |
| Issuer control surface | not included |
| Entity or holder clustering | not included |
| Arbitrage model | not included |

## 8. Blog readiness assessment

**Is this ready to become a blog draft?** Yes, as a descriptive blog draft, provided the post keeps the same claim boundary: on-chain rail activity under annotated market and policy windows, with no causal or trading claims.

**Strongest story:** Stablecoins are price-stable, not on-chain static. Across six USDC windows and three deployments, Base repeatedly dominates transfer activity, while deployment-local supply deltas and gross mint/burn churn move differently across chains and windows.

**Weakest gap:** The current panel can describe movement, but it cannot explain why the movement happened. Policy and F&G context are labels, not explanatory variables.

**Figures/tables for the blog:** Use the window context table, the signal movement table, a small gross-to-net churn table, and a chart separating transfer leadership from net supply direction.

**Suggested working title:** Stablecoins Are Price-Stable, Not On-Chain Static

**Alternative title:** Dollar Stablecoins Are Global Rails, But Their On-Chain Signals Are Not Quiet

## 9. Future scope

Future scope should stay separate from this USDC panel:

1. EURC and XSGD can be used later as geo-stablecoin auditability probes.
2. XSGD on Base has already been audited, but it is not part of this USDC panel.
3. EURC has not yet been probed.
4. CCTP, issuer control surface, liquidity depth, and related routing or market-structure work belong in later modules.
