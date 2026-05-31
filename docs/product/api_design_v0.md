# API design (v0.3)

Status: **read-only skeleton implemented** behind Cargo feature `api`.

## Principles

1. **Thin wrapper** — Handlers read the filesystem and deserialize manifests; they do not call `eth_getLogs` or recompute gates.
2. **Artifact-first** — Good nouns: `Run`, `Artifact`, `ArtifactManifest`, `EvidencePackage`, `ClaimBoundary`, `SourceSnapshot`.
3. **No shell proxy** — Bad pattern: `POST /run-shell-command`.
4. **Read-only** — No `POST /api/runs`, RPC, chart generation, or audit re-runs from HTTP.

The API exposes evidence artifacts and manifests to a **future frontend evidence browser**. It does not run audits, call RPC, or generate charts on the server.

## Command

```bash
cargo run --features api -- serve \
  --artifact-root out/ \
  --host 127.0.0.1 \
  --port 8080
```

- Default `artifact-root`: `out/`
- Binds loopback by default (local evidence review)

## Filesystem model

The server treats `--artifact-root` as a jail. Discovery (v0.3 skeleton):

| Pattern | Interpretation |
|---------|----------------|
| `{root}/{asset}/runs/{run_id}/artifact_manifest.json` | Primary run manifest |

`transfer-audit` writes `artifact_manifest.json` only after a **successful** run: all chains complete without hard errors, then checkpoint removed, then manifest written. If checkpoint cleanup or manifest write fails, the command errors and the run is not API-listed. Partial failed runs keep CSV/JSON outputs and checkpoint but **no** product manifest.

`cross-chain-summary` **upserts** the existing manifest (required): adds `cross_chain_summary.json` / `.md`, appends a `workflow_steps` entry for `cross-chain-summary`, and preserves `command: transfer-audit` plus transfer-audit artifacts. Re-running cross-chain-summary is idempotent for artifact entries.

Legacy fallback (`qa_report.json` without product manifest) is documented for future listing; not implemented yet.

## Endpoints (implemented)

### `GET /health`

```json
{ "status": "ok", "toolkit_version": "0.1.0" }
```

### `GET /api/runs`

Lists runs that have `artifact_manifest.json`.

```json
{
  "runs": [
    {
      "asset": "USDC",
      "run_id": "usdc_7d_20260501_20260508",
      "command": "transfer-audit",
      "generated_at": "2026-05-15T08:03:31.695921+00:00",
      "manifest_path": "usdc/runs/usdc_7d_20260501_20260508/artifact_manifest.json"
    }
  ]
}
```

### `GET /api/runs/{run_id}/manifest`

Returns full `ArtifactManifest` JSON.

Optional query: `?asset=USDC` — required when the same `run_id` exists under multiple assets.

### `GET /api/runs/{run_id}/artifacts`

Artifact metadata from the manifest (paths relative to `artifact_root`).

### `GET /api/artifacts/{*artifact_path}`

Serves raw file bytes. Path is relative to `artifact_root` (URL-encoded).

| Extension | Content-Type |
|-----------|----------------|
| `.json` | `application/json` |
| `.csv` | `text/csv` |
| `.md` | `text/markdown` |
| other | `application/octet-stream` |

Example:

```http
GET /api/artifacts/usdc/runs/usdc_7d_20260501_20260508/supply_audit.csv
```

## Error model

| HTTP | `error` code | When |
|------|----------------|------|
| 400 | `invalid_path` | `..`, `.` segments, trailing `/`, NUL, absolute path, directory when a file is required |
| 400 | `ambiguous_run_id` | Multiple assets share `run_id` without `?asset=` |
| 404 | `not_found` | Artifact file missing |
| 404 | `manifest_not_found` | No `artifact_manifest.json` for run |
| 500 | `io_error` | Unexpected read failure |

```json
{
  "error": "invalid_path",
  "message": "artifact path must stay under artifact root"
}
```

## Path traversal protection

Implementation: `src/api/path_jail.rs` + `artifact::resolve_artifact_under_root`.

1. Reject NUL, `\`, `..`, `.` segments, trailing `/`, and paths with no file segment before join.
2. Canonicalize `artifact_root` inside `resolve_artifact_under_root`, then canonicalize the target; require `target.starts_with(root)`.
3. For byte serving, require `canonical.is_file()`.
4. Reject symlink targets outside root (canonicalize follows links).

## Rust modules

```text
src/api/
  mod.rs           # serve()
  routes.rs        # axum router
  artifact_store.rs
  path_jail.rs
  error.rs
```

Enable with `cargo build --features api`.

## Future: evidence packages (not implemented)

```http
GET /api/packages
GET /api/packages/{package_id}/manifest
```

For `stablecoin-map-package` and bundled evidence; same read-only jail model.

## Roadmap: v0.4 run orchestration (not implemented)

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/api/runs` | Enqueue workflow |
| GET | `/api/runs/{run_id}/status` | Job status |
| GET | `/api/runs/{run_id}/logs` | Log tail/stream |
| POST | `/api/runs/{run_id}/cancel` | Cancel |

## Frontend contract (v0.5)

The browser consumes v0.3 GET endpoints only. Panels map to manifest sections (run overview, claim boundaries, supply invariant, transfers, DEX liquidity, one-hop deps, source snapshots). No chart generation on server.
