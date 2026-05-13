# Audit QA Gates

Every run evaluates the following gates per chain. Results are stored in the JSON output and printed to stdout.

Gates are `UNAVAILABLE` (or `unavailable` in CSVs) for chains that hard-errored before evaluation (config, env, or RPC failure).

## Gate Definitions

### Gate 1 — Metadata Call Pass

**ID**: `metadata_call_pass`

All four ERC-20 view calls must succeed without error:
- `name()`
- `symbol()`
- `decimals()`
- `totalSupply()` (current)

A single call failure sets this gate to `[FAIL]`.

### Gate 2 — Historical totalSupply Pass

**ID**: `historical_supply_pass`

`totalSupply()` must be resolvable at two specific blocks:
- `start_block - 1` (supply just before the window opens)
- `end_block` (or the resolved latest block)

**Allowed value sources for `start_block - 1`**:

| Provenance label | Condition | Value used |
|---|---|---|
| `on-chain` | Block exists and contract is deployed | RPC result |
| `pre-deployment zero` | `start_block - 1 < deployment_block` | Synthetic `0` |
| `genesis (block 0)` | `start_block == 1`, so `start_minus_1 == 0` | Synthetic `0` |
| `rpc-error` | Call failed for another reason | Gate fails |

Synthetic-zero values are legitimate: supply is definitionally 0 before the contract existed.
Calling `totalSupply()` at a pre-deployment block returns an execution error on most RPC nodes
(no contract code), which would produce a false `[FAIL]`. The synthetic path avoids that while
recording exact provenance in both the JSON (`total_supply_at_start_minus_1_provenance`) and stdout.

The `end_block` supply must always come from an on-chain call. If that call fails or the end block
cannot be resolved, the gate fails. The `errors` field records the reason.

### Gate 3 — No Duplicate Logs

**ID**: `no_duplicate_logs`

No two `Transfer` log records in the output may share the same `(chain, contract_address, tx_hash, log_index)` tuple.

Duplicates indicate a pagination overlap bug and must be resolved before supply reconstruction.

### Gate 4 — Transfer Value Decode Sample Pass

**ID**: `transfer_decode_sample_pass`

A random sample of up to 100 `Transfer` events must decode without error. The `value_raw` field must be a valid `U256` and the `from`/`to` fields must be valid 20-byte addresses.

### Gate 5 — All Transfer Logs Decoded

**ID**: `all_transfer_decode_pass`

Every `Transfer` log in the full fetch window must decode without error. Unlike Gate 4 (which samples), this gate covers the complete set. A failure means the CSV and mint/burn sums are incomplete; the `full_decode_error_count` field records how many logs failed. The `supply_invariant` gate (Gate 7) cannot be trusted when this gate fails.

### Gate 6 — Block Range Provenance Stamped

**ID**: `block_range_provenance`

The report must record:
- `from_block` as requested
- `to_block` as requested
- `generated_at` as a UTC ISO 8601 timestamp

All three fields must be non-null.

### Gate 7 — Supply Invariant

**ID**: `supply_invariant`

The accounting identity must hold exactly (zero discrepancy in raw token units):

```
totalSupply(end_block) - totalSupply(start_block - 1) == sum(mints) - sum(burns)
```

Any non-zero discrepancy is flagged `[FAIL]` and the raw integer delta is reported in `discrepancy_raw`. The gate is `UNAVAILABLE` when either historical supply call fails.

### Gate 8 — Control Event Query Status

**ID**: `control_event_query`

The query for control events (`Blacklisted`, `Paused`, `MinterConfigured`, etc.) must be attempted. Its outcome is recorded in `control_event_query_status` with one of the following values:

| Status | Meaning |
|---|---|
| `pass` | `eth_getLogs` succeeded and all matched events decoded without error |
| `partial` | `eth_getLogs` succeeded but one or more events failed to decode (ABI mismatch) |
| `error: <msg>` | `eth_getLogs` call failed; `<msg>` contains the RPC error |
| `skipped` | Not fetched because the chain hard-errored before reaching the control-event step |

An empty result set under `pass` status is acceptable and flagged `[INFO]`.
A `partial` or `error:` status is flagged `[WARN]`.

## Risk Flag Format

Each gate result is prefixed with one of five risk flags:

| Flag | Meaning |
|---|---|
| `[PASS]` | Gate passed; no action required |
| `[INFO]` | Informational; expected or benign outcome, e.g. no control events in window |
| `[WARN]` | Potential issue; warrants manual review but does not invalidate the audit |
| `[FAIL]` | Gate failed; audit result should not be relied upon without investigation |
| `[SKIP]` | Gate not evaluated because the chain encountered a hard error before this step |
