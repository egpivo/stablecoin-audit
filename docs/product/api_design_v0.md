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

The server treats `--artifact-root` as a jail.

### Canonical product contract

`artifact_manifest.json` is the **single source of truth** for product runs:

- Run discovery, artifact listings, claim boundaries, and package generation all read this file only.
- A run directory may contain CSV/JSON outputs from a partial or legacy workflow; without a **valid** `artifact_manifest.json` it is **incomplete**, **not API-listed**, and **not packageable**.
- Legacy directory discovery (for example inferring runs from `qa_report.json` alone) is **intentionally unsupported**.

| Pattern | Interpretation |
|---------|----------------|
| `{root}/{asset}/runs/{run_id}/artifact_manifest.json` | Valid product run manifest (schema `artifact-manifest-v0`) |

`transfer-audit` writes `artifact_manifest.json` only after a **successful** run: all chains complete without hard errors, then checkpoint removed, then manifest written. If checkpoint cleanup or manifest write fails, the command errors and the run is not API-listed. Partial failed runs keep CSV/JSON outputs and checkpoint but **no** product manifest.

`cross-chain-summary` **upserts** the existing manifest (required): adds `cross_chain_summary.json` / `.md`, appends a `workflow_steps` entry for `cross-chain-summary`, and preserves `command: transfer-audit` plus transfer-audit artifacts. Re-running cross-chain-summary is idempotent for artifact entries.

`stablecoin-map-package` reads `artifact_manifest.json` only (no directory scanning), validates listed artifacts exist on disk, writes `package_manifest.json` and `stablecoin_map_package.zip`.

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

Artifact metadata from the manifest (paths relative to `artifact_root`). Only paths listed in `artifact_manifest.json` are returned; extra files on disk are ignored.

### `POST /api/runs/{run_id}/package`

Build or replace `package_manifest.json` and `stablecoin_map_package.zip` from `artifact_manifest.json` (manifest-driven; no directory scanning). Optional query: `?asset=USDC`.

Returns `PackageManifest` JSON. Fails with `manifest_not_found` when `artifact_manifest.json` is missing, or `not_found` when a manifest-listed artifact file is missing on disk.

### `GET /api/runs/{run_id}/package`

Returns existing `package_manifest.json` metadata for the run, or `package_not_found` if not generated yet.

### `GET /api/runs/{run_id}/package/download`

Downloads `stablecoin_map_package.zip` for a generated package.

- **Content-Type:** `application/zip`
- **Content-Disposition:** `attachment; filename="{asset}_{run_id}_stablecoin-map-package.zip"`
- Returns `package_not_found` when the zip file is missing.
- Returns `package_corrupt` when `package_manifest.json` is missing or invalid.

Optional query: `?asset=USDC`.

### `POST /api/runs/{run_id}/package/verify`

Manifest-driven verification of a generated package. Reads `package_manifest.json`, recomputes the package content checksum from the zip (excluding the embedded `package_manifest.json` entry), and validates each listed artifact’s `checksum_sha256` against zip entry bytes.

Returns structured JSON:

```json
{
  "run_id": "usdc_7d_20260501_20260508",
  "asset": "USDC",
  "package_kind": "stablecoin-map-package",
  "package_valid": true,
  "expected_package_checksum_sha256": "…",
  "actual_package_checksum_sha256": "…",
  "artifacts": [
    {
      "path": "supply_audit.csv",
      "valid": true,
      "expected_checksum_sha256": "…",
      "actual_checksum_sha256": "…"
    }
  ]
}
```

Only artifacts listed in `package_manifest.json` are validated; extra zip entries are ignored for per-artifact results. Verification does not scan the run directory for files.

Optional query: `?asset=USDC`.

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
| 404 | `manifest_not_found` | No valid `artifact_manifest.json` for run |
| 404 | `package_not_found` | No `package_manifest.json` for run metadata, or package zip missing on download |
| 422 | `package_corrupt` | `package_manifest.json` missing or invalid when download/verify requires it |
| 500 | `io_error` | Unexpected read failure or invalid `artifact_manifest.json` JSON/schema |

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

## Package generation (manifest-driven)

- **Input:** valid `artifact_manifest.json` for the run.
- **Output:** `package_manifest.json` (metadata + included artifact refs copied from the product manifest) and `stablecoin_map_package.zip` (manifest + listed artifacts).
- **Checksum:** `package_checksum_sha256` is SHA-256 of zip entry bytes excluding `package_manifest.json` (stable while embedding the sidecar manifest).
- **Download:** `GET /api/runs/{run_id}/package/download` serves the zip bytes with deterministic attachment filename.
- **Verify:** `POST /api/runs/{run_id}/package/verify` checks package and artifact checksums from `package_manifest.json` only — not directory discovery.
- **Not supported:** inferring package contents by scanning the run directory; legacy `qa_report.json`-only discovery.

## Future: global package listing (not implemented)

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

## Frontend contract (v0.3 + browser)

The browser consumes v0.3 GET endpoints (and package POST for build/verify). Implemented at `/ui/`. See [`evidence_browser_v0.md`](evidence_browser_v0.md).

Panels map to manifest sections (run overview, claim boundaries, artifacts, package). No chart generation on server. `POST /api/runs` audit orchestration remains roadmap v0.4.
