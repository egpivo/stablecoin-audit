# Backend architecture (v0)

## Product positioning

`stablecoin-audit` is a **Rust toolkit for generating reproducible stablecoin audit evidence**. Auditors and researchers run CLI workflows against configured EVM deployments and block windows; the toolkit writes filesystem artifacts with explicit provenance and claim boundaries.

It is **not**:

- a risk oracle or safety score
- a reserve or attestation audit
- a country-adoption or geo dashboard
- a swap-routing or liquidity execution engine

The core product question:

> Given a stablecoin deployment, block window, and evidence source configuration, what can this toolkit prove, what artifacts does it generate, and what claims remain outside the evidence boundary?

Answers live in **artifacts + manifests**, not in a hosted dashboard’s business logic.

## Architecture layers

```text
┌─────────────────────────────────────────────────────────────┐
│  Frontend evidence browser (v0.5, future)                    │
│  Reads manifest + CSV/JSON/MD only — no audit logic         │
└───────────────────────────────┬─────────────────────────────┘
                                │ HTTP (read-only v0.3)
┌───────────────────────────────▼─────────────────────────────┐
│  Thin API (v0.3) — axum, artifact_root jail                  │
│  Runs, manifests, artifact bytes                               │
└───────────────────────────────┬─────────────────────────────┘
                                │ in-process calls (future: same crate)
┌───────────────────────────────▼─────────────────────────────┐
│  CLI — clap dispatch, stdout summaries, writes out/          │
└───────────────────────────────┬─────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────┐
│  Rust core library                                           │
│  domain · application · artifact · rpc · config · report   │
│  Owns workflows, contracts, reproducibility, packaging       │
└─────────────────────────────────────────────────────────────┘
```

### 1. Rust core library

**Owns:** domain models, workflow orchestration, artifact contracts, claim boundaries, source metadata, evidence packaging, RPC fetch/decode/report logic (existing `src/rpc/*`, `config`, `report`, etc.).

**Does not know:** HTTP routes, cookies, frontend frameworks.

**Callable by:** CLI today; API handlers in v0.3 (same crate, no logic duplication).

Suggested module boundaries (v0.2+):

| Module | Responsibility |
|--------|----------------|
| `domain/` | Asset/chain/window identifiers; shared value types; validation rules |
| `application/` | Workflow entrypoints and composition (thin in v0.2; grows without moving audit math) |
| `artifact/` | `ArtifactManifest`, writers, checksums, schema version |
| `rpc/` | Existing per-command implementations (unchanged semantics in v0.2) |
| `report/` | Output path helpers (`out/<asset>/runs/<run_id>/`) |
| `cli/` | Argument parsing and dispatch only |

### 2. CLI

**Owns:** Developer/auditor UX — `clap` structs, validation of flags, tokio runtime bootstrap, calling `rpc::*::run`, printing paths and gate summaries.

**Does not own:** New audit definitions (those stay in `rpc` until deliberately refactored into `application`).

Existing commands (0.1.0) remain: `transfer-audit`, `cross-chain-summary`, `resolve-window`, `metadata`, `stablecoin-map-package`, plus experimental `fetch` / `report` / `control-*`.

### 3. Thin API wrapper (v0.3)

**Owns:** Serving pre-generated artifacts from `--artifact-root`; listing runs; returning manifest JSON.

**Does not:** Re-run audits on GET; proxy shell commands; generate charts; call RPC.

**Status:** Read-only skeleton behind `cargo build --features api` and `stablecoin-audit serve`. See [`api_design_v0.md`](api_design_v0.md).

### 4. Frontend evidence browser (v0.5, future)

**Owns:** Presentation — evidence cards, tables, claim-boundary copy, links from claims to artifact paths.

**Reads:** `GET /api/runs`, manifest, and artifact bodies.

**Does not:** Recompute supply invariants, parse Transfer logs, or call RPC.

## Why the frontend is only an evidence browser

Audit semantics are versioned in Rust (decode rules, gate definitions, block pinning). If the frontend encodes those rules, evidence drifts from the toolkit: two implementations, two PASS/FAIL meanings.

The manifest’s `supported_claims` / `unsupported_claims` fields document what the **toolkit** attests. The UI renders that contract; it does not extend it.

## Filesystem layout (runs)

Default artifact root aligns with CLI output:

```text
out/
  <asset>/
    runs/
      <run_id>/
        artifact_manifest.json   # v0.2+ product manifest (alongside legacy files)
        provenance.json
        qa_report.json
        supply_audit.csv
        summary.md
        ...
    metadata.json                # metadata command
```

Published benchmarks under `docs/benchmarks/<run_id>/` mirror the same filenames for git-backed evidence.

## Roadmap

| Version | Goal | Implement now? |
|---------|------|----------------|
| **v0.2** | Module refactor + `ArtifactManifest` schema + product docs | Yes (skeleton) |
| **v0.3** | Read-only `serve` API over `artifact_root` | Skeleton shipped (`--features api`) |
| **v0.4** | POST runs, status, logs, cancel; job queue | Roadmap only |
| **v0.5** | Static/SPA evidence browser | Roadmap only |

## Public API boundaries (library)

**Stable for integrators (grow deliberately):**

- `stablecoin_audit::run_cli` — binary and tests
- `report::{ensure_run_out_dir, validate_run_id, default_run_id}`
- `artifact::manifest::*` — manifest types and serialization
- Future: `application::workflow::*` — named workflow runners without CLI

**Internal / unstable:**

- `rpc::*` — command-specific; may move behind `application` without semantic changes
- Checkpoint manifest (`transfer_checkpoint::CheckpointManifest`) — resume only, not the product manifest

## Relationship to existing artifacts

v0.1 commands already emit `provenance.json`, `qa_report.json`, `supply_audit.csv`, etc. v0.2 adds a **unifying** `artifact_manifest.json` that indexes those files and states claim boundaries. Legacy files remain until each workflow optionally writes the product manifest at end of run (incremental adoption in later PRs).
