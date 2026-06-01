# Audit architecture (v0)

Six layers from evidence collection to product delivery. Lower layers are implemented in Rust workflows; upper layers are now explicit product contracts.

```text
┌─────────────────────────────────────────────────────────────────┐
│  Product delivery — API, stablecoin-map package, future UI       │
│  Lists runs via artifact_manifest.json only                      │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Artifact / provenance — artifact_manifest.json, audit_plan.json │
│  Checksums, workflow_steps, artifact index                       │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Claim registry — supported_claims / unsupported_claims          │
│  Semantic audit output (not the audit math itself)               │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Audit engines — transfer-audit, cross-chain-summary             │
│  RPC fetch, decode, supply reconciliation, QA gates              │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Canonical audit tables — stable CSV/JSON contracts              │
│  evidence_sources, deployment_registry, chain_windows,           │
│  canonical_transfers, supply_snapshots                           │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Evidence collection — RPC logs, contract calls, derived rows    │
│  Captured into evidence_sources.json with lineage metadata       │
└─────────────────────────────────────────────────────────────────┘
```

## Layer responsibilities

| Layer | Owns | Does not own |
|-------|------|--------------|
| Evidence collection | Source ids, block/timestamp ranges, provider metadata | Claim wording |
| Canonical audit tables | Stable schemas for transfers and supply snapshots | Workflow-specific CSV layouts |
| Audit engines | Fetch/decode/reconcile/evaluate gates | HTTP/API routing |
| Claim registry | Claim ids, statements, default limitations | New analysis without evidence |
| Artifact / provenance | Manifest index, checksums, workflow trace | Re-running audits on GET |
| Product delivery | Read-only serve, package zip/verify | Directory scanning discovery |

## Why `artifact_manifest.json` is not the audit

The manifest is the **product run contract**: it indexes artifacts, records workflow steps, and stores the claim registry for a completed run. It does not perform RPC fetches, decode Transfer logs, or evaluate supply invariants. Those operations live in audit engines and produce evidence + canonical tables first.

## Why claims are the semantic audit output

`supported_claims` and `unsupported_claims` state what the toolkit attests, with evidence paths and explicit limitations. Readers should not infer claims by scanning the run directory or reading CSV headers alone. The central claim catalog (`src/audit/claims.rs`) defines canonical claim ids; workflows attach run-specific evidence paths at manifest write time.

## Unsupported until evidence exists

Bridge backing and liquidity exposure are represented only as **unsupported** catalog claims until corresponding evidence sources exist:

- `bridge_backing_not_verified_without_bridge_collateral` — no bridge collateral fetch
- `liquidity_exposure_not_measured` — no DEX/CEX/oracle depth series

No bridge-backing-audit or liquidity-exposure-audit engine is implemented in v0.

## Module map

| Module | Role |
|--------|------|
| `src/audit/contracts.rs` | Domain structs and schema ids |
| `src/audit/canonical.rs` | Writers for canonical table artifacts |
| `src/audit/claims.rs` | Central claim catalog |
| `src/audit/supply.rs` | Supply reconciliation logic (separable from RPC) |
| `src/artifact/` | Manifest I/O, workflow upserts |
| `src/rpc/transfer_audit.rs` | Transfer-audit engine (calls audit + artifact layers) |

See also: [`claim_registry_v0.md`](claim_registry_v0.md), [`canonical_audit_tables_v0.md`](canonical_audit_tables_v0.md), [`audit_semantics_v0.md`](audit_semantics_v0.md).
