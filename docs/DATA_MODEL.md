# Data Model

## TokenConfig

Loaded from `configs/tokens/<asset>.<chain>.yml`.

| Field | Type | Description |
|---|---|---|
| `asset` | `String` | Token symbol, e.g. `USDC` |
| `chain` | `String` | Chain name, e.g. `ethereum` |
| `chain_id` | `u64` | EIP-155 chain ID |
| `contract_address` | `String` | Hex address with `0x` prefix |
| `decimals` | `u8` | Token decimal places |
| `issuer` | `String` | Issuing entity, e.g. `Circle` |
| `form` | `String` | `native` or `bridged` |
| `rpc_url_env` | `String` | Name of env var holding the RPC URL |
| `deployment_block` | `Option<u64>` | Block where the contract was deployed |
| `expected_interfaces` | `Vec<String>` | Interface tags to verify |

## ChainMetadata (Milestone 1 output)

One record per (asset, chain) pair per run.

| Field | Type | Description |
|---|---|---|
| `chain` | `String` | Chain name |
| `chain_id` | `u64` | EIP-155 chain ID |
| `contract_address` | `String` | Hex contract address |
| `issuer` | `String` | Issuing entity |
| `form` | `String` | `native` or `bridged` |
| `expected_interfaces` | `Vec<String>` | Expected interface tags |
| `name` | `Option<String>` | Result of `name()` |
| `symbol` | `Option<String>` | Result of `symbol()` |
| `decimals` | `Option<u8>` | Result of `decimals()` |
| `total_supply_live_probe` | `Option<String>` | `totalSupply()` at provider default (latest) block; decimal string. **Not pinned to the audit window.** Used only for `metadata_call_pass`. Do not use in supply invariant calculations. |
| `total_supply_live_probe_note` | `String` | Describes the provenance: `"live call at provider latest block; not pinned to window end block"`, `"skipped: ..."`, or `"rpc-error"`. |
| `total_supply_at_start_minus_1` | `Option<String>` | `totalSupply()` at `start_block - 1`; decimal string. May be a synthetic zero for pre-deployment windows — see `total_supply_at_start_minus_1_provenance`. |
| `total_supply_at_start_minus_1_provenance` | `String` | How the start supply was obtained: `"on-chain"`, `"pre-deployment zero: block N < deployment_block M"`, `"genesis (block 0)"`, `"rpc-error"`, or `"skipped: ..."`. |
| `total_supply_at_end` | `Option<String>` | `totalSupply()` at `resolved_end_block`; pinned historical call. Used in supply invariant. |
| `start_block` | `u64` | Requested start of window |
| `end_block` | `Option<u64>` | Requested end (None = latest) |
| `resolved_end_block` | `Option<u64>` | Actual block number used for end |
| `metadata_call_pass` | `bool` | All four ERC-20 view calls succeeded |
| `historical_supply_pass` | `bool` | Both historical supply calls succeeded |
| `errors` | `Vec<String>` | Any call-level error messages |

## MetadataReport

Top-level output written to `out/<asset>/metadata.json`.

| Field | Type | Description |
|---|---|---|
| `asset` | `String` | Token symbol |
| `generated_at` | `String` | UTC ISO 8601 timestamp |
| `chains` | `Vec<ChainMetadata>` | One entry per chain |

## TransferEvent

Written by experimental transfer-log commands:
- `fetch` -> `out/<asset>/transfers_<chain>.csv`
- `transfer-audit` -> `out/<asset>/decoded_transfers.csv`

| Field | Type | Description |
|---|---|---|
| `chain` | `String` | Chain name |
| `contract_address` | `String` | Token contract address |
| `block_number` | `u64` | Block containing the log |
| `tx_hash` | `String` | Transaction hash |
| `log_index` | `u64` | Log index within the block |
| `from` | `String` | Sender address (`0x000...` for mint) |
| `to` | `String` | Receiver address (`0x000...` for burn) |
| `value_raw` | `String` | Raw token amount as U256 decimal integer string (no scaling) |
| `value_decimal` | `String` | Token amount formatted with token decimals (e.g. `1.000000`) |
| `kind` | `String` | `mint`, `burn`, or `transfer` |

> `value_u256: U256` exists in the in-memory struct for arithmetic but is tagged `#[serde(skip)]` and does not appear in CSV output.

## ControlEvent

Written to `out/<asset>/control_events_<chain>.csv` by the `fetch` subcommand (transfer logs path) and, in the v0.2 experimental path, by `control-audit` when issuer control logs are decoded to the same CSV shape.

| Field | Type | Description |
|---|---|---|
| `chain` | `String` | Chain name |
| `block_number` | `u64` | Block containing the log |
| `tx_hash` | `String` | Transaction hash |
| `log_index` | `u64` | Log index |
| `event_name` | `String` | e.g. `Blacklisted`, `Paused`, `MinterConfigured` |
| `args_json` | `String` | Compact JSON object of decoded event arguments |
| `decode_status` | `String` | `decoded`, `decode_error`, or `unknown_signature` |

## QaReport

Written to `out/<asset>/qa_report.json` by experimental report paths (`report` and `transfer-audit`).

| Field | Type | Description |
|---|---|---|
| `asset` | `String` | Token symbol |
| `generated_at` | `String` | UTC ISO 8601 timestamp |
| `chains` | `Vec<QaChain>` | One entry per chain |

Each `QaChain` has a `gates` object with five string fields:

| Gate field | Description |
|---|---|
| `no_duplicate_logs` | No `(tx_hash, log_index)` duplicates in the raw log set |
| `transfer_decode_sample` | Random sample of up to 100 logs decoded without error |
| `all_transfer_decode` | Every log in the full window decoded without error |
| `supply_invariant` | `sum(mints) - sum(burns) == totalSupply(end) - totalSupply(start-1)` |
| `control_event_query` | Control event `eth_getLogs` call status |

Values are `PASS`, `FAIL`, `UNAVAILABLE`, or `WARN`. Gates are `UNAVAILABLE` for chains that hard-errored before evaluation (config/env/RPC errors).

## SupplyInvariant (experimental)

Computed per chain per window during `fetch` and `transfer-audit`.

The core accounting identity:

```
totalSupply(end_block) - totalSupply(start_block - 1) == sum(mints) - sum(burns)
```

All arithmetic uses raw `U256`/`I256` token units (no decimal scaling). Results are stored as raw integer strings in `_raw` fields. The `total_supply_at_start_minus_1` and `total_supply_at_end` boundary fields are decimal-scaled strings.

## CrossChainSummary (experimental, Milestone 4)

Written to `out/<asset>/cross_chain_summary.json` and `cross_chain_summary.md` by `cross-chain-summary` (`--features experimental`). Inputs must come from a **single** `transfer-audit` run: same `qa_report.json` provenance window as every row in `supply_audit.csv`, **at least two chains**, and aligned per-chain QA vs supply fingerprints. `onchain_delta` values are signed (`I256`); `sum_onchain_delta_raw` is the sum of those strings when every chain has a delta and the sum does not overflow.

| Field | Type | Description |
|---|---|---|
| `schema_version` | `u32` | Currently `2` |
| `asset` | `String` | Token symbol |
| `generated_at` | `String` | When this summary was produced |
| `transfer_audit_qa_generated_at` | `String` | `qa_report.json` top-level `generated_at` |
| `transfer_audit_provenance_generated_at` | `String` | `qa_report.json` `provenance.generated_at` |
| `window_from_block` | `u64` | Must match every summarized chain and supply row |
| `window_to_block_requested` | `Option<String>` | Must match every supply row `to_block_requested` (after trim / numeric or `latest` rules) |
| `chain_count` | `usize` | Number of chains (≥ 2) |
| `sum_onchain_delta_raw` | `Option<String>` | Sum of per-chain signed deltas as decimal string, or absent |
| `chains` | `Vec<...>` | One object per chain: ids, window, **QA `gates`**, activity counts, `total_supply_at_end_decimal` (not raw base units), `onchain_delta_raw` |
| `warnings` | `Vec<String>` | e.g. bridge double-count disclaimer |

## risk_flags.md (experimental)

Human-readable Markdown (not JSON-schema’d). Two producers:

- **`fetch`:** `out/<asset>/risk_flags.md` — per-chain transfer QA gates (dup / decode / supply invariant) plus issuer **control** events read back from `control_events_<chain>.csv` when present.
- **`control-audit`:** same path **`out/<asset>/risk_flags.md`** — control QA gates and event listing (overwrites any prior file from another command in the same asset dir).

For a combined transfer+control narrative after `transfer-audit`, use its outputs; re-run `fetch` or `control-audit` only when you intend to refresh that path’s `risk_flags.md`.
