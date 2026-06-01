/**
 * GitHub Pages read-only demo: static JSON under ../demo-api and ../demo-artifacts.
 */
(function () {
  const cfg = window.STABLECOIN_AUDIT_DEMO;
  if (!cfg?.enabled) return;

  /** Resolve paths relative to the current UI page (works with /repo/ui and /repo/ui/). */
  function demoUrl(relativePath) {
    return new URL(relativePath, window.location.href).href;
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
      const err = new Error(`Demo bundle not found (${res.status})`);
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
          "This public demo is read-only. Run audits locally with the Rust CLI."
      );
      err.status = 405;
      throw err;
    }

    const { asset, pathOnly } = parseAssetQuery(path);

    if (pathOnly === "/health" || pathOnly === "/api/health") {
      return fetchJson(demoUrl("../demo-api/health.json"));
    }

    if (pathOnly === "/api/runs") {
      return fetchJson(demoUrl("../demo-api/runs.json"));
    }

    const runMatch = pathOnly.match(/^\/api\/runs\/([^/]+)\/(manifest|artifacts|status|logs)$/);
    if (runMatch) {
      const runId = decodeURIComponent(runMatch[1]);
      const kind = runMatch[2];
      const assetDir = (asset || cfg.defaultRun?.asset || "USDC").toLowerCase();
      return fetchJson(demoUrl(`../demo-api/${assetDir}/${runId}/${kind}.json`));
    }

    const artMatch = pathOnly.match(/^\/api\/artifacts\/(.+)$/);
    if (artMatch) {
      const rel = decodeURIComponent(artMatch[1]);
      const url = demoUrl(`../demo-artifacts/${rel}`);
      const res = await fetch(url);
      if (!res.ok) {
        const err = new Error("Artifact not included in this GitHub Pages demo bundle.");
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

    const err = new Error(`Demo API: no handler for ${pathOnly}`);
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
      Large transfer CSVs are omitted from this bundle. Clone the repo to run full audits locally.
    `;
    document.body.prepend(banner);

    [
      "btn-open-audit-modal",
      "btn-clean-history",
      "btn-landing-start-audit",
      "btn-sidebar-new-audit",
      "btn-empty-new-audit",
      "btn-clean-confirm",
      "btn-run-local-audit",
      "btn-empty-back-landing",
      "btn-back-landing",
    ].forEach((id) => {
      const el = document.getElementById(id);
      if (el) el.hidden = true;
    });

    const empty = document.getElementById("empty-state");
    if (empty) {
      empty.hidden = true;
      const card = empty.querySelector(".empty-state-card");
      if (card) card.hidden = true;
    }

    const loadDemo = document.getElementById("btn-landing-load-demo");
    if (loadDemo) loadDemo.hidden = true;
  };

  window.showGithubPagesDemoLoadError = function showGithubPagesDemoLoadError(message) {
    const empty = document.getElementById("empty-state");
    if (!empty) return;
    empty.hidden = false;
    empty.innerHTML = `
      <div class="empty-state-card demo-error-card">
        <h2>Demo evidence could not load</h2>
        <p class="error">${message}</p>
        <p class="muted">Try a hard refresh (cache). If this persists, open an issue on the repository.</p>
      </div>
    `;
  };
})();
