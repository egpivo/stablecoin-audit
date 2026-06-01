/** @typedef {{ asset: string, run_id: string, command: string, generated_at: string, manifest_path: string }} RunDescriptor */

const runListEl = document.getElementById("run-list");
const runListStatusEl = document.getElementById("run-list-status");
const runDetailEl = document.getElementById("run-detail");
const emptyStateEl = document.getElementById("empty-state");
const auditSummaryCardsEl = document.getElementById("audit-summary-cards");
const evidenceSummaryCardsEl = document.getElementById("evidence-summary-cards");
const claimsCompactEl = document.getElementById("claims-compact");
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
const reqAssetEl = document.getElementById("req-asset");
const reqRunIdEl = document.getElementById("req-run-id");
const reqChainEl = document.getElementById("req-chain");
const reqFromBlockEl = document.getElementById("req-from-block");
const reqToBlockEl = document.getElementById("req-to-block");
const reqFreshEl = document.getElementById("req-fresh");
const reqMessageEl = document.getElementById("request-builder-message");
const reqCommandEl = document.getElementById("request-builder-command");
const btnCopyRequestCommand = document.getElementById("btn-copy-request-command");

/** @type {RunDescriptor | null} */
let selectedRun = null;
/** @type {string} */
let packageStatusLabel = "—";

const API = "";

const SUPPORTED_SUMMARY = [
  { ids: ["transfer_activity_reconstructible"], label: "Transfer reconstruction" },
  { ids: ["supply_snapshot_available"], label: "Supply snapshot availability" },
  {
    ids: ["supply_reconciliation_available"],
    label: "Supply reconciliation for configured window",
  },
  {
    ids: ["cross_chain_per_deployment_comparison"],
    label: "Cross-chain per-deployment comparison",
  },
];

const UNSUPPORTED_SUMMARY = [
  {
    label: "Reserves / redemption",
    ids: [
      "fiat_reserve_not_verified",
      "redemption_capacity",
      "circulating_supply_not_verified",
    ],
  },
  { label: "Peg stability", ids: ["peg_stability"] },
  {
    label: "Bridge backing",
    ids: ["bridge_backing_not_verified_without_bridge_collateral"],
  },
  {
    label: "User geography / identity",
    ids: ["user_geography", "holder_identity"],
  },
  { label: "Actual swap routing", ids: ["actual_swap_routing"] },
  {
    label: "Issuer intent / stress transmission",
    ids: ["issuer_intent", "stress_transmission"],
  },
  {
    label: "Liquidity exposure",
    ids: ["liquidity_exposure_not_measured"],
  },
];

let generatedRequestCommand = "";

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

function shellArg(value) {
  const v = String(value ?? "");
  if (v.length === 0) return "''";
  if (/^[A-Za-z0-9._:/-]+$/.test(v)) return v;
  return `'${v.replace(/'/g, `'\\''`)}'`;
}

function basename(path) {
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

function findArtifact(artifacts, { kind, suffix }) {
  if (!artifacts) return null;
  if (kind) {
    const byKind = artifacts.find((a) => a.kind === kind);
    if (byKind) return byKind;
  }
  if (suffix) {
    return artifacts.find((a) => a.path.endsWith(suffix)) || null;
  }
  return null;
}

function artifactLinks(artifacts, labels) {
  return labels
    .map(({ kind, suffix, label }) => {
      const art = findArtifact(artifacts, { kind, suffix });
      if (!art) return null;
      const href = `/api/artifacts/${encodeURI(art.path)}`;
      return `<a class="evidence-link" href="${href}" target="_blank" rel="noopener">${escapeHtml(label || basename(art.path))}</a>`;
    })
    .filter(Boolean)
    .join(" · ");
}

function rowCountLabel(art) {
  if (!art) return "—";
  if (art.row_count != null) return `${art.row_count.toLocaleString()} rows`;
  return "Listed (row count not recorded)";
}

async function fetchQaReport(artifacts) {
  const qaArt = findArtifact(artifacts, { kind: "qa_report", suffix: "qa_report.json" });
  if (!qaArt) return null;
  try {
    return await apiFetch(`/api/artifacts/${encodeURI(qaArt.path)}`);
  } catch {
    return null;
  }
}

function deriveAuditStatus(qaReport, manifest) {
  if (!qaReport?.chains?.length) {
    return manifest?.warnings?.length
      ? { label: "Complete with warnings", tone: "warn" }
      : { label: "Complete", tone: "ok" };
  }

  const allGateValues = qaReport.chains.flatMap((c) =>
    Object.values(c.gates || {}).map((g) => String(g || "").toUpperCase())
  );
  const failChains = qaReport.chains.filter((c) =>
    Object.values(c.gates || {}).some((g) => String(g || "").toUpperCase() === "FAIL")
  );
  if (failChains.length > 0) {
    return {
      label: `Gate FAIL on ${failChains.map((c) => c.chain).join(", ")}`,
      tone: "fail",
    };
  }

  const hasUnavailable = allGateValues.some((g) => g === "UNAVAILABLE");
  if (hasUnavailable) {
    return { label: "Complete — some gates UNAVAILABLE", tone: "warn" };
  }

  return { label: "Complete — gates PASS", tone: "ok" };
}

function deriveSupplyReconciliation(qaReport) {
  if (!qaReport?.chains?.length) return { label: "See QA report", tone: "neutral" };

  const statuses = qaReport.chains.map((c) =>
    String(c.gates?.supply_invariant_pass || "UNAVAILABLE").toUpperCase()
  );
  const failChains = qaReport.chains.filter(
    (c) => String(c.gates?.supply_invariant_pass || "UNAVAILABLE").toUpperCase() === "FAIL"
  );
  if (failChains.length > 0) {
    return {
      label: `FAIL on ${failChains.map((c) => c.chain).join(", ")}`,
      tone: "fail",
    };
  }

  const passCount = statuses.filter((s) => s === "PASS").length;
  const unavailableCount = statuses.filter((s) => s === "UNAVAILABLE").length;

  if (passCount === statuses.length) {
    return {
      label: `PASS (${qaReport.chains.length} chain${qaReport.chains.length === 1 ? "" : "s"})`,
      tone: "ok",
    };
  }

  if (unavailableCount === statuses.length) {
    return {
      label: `UNAVAILABLE (${qaReport.chains.length} chain${qaReport.chains.length === 1 ? "" : "s"})`,
      tone: "warn",
    };
  }

  return {
    label: `PARTIAL (${passCount} PASS, ${unavailableCount} UNAVAILABLE)`,
    tone: "warn",
  };
}

function validateRequestBuilderInput() {
  const asset = (reqAssetEl.value || "").trim();
  const runId = (reqRunIdEl.value || "").trim();
  const chain = (reqChainEl.value || "").trim().toLowerCase();
  const fromBlockRaw = (reqFromBlockEl.value || "").trim();
  const toBlockRaw = (reqToBlockEl.value || "").trim();
  const identifierPattern = /^[A-Za-z0-9_-]+$/;

  if (!asset) return { error: "asset is required." };
  if (!runId) return { error: "run_id is required." };
  if (!chain) return { error: "chain is required." };
  if (!identifierPattern.test(asset)) {
    return { error: "asset must match [A-Za-z0-9_-]+" };
  }
  if (!identifierPattern.test(runId)) {
    return { error: "run_id must match [A-Za-z0-9_-]+" };
  }
  if (!identifierPattern.test(chain)) {
    return { error: "chain must match [A-Za-z0-9_-]+" };
  }
  if (!fromBlockRaw || !toBlockRaw) return { error: "from_block and to_block are required." };

  const fromBlock = Number(fromBlockRaw);
  const toBlock = Number(toBlockRaw);
  if (!Number.isInteger(fromBlock) || fromBlock < 0) {
    return { error: "from_block must be a non-negative integer." };
  }
  if (fromBlock === 0) {
    return { error: "from_block 0 is not supported" };
  }
  if (!Number.isInteger(toBlock) || toBlock < 0) {
    return { error: "to_block must be a non-negative integer." };
  }
  if (toBlock < fromBlock) {
    return { error: "to_block must be greater than or equal to from_block." };
  }

  return {
    asset,
    runId,
    chain,
    fromBlock,
    toBlock,
    fresh: !!reqFreshEl.checked,
  };
}

function buildRequestCommand() {
  const parsed = validateRequestBuilderInput();
  if (parsed.error) {
    generatedRequestCommand = "";
    reqMessageEl.textContent = parsed.error;
    reqMessageEl.classList.add("request-builder-error");
    reqCommandEl.textContent =
      "cargo run -- transfer-audit --asset <ASSET> --run-id <RUN_ID> \\\n  --window <chain>:<from_block>:<to_block>";
    btnCopyRequestCommand.disabled = true;
    return;
  }

  const windowArg = `${parsed.chain}:${parsed.fromBlock}:${parsed.toBlock}`;
  let command =
    `cargo run -- transfer-audit --asset ${shellArg(parsed.asset)} --run-id ${shellArg(parsed.runId)} \\\n` +
    `  --window ${shellArg(windowArg)}`;
  if (parsed.fresh) command += " \\\n  --fresh";

  generatedRequestCommand = command;
  reqCommandEl.textContent = command;
  reqMessageEl.textContent =
    "Copy and run this command in your terminal. Then refresh the run list to inspect generated evidence.";
  reqMessageEl.classList.remove("request-builder-error");
  btnCopyRequestCommand.disabled = false;
}

async function copyRequestCommand() {
  if (!generatedRequestCommand) return;
  try {
    await navigator.clipboard.writeText(generatedRequestCommand);
    reqMessageEl.textContent = "Command copied.";
    reqMessageEl.classList.remove("request-builder-error");
  } catch {
    const temp = document.createElement("textarea");
    temp.value = generatedRequestCommand;
    document.body.appendChild(temp);
    temp.select();
    document.execCommand("copy");
    document.body.removeChild(temp);
    reqMessageEl.textContent = "Command copied.";
    reqMessageEl.classList.remove("request-builder-error");
  }
}

function setRequestBuilderFromRun(manifest, qaReport) {
  if (!manifest) return;
  reqAssetEl.value = (manifest.asset || reqAssetEl.value || "USDC").toUpperCase();
  reqRunIdEl.value = manifest.run_id || reqRunIdEl.value || "";

  const firstChain = qaReport?.chains?.[0];
  if (firstChain) {
    reqChainEl.value = firstChain.chain || reqChainEl.value || "ethereum";
    if (firstChain.from_block != null) reqFromBlockEl.value = firstChain.from_block;
    if (firstChain.resolved_to_block != null) reqToBlockEl.value = firstChain.resolved_to_block;
  }
  buildRequestCommand();
}

function formatChainsWindows(manifest, qaReport) {
  if (qaReport?.chains?.length) {
    return qaReport.chains
      .map((c) => {
        const blocks = `${c.from_block} → ${c.resolved_to_block ?? "?"}`;
        return `${c.chain} (${blocks})`;
      })
      .join("; ");
  }
  const snaps = manifest.source_snapshots || [];
  if (snaps.length) {
    return snaps
      .map((s) => {
        const w =
          s.window_start || s.window_end
            ? ` ${s.window_start || "?"} → ${s.window_end || "?"}`
            : "";
        return `${s.source_name}${w}`;
      })
      .join("; ");
  }
  return "—";
}

function summaryCard(label, value, tone = "neutral") {
  return `
    <div class="summary-card tone-${tone}">
      <div class="summary-card-label">${escapeHtml(label)}</div>
      <div class="summary-card-value">${value}</div>
    </div>
  `;
}

function evidenceCard(title, bodyHtml, tone = "neutral") {
  return `
    <article class="evidence-card tone-${tone}">
      <h3>${escapeHtml(title)}</h3>
      ${bodyHtml}
    </article>
  `;
}

function renderAuditSummary(manifest, artifacts, qaReport) {
  const status = deriveAuditStatus(qaReport, manifest);
  const supply = deriveSupplyReconciliation(qaReport);
  const transfersArt =
    findArtifact(artifacts, { kind: "canonical_transfers", suffix: "canonical_transfers.csv" }) ||
    findArtifact(artifacts, { kind: "transfer_log", suffix: "decoded_transfers.csv" });
  const snapshotsArt = findArtifact(artifacts, {
    kind: "supply_snapshots",
    suffix: "supply_snapshots.csv",
  });

  auditSummaryCardsEl.innerHTML = [
    summaryCard("Audit status", escapeHtml(status.label), status.tone),
    summaryCard(
      "Asset / run",
      `${escapeHtml(manifest.asset || "—")} · <code>${escapeHtml(manifest.run_id || "—")}</code>`,
      "neutral"
    ),
    summaryCard(
      "Chains / windows",
      escapeHtml(formatChainsWindows(manifest, qaReport)),
      "neutral"
    ),
    summaryCard("Transfer rows reconstructed", escapeHtml(rowCountLabel(transfersArt)), "neutral"),
    summaryCard("Supply snapshots captured", escapeHtml(rowCountLabel(snapshotsArt)), "neutral"),
    summaryCard("Supply reconciliation", escapeHtml(supply.label), supply.tone),
    summaryCard("Evidence package", escapeHtml(packageStatusLabel), packageStatusLabel === "Built" ? "ok" : "neutral"),
  ].join("");
}

function renderEvidenceSummary(manifest, artifacts, qaReport) {
  const transfersArt =
    findArtifact(artifacts, { kind: "canonical_transfers", suffix: "canonical_transfers.csv" }) ||
    findArtifact(artifacts, { kind: "transfer_log", suffix: "decoded_transfers.csv" });
  const snapshotsArt = findArtifact(artifacts, {
    kind: "supply_snapshots",
    suffix: "supply_snapshots.csv",
  });
  const supplyArt = findArtifact(artifacts, { kind: "supply_audit", suffix: "supply_audit.csv" });
  const qaArt = findArtifact(artifacts, { kind: "qa_report", suffix: "qa_report.json" });
  const supply = deriveSupplyReconciliation(qaReport);

  const transferLinks = artifactLinks(artifacts, [
    { suffix: "canonical_transfers.csv", label: "canonical_transfers.csv" },
    { suffix: "decoded_transfers.csv", label: "decoded_transfers.csv" },
  ]);

  const snapshotLinks = artifactLinks(artifacts, [
    { suffix: "supply_snapshots.csv", label: "supply_snapshots.csv" },
  ]);

  const reconciliationLinks = artifactLinks(artifacts, [
    { suffix: "supply_audit.csv", label: "supply_audit.csv" },
    { suffix: "supply_audit.md", label: "supply_audit.md" },
    { suffix: "qa_report.json", label: "qa_report.json" },
  ]);

  const scopeLinks = artifactLinks(artifacts, [
    { suffix: "provenance.json", label: "provenance.json" },
    { suffix: "chain_windows.json", label: "chain_windows.json" },
    { suffix: "deployment_registry.json", label: "deployment_registry.json" },
    { suffix: "audit_plan.json", label: "audit_plan.json" },
  ]);

  evidenceSummaryCardsEl.innerHTML = [
    evidenceCard(
      "Transfer reconstruction",
      `<p class="evidence-stat">${escapeHtml(rowCountLabel(transfersArt))}</p>
       <p class="evidence-links">${transferLinks || '<span class="muted">No transfer artifacts listed</span>'}</p>`,
      transfersArt ? "ok" : "neutral"
    ),
    evidenceCard(
      "Supply snapshots",
      `<p class="evidence-stat">${escapeHtml(rowCountLabel(snapshotsArt))}</p>
       <p class="evidence-links">${snapshotLinks || '<span class="muted">No snapshot artifacts listed</span>'}</p>`,
      snapshotsArt ? "ok" : "neutral"
    ),
    evidenceCard(
      "Supply reconciliation",
      `<p class="evidence-stat">${escapeHtml(supply.label)}</p>
       <p class="evidence-links">${reconciliationLinks || '<span class="muted">No reconciliation artifacts listed</span>'}</p>`,
      supply.tone
    ),
    evidenceCard(
      "Provenance / scope",
      `<p class="evidence-stat">${escapeHtml(formatChainsWindows(manifest, qaReport))}</p>
       <p class="evidence-links">${scopeLinks || '<span class="muted">No scope artifacts listed</span>'}</p>`,
      "neutral"
    ),
  ].join("");
}

function claimIds(claims) {
  return new Set((claims || []).map((c) => c.claim));
}

function renderClaimsCompact(manifest) {
  const supportedIds = claimIds(manifest.supported_claims);
  const unsupportedIds = claimIds(manifest.unsupported_claims);

  const supportedLines = SUPPORTED_SUMMARY.filter((g) =>
    g.ids.some((id) => supportedIds.has(id))
  ).map((g) => `<li>${escapeHtml(g.label)}</li>`);

  const unsupportedLines = UNSUPPORTED_SUMMARY.filter((g) =>
    g.ids.some((id) => unsupportedIds.has(id))
  ).map((g) => `<li>${escapeHtml(g.label)}</li>`);

  claimsCompactEl.innerHTML = `
    <div class="claims-compact-grid">
      <div class="claims-compact-block supported">
        <h3>Supported</h3>
        <ul>${supportedLines.length ? supportedLines.join("") : "<li class='muted'>None listed in manifest</li>"}</ul>
      </div>
      <div class="claims-compact-block unsupported">
        <h3>Unsupported</h3>
        <ul>${unsupportedLines.length ? unsupportedLines.join("") : "<li class='muted'>None listed in manifest</li>"}</ul>
      </div>
    </div>
  `;
}

function renderClaimCard(claim) {
  const note =
    claim.statement ||
    claim.caveat ||
    (claim.limitations?.length ? claim.limitations.join(" ") : "");
  const evidence =
    claim.evidence_artifacts?.length > 0
      ? `Evidence: ${claim.evidence_artifacts.join(", ")}`
      : "";
  const warnings =
    claim.warnings?.length > 0 ? `Warnings: ${claim.warnings.join("; ")}` : "";

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

function renderClaimsFull(manifest) {
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
  if (!artifacts?.length) {
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
        <dt>Status</dt><dd>Built</dd>
        <dt>Kind</dt><dd>${escapeHtml(pkg.package_kind || "—")}</dd>
        <dt>Generated</dt><dd>${escapeHtml(formatDate(pkg.generated_at))}</dd>
        <dt>Artifacts</dt><dd>${pkg.artifacts?.length ?? 0}</dd>
        <dt>Package checksum</dt><dd><code>${escapeHtml((pkg.package_checksum_sha256 || "").slice(0, 16))}…</code></dd>
      </dl>
    </div>
  `;
}

function setPackageJson(text) {
  packageResultEl.textContent = text;
}

function setPackageButtons(hasRun, hasPackage = false) {
  btnBuildPackage.disabled = !hasRun;
  btnDownloadPackage.disabled = !hasPackage;
  btnVerifyPackage.disabled = !hasPackage;
}

async function loadPackagePanel(run, manifest, artifacts, qaReport) {
  setPackageButtons(!!run, false);
  packageStatusLabel = run ? "Not built" : "—";

  if (!run) {
    packageContentEl.innerHTML =
      '<p class="panel-status">Select a run to inspect package options.</p>';
    setPackageJson("No package JSON loaded yet.");
    return;
  }

  try {
    const pkg = await apiFetch(
      `/api/runs/${encodeURIComponent(run.run_id)}/package${assetQuery(run)}`
    );
    renderPackageInfo(pkg);
    setPackageJson(JSON.stringify(pkg, null, 2));
    packageStatusLabel = "Built";
    setPackageButtons(true, true);
  } catch (err) {
    setPackageJson("No package JSON loaded yet.");
    if (err.status === 404) {
      packageContentEl.innerHTML =
        '<p class="panel-status">No package built yet. Use <strong>Build package</strong> to generate <code>stablecoin_map_package.zip</code> from the manifest.</p>';
      packageStatusLabel = "Not built";
    } else {
      packageContentEl.innerHTML = `<p class="panel-status error">${escapeHtml(err.message)}</p>`;
      packageStatusLabel = "Error";
    }
    setPackageButtons(true, false);
  }

  renderAuditSummary(manifest, artifacts, qaReport);
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
    const artResp = await apiFetch(
      `/api/runs/${encodeURIComponent(run.run_id)}/artifacts${assetQuery(run)}`
    );
    const artifacts = artResp.artifacts || [];
    const qaReport = await fetchQaReport(artifacts);

    renderAuditSummary(manifest, artifacts, qaReport);
    renderEvidenceSummary(manifest, artifacts, qaReport);
    renderClaimsCompact(manifest);
    renderClaimsFull(manifest);
    renderArtifacts(artifacts);
    setRequestBuilderFromRun(manifest, qaReport);

    await loadPackagePanel(run, manifest, artifacts, qaReport);
  } catch (err) {
    auditSummaryCardsEl.innerHTML = `<p class="panel-status error">${escapeHtml(err.message)}</p>`;
    evidenceSummaryCardsEl.innerHTML = "";
  }
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
  setPackageJson("Building package…");
  document.getElementById("package-json-details").open = true;
  try {
    const pkg = await apiFetch(
      `/api/runs/${encodeURIComponent(selectedRun.run_id)}/package${assetQuery(selectedRun)}`,
      { method: "POST" }
    );
    renderPackageInfo(pkg);
    setPackageJson(JSON.stringify(pkg, null, 2));
    packageStatusLabel = "Built";
    setPackageButtons(true, true);
    const manifest = await apiFetch(
      `/api/runs/${encodeURIComponent(selectedRun.run_id)}/manifest${assetQuery(selectedRun)}`
    );
    const artResp = await apiFetch(
      `/api/runs/${encodeURIComponent(selectedRun.run_id)}/artifacts${assetQuery(selectedRun)}`
    );
    const qaReport = await fetchQaReport(artResp.artifacts);
    renderAuditSummary(manifest, artResp.artifacts, qaReport);
  } catch (err) {
    setPackageJson(`Error: ${err.message}`);
  }
});

btnDownloadPackage.addEventListener("click", () => {
  if (!selectedRun) return;
  const url = `/api/runs/${encodeURIComponent(selectedRun.run_id)}/package/download${assetQuery(selectedRun)}`;
  window.open(url, "_blank");
});

btnVerifyPackage.addEventListener("click", async () => {
  if (!selectedRun) return;
  setPackageJson("Verifying package…");
  document.getElementById("package-json-details").open = true;
  try {
    const report = await apiFetch(
      `/api/runs/${encodeURIComponent(selectedRun.run_id)}/package/verify${assetQuery(selectedRun)}`,
      { method: "POST" }
    );
    setPackageJson(JSON.stringify(report, null, 2));
  } catch (err) {
    setPackageJson(`Error: ${err.message}`);
  }
});

[reqAssetEl, reqRunIdEl, reqChainEl, reqFromBlockEl, reqToBlockEl, reqFreshEl].forEach((el) => {
  el.addEventListener("input", buildRequestCommand);
  el.addEventListener("change", buildRequestCommand);
});
btnCopyRequestCommand.addEventListener("click", copyRequestCommand);

async function checkHealth() {
  try {
    const health = await apiFetch("/health");
    healthStatusEl.textContent = `API ok · toolkit ${health.toolkit_version}`;
  } catch {
    healthStatusEl.textContent = "API unreachable";
  }
}

checkHealth();
reqAssetEl.value = "USDC";
reqChainEl.value = "ethereum";
buildRequestCommand();
loadRuns();
