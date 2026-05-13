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

Written to `out/<asset>/qa_report.json` by `transfer-audit` (experimental). The `report` subcommand reads this file (with `provenance.json` and `supply_audit.csv`) to emit v0.1.5 stress outputs and does not overwrite `qa_report.json`.

| Field | Type | Description |
|---|---|---|
| `asset` | `String` | Token symbol |
| `generated_at` | `String` | UTC ISO 8601 timestamp |
| `provenance` | object | Window metadata (`from_block`, `to_block_requested`, `generated_at`) |
| `chains` | `Vec<QaChain>` | One entry per chain |

Each `QaChain` has a `gates` object with six string fields (`PASS`, `FAIL`, `UNAVAILABLE`, etc.):

| Gate field | Description |
|---|---|
| `metadata_call_pass` | Token `name`/`symbol`/`decimals` RPC calls succeeded |
| `historical_supply_pass` | `totalSupply` at window boundaries could be read when required |
| `no_duplicate_logs_pass` | No `(tx_hash, log_index)` duplicates in the deduped transfer set |
| `transfer_decode_pass` | Transfer logs in the window decode without error |
| `supply_invariant_pass` | `sum(mints) - sum(burns)` matches on-chain supply delta for the window |
| `provenance_stamped` | Contract and window metadata align with provenance stamping rules |

Gates are `UNAVAILABLE` when a chain hard-errored before evaluation (config/env/RPC errors).

## SupplyInvariant (experimental)

Computed per chain per window during `fetch` and `transfer-audit`.

The core accounting identity:

```
totalSupply(end_block) - totalSupply(start_block - 1) == sum(mints) - sum(burns)
```

All arithmetic uses raw `U256`/`I256` token units (no decimal scaling). Results are stored as raw integer strings in `_raw` fields. The `total_supply_at_start_minus_1` and `total_supply_at_end` boundary fields are decimal-scaled strings.
