# Claim registry (v0)

Canonical claim definitions live in `src/audit/claims.rs`. Workflows must not hand-write claim statements for catalog ids.

## ClaimDefinition fields

| Field | Meaning |
|-------|---------|
| `claim_id` | Stable identifier (snake_case) |
| `default_status` | `supported`, `conditional`, or `unsupported` |
| `statement` | Human-readable attestation or boundary |
| `required_evidence_kinds` | `ArtifactKind` values resolved to paths at manifest write |
| `limitations` | Scope limits and failure interpretation |
| `warnings` | Default claim-local warnings (empty if none) |
| `produced_by` | Engine command (`transfer-audit`, `cross-chain-summary`) |

## Manifest ClaimBoundary

At runtime, `instantiate_claim(claim_id, available_paths)` builds a `ClaimBoundary`:

- Base identity, statement, status, and limitations from the catalog
- `evidence_artifacts` resolved from `required_evidence_kinds` against files listed in `artifact_manifest.json` (prefers canonical artifacts when present)
- Run-specific warnings merged without duplicating catalog warnings

## Catalog claim ids (v0)

### Transfer-audit supported

| claim_id | status |
|----------|--------|
| `transfer_activity_reconstructible` | conditional |
| `supply_snapshot_available` | conditional |
| `supply_reconciliation_available` | conditional |

### Cross-chain-summary supported

| claim_id | status |
|----------|--------|
| `cross_chain_per_deployment_comparison` | conditional |
| `per_chain_totalSupply_not_circulating_supply` | conditional |

### Unsupported (explicit boundaries)

| claim_id | produced_by | Notes |
|----------|-------------|-------|
| `circulating_supply_not_verified` | transfer-audit | Cross-chain circulating supply |
| `fiat_reserve_not_verified` | transfer-audit | No bank/attestation data |
| `liquidity_exposure_not_measured` | transfer-audit | No DEX/CEX/oracle depth |
| `bridge_backing_not_verified_without_bridge_collateral` | cross-chain-summary | No bridge collateral fetch |
| `peg_stability` | transfer-audit | Out of scope |
| `redemption_capacity` | transfer-audit | Out of scope |
| `user_geography` | transfer-audit | Out of scope |
| `holder_identity` | transfer-audit | Out of scope |
| `actual_swap_routing` | transfer-audit | Out of scope |
| `issuer_intent` | transfer-audit | Out of scope |
| `stress_transmission` | transfer-audit | Out of scope |

Unsupported claims have empty `required_evidence_kinds`. They must never appear as measured/supported claims without new evidence sources and catalog updates.

## Idempotent upsert

`cross-chain-summary` upserts catalog claims by `claim_id`: existing entries are replaced with the current catalog definition and freshly resolved evidence paths. Running upsert twice yields the same claim count.

## Audit plan alignment

`audit_plan.json` `out_of_scope` lists unsupported catalog ids for the run (from `audit_plan_out_of_scope_ids()`). Requested checks remain workflow-specific (`transfer_log_fetch`, `supply_invariant_per_chain`, etc.).
