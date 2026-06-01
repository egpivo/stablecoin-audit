# Evidence browser (v0)

Status: **minimal local browser** served by the read-only API at `/ui/`.

## Purpose

The evidence browser makes one completed audit run easy to inspect for demos and product review. It reads generated artifacts and manifests only — it does not run audits, call RPC, compute claims, or infer conclusions beyond what `artifact_manifest.json` lists.

This is not a risk dashboard, reserve audit, peg scorer, or liquidity tool.

## Architecture

```text
Rust CLI (transfer-audit, cross-chain-summary)
    → filesystem artifacts under out/<asset>/runs/<run_id>/
    → artifact_manifest.json (product contract)
    → read-only Axum API (--features api)
    → static evidence browser at /ui/
```

The browser is vanilla HTML/CSS/JavaScript under `ui/`. No build step. The API serves those files with `tower-http` `ServeDir` from the repo root at compile time (`CARGO_MANIFEST_DIR/ui`).

## API endpoints used

| Method | Path | Panel |
|--------|------|-------|
| `GET` | `/health` | Footer status |
| `GET` | `/api/runs` | Run list |
| `GET` | `/api/runs/{run_id}/manifest?asset=` | Run overview, claim boundaries |
| `GET` | `/api/runs/{run_id}/artifacts?asset=` | Artifact table |
| `GET` | `/api/artifacts/{path}` | Artifact download links |
| `GET` | `/api/runs/{run_id}/package?asset=` | Package info (if built) |
| `POST` | `/api/runs/{run_id}/package?asset=` | Build package |
| `GET` | `/api/runs/{run_id}/package/download?asset=` | Download zip |
| `POST` | `/api/runs/{run_id}/package/verify?asset=` | Verify checksums |

All run-scoped routes pass `?asset=` when the run descriptor includes an asset (required when the same `run_id` exists under multiple asset directories).

## What the browser shows

1. **Run list** — asset, run_id, command, generated_at from `/api/runs`.
2. **Run overview** — asset, run_id, command, toolkit version, inputs, workflow steps, source snapshots / windows, manifest warnings.
3. **Claim boundaries** — supported/conditional claims and unsupported (out-of-scope) claims from the manifest. Visually prominent two-column layout.
4. **Artifact table** — kind, path, format, row_count, description; download via `/api/artifacts/{path}`.
5. **Package panel** — build, download, verify buttons when package endpoints are available; shows package metadata when already built.
6. **Demo note** — fixed banner stating what the browser does not claim.

## What it does not show

- Reserve adequacy, peg stability, redemption capacity, user geography, issuer intent, or actual swap routing (also stated in the UI banner).
- Live RPC or job orchestration.
- Charts, scores, or derived risk metrics.
- Files on disk that are not listed in `artifact_manifest.json`.
- Runs without a valid `artifact_manifest.json`.

## Run instructions

### 1. Produce a completed run (if needed)

```bash
cargo run -- transfer-audit --asset USDC --run-id demo_001 \
  --window ethereum:24000000:24001000
```

Successful runs write `out/<asset>/runs/<run_id>/artifact_manifest.json`.

Optional: add cross-chain summary and build a package from the CLI or browser.

### 2. Start the API + browser

```bash
cargo run --features api -- serve --artifact-root out/
```

Open:

```text
http://127.0.0.1:8080/ui/
```

The server logs the browser URL on startup.

### 3. Verify

- Run list populates from `/api/runs`.
- Selecting a run loads manifest and artifacts.
- Claim boundaries render supported and unsupported sections.
- At least one artifact download link returns file bytes.

## Screenshots

Capture after starting the server with at least one completed run:

| File | Description |
|------|-------------|
| `docs/product/screenshots/evidence_browser_run_list.png` | Run list + overview (placeholder — capture locally) |
| `docs/product/screenshots/evidence_browser_claims.png` | Claim boundaries panel (placeholder — capture locally) |

To capture:

```bash
cargo run --features api -- serve --artifact-root out/
# open http://127.0.0.1:8080/ui/ and screenshot each panel
```

## Known limitations

- **Manifest required** — Legacy benchmark dirs under `docs/benchmarks/` without `artifact_manifest.json` do not appear in the run list.
- **Same-origin only** — UI is served from the API; no separate dev server or CORS layer. Running UI files directly from disk will fail API calls.
- **No auth** — Local loopback demo only; not hardened for production exposure.
- **No charts** — Tabular and textual evidence only; no server-side chart generation.
- **Package build is synchronous** — Large runs may take noticeable time on POST package.
- **Single artifact root** — One `--artifact-root` per server process.

## Related docs

- API design: [`api_design_v0.md`](api_design_v0.md)
- Manifest schema: [`artifact_manifest_schema_v0.md`](artifact_manifest_schema_v0.md)
- Article notes: [`article_notes_stablecoin_audit_toolkit.md`](article_notes_stablecoin_audit_toolkit.md)
