# Refactor plan (v0.2)

## Current pain points

1. **`src/lib.rs` is overloaded** — ~500 lines with embedded `mod cli { ... }`, clap types, dispatch, and integration tests. Hard to navigate and risky to extend for API/serve.
2. **No product manifest** — Consumers must know per-command file names (`provenance.json`, `qa_report.json`, …) and README tables; no unified claim boundary document in-repo.
3. **Workflow vs transport mixed** — Future HTTP layer would tempt copy-paste from CLI match arms without a clear “core vs cli vs api” split.
4. **Two manifest concepts unnamed in docs** — Checkpoint resume JSON vs product evidence index.

## Proposed minimal refactor (v0.2)

**Goal:** Module boundaries and manifest types **without** changing audit semantics or removing commands.

### Files to create

| Path | Purpose |
|------|---------|
| `src/cli/mod.rs` | `run()`, tokio runtime |
| `src/cli/commands.rs` | `Cli`, `Commands`, dispatch `match` (moved from `lib.rs`) |
| `src/domain/mod.rs` | Module root |
| `src/domain/asset.rs` | `validate_identifier` |
| `src/domain/chain.rs` | Placeholder for chain id / name types |
| `src/domain/window.rs` | Placeholder for window spec documentation |
| `src/domain/artifact.rs` | Re-export `artifact::manifest` types for domain consumers |
| `src/application/mod.rs` | Module root |
| `src/application/workflow.rs` | Workflow name constants; future hooks |
| `src/artifact/mod.rs` | Module root |
| `src/artifact/manifest.rs` | `ArtifactManifest` + serde types |
| `src/artifact/writer.rs` | `write_artifact_manifest` |
| `docs/product/*.md` | Product architecture (this set) |
| `.local/productization_plan_v0.md` | Working plan |

### Files to modify

| Path | Change |
|------|--------|
| `src/lib.rs` | Declare modules; `run_cli` → `cli::run`; keep integration tests |
| `Cargo.toml` | No change in v0.2 skeleton (axum deferred to v0.3) |

### Files that must **not** change (semantics)

| Path | Reason |
|------|--------|
| `src/rpc/transfer_audit.rs` | Supply invariant, gates, CSV columns |
| `src/rpc/cross_chain_summary.rs` | Rollup logic |
| `src/rpc/resolve_window.rs` | UTC → block mapping |
| `src/rpc/metadata.rs` | Metadata probes |
| `src/stablecoin_map.rs` | Map package outputs |
| `configs/tokens/*.yml` | Token definitions |
| `docs/AUDIT_GATES.md` | Gate definitions |

Incremental follow-up (post–v0.2 skeleton): call `artifact::writer::write_artifact_manifest` from `write_run_artifacts` — separate PR to avoid coupling refactor with audit output changes.

## Public API boundaries after refactor

```rust
// Stable entrypoints
pub fn run_cli(...) -> Result<()>;
pub use report::{default_run_id, ensure_run_out_dir, validate_run_id};
pub use artifact::manifest::{ArtifactManifest, ArtifactRef, ...};
pub use domain::asset::validate_identifier;

// Internal (for now)
pub mod rpc;
pub mod cli;
```

## Module dependency direction

```text
cli → application (future) → rpc
cli → domain (validation)
rpc → report, config, decode, artifact (future manifest write)
artifact → domain (types only)
api (v0.3) → artifact, report path helpers
```

No `domain` → `rpc` dependency (domain stays pure).

## Test plan

| Test | Command / location |
|------|-------------------|
| Existing integration tests | `src/lib.rs` `mod tests` — CLI rejections, cross-chain fixture |
| Config load | `src/config/mod.rs` |
| Checkpoint | `src/rpc/transfer_checkpoint.rs` |
| Transfer artifacts | `src/rpc/transfer_audit.rs` |
| **New** manifest JSON roundtrip | `src/artifact/manifest.rs` |
| **New** manifest writer path | `src/artifact/writer.rs` |
| Full suite | `cargo test` |
| Format | `cargo fmt --check` |

Manual smoke (auditor):

```bash
cargo build
cargo run -- transfer-audit --asset USDC --run-id refactor_smoke \
  --window ethereum:24000000:24001000
cargo run -- cross-chain-summary --asset USDC --run-id refactor_smoke
```

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| CLI dispatch regression | Move-only extract; keep `run_cli` tests unchanged |
| Manifest/schema churn | Version field `artifact-manifest-v0`; additive fields only |
| Duplicate validation | Single `domain::asset::validate_identifier` used by CLI |
| Scope creep into v0.3 | No axum in v0.2; document API only |

## What should not change

- CLI subcommand names and flags
- Output CSV/JSON column schemas
- QA gate names and PASS/FAIL rules
- README command table (may add one line on future manifest in a later PR)
- Benchmark fixtures under `docs/benchmarks/`

## v0.3 skeleton (shipped)

- `cargo build --features api` — `src/api/`, `Commands::Serve`
- Path jail + run listing + axum integration tests

## v0.3 follow-on

- ~~Emit `artifact_manifest.json` from `transfer-audit`~~ (shipped)
- ~~Emit / update manifest from `cross-chain-summary`~~ (shipped — upsert)
- Legacy run discovery without product manifest
- Package endpoints (`GET /api/packages/...`)
