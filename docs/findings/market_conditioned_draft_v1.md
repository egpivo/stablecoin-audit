# Geo-policy-conditioned USDC audit — descriptive findings draft v1

**Status:** Two windows published; four pending transfer-audit runs.
**Generated:** 2026-05-19

> **Framing:** USDC is a global dollar stablecoin rail deployed across multiple EVM chains. This audit examines chain-local USDC supply accounting across windows annotated by three independent context layers: on-chain audit metrics, market sentiment regime, and U.S. policy/macro event context. The three layers are recorded and reported separately. No layer is treated as a causal variable for any other.
>
> **Scope:** On-chain supply accounting only — transfer events, mint/burn counts, supply invariant, and gross-to-net activity ratios. No reserve attestation, peg analysis, liquidity, zk rollup accounting, CCTP route matching, holder balance delta, or control events.
>
> **Causality disclaimer:** Market regime labels (Fear & Greed), U.S. regulatory event annotations, and U.S. macro-policy event annotations are contextual only. No claim is made that F&G values, the CLARITY Act, or Fed Chair transition activity caused any on-chain metric. All cross-layer comparisons are descriptive and association-only.

---

## Three-layer framework

Each audit window is annotated independently across three layers:

| Layer | Source | What it records |
|-------|--------|-----------------|
| **Layer 1 — On-chain audit** | Transfer logs, ERC-20 view calls | Transfer counts, mint/burn events, net supply Δ, gross-to-net ratio, supply invariant pass/fail |
| **Layer 2 — Market sentiment** | alternative.me Fear & Greed Index | Daily F&G value (0–100), classification, 7-day window mean/min/max, dominant regime |
| **Layer 3 — U.S. policy/macro context** | `data/external/event_context.csv` | Regulatory and macro-policy events within ±14 days of window, classified by channel (see `docs/EVENT_CHANNEL_TAXONOMY.md`) |

Layers 2 and 3 annotate the external context in which the on-chain activity occurred. They do not explain it.

---

## Layer 1: On-chain audit metrics (published windows)

### usdc_7d_20241117_20241124

**Window:** 2024-11-17T00:00:00Z → 2024-11-24T00:00:00Z | 7 days

| Chain | Transfers | Senders | Recipients | Mints | Burns | Net supply Δ (USDC) | Supply invariant |
|-------|----------:|--------:|-----------:|------:|------:|--------------------:|------------------|
| Ethereum | 748,862 | 150,205 | 167,721 | 4,750 | 5,100 | +$1,270,635,305 | PASS |
| Base | 5,571,685 | 218,519 | 268,817 | 8,983 | 6,415 | −$40,481,624 | PASS |
| Arbitrum | 4,142,603 | 110,305 | 129,724 | 7,062 | 5,958 | +$82,689,240 | PASS |
| **Total** | **10,463,150** | | | **20,795** | **17,473** | | |

Gross-to-net ratio: unavailable (sum_mints_raw / sum_burns_raw not stored in current benchmark artifact; re-run in progress).

QA gates: all 3 chains — metadata PASS, hist_supply PASS, supply_invariant PASS, decode PASS, no_dup PASS, provenance PASS.

---

### usdc_7d_20260501_20260508

**Window:** 2026-05-01T00:00:00Z → 2026-05-08T00:00:00Z | 7 days

| Chain | Transfers | Net supply Δ (USDC) | Gross-to-net ratio | Supply invariant |
|-------|----------:|--------------------:|-------------------:|------------------|
| Ethereum | 3,130,346 | +$1,219,106,048 | 6.7837 | PASS |
| Base | 17,208,596 | −$40,678,928 | 21.1500 | PASS |
| Arbitrum | 4,866,248 | +$216,193,007 | 4.7887 | PASS |
| **Total** | **25,205,190** | | | |

QA gates: all 3 chains PASS across all gates.

---

## Layer 2: Market sentiment context (published windows)

| Window | F&G mean | F&G range | Regime | Days in window | F&G days available |
|--------|----------:|-----------|--------|---------------:|-------------------:|
| usdc_7d_20241117_20241124 | 87.86 | 82–94 | Extreme Greed | 7 | 7 |
| usdc_7d_20260501_20260508 | 42.14 | 26–50 | Fear | 7 | 7 |

---

## Layer 3: U.S. policy/macro event context (published windows)

Neither published window falls within 14 days of a registered event. The CLARITY Act committee advance (2026-05-14) is 6 days after the end of `usdc_7d_20260501_20260508` and is annotated as `post_window` in `window_event_context.csv`.

| Event | Date | Channels | Nearest published window | Position | Days from window end |
|-------|------|----------|--------------------------|----------|---------------------:|
| CLARITY Act committee advance | 2026-05-14 | `regulatory_clarity` | usdc_7d_20260501_20260508 | post_window | 6 |
| Fed Chair transition / Kevin Warsh | 2026-05-19 | `rate_expectations\|risk_appetite` | usdc_7d_20260501_20260508 | post_window | 11 |

---

## Cross-layer descriptive observations (published windows only)

These observations describe what the data shows. They do not attribute on-chain outcomes to market or policy context.

**On-chain (Layer 1):**

1. Supply invariant held on all chains in both windows. On-chain accounting was consistent with the decoded transfer event stream in both cases.

2. Transfer volume in the May 2026 window was approximately 2.4× the Nov 2024 window (25.2M vs 10.5M), driven primarily by Base (17.2M vs 5.6M). Within each window, the chain ordering by transfer count was Base > Arbitrum > Ethereum.

3. Ethereum net supply expansion was similar in magnitude across both windows (~$1.2–1.3B). Base showed net supply contraction in both windows at nearly identical absolute values ($40.5M and $40.7M). Two windows are insufficient to determine whether this reflects a structural pattern.

4. The Base gross-to-net ratio in May 2026 was 21.15 — roughly 21× gross mint+burn activity relative to the absolute net supply change. This metric is unavailable for Nov 2024.

5. Arbitrum net supply expansion was larger in May 2026 (+$216M vs +$83M). Whether this reflects deployment-level changes in Arbitrum USDC activity cannot be determined from two windows.

**Market sentiment (Layer 2):**

6. The Nov 2024 window occurred under Extreme Greed conditions (mean 87.86). The May 2026 window occurred under Fear (mean 42.14). Supply invariant held under both regime labels. No inference about regime effects on supply mechanics is supported by two windows.

**Policy/macro context (Layer 3):**

7. Both published windows precede the registered U.S. policy events (CLARITY Act, Fed Chair transition). They can serve as pre-event baselines for comparison with windows that overlap those events. No comparison is available yet because the overlapping windows have not completed transfer-audit.

---

## Pending windows

| Window | F&G regime | Policy/macro context (Layer 3) | Audit status |
|--------|-----------|--------------------------------|--------------|
| usdc_7d_20260218_20260225 | Extreme Fear (mean 7.7) | None within ±14d | Awaiting transfer-audit |
| usdc_7d_20260507_20260514 | Neutral (mean 44.1) | CLARITY Act: `post_window`, 0d after end | Transfer-audit running |
| usdc_7d_20260514_20260521 | Fear (mean 31.3, 6/7 F&G days) | CLARITY Act: `within` day 0; Fed transition: `within` day +5 | Queued |
| usdc_7d_20260512_20260519 | Fear (mean 36.3) | CLARITY Act: `within` day +2; Fed transition: `post_window` 0d after end | Queued |

Note on `usdc_7d_20241117_20241124`: benchmark supply_audit.csv was reconstructed from cross_chain_summary.json and lacks sum_mints_raw / sum_burns_raw. A re-run is queued to populate gross-to-net ratio.

---

## Data completeness

See `data/benchmarks/data_completeness.csv` for machine-readable per-window status.

Key gaps before full cross-layer analysis:
- Gross-to-net ratio for `usdc_7d_20241117_20241124` (transfer-audit re-run queued)
- Transfer-audit completion for the four pending windows
- One additional F&G data pull after 2026-05-20 for `usdc_7d_20260514_20260521` (currently 6/7 days)

---

## Future scope

### Geo stablecoin extension

Compare dollar, euro, Singapore-dollar, and regional stablecoins only after the USDC policy-conditioned panel is complete.

The extension adds an asset dimension to the existing window × chain panel:

- **Dollar:** USDC (this study)
- **Euro:** EURC (Circle), EURS (Stasis) — same EVM chains where deployed
- **Singapore dollar:** XSGD (Xfers/StraitsX) — targeted chains only
- **Regional:** HKDC, JPYC, or other regulated fiat-backed tokens as available on auditable EVM chains

For each asset, the same three-layer framework applies: on-chain audit (Layer 1), local market sentiment proxy (Layer 2), and jurisdiction-relevant policy/macro context (Layer 3). The event channel taxonomy (`docs/EVENT_CHANNEL_TAXONOMY.md`) would be extended with channels relevant to non-U.S. jurisdictions — e.g. MiCA reserve rules for EURC, MAS guidance for XSGD.

**This extension is not started.** No non-USDC assets, new chains, zk rollups, CCTP, liquidity routing, or holder clustering are added in the current phase.
