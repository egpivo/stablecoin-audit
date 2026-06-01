/**
 * GitHub Pages read-only demo: static JSON under /demo-api and /demo-artifacts.
 * Enabled via window.STABLECOIN_AUDIT_DEMO in index.html (injected by export script).
 */
(function () {
  const cfg = window.STABLECOIN_AUDIT_DEMO;
  if (!cfg?.enabled) return;

  function siteBasePath() {
    const path = window.location.pathname;
    const ui = path.indexOf("/ui/");
    if (ui >= 0) return path.slice(0, ui + 1);
    if (path.endsWith("/ui")) return path.slice(0, -2) || "/";
    return "/";
  }

  function apiRoot() {
    return `${siteBasePath()}demo-api`;
  }

  function artifactsRoot() {
    return `${siteBasePath()}demo-artifacts`;
  }

  function parseAssetQuery(path) {
    const q = path.indexOf("?");
    if (q < 0) return { asset: null, pathOnly: path };
    const params = new URLSearchParams(path.slice(q));
    return { asset: params.get("asset"), pathOnly: path.slice(0, q) };
  }

  async function fetchJson(url) {
    const res = await fetch(url);
    if (!res.ok) {
      const err = new Error(res.statusText || `HTTP ${res.status}`);
      err.status = res.status;
      throw err;
    }
    return res.json();
  }

  window.demoApiFetch = async function demoApiFetch(path, options = {}) {
    const method = (options.method || "GET").toUpperCase();
    if (method !== "GET") {
      const err = new Error(
        cfg.readOnlyMessage ||
          "GitHub Pages demo is read-only. Clone the repo and run audits locally."
      );
      err.status = 405;
      throw err;
    }

    const { asset, pathOnly } = parseAssetQuery(path);
    const root = apiRoot();

    if (pathOnly === "/health" || pathOnly === "/api/health") {
      return fetchJson(`${root}/health.json`);
    }

    if (pathOnly === "/api/runs") {
      return fetchJson(`${root}/runs.json`);
    }

    const runMatch = pathOnly.match(/^\/api\/runs\/([^/]+)\/(manifest|artifacts|status|logs)$/);
    if (runMatch) {
      const runId = decodeURIComponent(runMatch[1]);
      const kind = runMatch[2];
      const assetDir = (asset || cfg.defaultRun?.asset || "USDC").toLowerCase();
      return fetchJson(`${root}/${assetDir}/${runId}/${kind}.json`);
    }

    const artMatch = pathOnly.match(/^\/api\/artifacts\/(.+)$/);
    if (artMatch) {
      const rel = decodeURIComponent(artMatch[1]);
      const url = `${artifactsRoot()}/${rel}`;
      const res = await fetch(url);
      if (!res.ok) {
        const err = new Error("Artifact not included in GitHub Pages demo bundle.");
        err.status = res.status;
        throw err;
      }
      const ct = res.headers.get("content-type") || "";
      if (ct.includes("json") || rel.endsWith(".json")) return res.json();
      return res;
    }

    if (pathOnly.startsWith("/api/runs/") && pathOnly.includes("/package")) {
      const err = new Error("Evidence bundle not built in GitHub Pages demo.");
      err.status = 404;
      throw err;
    }

    const err = new Error(`Demo API: no static handler for ${pathOnly}`);
    err.status = 404;
    throw err;
  };

  window.applyGithubPagesDemoChrome = function applyGithubPagesDemoChrome() {
    const label = cfg.label || "GitHub Pages demo";
    document.querySelectorAll(".mode-badge").forEach((el) => {
      el.textContent = label;
    });

    const banner = document.createElement("div");
    banner.className = "github-pages-demo-banner";
    banner.setAttribute("role", "note");
    banner.innerHTML = `
      <strong>${label}</strong> — read-only snapshot (<code>run_id=github_pages_demo</code>).
      Transfer tables are omitted from this bundle. Run full audits locally with the Rust CLI.
    `;
    document.body.prepend(banner);

    const hide = [
      "btn-open-audit-modal",
      "btn-clean-history",
      "btn-landing-start-audit",
      "btn-sidebar-new-audit",
      "btn-empty-new-audit",
      "btn-clean-confirm",
      "btn-run-local-audit",
    ];
    hide.forEach((id) => {
      const el = document.getElementById(id);
      if (el) el.hidden = true;
    });

    const loadDemo = document.getElementById("btn-landing-load-demo");
    if (loadDemo) {
      loadDemo.textContent = "Open demo evidence";
    }
  };
})();
