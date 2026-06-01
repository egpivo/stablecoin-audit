# Canonical audit tables (v0)

Stable product contracts written by `transfer-audit` via `src/audit/canonical.rs`. Legacy workflow outputs (`decoded_transfers.csv`, `supply_audit.csv`) remain for backward compatibility; new readers should prefer canonical artifacts.

## Artifacts

| File | Schema id | ArtifactKind | Format |
|------|-----------|--------------|--------|
| `evidence_sources.json` | `evidence-sources-v0` | `evidence_sources` | json |
| `deployment_registry.json` | `deployment-registry-v0` | `deployment_registry` | json |
| `chain_windows.json` | `chain-windows-v0` | `chain_windows` | json |
| `canonical_transfers.csv` | `canonical-transfers-v0` | `canonical_transfers` | csv |
| `supply_snapshots.csv` | `supply-snapshots-v0` | `supply_snapshots` | csv |

Each listed artifact includes `checksum_sha256` and optional `schema` on its `ArtifactRef` in `artifact_manifest.json`.

## evidence_sources.json

Top-level: `{ "schema": "evidence-sources-v0", "sources": [ EvidenceSource, ... ] }`

Per source:

| Field | Type |
|-------|------|
| `source_id` | string — e.g. `{run_id}:rpc:ethereum:transfer_logs` |
| `source_type` | `rpc` \| `explorer_api` \| `contract_call` \| `derived` \| `manual` |
| `chain`, `chain_id` | string, u64 |
| `provider_name` | optional |
| `rpc_url_redacted` | optional |
| `block_range` | optional `{ start_block, end_block }` |
| `timestamp_range` | optional `{ start, end }` RFC 3339 |
| `captured_at` | RFC 3339 UTC |
| `notes` | optional |

## deployment_registry.json

Schema: `deployment-registry-v0`. Fields: `asset`, `run_id`, `deployments[]` with `chain`, `chain_id`, `address`, `token_standard`, `decimals`, `symbol`, `role` (`canonical` \| `bridged` \| `wrapped` \| `unknown`), `evidence_source_ids`.

## chain_windows.json

Schema: `chain-windows-v0`. Fields: `asset`, `run_id`, `windows[]` with `chain`, `chain_id`, `start_block`, `end_block`, optional timestamps, `evidence_source_ids`.

## canonical_transfers.csv

Stable transfer log table (preferred over `decoded_transfers.csv`).

Columns: `chain`, `chain_id`, `block_number`, `block_timestamp`, `tx_hash`, `log_index`, `contract_address`, `from_address`, `to_address`, `raw_amount`, `normalized_amount`, `decimals`, `event_type` (`transfer` \| `mint` \| `burn` \| `unknown`), `evidence_source_id`.

Rust type: `CanonicalTransferRecord` in `src/audit/contracts.rs`.

## supply_snapshots.csv

Pinned `totalSupply()` snapshots (preferred boundary contract over aggregate columns in `supply_audit.csv` alone).

Columns: `chain`, `chain_id`, `contract_address`, `block_number`, `block_timestamp`, `raw_total_supply`, `normalized_total_supply`, `decimals`, `method` (`totalSupply`), `evidence_source_id`.

Rust type: `SupplySnapshotRecord`.

## Relationship to supply_audit.csv

`supply_audit.csv` remains the workflow report: mint/burn aggregates, invariant gates, and QA columns per chain. `supply_snapshots.csv` holds boundary snapshots only. Supply reconciliation logic lives in `src/audit/supply.rs` and is invoked by transfer-audit internally.

## Validation

Manifest writers require claim evidence paths to appear in `artifacts` and exist on disk when the manifest is written. Canonical artifacts are collected by filename presence under the run directory (manifest-driven; no directory scanning in API/package layers).
