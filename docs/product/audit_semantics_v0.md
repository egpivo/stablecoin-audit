# Audit semantics (v0)

Schema ids: `audit-plan-v0`, `artifact-manifest-v0` (claims live in manifest only).

## Why this layer exists

`artifact_manifest.json` indexes files, checksums, workflow steps, and **records** claim boundaries. It is the product run contract for discovery, packaging, and API listing — but it is **not** the audit itself.

The audit semantics layer separates:

| Layer | Responsibility | Primary artifacts |
|-------|----------------|-------------------|
| **Evidence** | Raw and derived data from configured sources | `decoded_transfers.csv`, `supply_audit.csv`, `qa_report.json`, `provenance.json` |
| **Audit engine** | Fetch, decode, aggregate, gate evaluation | CLI workflows (`transfer-audit`, `cross-chain-summary`) |
| **Claim** | What the toolkit attests, with status, evidence, and limits | `supported_claims` / `unsupported_claims` in `artifact_manifest.json` |
| **Artifact** | Checksum-backed file index and workflow trace | `artifact_manifest.json`, per-run CSV/JSON/MD |
| **Product** | Discovery, packaging, read-only API | `GET /api/runs`, stablecoin-map package |

`supported_claims` and `unsupported_claims` are the **semantic audit output**: they state what was checked, what evidence backs each statement, and what remains explicitly out of scope. Readers must not infer claims by scanning the run directory.

## Architecture

```text
┌──────────────────────────────────────────────────────────────────┐
│  Product layer (API, package, future evidence browser)            │
│  Lists runs via artifact_manifest.json only                         │
└───────────────────────────────┬──────────────────────────────────┘
                                │ reads manifest + artifact bytes
┌───────────────────────────────▼──────────────────────────────────┐
│  Artifact layer                                                   │
│  artifact_manifest.json · audit_plan.json · CSV/JSON/MD outputs   │
│  Checksums, workflow_steps, artifact refs                           │
└───────────────────────────────┬──────────────────────────────────┘
                                │ emits after successful workflows
┌───────────────────────────────▼──────────────────────────────────┐
│  Claim layer                                                        │
│  supported_claims / unsupported_claims (ClaimBoundary v0)         │
│  status · statement · evidence_artifacts · limitations · warnings   │
└───────────────────────────────┬──────────────────────────────────┘
                                │ evaluated from evidence + gates
┌───────────────────────────────▼──────────────────────────────────┐
│  Audit engine layer                                               │
│  transfer-audit · cross-chain-summary                             │
│  RPC fetch, decode, supply invariant, QA gates                    │
└───────────────────────────────┬──────────────────────────────────┘
                                │ produces
┌───────────────────────────────▼──────────────────────────────────┐
│  Evidence layer                                                   │
│  Block-pinned logs, supply snapshots, provenance, QA reports      │
└──────────────────────────────────────────────────────────────────┘
```

Audit scope is declared **before or at manifest write** in `audit_plan.json` (see below). Claims are emitted **after** evidence is on disk.

## `audit_plan.json` (`audit-plan-v0`)

File name: `audit_plan.json` (sibling to `artifact_manifest.json` in a run directory).

| Field | Type | Meaning |
|-------|------|---------|
| `schema` | string | Always `"audit-plan-v0"` |
| `asset` | string | Uppercase asset symbol |
| `run_id` | string | Run directory name |
| `audit_window` | `AuditWindow` | Per-chain block windows |
| `deployments` | `DeploymentScope[]` | Chain + contract address in scope |
| `requested_checks` | string[] | Checks the run intends to perform |
| `out_of_scope` | string[] | Explicit non-claims for this run |
| `data_sources` | `DataSourceRef[]` | e.g. `rpc:ethereum` / `evm_rpc` |

`transfer-audit` writes `audit_plan.json` when building the product manifest. If a valid plan already exists (matching `asset` and `run_id`), it is accepted as-is. Wrong schema is rejected.

The plan is listed in `artifact_manifest.json` as kind `audit_plan` with `checksum_sha256`.

## `ClaimBoundary` (claim registry v0)

Each entry in `supported_claims` or `unsupported_claims`:

| Field | Type | Meaning |
|-------|------|---------|
| `claim` | string | Stable claim id |
| `status` | `ClaimStatus` | `supported`, `conditional`, `unsupported` |
| `statement` | string | Human-readable attestation or boundary |
| `evidence_artifacts` | string[] | Relative paths; must appear in `artifacts` |
| `limitations` | string[] | Scope limits, definitions, failure interpretation |
| `warnings` | string[] | Claim-local caveats (empty if none) |
| `caveat` | string | Legacy; prefer `limitations` |

### Transfer-audit claims (v0)

| claim id | status | Role |
|----------|--------|------|
| `transfer_activity_reconstructible` | conditional | Transfer logs fetched/decoded in window |
| `supply_snapshot_available` | conditional | Pinned supply boundaries and mint/burn aggregates |
| `circulating_supply_not_verified` | unsupported | Cross-chain circulating supply not attested |

### Shared unsupported boundaries (`audit-product`)

| claim id | status | Role |
|----------|--------|------|
| `bridge_backing_not_verified_without_bridge_collateral` | unsupported | No bridge collateral attestation; re-emitted by transfer-audit and cross-chain-summary |

### Cross-chain-summary claims (v0)

| claim id | status | Role |
|----------|--------|------|
| `cross_chain_per_deployment_comparison` | conditional | Per-deployment rollup on one schema |
| `per_chain_totalSupply_not_circulating_supply` | conditional | Sum of per-chain totals ≠ circulating supply |

Cross-chain upsert merges claims by `claim` id (idempotent replace).

## Relationship to `artifact_manifest.json`

- **Manifest** = product index + claim registry + workflow trace + checksums.
- **Audit plan** = declared scope and requested checks for the run.
- **Neither** replaces on-chain evidence or external attestation data.

A run without a valid `artifact_manifest.json` is incomplete for product surfaces (`GET /api/runs`, package generation). Claims inside a valid manifest are the authoritative semantic output for that run.
