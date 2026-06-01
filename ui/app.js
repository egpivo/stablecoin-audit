/** @typedef {{ asset: string, run_id: string, command: string, generated_at: string, manifest_path: string }} RunDescriptor */

const runListEl = document.getElementById("run-list");
const runListStatusEl = document.getElementById("run-list-status");
const runDetailEl = document.getElementById("run-detail");
const emptyStateEl = document.getElementById("empty-state");
const overviewContentEl = document.getElementById("overview-content");
const supportedClaimsEl = document.getElementById("supported-claims");
const unsupportedClaimsEl = document.getElementById("unsupported-claims");
const artifactsBodyEl = document.getElementById("artifacts-body");
const artifactsStatusEl = document.getElementById("artifacts-status");
const packageContentEl = document.getElementById("package-content");
const packageResultEl = document.getElementById("package-result");
const btnBuildPackage = document.getElementById("btn-build-package");
const btnDownloadPackage = document.getElementById("btn-download-package");
const btnVerifyPackage = document.getElementById("btn-verify-package");
const healthStatusEl = document.getElementById("health-status");

/** @type {RunDescriptor | null} */
let selectedRun = null;

const API = "";

async function apiFetch(path, options = {}) {
  const res = await fetch(`${API}${path}`, options);
  if (!res.ok) {
    let message = res.statusText;
    try {
      const body = await res.json();
      if (body.message) message = body.message;
      else if (body.error) message = body.error;
    } catch {
      /* ignore */
    }
    const err = new Error(message);
    err.status = res.status;
    throw err;
  }
  const ct = res.headers.get("content-type") || "";
  if (ct.includes("application/json")) return res.json();
  return res;
}

function assetQuery(run) {
  return run?.asset ? `?asset=${encodeURIComponent(run.asset)}` : "";
}

function formatDate(iso) {
  if (!iso) return "—";
  try {
    return new Date(iso).toLocaleString(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    });
  } catch {
    return iso;
  }
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

function renderClaimCard(claim) {
  const note =
    claim.statement ||
    claim.caveat ||
    (claim.limitations && claim.limitations.length
      ? claim.limitations.join(" ")
      : "");
  const evidence =
    claim.evidence_artifacts?.length > 0
      ? `Evidence: ${claim.evidence_artifacts.join(", ")}`
      : "";
  const warnings =
    claim.warnings?.length > 0
      ? `Warnings: ${claim.warnings.join("; ")}`
      : "";

  return `
    <article class="claim-card">
      <div>
        <span class="claim-id">${escapeHtml(claim.claim)}</span>
        <span class="claim-status">${escapeHtml(claim.status || "")}</span>
      </div>
      ${note ? `<p class="claim-statement">${escapeHtml(note)}</p>` : ""}
      ${evidence || warnings ? `<div class="claim-meta">${escapeHtml([evidence, warnings].filter(Boolean).join(" · "))}</div>` : ""}
    </article>
  `;
}

function renderOverview(manifest) {
  const inputs =
    manifest.inputs?.length > 0
      ? manifest.inputs.map((i) => `${i.name}=${i.value}`).join(", ")
      : "—";

  const steps = manifest.workflow_steps || [];
  const snapshots = manifest.source_snapshots || [];

  let chainsHtml = "—";
  if (snapshots.length > 0) {
    chainsHtml = snapshots
      .map((s) => {
        const window =
          s.window_start || s.window_end
            ? `${s.window_start || "?"} → ${s.window_end || "?"}`
            : "";
        return `<li><strong>${escapeHtml(s.source_name)}</strong>${window ? ` — ${escapeHtml(window)}` : ""}</li>`;
      })
      .join("");
    chainsHtml = `<ul>${chainsHtml}</ul>`;
  }

  overviewContentEl.innerHTML = `
    <dl class="overview-grid">
      <div class="overview-item"><dt>Asset</dt><dd>${escapeHtml(manifest.asset || "—")}</dd></div>
      <div class="overview-item"><dt>Run ID</dt><dd><code>${escapeHtml(manifest.run_id || "—")}</code></dd></div>
      <div class="overview-item"><dt>Command</dt><dd>${escapeHtml(manifest.command || "—")}</dd></div>
      <div class="overview-item"><dt>Generated</dt><dd>${escapeHtml(formatDate(manifest.generated_at))}</dd></div>
      <div class="overview-item"><dt>Toolkit</dt><dd>${escapeHtml(manifest.toolkit_version || "—")}</dd></div>
      <div class="overview-item"><dt>Inputs</dt><dd>${escapeHtml(inputs)}</dd></div>
    </dl>
    ${
      steps.length > 0
        ? `<div class="workflow-steps"><h3>Workflow steps</h3><ol>${steps
            .map(
              (s) =>
                `<li><strong>${escapeHtml(s.command)}</strong> — ${escapeHtml(formatDate(s.completed_at))}${s.artifacts?.length ? ` (${escapeHtml(s.artifacts.join(", "))})` : ""}</li>`
            )
            .join("")}</ol></div>`
        : ""
    }
    ${
      snapshots.length > 0
        ? `<div class="workflow-steps"><h3>Source snapshots / windows</h3>${chainsHtml}</div>`
        : ""
    }
    ${
      manifest.warnings?.length > 0
        ? `<div class="warnings-list"><strong>Manifest warnings</strong><ul>${manifest.warnings.map((w) => `<li>${escapeHtml(w)}</li>`).join("")}</ul></div>`
        : ""
    }
  `;
}

function renderClaims(manifest) {
  const supported = manifest.supported_claims || [];
  const unsupported = manifest.unsupported_claims || [];

  supportedClaimsEl.innerHTML =
    supported.length > 0
      ? supported.map(renderClaimCard).join("")
      : '<p class="panel-status">No supported claims listed.</p>';

  unsupportedClaimsEl.innerHTML =
    unsupported.length > 0
      ? unsupported.map(renderClaimCard).join("")
      : '<p class="panel-status">No unsupported claims listed.</p>';
}

function renderArtifacts(artifacts) {
  artifactsBodyEl.innerHTML = "";
  if (!artifacts || artifacts.length === 0) {
    artifactsStatusEl.textContent = "No artifacts in manifest.";
    return;
  }
  artifactsStatusEl.textContent = `${artifacts.length} artifact(s)`;

  for (const art of artifacts) {
    const tr = document.createElement("tr");
    const href = `/api/artifacts/${encodeURI(art.path)}`;
    tr.innerHTML = `
      <td>${escapeHtml(art.kind)}</td>
      <td class="path-cell">${escapeHtml(art.path)}</td>
      <td>${escapeHtml(art.format)}</td>
      <td>${art.row_count != null ? art.row_count : "—"}</td>
      <td>${escapeHtml(art.description || "")}</td>
      <td><a class="download-link" href="${href}" download target="_blank" rel="noopener">Download</a></td>
    `;
    artifactsBodyEl.appendChild(tr);
  }
}

function renderPackageInfo(pkg) {
  packageContentEl.innerHTML = `
    <div class="package-info">
      <dl>
        <dt>Kind</dt><dd>${escapeHtml(pkg.package_kind || "—")}</dd>
        <dt>Generated</dt><dd>${escapeHtml(formatDate(pkg.generated_at))}</dd>
        <dt>Artifacts</dt><dd>${pkg.artifacts?.length ?? 0}</dd>
        <dt>Package checksum</dt><dd><code>${escapeHtml((pkg.package_checksum_sha256 || "").slice(0, 16))}…</code></dd>
      </dl>
    </div>
  `;
}

function setPackageButtons(hasRun, hasPackage = false) {
  btnBuildPackage.disabled = !hasRun;
  btnDownloadPackage.disabled = !hasPackage;
  btnVerifyPackage.disabled = !hasPackage;
}

async function loadPackagePanel(run) {
  packageResultEl.hidden = true;
  packageResultEl.textContent = "";
  setPackageButtons(!!run, false);

  if (!run) {
    packageContentEl.innerHTML =
      '<p class="panel-status">Select a run to inspect package options.</p>';
    return;
  }

  try {
    const pkg = await apiFetch(
      `/api/runs/${encodeURIComponent(run.run_id)}/package${assetQuery(run)}`
    );
    renderPackageInfo(pkg);
    setPackageButtons(true, true);
  } catch (err) {
    if (err.status === 404) {
      packageContentEl.innerHTML =
        '<p class="panel-status">No package built yet. Use <strong>Build package</strong> to generate <code>stablecoin_map_package.zip</code> from the manifest.</p>';
    } else {
      packageContentEl.innerHTML = `<p class="panel-status error">${escapeHtml(err.message)}</p>`;
    }
    setPackageButtons(true, false);
  }
}

async function selectRun(run) {
  selectedRun = run;
  document.querySelectorAll(".run-item button").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.runId === run.run_id);
  });

  runDetailEl.hidden = false;
  emptyStateEl.hidden = true;

  try {
    const manifest = await apiFetch(
      `/api/runs/${encodeURIComponent(run.run_id)}/manifest${assetQuery(run)}`
    );
    renderOverview(manifest);
    renderClaims(manifest);

    const artResp = await apiFetch(
      `/api/runs/${encodeURIComponent(run.run_id)}/artifacts${assetQuery(run)}`
    );
    renderArtifacts(artResp.artifacts);
  } catch (err) {
    overviewContentEl.innerHTML = `<p class="panel-status error">${escapeHtml(err.message)}</p>`;
  }

  await loadPackagePanel(run);
}

async function loadRuns() {
  runListStatusEl.textContent = "Loading runs…";
  runListEl.innerHTML = "";

  try {
    const data = await apiFetch("/api/runs");
    const runs = data.runs || [];

    if (runs.length === 0) {
      runListStatusEl.textContent =
        "No runs found. Complete a transfer-audit run with artifact_manifest.json under out/.";
      return;
    }

    runListStatusEl.textContent = `${runs.length} run(s)`;

    for (const run of runs) {
      const li = document.createElement("li");
      li.className = "run-item";
      const btn = document.createElement("button");
      btn.type = "button";
      btn.dataset.runId = run.run_id;
      btn.innerHTML = `
        <div class="run-asset">${escapeHtml(run.asset)}</div>
        <div class="run-id">${escapeHtml(run.run_id)}</div>
        <div class="run-meta">${escapeHtml(run.command)} · ${escapeHtml(formatDate(run.generated_at))}</div>
      `;
      btn.addEventListener("click", () => selectRun(run));
      li.appendChild(btn);
      runListEl.appendChild(li);
    }

    await selectRun(runs[0]);
  } catch (err) {
    runListStatusEl.textContent = `Failed to load runs: ${err.message}`;
    runListStatusEl.classList.add("error");
  }
}

btnBuildPackage.addEventListener("click", async () => {
  if (!selectedRun) return;
  packageResultEl.hidden = false;
  packageResultEl.textContent = "Building package…";
  try {
    const pkg = await apiFetch(
      `/api/runs/${encodeURIComponent(selectedRun.run_id)}/package${assetQuery(selectedRun)}`,
      { method: "POST" }
    );
    renderPackageInfo(pkg);
    packageResultEl.textContent = JSON.stringify(pkg, null, 2);
    setPackageButtons(true, true);
  } catch (err) {
    packageResultEl.textContent = `Error: ${err.message}`;
  }
});

btnDownloadPackage.addEventListener("click", () => {
  if (!selectedRun) return;
  const url = `/api/runs/${encodeURIComponent(selectedRun.run_id)}/package/download${assetQuery(selectedRun)}`;
  window.open(url, "_blank");
});

btnVerifyPackage.addEventListener("click", async () => {
  if (!selectedRun) return;
  packageResultEl.hidden = false;
  packageResultEl.textContent = "Verifying package…";
  try {
    const report = await apiFetch(
      `/api/runs/${encodeURIComponent(selectedRun.run_id)}/package/verify${assetQuery(selectedRun)}`,
      { method: "POST" }
    );
    packageResultEl.textContent = JSON.stringify(report, null, 2);
  } catch (err) {
    packageResultEl.textContent = `Error: ${err.message}`;
  }
});

async function checkHealth() {
  try {
    const health = await apiFetch("/health");
    healthStatusEl.textContent = `API ok · toolkit ${health.toolkit_version}`;
  } catch {
    healthStatusEl.textContent = "API unreachable";
  }
}

checkHealth();
loadRuns();
