# Event channel taxonomy

Five channel codes classify the mechanism by which a macro or regulatory event is *expected* to interact with the context around USDC on-chain activity. These codes are annotation labels attached to entries in `data/external/event_context.csv`.

**No channel code implies causality.** Channels describe a theorized pathway that analysts may want to examine — not a claim that the event caused any mint, burn, or transfer activity.

An event may carry more than one channel, recorded as pipe-delimited values in the `expected_channel` field (e.g. `rate_expectations|risk_appetite`).

---

## Channels

| Code | Name | Description |
|------|------|-------------|
| `regulatory_clarity` | Regulatory clarity | Changes to the legal or regulatory classification of digital assets or stablecoin issuers — e.g. passage of market-structure legislation, SEC guidance, CFTC rulemaking, or court decisions that alter issuer obligations. |
| `stablecoin_reserve_rules` | Stablecoin reserve rules | Proposed or enacted rules governing stablecoin reserve composition, redemption rights, disclosure, or attestation requirements — e.g. the GENIUS Act, OCC stablecoin guidance, or Basel treatment of stablecoin exposures. |
| `rate_expectations` | Rate expectations | Changes to Fed funds rate expectations, FOMC forward guidance, yield-curve shape, or policy-rate signals that affect the cost or attractiveness of dollar-denominated instruments held as USDC reserves or substitutes. |
| `dollar_liquidity` | Dollar liquidity | Broad USD liquidity conditions — Federal Reserve balance-sheet operations, repo/reverse-repo volume, Treasury general account flows, or central-bank FX swap lines that affect the dollar funding environment. |
| `risk_appetite` | Risk appetite | Shifts in general risk appetite proxied by macro or market indicators — VIX spikes, credit-spread widening, global equity drawdowns, crypto market-structure events (exchange failures, protocol exploits), or de-risking episodes that may correlate with stablecoin inflows/outflows. |

---

## Current event catalog (see `data/external/event_context.csv`)

| event_id | event_type | channel(s) |
|----------|-----------|------------|
| `clarity_act_committee_2026_05_14` | regulatory | `regulatory_clarity` |
| `fed_chair_warsh_transition_2026_05_19` | macro_policy | `rate_expectations\|risk_appetite` |

---

## Adding new events

Add a row to `data/external/event_context.csv` with:
- `event_type`: `regulatory`, `macro_policy`, `court_ruling`, `agency_guidance`, or `market_event`
- `expected_channel`: one or more codes from the table above, pipe-delimited if multiple
- `source_url`: primary source URL (leave blank if not yet confirmed)

Re-run `scripts/build_window_event_context.py` after adding events to refresh `data/external/window_event_context.csv`.
