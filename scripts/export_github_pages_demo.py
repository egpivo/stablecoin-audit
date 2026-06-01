#!/usr/bin/env python3
"""
Export a static GitHub Pages demo under docs/.

Uses a dummy run_id (github_pages_demo) so it never collides with local runs.
Source: out/<asset>/runs/<source_run_id>/ or --source-run.

Large transfer CSVs are omitted from git; the manifest is filtered to shipped files only.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
UI_SRC = ROOT / "ui"

DEMO_RUN_ID = "github_pages_demo"
DEMO_ASSET = "USDC"
DEMO_ASSET_DIR = "usdc"

# Shipped to Pages (skip multi-MB transfer tables).
SHIP_GLOBS = (
    "artifact_manifest.json",
    "audit_plan.json",
    "evidence_sources.json",
    "deployment_registry.json",
    "chain_windows.json",
    "supply_snapshots.csv",
    "qa_report.json",
    "provenance.json",
    "summary.md",
    "supply_audit.csv",
    "supply_audit.md",
    "execution_log.ndjson",
)

MAX_FILE_BYTES = 450_000


def rewrite_run_id(obj, old: str, new: str):
    if isinstance(obj, dict):
        for k, v in obj.items():
            if k in ("run_id", "package_id") and isinstance(v, str) and v == old:
                obj[k] = new
            elif k == "message" and isinstance(v, str):
                obj[k] = v.replace(old, new)
            else:
                rewrite_run_id(v, old, new)
    elif isinstance(obj, list):
        for item in obj:
            rewrite_run_id(item, old, new)


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def save_json(path: Path, data) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def build_artifacts_response(manifest: dict, prefix: str) -> dict:
    from_kind = manifest.get("artifacts", [])
    artifacts = []
    for a in from_kind:
        path = a.get("path", "")
        artifacts.append(
            {
                "kind": a.get("kind"),
                "path": f"{prefix}/{path}",
                "format": a.get("format"),
                "row_count": a.get("row_count"),
                "checksum_sha256": a.get("checksum_sha256"),
                "description": a.get("description", ""),
            }
        )
    return {
        "run_id": manifest["run_id"],
        "asset": manifest["asset"],
        "artifacts": artifacts,
    }


def parse_logs(ndjson_path: Path) -> dict:
    entries = []
    for line in ndjson_path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line:
            entries.append(json.loads(line))
    return {"entries": entries}


def main() -> None:
    parser = argparse.ArgumentParser(description="Build docs/ for GitHub Pages demo")
    parser.add_argument(
        "--source-run",
        default="article_ui_demo",
        help="Run directory name under out/usdc/runs/",
    )
    parser.add_argument(
        "--artifact-root",
        type=Path,
        default=ROOT / "out",
        help="Artifact root (default: out/)",
    )
    args = parser.parse_args()

    source_dir = args.artifact_root / DEMO_ASSET_DIR / "runs" / args.source_run
    if not source_dir.is_dir():
        raise SystemExit(
            f"Source run not found: {source_dir}\n"
            "Run transfer-audit locally first, or pass --source-run / --artifact-root."
        )

    # Clean prior export (keep product screenshots README area)
    for name in ("ui", "demo-api", "demo-artifacts", "demo", "index.html", "GITHUB_PAGES.md"):
        target = DOCS / name
        if target.is_dir():
            shutil.rmtree(target)
        elif target.is_file():
            target.unlink()

    # UI
    ui_out = DOCS / "ui"
    shutil.copytree(UI_SRC, ui_out)

    # Inject demo config into index.html
    index = (ui_out / "index.html").read_text(encoding="utf-8")
    inject = """  <script>
    window.STABLECOIN_AUDIT_DEMO = {
      enabled: true,
      label: "GitHub Pages demo",
      readOnlyMessage: "This public demo is read-only. Run audits locally with the Rust CLI.",
      defaultRun: { asset: "USDC", run_id: "github_pages_demo" }
    };
  </script>
  <script src="demo-mode.js?v=3"></script>
  <script src="app.js?v=3" type="module"></script>
"""
    index = index.replace(
        '  <script src="app.js" type="module"></script>',
        inject,
    )
    (ui_out / "index.html").write_text(index, encoding="utf-8")

    # Artifacts on disk
    dest_run = DOCS / "demo-artifacts" / DEMO_ASSET_DIR / "runs" / DEMO_RUN_ID
    dest_run.mkdir(parents=True, exist_ok=True)
    shipped_names: set[str] = set()

    for pattern in SHIP_GLOBS:
        for src in source_dir.glob(pattern):
            if not src.is_file():
                continue
            if src.stat().st_size > MAX_FILE_BYTES:
                print(f"skip (too large): {src.name}")
                continue
            shutil.copy2(src, dest_run / src.name)
            shipped_names.add(src.name)

    manifest = load_json(dest_run / "artifact_manifest.json")
    old_run = manifest.get("run_id", args.source_run)
    manifest["run_id"] = DEMO_RUN_ID
    manifest["artifacts"] = [a for a in manifest["artifacts"] if a.get("path") in shipped_names]
    rewrite_run_id(manifest, old_run, DEMO_RUN_ID)
    save_json(dest_run / "artifact_manifest.json", manifest)

    for name in shipped_names:
        if name.endswith(".json"):
            p = dest_run / name
            if p.exists():
                data = load_json(p)
                rewrite_run_id(data, old_run, DEMO_RUN_ID)
                save_json(p, data)

    if (dest_run / "qa_report.json").exists():
        qa = load_json(dest_run / "qa_report.json")
        rewrite_run_id(qa, old_run, DEMO_RUN_ID)
        save_json(dest_run / "qa_report.json", qa)

    prefix = f"{DEMO_ASSET_DIR}/runs/{DEMO_RUN_ID}"
    api_run = DOCS / "demo-api" / DEMO_ASSET_DIR / DEMO_RUN_ID
    api_run.mkdir(parents=True, exist_ok=True)

    save_json(api_run / "manifest.json", manifest)
    save_json(api_run / "artifacts.json", build_artifacts_response(manifest, prefix))

    log_path = dest_run / "execution_log.ndjson"
    if log_path.exists():
        logs = parse_logs(log_path)
        rewrite_run_id(logs, old_run, DEMO_RUN_ID)
        save_json(api_run / "logs.json", logs)

    save_json(
        api_run / "status.json",
        {
            "run_id": DEMO_RUN_ID,
            "asset": DEMO_ASSET,
            "status": "succeeded",
            "error": None,
            "has_manifest": True,
        },
    )

    save_json(
        DOCS / "demo-api" / "runs.json",
        {
            "runs": [
                {
                    "asset": DEMO_ASSET,
                    "run_id": DEMO_RUN_ID,
                    "command": manifest.get("command"),
                    "generated_at": manifest.get("generated_at"),
                    "manifest_path": f"{prefix}/artifact_manifest.json",
                }
            ]
        },
    )

    save_json(
        DOCS / "demo-api" / "health.json",
        {"status": "ok", "toolkit_version": manifest.get("toolkit_version", "0.1.0"), "mode": "github-pages-demo"},
    )

    # Site root
    (DOCS / "index.html").write_text(
        """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Stablecoin Audit — GitHub Pages demo</title>
  <meta http-equiv="refresh" content="0; url=ui/">
  <link rel="canonical" href="ui/">
</head>
<body>
  <p><a href="ui/">Open Evidence Console (GitHub Pages demo)</a></p>
</body>
</html>
""",
        encoding="utf-8",
    )

    (DOCS / "demo" / "README.md").parent.mkdir(parents=True, exist_ok=True)
    (DOCS / "demo" / "README.md").write_text(
        f"""# GitHub Pages static demo

This folder documents the **read-only public demo** served from GitHub Pages.

- **Dummy run id:** `{DEMO_RUN_ID}` (not a live audit; recorded evidence only)
- **Asset:** {DEMO_ASSET}, ethereum blocks 24000000–24000100 (example window)
- **No RPC, no local audit:** `POST /api/runs` and clean-history are disabled in the UI

Regenerate the site bundle:

```bash
python3 scripts/export_github_pages_demo.py
# optional: --source-run demo_001 --artifact-root out/
```

Then enable Pages: repository **Settings → Pages → Build from branch `main`, folder `/docs`**.

Live URL (after deploy): `https://<org>.github.io/stablecoin-audit/ui/`
""",
        encoding="utf-8",
    )

    (DOCS / "GITHUB_PAGES.md").write_text(
        (DOCS / "demo" / "README.md").read_text(encoding="utf-8"),
        encoding="utf-8",
    )

    print(f"Exported GitHub Pages demo to {DOCS}")
    print(f"  run_id={DEMO_RUN_ID}  artifacts={len(shipped_names)} files")


if __name__ == "__main__":
    main()
