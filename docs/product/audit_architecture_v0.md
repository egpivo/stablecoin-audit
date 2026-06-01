# Audit architecture (v0)

Six conceptual layers from evidence to product delivery. **Implementation today differs from the target layering** — see [`audit_product_pipeline_v0.md`](audit_product_pipeline_v0.md) for the accurate current-vs-roadmap diagrams.

---

## Current v0 implementation (shipped)

In v0, layers 1–4 collapse into the **`transfer-audit` single workflow**. There is no standalone evidence-collection service or independent engine fleet yet.

```text
CLI (transfer-audit / cross-chain-summary)
  → audit_plan.json
  → transfer-audit workflow
       (RPC fetch · decode · QA · supply.rs · canonical + legacy artifacts)
  → claim registry (claims.rs)
  → artifact_manifest.json
  → optional cross-chain-summary upsert
  → read-only delivery (API · /ui/ · package)
```

### Layer responsibilities (as implemented)

| Layer | v0 reality | Module |
|-------|------------|--------|
| Audit request | CLI flags only — no `POST /api/runs` | `cli/`, `rpc/*` |
| Audit plan | `audit_plan.json` before evidence | `artifact/audit_plan.rs` |
| Evidence + engines | **Merged** in `transfer-audit` | `rpc/transfer_audit.rs`, `audit/supply.rs` |
| Canonical tables | Written inside transfer-audit | `audit/canonical.rs`, `audit/contracts.rs` |
| Claim registry | Instantiated at manifest write | `audit/claims.rs` |
| Artifact / manifest | Product run contract | `artifact/transfer_audit_manifest.rs` |
| Product delivery | Read-only — no audit on GET | `api/`, `ui/`, `artifact/stablecoin_map_package.rs` |

### Engine status

| Engine | Status |
|--------|--------|
| `transfer-audit` | Implemented |
| `cross-chain-summary` | Implemented (separate command, manifest upsert) |
| Supply logic | Implemented in `audit/supply.rs`, invoked by transfer-audit |
| `supply-audit` CLI | Not implemented |
| `bridge-backing-audit` | Not implemented — unsupported claim only |
| `liquidity-exposure-audit` | Not implemented — unsupported claim only |

### Artifact naming (current)

**Prefer (canonical contracts):**

- `canonical_transfers.csv`
- `supply_snapshots.csv`
- `deployment_registry.json`
- `chain_windows.json`
- `evidence_sources.json`

**Also written (legacy / workflow):**

- `decoded_transfers.csv` — legacy transfer table; not the long-term canonical contract
- `supply_audit.csv` — per-chain supply invariant summary
- `qa_report.json`, `provenance.json`, `summary.md`, `supply_audit.md`

---

## Target architecture (roadmap)

Future state **separates** evidence collection, canonical tables, and independent engines. Not shipped as separate services today.

```text
┌─────────────────────────────────────────────────────────────────┐
│  Product delivery — API, /ui/, package, reports                  │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Artifact / provenance — artifact_manifest.json, audit_plan.json │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Claim registry — supported_claims / unsupported_claims          │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Independent audit engines (future)                              │
│  transfer-audit · supply-audit · bridge-backing · liquidity · …   │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Canonical audit tables                                          │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Evidence collection — RPC logs, contract calls, source registry │
└───────────────────────────────┬─────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────┐
│  Audit request — HTTP orchestration + CLI                        │
└─────────────────────────────────────────────────────────────────┘
```

---

## Why `artifact_manifest.json` is not the audit

The manifest is the **product run contract**: it indexes artifacts, records workflow steps, and stores the claim registry for a completed run. It does not perform RPC fetches, decode Transfer logs, or evaluate supply invariants. Those operations live in audit engines (today: inside `transfer-audit`) and produce evidence + canonical tables first.

## Why claims are the semantic audit output

`supported_claims` and `unsupported_claims` state what the toolkit attests, with evidence paths and explicit limitations. Readers should not infer claims by scanning the run directory or reading CSV headers alone. The central claim catalog (`src/audit/claims.rs`) defines canonical claim ids; workflows attach run-specific evidence paths at manifest write time.

## What this system can claim today

See full list in [`audit_product_pipeline_v0.md`](audit_product_pipeline_v0.md#what-this-system-can-claim-today). In short: configured Transfer reconstruction, supply snapshots, per-chain supply invariants, and (after cross-chain-summary) cross-deployment comparison — **not** reserves, peg, redemption, bridge collateral, or liquidity depth.

## Why this is not just data engineering

Delivery layers (API, package, UI) expose artifacts. Audit semantics live in `audit_plan.json`, canonical schemas, `claims.rs`, and manifest claim boundaries. The key product boundary is **claim support** — what evidence supports which claim, and what remains unsupported.

## Unsupported until evidence exists

Bridge backing and liquidity exposure are **unsupported catalog claims** until dedicated evidence sources and engines exist:

- `bridge_backing_not_verified_without_bridge_collateral`
- `liquidity_exposure_not_measured`

No `bridge-backing-audit` or `liquidity-exposure-audit` engine is implemented in v0.

## Module map

| Module | Role |
|--------|------|
| `src/audit/contracts.rs` | Domain structs and schema ids |
| `src/audit/canonical.rs` | Writers for canonical table artifacts |
| `src/audit/claims.rs` | Central claim catalog |
| `src/audit/supply.rs` | Supply reconciliation (inside transfer-audit today) |
| `src/artifact/` | Manifest I/O, workflow upserts |
| `src/rpc/transfer_audit.rs` | Transfer-audit workflow |
| `src/rpc/cross_chain_summary.rs` | Cross-chain manifest upsert |

See also: [`claim_registry_v0.md`](claim_registry_v0.md), [`canonical_audit_tables_v0.md`](canonical_audit_tables_v0.md), [`audit_semantics_v0.md`](audit_semantics_v0.md).
