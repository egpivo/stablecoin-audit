# GitHub Pages static demo

This folder documents the **read-only public demo** served from GitHub Pages.

- **Dummy run id:** `github_pages_demo` (not a live audit; recorded evidence only)
- **Asset:** USDC, ethereum blocks 24000000–24000100 (example window)
- **No RPC, no local audit:** `POST /api/runs` and clean-history are disabled in the UI

Regenerate the site bundle:

```bash
python3 scripts/export_github_pages_demo.py
# optional: --source-run demo_001 --artifact-root out/
```

Then enable Pages: repository **Settings → Pages → Build from branch `main`, folder `/docs`**.

Live URL (after deploy): `https://<org>.github.io/stablecoin-audit/ui/`
