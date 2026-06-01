# Evidence console (v0)

Status: **developer-facing evidence console** served by the read-only API at `/ui/`.

## Purpose

The evidence console makes one completed audit run understandable in under 30 seconds: what was audited, what evidence was generated, which claims are supported, what is explicitly out of scope, and how to verify artifacts.

It reads generated artifacts and `artifact_manifest.json` only. It does not run audits on its own (except optional local developer mode via `POST /api/runs`), call RPC from the browser, compute claims, or show risk scores.

This is not a trading dashboard, DeFi analytics UI, reserve audit, peg scorecard, or generic blockchain explorer.

## What changed (v0 redesign)

| Before | After |
|--------|--------|
| Artifact-heavy inspector layout | **Run header + metric cards**; **Overview** tab for 30-second scan |
| Request builder squeezed into sidebar | **Modal** triggered by **New local audit** (header + empty states) |
| Long vertical stack | **Tabs:** Overview · Claims · Logs · Artifacts · Package |
| Sidebar overloaded | **Runs-only sidebar** with filter and status pills |
| Long unsupported claim list | **Grouped out-of-scope** categories on Claims tab |
| Large disclaimer banners | Short info line + compact scope note |
| Raw JSON prominent | Collapsed package JSON; artifact table with filters |

## Design source (Figma)

- **Main mockup:** [Stablecoin Audit Evidence Console v0](https://www.figma.com/design/3D2mB6WpBaXt6nODIbDCOc/Stablecoin-Audit-Evidence-Console-v0?node-id=1-2)
- **Architecture diagram (FigJam):** [Stablecoin audit pipeline](https://www.figma.com/board/cfflnhHAXwXpCLHeOW058V/Stablecoin-audit-pipeline)

## Screenshots

Committed product diagram: [`screenshots/architecture_pipeline.svg`](screenshots/architecture_pipeline.svg).

UI captures and article figures (including GIFs) live under **`.local/blog/figures/`** (gitignored; not in pre-commit). Regenerate with `.local/blog/scripts/capture_product_demo.py` after a completed run.

Recapture live UI after a completed run:

```bash
cargo run --features api -- serve --artifact-root out/
open http://127.0.0.1:8080/ui/
# Select a run; screenshot the main console pane
```

## Architecture

```text
Rust CLI (transfer-audit, cross-chain-summary)
    → filesystem artifacts under out/<asset>/runs/<run_id>/
    → artifact_manifest.json (product contract)
    → read-only Axum API (--features api)
    → static evidence console at /ui/
```

Implementation: vanilla HTML/CSS/JavaScript under `ui/`. No build step. `tower-http` `ServeDir` from `CARGO_MANIFEST_DIR/ui`.

## UI hierarchy

1. **Left sidebar — Runs** — filter, asset, run_id, status pill, chain count, timestamp; local audit request at bottom.
2. **Audit summary** — PASS/WARN/FAIL, asset/run, chain window, transfer rows, supply snapshots, reconciliation, package status.
3. **Claim boundaries + execution log** — supported claims; out-of-scope grouped (off-chain, market/liquidity, identity/geography, bridge/routing/stress); terminal-style log with PASS/WARN/FAIL highlighting.
4. **Evidence package** — artifact count, checksum, build / download / verify; collapsed package JSON.
5. **Raw artifacts** — collapsed table with download links.

## API endpoints used

| Method | Path | Panel |
|--------|------|-------|
| `GET` | `/health` | Footer |
| `GET` | `/api/runs` | Run list |
| `GET` | `/api/runs/{run_id}/manifest?asset=` | Summary, claims |
| `GET` | `/api/runs/{run_id}/artifacts?asset=` | Artifact table, QA fetch |
| `GET` | `/api/artifacts/{path}` | Downloads, `qa_report.json`, `execution_log.ndjson` |
| `GET` | `/api/runs/{run_id}/package?asset=` | Package panel |
| `POST` | `/api/runs/{run_id}/package?asset=` | Build package |
| `GET` | `/api/runs/{run_id}/package/download?asset=` | Download zip |
| `POST` | `/api/runs/{run_id}/package/verify?asset=` | Verify |
| `POST` | `/api/runs` | Local run (developer mode) |
| `GET` | `/api/runs/{run_id}/status?asset=` | Local run polling |
| `GET` | `/api/runs/{run_id}/logs?asset=` | Local run polling |

## What the UI does not claim

Reserve adequacy, peg stability, redemption capacity, user geography, issuer intent, actual swap routing, or any metric not present in listed artifacts.

## Run locally

```bash
# Produce a run (if needed)
cargo run -- transfer-audit --asset USDC --run-id demo_001 \
  --window ethereum:24000000:24001000

# Serve API + console
cargo run --features api -- serve --artifact-root out/
open http://127.0.0.1:8080/ui/
```

Local **Run audit locally** requires RPC env (`.env`) on the machine running the API.

## Known limitations

- **Manifest required** — runs without `artifact_manifest.json` do not appear in the list.
- **Same-origin** — open via the API URL, not `file://`.
- **No auth** — loopback demo only.
- **Run list status** — derived from `qa_report.json` when present (extra GET per run on load).
- **No charts** — tabular/textual evidence only.

## Related docs

- [`api_design_v0.md`](api_design_v0.md)
- [`artifact_manifest_schema_v0.md`](artifact_manifest_schema_v0.md)
- [`article_notes_stablecoin_audit_toolkit.md`](article_notes_stablecoin_audit_toolkit.md)
