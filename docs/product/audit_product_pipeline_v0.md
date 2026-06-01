# Audit product pipeline (v0)

This document separates **what ships today** (verified end-to-end through runs such as `demo_001`) from **target architecture** on the roadmap. Do not read the target diagram as already implemented.

Validation helper for completed transfer-audit runs: `audit::validate_transfer_audit_product_run(out_dir)`.

See also: [`audit_architecture_v0.md`](audit_architecture_v0.md), [`backend_architecture_v0.md`](backend_architecture_v0.md), [`claim_registry_v0.md`](claim_registry_v0.md), [`canonical_audit_tables_v0.md`](canonical_audit_tables_v0.md).

---

## Current v0 implementation (shipped)

Audit execution starts from **CLI commands only**. There is no HTTP audit request orchestration (`POST /api/runs` remains roadmap).

```text
┌─────────────────────────────────────────────────────────────┐
│  CLI audit request                                          │
│  transfer-audit  (--asset, --run-id, --window …)            │
│  cross-chain-summary  (optional second command, ≥2 chains)    │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  audit_plan.json                                            │
│  scope · requested_checks · out_of_scope · data_sources     │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  transfer-audit — single workflow (rpc/transfer_audit.rs)   │
│    RPC Transfer log fetch                                   │
│    → decode                                                 │
│    → QA gates                                               │
│    → supply reconciliation (audit/supply.rs)              │
│    → canonical tables + legacy workflow artifacts           │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  Claim registry (audit/claims.rs)                           │
│  instantiate_claims() → supported / unsupported boundaries  │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  artifact_manifest.json                                     │
│  artifacts · workflow_steps · checksums · claims            │
└───────────────────────────────┬─────────────────────────────┘
                                │ optional
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  cross-chain-summary — manifest upsert                      │
│  adds cross_chain_summary.* + workflow_steps entry          │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  Product delivery (read-only)                                 │
│  API · /ui/ evidence browser · package zip/verify/download  │
│  markdown / csv / json artifacts on disk                    │
└─────────────────────────────────────────────────────────────┘
```

### Engine status (current)

| Component | Status |
|-----------|--------|
| `transfer-audit` | **Implemented** — single CLI workflow (fetch, decode, QA, supply, artifacts, manifest) |
| `cross-chain-summary` | **Implemented** — separate CLI command; upserts `artifact_manifest.json` |
| Supply logic (`audit/supply.rs`) | **Implemented inside transfer-audit** — not an independent CLI |
| `supply-audit` CLI | **Not implemented** |
| `bridge-backing-audit` | **Not implemented** — unsupported claim only |
| `liquidity-exposure-audit` | **Not implemented** — unsupported claim only |
| `POST /api/runs` orchestration | **Not implemented** — roadmap v0.4 |

### Current stage detail

#### 1. CLI → `audit_plan.json`

**Module:** `artifact/audit_plan.rs` via `ensure_audit_plan` (called at start of `transfer-audit`).

Declares scope: asset, run_id, block windows, deployments, **requested_checks**, and **out_of_scope** claim ids from `audit::audit_plan_out_of_scope_ids()`. If a valid plan already exists for the run, it is accepted unchanged.

**Example `requested_checks`** (from `transfer_audit_manifest.rs`):

- `transfer_log_fetch`
- `transfer_decode`
- `mint_burn_aggregation`
- `supply_invariant_per_chain`
- `qa_gates`

**Example `out_of_scope`** (claim ids — not checks to run):

- `circulating_supply_not_verified`
- `bridge_backing_not_verified_without_bridge_collateral`
- `fiat_reserve_not_verified`
- `liquidity_exposure_not_measured`
- `peg_stability`
- `redemption_capacity`
- `user_geography`
- `holder_identity`
- `actual_swap_routing`
- `issuer_intent`
- `stress_transmission`

Bridge backing, reserve attestation, issuer bank verification, and liquidity exposure belong in **out_of_scope** / **unsupported_claims**, not in `requested_checks`.

#### 2–4. `transfer-audit` single workflow

Evidence collection, canonical table writes, supply reconciliation, and QA gates are **not separate pipeline stages** in v0 — they run inside one `transfer-audit` command.

**Modules:** `rpc/transfer_audit.rs`, `audit/canonical.rs`, `audit/supply.rs`, `audit/contracts.rs`

**Preferred canonical artifacts:**

| File | Schema |
|------|--------|
| `evidence_sources.json` | `evidence-sources-v0` |
| `deployment_registry.json` | `deployment-registry-v0` |
| `chain_windows.json` | `chain-windows-v0` |
| `canonical_transfers.csv` | `canonical-transfers-v0` |
| `supply_snapshots.csv` | `supply-snapshots-v0` |

**Legacy / workflow outputs** (still written; backward compatible):

| File | Role |
|------|------|
| `decoded_transfers.csv` | Legacy transfer log table — prefer `canonical_transfers.csv` for new readers |
| `supply_audit.csv` | Per-chain mint/burn vs totalSupply delta summary |
| `supply_audit.md`, `summary.md`, `provenance.json`, `qa_report.json` | Workflow reports and QA gates |

#### 5. Claim registry

**Module:** `audit/claims.rs`

Manifest writers call `instantiate_claims()` — they do **not** hand-write claim statements. Workflows attach run-specific `evidence_artifacts` paths resolved from declared manifest artifacts.

**Supported (conditional) after transfer-audit:**

- `transfer_activity_reconstructible`
- `supply_snapshot_available`
- `supply_reconciliation_available`

**Additional supported (conditional) after cross-chain-summary:**

- `cross_chain_per_deployment_comparison`
- `per_chain_totalSupply_not_circulating_supply`

#### 6. `artifact_manifest.json`

**Module:** `artifact/transfer_audit_manifest.rs`, `artifact/cross_chain_summary_manifest.rs`, `artifact/writer.rs`

- Lists **only declared artifacts** (no directory scanning)
- Each artifact includes `checksum_sha256`
- Canonical artifacts include `schema` ids
- Claim evidence paths must appear in `artifacts` and exist on disk at write time
- Written only after a **successful** transfer-audit (no manifest on hard-error partial runs)
- `cross-chain-summary` **upserts** an existing manifest (does not replace transfer-audit artifacts)

#### 7. Product delivery (read-only, implemented)

**Modules:** `api/`, `ui/`, `artifact/stablecoin_map_package.rs`

| Surface | Status |
|---------|--------|
| Read-only API (`serve --features api`) | Implemented |
| `/ui/` evidence browser | Implemented |
| Package build / download / verify | Implemented |
| `POST /api/runs` audit orchestration | **Roadmap only** |

API, browser, and package layers read `artifact_manifest.json` only — they do not scan run directories or re-run audit logic.

---

## Target architecture (roadmap — not fully implemented)

Future state separates evidence collection, canonical tables, and independent audit engines. **This diagram is aspirational**; several boxes do not exist as standalone commands today.

```text
┌─────────────────────────────────────────────────────────────┐
│  Audit request — HTTP API and/or CLI orchestration           │
│  POST /api/runs · job queue · status · logs · cancel        │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  audit_plan.json                                            │
│  scope · requested_checks · out_of_scope                      │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  Evidence collection (independent layer)                    │
│  RPC logs · totalSupply · metadata · raw source snapshots   │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  Canonical audit tables                                     │
│  canonical_transfers.csv · supply_snapshots.csv · …         │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  Independent audit engines                                  │
│    transfer-audit                                           │
│    supply-audit          ← future CLI                       │
│    bridge-backing-audit  ← future; needs collateral sources │
│    liquidity-exposure-audit ← future; needs market data     │
│    cross-chain-summary                                      │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  Claim registry → artifact_manifest.json                    │
└───────────────────────────────┬─────────────────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  Product delivery — API · UI · packages · reports           │
└─────────────────────────────────────────────────────────────┘
```

| Roadmap item | Notes |
|--------------|-------|
| `POST /api/runs` | v0.4 — enqueue workflows, poll status |
| Independent evidence collection | Refactor from monolithic `transfer-audit` |
| `supply-audit` CLI | Supply logic already in `supply.rs`; expose as separate engine |
| `bridge-backing-audit` | Requires bridge collateral / attestation evidence sources |
| `liquidity-exposure-audit` | Requires DEX/CEX/oracle depth series |

---

## What this system can claim today

Within configured asset, chain, and block windows, when QA gates PASS:

- Transfer logs in scope were fetched and decoded (`transfer_activity_reconstructible`).
- Pinned `totalSupply` boundaries are available per chain (`supply_snapshot_available`).
- Mint/burn aggregates are compared to pinned supply deltas per chain (`supply_reconciliation_available`).
- After cross-chain-summary (≥2 chains): per-deployment metrics are comparable on one schema (`cross_chain_per_deployment_comparison`).

**Cannot claim today** (listed as unsupported / out_of_scope unless new evidence sources are added):

- Fiat reserves or issuer bank account verification
- Legal redemption rights or redemption capacity
- Bridge collateral or backing adequacy
- Peg or price stability
- Liquidity depth or market exposure
- User geography, holder identity, issuer intent, or actual swap routing
- Circulating supply across chains (double-count risk)

---

## Why this is not just data engineering

The API, package zip, and `/ui/` browser are **delivery mechanisms**. They serve pre-generated bytes and manifest JSON; they do not define audit semantics.

Audit meaning lives in:

- `audit_plan.json` — declared scope and explicit out-of-scope boundaries
- Canonical artifacts — versioned schemas (`canonical-transfers-v0`, etc.)
- `audit/claims.rs` — central claim catalog and instantiation rules
- `artifact_manifest.json` — `supported_claims`, `unsupported_claims`, and `evidence_artifacts` links

The product boundary is **claim support**: which artifacts support which claim, under which limitations, and what remains unsupported. Readers should not infer claims from raw CSV columns or directory listings alone.

---

## Module boundary: `src/audit/`

| File | Responsibility |
|------|----------------|
| `contracts.rs` | Stable schemas and schema ids |
| `canonical.rs` | Canonical artifact writers |
| `supply.rs` | Supply reconciliation (called by transfer-audit today) |
| `claims.rs` | Claim catalog and instantiation |
| `pipeline.rs` | Stage validation for completed runs |
