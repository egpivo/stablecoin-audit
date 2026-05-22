# Run and publish additional USDC benchmark windows

Windows are registered in [`windows.csv`](windows.csv) for the **geo-policy-conditioned** USDC audit.
Three independent context layers are maintained and reported separately:

1. **On-chain audit metrics** — transfer events, supply invariant, mint/burn counts, gross-to-net ratio per chain.
2. **Market sentiment** (`data/external/window_sentiment_summary.csv`) — Fear & Greed regime label, association-only.
3. **U.S. policy/macro event context** (`data/external/window_event_context.csv`) — temporal proximity annotations by channel; see `docs/EVENT_CHANNEL_TAXONOMY.md`.

---

## Registered windows

| `window_id` | UTC span (from inclusive, to exclusive) | F&G (7d mean) | Regime | Audit status |
|-------------|------------------------------------------|---------------|--------|--------------|
| `usdc_7d_20241117_20241124` | 2024-11-17 → 2024-11-24 | **87.9** | Extreme Greed | **Published** |
| `usdc_7d_20260218_20260225` | 2026-02-18 → 2026-02-25 | **7.7** | Extreme Fear | Pending `transfer-audit` |
| `usdc_7d_20260501_20260508` | 2026-05-01 → 2026-05-08 | **42.1** | Fear | **Published** |
| `usdc_7d_20260507_20260514` | 2026-05-07 → 2026-05-14 | **44.1** | Neutral | Pending `transfer-audit` |
| `usdc_7d_20260514_20260521` | 2026-05-14 → 2026-05-21 | **33.8** (4d\*) | Fear | Pending `transfer-audit` |
| `usdc_7d_20260512_20260519` | 2026-05-12 → 2026-05-19 | **37.7** (6d\*) | Fear | Pending `transfer-audit` |

\* Partial F&G coverage — fewer than 7 days found in `fear_greed_daily.csv`. Run `scripts/fetch_fear_greed.py` to extend.

### Event context overlay

| Event | Date | Type | Nearby windows |
|-------|------|------|----------------|
| CLARITY Act committee advance | 2026-05-14 | regulatory | `20260507_20260514` (day after), `20260514_20260521` (within, day 0), `20260512_20260519` (within, day +2) |
| Fed Chair transition / Kevin Warsh | 2026-05-19 | macro_policy | `20260514_20260521` (within, day +5), `20260512_20260519` (day after window end) |

Annotations are in `data/external/window_event_context.csv` — see `position` and `days_from_window_start` columns.
**No causal relationship is claimed or implied.** Events annotate context; on-chain metrics are reported separately.

Re-scan F&G windows anytime:

```bash
python3 scripts/suggest_fng_windows.py
```

---

## Per-window workflow

Replace `WINDOW_ID`, `FROM`, and `TO` for each row.

### 1. Resolve blocks

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-05-07T00:00:00Z \
  --to   2026-05-14T00:00:00Z
```

### 2. Transfer audit

```bash
cargo run --release -- transfer-audit \
  --asset USDC \
  --run-id usdc_7d_20260507_20260514 \
  --window arbitrum:FROM:TO \
  --window base:FROM:TO \
  --window ethereum:FROM:TO
```

### 3. Cross-chain summary

```bash
cargo run --release -- cross-chain-summary \
  --asset USDC \
  --run-id usdc_7d_20260507_20260514
```

### 4. Publish into `docs/benchmarks/`

```bash
./scripts/publish_benchmark.sh usdc_7d_20260507_20260514
```

### 5. Refresh analysis files

```bash
python3 scripts/join_window_sentiment.py
python3 scripts/build_market_conditioned_panel.py
python3 scripts/build_window_event_context.py
```

---

## Concrete resolve-window commands

**Pre-CLARITY baseline (2026-05-07 → 2026-05-14):**

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-05-07T00:00:00Z \
  --to   2026-05-14T00:00:00Z
```

**Post-CLARITY / Fed transition (2026-05-14 → 2026-05-21):**

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-05-14T00:00:00Z \
  --to   2026-05-21T00:00:00Z
```

**Event-centered (2026-05-12 → 2026-05-19):**

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-05-12T00:00:00Z \
  --to   2026-05-19T00:00:00Z
```

**Extreme-fear week (2026-02-18 → 2026-02-25):**

```bash
cargo run --release -- resolve-window \
  --chains ethereum,base,arbitrum \
  --from 2026-02-18T00:00:00Z \
  --to   2026-02-25T00:00:00Z
```

---

## Findings structure

When reporting across windows, separate the three layers:

```
### On-chain audit metrics
transfer_event_count, onchain_delta_usdc, supply_invariant_status, accounting_pass_rate, …

### Market regime (F&G)
mean_fng, dominant_regime, analysis_regime — association label, not a cause

### Regulatory/macro event context
Events within window or ±14 days — annotation only, no causal claim
```

---

## Claims policy

**Layer 1 (on-chain audit):** Reports what the decoded transfer-event stream shows on each chain in the declared block window. No inference beyond on-chain supply accounting.

**Layer 2 (market sentiment):** F&G labels stratify windows by regime for descriptive comparison only — not causality, safety scores, or trading signals.

**Layer 3 (policy/macro context):** Regulatory and macro-policy events are annotated by temporal proximity and channel (see `docs/EVENT_CHANNEL_TAXONOMY.md`). No claim is made that any event caused any mint, burn, or transfer activity. CLARITY Act, Fed leadership changes, and any other events are **contextual annotations only**.

**Excluded from all layers:** bridge netting, reserve attestation, peg analysis, purchasing-power analysis, zk rollup accounting, CCTP route matching, holder balance delta, control events, liquidity depth, EURC/XSGD/USDT/other assets.
