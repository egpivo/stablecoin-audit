# Audit product pipeline (v0)

End-to-end flow for a completed transfer-audit run (the only implemented engine in v0):

```text
Stablecoin audit request
  → audit_plan.json
  → evidence collection
  → canonical audit tables
  → audit engines (transfer-audit)
  → claim registry
  → artifact_manifest.json
  → product delivery (API / package)
```

Validation helper: `audit::validate_transfer_audit_product_run(out_dir)`.

## Stage 1 — Audit request → `audit_plan.json`

**Module:** `artifact/audit_plan.rs` (written by `audit::ensure_audit_plan` before evidence files)

Declares scope: asset, run_id, block windows, deployments, requested checks, and **out-of-scope** claim ids from the central catalog (`audit::audit_plan_out_of_scope_ids()`).

If a valid plan already exists for the run, it is accepted unchanged.

## Stage 2 — Evidence collection

**Module:** `rpc/transfer_audit.rs` (RPC fetch, decode, QA)

Raw evidence is captured into workflow files and registered in `evidence_sources.json` when canonical tables are written.

## Stage 3 — Canonical audit tables

**Module:** `audit/canonical.rs` + `audit/contracts.rs`

Stable product contracts (written alongside legacy workflow outputs):

| File | Schema |
|------|--------|
| `evidence_sources.json` | `evidence-sources-v0` |
| `deployment_registry.json` | `deployment-registry-v0` |
| `chain_windows.json` | `chain-windows-v0` |
| `canonical_transfers.csv` | `canonical-transfers-v0` |
| `supply_snapshots.csv` | `supply-snapshots-v0` |

Legacy files (`decoded_transfers.csv`, `supply_audit.csv`, etc.) remain for backward compatibility.

## Stage 4 — Audit engine

**Module:** `rpc/transfer_audit.rs` + `audit/supply.rs`

`transfer-audit` is the sole implemented engine. Supply reconciliation logic lives in `audit/supply.rs`, separate from RPC fetch code.

## Stage 5 — Claim registry

**Module:** `audit/claims.rs`

Manifest writers call `instantiate_claims()` — they do **not** hand-write claim statements. Workflows attach only run-specific `evidence_artifacts` paths resolved from declared manifest artifacts.

### Supported (conditional) for transfer-audit

- `transfer_activity_reconstructible`
- `supply_snapshot_available`
- `supply_reconciliation_available`

### Unsupported until real evidence exists

Transfer-audit must **not** treat transfer logs alone as proof of:

- circulating supply (`circulating_supply_not_verified`)
- bridge backing (`bridge_backing_not_verified_without_bridge_collateral`, shared `audit-product` boundary also re-emitted by cross-chain-summary)
- fiat reserves (`fiat_reserve_not_verified`)
- liquidity exposure (`liquidity_exposure_not_measured`)

Plus explicit out-of-scope boundaries: peg, redemption, user geography, holder identity, swap routing, issuer intent, stress transmission.

## Stage 6 — `artifact_manifest.json`

**Module:** `artifact/transfer_audit_manifest.rs` + `artifact/writer.rs`

Product run contract:

- Lists **only declared artifacts** (no directory scanning)
- Each artifact includes `checksum_sha256`
- Canonical artifacts include `schema` ids
- Claim evidence paths must appear in `artifacts` and exist on disk at write time
- Written only after a **successful** transfer-audit (no manifest on hard-error partial runs)

## Stage 7 — Product delivery

**Module:** `api/` (read-only), `artifact/stablecoin_map_package.rs` (package)

API and package layers read `artifact_manifest.json` only — they do not scan run directories or re-run audit logic.

## Module boundary: `src/audit/`

| File | Responsibility |
|------|----------------|
| `contracts.rs` | Stable schemas and schema ids |
| `canonical.rs` | Canonical artifact writers |
| `supply.rs` | Supply reconciliation outside RPC |
| `claims.rs` | Claim catalog and instantiation |
| `pipeline.rs` | Stage validation for completed runs |

See also: [`audit_architecture_v0.md`](audit_architecture_v0.md), [`claim_registry_v0.md`](claim_registry_v0.md), [`canonical_audit_tables_v0.md`](canonical_audit_tables_v0.md).
