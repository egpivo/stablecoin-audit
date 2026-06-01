/** @typedef {{ asset: string, run_id: string, command: string, generated_at: string, manifest_path: string, _statusInfo?: { label: string, tone: string, chains: string } }} RunDescriptor */

const runListEl = document.getElementById("run-list");
const runListStatusEl = document.getElementById("run-list-status");
const sidebarEmptyEl = document.getElementById("sidebar-empty");
const runDetailEl = document.getElementById("run-detail");
const emptyStateEl = document.getElementById("empty-state");
const runHeaderEl = document.getElementById("run-header");
const metricCardsEl = document.getElementById("metric-cards");
const overviewSummaryCardsEl = document.getElementById("overview-summary-cards");
const overviewLogPreviewEl = document.getElementById("overview-log-preview");
const overviewPackageStripEl = document.getElementById("overview-package-strip");
const claimsCompactEl = document.getElementById("claims-compact");
const claimsTabBodyEl = document.getElementById("claims-tab-body");
const supportedClaimsEl = document.getElementById("supported-claims");
const unsupportedClaimsEl = document.getElementById("unsupported-claims");
const evidenceExecutionLogEl = document.getElementById("evidence-execution-log");
const localRunBannerEl = document.getElementById("local-run-status");
const artifactsBodyEl = document.getElementById("artifacts-body");
const artifactsStatusEl = document.getElementById("artifacts-status");
const packageContentEl = document.getElementById("package-content");
const bundleVerifyAlertSlotEl = document.getElementById("bundle-verify-alert-slot");
const packageResultEl = document.getElementById("package-result");
const btnBuildPackage = document.getElementById("btn-build-package");
const btnDownloadPackage = document.getElementById("btn-download-package");
const btnVerifyPackage = document.getElementById("btn-verify-package");
const healthStatusEl = document.getElementById("health-status");
const runFilterEl = document.getElementById("run-filter");
const auditModalEl = document.getElementById("audit-modal");
const reqAssetEl = document.getElementById("req-asset");
const reqRunIdEl = document.getElementById("req-run-id");
const reqChainEl = document.getElementById("req-chain");
const reqFromBlockEl = document.getElementById("req-from-block");
const reqToBlockEl = document.getElementById("req-to-block");
const reqFreshEl = document.getElementById("req-fresh");
const reqMessageEl = document.getElementById("request-builder-message");
const reqCommandEl = document.getElementById("request-builder-command");
const btnCopyRequestCommand = document.getElementById("btn-copy-request-command");
const btnRunLocalAudit = document.getElementById("btn-run-local-audit");
const runExecutionPanel = document.getElementById("run-execution-panel");
const runExecutionLogEl = document.getElementById("run-execution-log");
const runProgressPanelEl = document.getElementById("run-progress-panel");
const runProgressStepsEl = document.getElementById("run-progress-steps");
const runProgressCurrentStageEl = document.getElementById("run-progress-current-stage");
const runProgressBarEl = document.getElementById("run-progress-bar");
const runProgressStatusLabelEl = document.getElementById("run-progress-status-label");
const runProgressElapsedEl = document.getElementById("run-progress-elapsed");
const btnModalCancel = document.getElementById("btn-modal-cancel");
const emptyCliExampleEl = document.getElementById("empty-cli-example");
const viewLandingEl = document.getElementById("view-landing");
const viewConsoleEl = document.getElementById("view-console");
const landingHealthEl = document.getElementById("landing-health-status");
const cleanModalEl = document.getElementById("clean-modal");
const cleanModalStatusEl = document.getElementById("clean-modal-status");

const EXAMPLE_CLI = `cargo run -- transfer-audit --asset USDC --run-id demo_001 \\
  --window ethereum:24000000:24001000`;

/** Matches configs/tokens/*.yml deployments */
const ASSET_CHAINS = {
  USDC: ["ethereum", "base", "arbitrum"],
  EURC: ["ethereum", "base"],
  XSGD: ["base", "polygon"],
};

const RUN_STAGES = [
  { id: "prepare", label: "Preparing request" },
  { id: "fetch", label: "Fetching Transfer logs" },
  { id: "decode", label: "Decoding / dedup" },
  { id: "supply", label: "Checking supply invariant" },
  { id: "write", label: "Writing artifacts" },
  { id: "manifest", label: "Writing artifact_manifest.json" },
  { id: "done", label: "Completed" },
];

let runStartTime = 0;
/** @type {ReturnType<typeof setInterval> | null} */
let elapsedTimer = null;

/** @type {RunDescriptor | null} */
let selectedRun = null;
let packageStatusLabel = "—";
let packageVerifiedLabel = "";
/** @type {object | null} */
let currentBundleManifest = null;
const bundleActionHintEl = document.getElementById("bundle-action-hint");
let artifactFilter = "all";
let activeTab = "overview";
/** @type {object | null} */
let currentManifest = null;
/** @type {object[]} */
let currentArtifacts = [];
/** @type {object | null} */
let currentQaReport = null;
/** @type {object[]} */
let executionLogEntries = [];

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
    category: "Off-chain claims",
    label: "Reserves / redemption",
    ids: ["fiat_reserve_not_verified", "redemption_capacity"],
  },
  { category: "Off-chain claims", label: "Issuer intent", ids: ["issuer_intent"] },
  { category: "Market claims", label: "Peg stability", ids: ["peg_stability"] },
  {
    category: "Market claims",
    label: "Liquidity exposure",
    ids: ["liquidity_exposure_not_measured"],
  },
  { category: "Market claims", label: "Stress transmission", ids: ["stress_transmission"] },
  {
    category: "Identity / geography claims",
    label: "User geography / holder identity",
    ids: ["user_geography", "holder_identity"],
  },
  {
    category: "Bridge / routing claims",
    label: "Bridge backing",
    ids: ["bridge_backing_not_verified_without_bridge_collateral"],
  },
  {
    category: "Bridge / routing claims",
    label: "Circulating supply not verified",
    ids: ["circulating_supply_not_verified"],
  },
  {
    category: "Bridge / routing claims",
    label: "Actual swap routing",
    ids: ["actual_swap_routing"],
  },
];

let generatedRequestCommand = "";
/** @type {ReturnType<typeof setInterval> | null} */
let pollTimer = null;
/** @type {RunDescriptor[]} */
let allRuns = [];
let consoleBootstrapped = false;

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

function shortChecksum(hex) {
  if (!hex) return "—";
  if (hex.length <= 16) return hex;
  return `${hex.slice(0, 8)}…${hex.slice(-6)}`;
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
      ? { label: "WARN", tone: "warn" }
      : { label: "PASS", tone: "ok" };
  }

  const failChains = qaReport.chains.filter((c) =>
    Object.values(c.gates || {}).some((g) => String(g || "").toUpperCase() === "FAIL")
  );
  if (failChains.length > 0) {
    return { label: "FAIL", tone: "fail" };
  }

  const allGateValues = qaReport.chains.flatMap((c) =>
    Object.values(c.gates || {}).map((g) => String(g || "").toUpperCase())
  );
  if (allGateValues.some((g) => g === "UNAVAILABLE")) {
    return { label: "WARN", tone: "warn" };
  }

  return { label: "PASS", tone: "ok" };
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
    return { label: "FAIL", tone: "fail" };
  }

  const passCount = statuses.filter((s) => s === "PASS").length;
  const unavailableCount = statuses.filter((s) => s === "UNAVAILABLE").length;

  if (passCount === statuses.length) {
    return { label: "PASS", tone: "ok" };
  }
  if (unavailableCount === statuses.length) {
    return { label: "UNAVAILABLE", tone: "warn" };
  }
  return { label: "PARTIAL", tone: "warn" };
}

function formatChainsWindows(manifest, qaReport) {
  if (qaReport?.chains?.length) {
    return qaReport.chains
      .map((c) => {
        const blocks = `${c.from_block} → ${c.resolved_to_block ?? "?"}`;
        return `${c.chain} blocks ${blocks}`;
      })
      .join("; ");
  }
  const snaps = manifest?.source_snapshots || [];
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

function chainSummaryForRun(manifest, qaReport) {
  if (qaReport?.chains?.length) {
    const names = qaReport.chains.map((c) => c.chain).filter(Boolean);
    if (names.length === 1) return names[0];
    return `${names.length} chains`;
  }
  const snaps = manifest?.source_snapshots || [];
  if (snaps.length === 1) return snaps[0].source_name || "—";
  if (snaps.length > 1) return `${snaps.length} chains`;
  return "—";
}

function rowCountLabel(art) {
  if (!art) return "—";
  if (art.row_count != null) return art.row_count.toLocaleString();
  return "Listed";
}

function highlightLogMessage(message) {
  let escaped = escapeHtml(String(message ?? ""));
  escaped = escaped.replace(
    /\[([A-Za-z0-9_ -]+)\]/g,
    '<span class="log-chain">[$1]</span>'
  );
  escaped = escaped.replace(
    /\b(ethereum|arbitrum|base|polygon)\b/gi,
    '<span class="log-chain">$1</span>'
  );
  return escaped
    .replace(/\bPASS\b/g, '<span class="hl-pass">PASS</span>')
    .replace(/\bWARN(?:ING)?\b/g, '<span class="hl-warn">WARN</span>')
    .replace(/\bFAIL(?:ED)?\b/g, '<span class="hl-fail">FAIL</span>');
}

function formatLogEntryHtml(entry) {
  const level = String(entry.level || "info").toLowerCase();
  const levelClass = level.includes("error")
    ? "log-level-error"
    : level.includes("warn")
      ? "log-level-warn"
      : "log-level-info";
  const ts = entry.timestamp
    ? `<span class="log-ts">${escapeHtml(entry.timestamp)}</span> `
    : "";
  return `<span class="log-line">${ts}<span class="log-level ${levelClass}">${escapeHtml(level)}</span>: ${highlightLogMessage(entry.message)}</span>`;
}

function renderLogEntries(targetEl, entries, fallbackText) {
  if (!entries?.length) {
    targetEl.innerHTML = `<span class="log-line muted">${escapeHtml(fallbackText || "(no log entries)")}</span>`;
    return;
  }
  targetEl.innerHTML = entries.map(formatLogEntryHtml).join("");
}

function renderExecutionLog(status, logs) {
  if (status?.error && !(logs?.entries?.length)) {
    runExecutionLogEl.textContent = status.error;
    return;
  }
  renderLogEntries(runExecutionLogEl, logs?.entries || [], "(no log entries yet)");
}

async function loadExecutionLogEntries(artifacts) {
  const logArt = findArtifact(artifacts, { suffix: "execution_log.ndjson" });
  if (!logArt) {
    executionLogEntries = [];
    return;
  }
  try {
    const res = await fetch(`${API}/api/artifacts/${encodeURI(logArt.path)}`);
    if (!res.ok) throw new Error(res.statusText);
    const text = await res.text();
    executionLogEntries = [];
    for (const line of text.split("\n")) {
      if (!line.trim()) continue;
      try {
        executionLogEntries.push(JSON.parse(line));
      } catch {
        executionLogEntries.push({ level: "raw", message: line, timestamp: "" });
      }
    }
  } catch {
    executionLogEntries = [];
  }
}

function refreshLogPanels() {
  renderLogEntries(
    evidenceExecutionLogEl,
    executionLogEntries,
    "No execution_log.ndjson listed in artifact_manifest.json for this run."
  );
  const tail = executionLogEntries.slice(-8);
  renderLogEntries(
    overviewLogPreviewEl,
    tail,
    "No execution trace yet."
  );
}

function initAssetChainSelects() {
  reqAssetEl.innerHTML = "";
  for (const asset of Object.keys(ASSET_CHAINS)) {
    const opt = document.createElement("option");
    opt.value = asset;
    opt.textContent = asset;
    reqAssetEl.appendChild(opt);
  }
  reqAssetEl.value = "USDC";
  populateChainOptions("USDC");
}

function populateChainOptions(asset) {
  const upper = String(asset || "USDC").toUpperCase();
  const chains = ASSET_CHAINS[upper] || [];
  const prev = reqChainEl.value;
  reqChainEl.innerHTML = "";
  for (const chain of chains) {
    const opt = document.createElement("option");
    opt.value = chain;
    opt.textContent = chain;
    reqChainEl.appendChild(opt);
  }
  if (chains.includes(prev)) reqChainEl.value = prev;
  else if (chains.length) reqChainEl.value = chains[0];
}

const EVENT_TO_STAGE_IDX = {
  audit_start: 0,
  chain_fetch_start: 1,
  fetch_chunk_complete: 1,
  decode_complete: 2,
  dedup_complete: 2,
  qa_gate_result: 3,
  artifacts_written: 4,
  manifest_written: 5,
  audit_complete: 6,
};

const STAGE_FIELD_TO_IDX = {
  preparing: 0,
  fetching_logs: 1,
  decoding_dedup: 2,
  checking_supply: 3,
  writing_artifacts: 4,
  writing_manifest: 5,
  complete: 6,
  failed: -1,
};

/** @param {object} entry */
function resolveStageIndexFromEntry(entry) {
  if (entry.event && EVENT_TO_STAGE_IDX[entry.event] !== undefined) {
    return EVENT_TO_STAGE_IDX[entry.event];
  }
  if (entry.stage && STAGE_FIELD_TO_IDX[entry.stage] !== undefined) {
    const idx = STAGE_FIELD_TO_IDX[entry.stage];
    return idx >= 0 ? idx : null;
  }
  return resolveStageIndexFromMessage(entry.message);
}

/** @param {string | undefined} message */
function resolveStageIndexFromMessage(message) {
  const t = String(message || "").toLowerCase();
  if (!t) return null;
  if (/artifact_manifest\.json written|manifest_written/.test(t)) return 5;
  if (/writing run artifacts|artifacts_written|writing artifacts/.test(t)) return 4;
  if (/supply_invariant|qa_gate_result/.test(t)) return 3;
  if (/unique.*dup|dedup_complete|decoded rows before dedup|decode_complete/.test(t)) {
    return 2;
  }
  if (
    /fetch chunk|chain_fetch_start|fetching transfer logs|resuming transfer log fetch/.test(t)
  ) {
    return 1;
  }
  if (/audit_start|starting transfer-audit/.test(t)) return 0;
  if (/audit_complete|completed successfully/.test(t)) return 6;
  return null;
}

/** @param {object[]} entries @param {string} apiStatus */
function buildProgressModel(entries, apiStatus) {
  const list = entries || [];
  const hasStructured = list.some((e) => e.event);
  let maxIdx = -1;
  for (const entry of list) {
    const idx = resolveStageIndexFromEntry(entry);
    if (idx !== null && idx >= 0) maxIdx = Math.max(maxIdx, idx);
  }

  const failed = apiStatus === "failed";
  const succeeded = apiStatus === "succeeded";
  const running = apiStatus === "running" || apiStatus === "queued";
  const useGranular = hasStructured;

  if (!useGranular) {
    const lastMsg =
      list.length > 0 ? String(list[list.length - 1].message || "") : "";
    const lastError = [...list]
      .reverse()
      .find((e) => e.level === "error" || e.level === "fail");
    return {
      mode: "coarse",
      failed,
      succeeded,
      running,
      statusLabel: failed
        ? "Failed"
        : succeeded
          ? "Completed"
          : apiStatus === "queued"
            ? "Queued"
            : apiStatus === "running"
              ? "Running"
              : "Idle",
      currentLabel: failed
        ? lastError
          ? String(lastError.message || "Audit failed")
          : "Audit failed"
        : succeeded
          ? "Completed"
          : running
            ? lastMsg || "Running — see execution log below"
            : "",
    };
  }

  let activeIdx = maxIdx >= 0 ? maxIdx : 0;
  if (succeeded) activeIdx = RUN_STAGES.length - 1;
  if (failed && maxIdx < 0) activeIdx = 0;

  const currentLabel = RUN_STAGES[Math.min(activeIdx, RUN_STAGES.length - 1)].label;
  const lastError = failed
    ? [...list].reverse().find((e) => e.level === "error" || e.level === "fail")
    : null;

  return {
    mode: "granular",
    failed,
    succeeded,
    running,
    activeIdx,
    maxIdx,
    statusLabel: failed
      ? "Failed"
      : succeeded
        ? "Completed"
        : apiStatus === "queued"
          ? "Queued"
          : apiStatus === "running"
            ? "Running"
            : "Idle",
    currentLabel,
    lastErrorMessage: lastError ? String(lastError.message || "") : "",
  };
}

function renderRunProgress(apiStatus, entries) {
  if (!runProgressPanelEl) return;
  runProgressPanelEl.hidden = false;
  const model = buildProgressModel(entries, apiStatus);

  runProgressStatusLabelEl.textContent = model.statusLabel;

  if (runProgressBarEl) {
    const indeterminate =
      model.mode === "coarse"
        ? model.running && !model.failed && !model.succeeded
        : model.running && !model.failed && !model.succeeded;
    runProgressBarEl.classList.toggle("indeterminate", indeterminate);
    runProgressBarEl.style.width = model.succeeded ? "100%" : "";
  }

  if (model.mode === "coarse") {
    if (runProgressStepsEl) {
      runProgressStepsEl.hidden = true;
      runProgressStepsEl.innerHTML = "";
    }
    if (runProgressCurrentStageEl) {
      runProgressCurrentStageEl.hidden = false;
      runProgressCurrentStageEl.textContent = model.currentLabel;
      runProgressCurrentStageEl.classList.toggle("is-error", model.failed);
    }
    return;
  }

  if (runProgressCurrentStageEl) {
    runProgressCurrentStageEl.hidden = false;
    let stageText = `Current: ${model.currentLabel}`;
    if (model.failed && model.lastErrorMessage) {
      stageText += ` — ${model.lastErrorMessage}`;
    }
    runProgressCurrentStageEl.textContent = stageText;
    runProgressCurrentStageEl.classList.toggle("is-error", model.failed);
  }
  if (!runProgressStepsEl) return;
  runProgressStepsEl.hidden = false;

  const { activeIdx, failed, succeeded, running } = model;
  runProgressStepsEl.innerHTML = RUN_STAGES.map((stage, i) => {
    let state = "";
    if (failed && i === activeIdx) state = "failed";
    else if (succeeded) state = "done";
    else if (i < activeIdx) state = "done";
    else if (i === activeIdx && (running || succeeded)) state = "active";
    return `<li class="${state}"><span class="step-dot" aria-hidden="true"></span>${escapeHtml(stage.label)}</li>`;
  }).join("");
}

function startElapsedTimer() {
  runStartTime = Date.now();
  if (elapsedTimer) clearInterval(elapsedTimer);
  const tick = () => {
    const sec = Math.floor((Date.now() - runStartTime) / 1000);
    const m = Math.floor(sec / 60);
    const s = sec % 60;
    if (runProgressElapsedEl) {
      runProgressElapsedEl.textContent = `${m}:${String(s).padStart(2, "0")} elapsed`;
    }
  };
  tick();
  elapsedTimer = setInterval(tick, 1000);
}

function stopElapsedTimer() {
  if (elapsedTimer) {
    clearInterval(elapsedTimer);
    elapsedTimer = null;
  }
}

function setRunningUi(running) {
  const invalid = !!validateRequestBuilderInput().error;
  btnRunLocalAudit.disabled = running || invalid;
  btnRunLocalAudit.classList.toggle("is-running", running);
  const spinner = btnRunLocalAudit.querySelector(".btn-spinner");
  if (spinner) spinner.hidden = !running;
  if (btnModalCancel) btnModalCancel.textContent = running ? "Close" : "Cancel";
  if (!running) stopElapsedTimer();
}

function resetRunProgressUi() {
  if (runProgressPanelEl) runProgressPanelEl.hidden = true;
  if (runProgressElapsedEl) runProgressElapsedEl.textContent = "";
  setRunningUi(false);
}

function openAuditModal() {
  auditModalEl.hidden = false;
  auditModalEl.setAttribute("aria-hidden", "false");
  if (!pollTimer) {
    runExecutionPanel.hidden = true;
    resetRunProgressUi();
  }
  buildRequestCommand();
}

function closeAuditModal() {
  if (pollTimer) return;
  auditModalEl.hidden = true;
  auditModalEl.setAttribute("aria-hidden", "true");
  resetRunProgressUi();
}

function switchTab(tabId) {
  activeTab = tabId;
  document.querySelectorAll(".tab").forEach((btn) => {
    const on = btn.dataset.tab === tabId;
    btn.classList.toggle("active", on);
    btn.setAttribute("aria-selected", on ? "true" : "false");
  });
  document.querySelectorAll(".tab-panel").forEach((panel) => {
    const id = panel.id.replace("tab-", "");
    const on = id === tabId;
    panel.classList.toggle("active", on);
    panel.hidden = !on;
  });
}

function summaryCard(label, value, tone = "neutral") {
  return `
    <div class="summary-card tone-${tone}">
      <div class="summary-card-label">${escapeHtml(label)}</div>
      <div class="summary-card-value">${value}</div>
    </div>
  `;
}

function metricCard(label, value, tone = "neutral") {
  return `
    <div class="metric-card tone-${tone}">
      <div class="metric-card-label">${escapeHtml(label)}</div>
      <div class="metric-card-value">${value}</div>
    </div>
  `;
}

function renderRunHeader(manifest, artifacts, qaReport) {
  const status = deriveAuditStatus(qaReport, manifest);
  const transfersArt =
    findArtifact(artifacts, { kind: "canonical_transfers", suffix: "canonical_transfers.csv" }) ||
    findArtifact(artifacts, { kind: "transfer_log", suffix: "decoded_transfers.csv" });
  const snapshotsArt = findArtifact(artifacts, {
    kind: "supply_snapshots",
    suffix: "supply_snapshots.csv",
  });
  const supply = deriveSupplyReconciliation(qaReport);
  const transferCount = transfersArt?.row_count != null ? transfersArt.row_count.toLocaleString() : "—";
  const snapCount = snapshotsArt?.row_count != null ? snapshotsArt.row_count.toLocaleString() : "—";

  const pkgPart =
    packageStatusLabel === "Built"
      ? packageVerifiedLabel || "bundle built"
      : packageStatusLabel.toLowerCase();

  runHeaderEl.innerHTML = `
    <div class="run-header-title">
      <h2>${escapeHtml(manifest.asset || "—")} / <span class="run-id-inline">${escapeHtml(manifest.run_id || "—")}</span></h2>
      <span class="status-pill tone-${status.tone}">${escapeHtml(status.label)}</span>
    </div>
    <p class="run-header-sub">${escapeHtml(formatChainsWindows(manifest, qaReport))}</p>
    <div class="run-header-meta">
      <span>Toolkit ${escapeHtml(manifest.toolkit_version || "—")}</span>
      <span>Generated ${escapeHtml(formatDate(manifest.generated_at))}</span>
      <span>${escapeHtml(chainSummaryForRun(manifest, qaReport))}</span>
    </div>
    <p class="run-header-highlight">
      ${escapeHtml(status.label)} · ${escapeHtml(transferCount)} transfers · ${escapeHtml(snapCount)} supply snapshots · ${escapeHtml(supply.label)} supply recon · ${escapeHtml(pkgPart)}
    </p>
  `;
}

function renderMetricCards(manifest, artifacts, qaReport) {
  const supply = deriveSupplyReconciliation(qaReport);
  const transfersArt =
    findArtifact(artifacts, { kind: "canonical_transfers", suffix: "canonical_transfers.csv" }) ||
    findArtifact(artifacts, { kind: "transfer_log", suffix: "decoded_transfers.csv" });
  const snapshotsArt = findArtifact(artifacts, {
    kind: "supply_snapshots",
    suffix: "supply_snapshots.csv",
  });
  const steps = manifest.workflow_steps?.length ?? 0;

  metricCardsEl.innerHTML = [
    metricCard("Transfers reconstructed", escapeHtml(rowCountLabel(transfersArt)), "neutral"),
    metricCard("Supply snapshots", escapeHtml(rowCountLabel(snapshotsArt)), "neutral"),
    metricCard("Supply reconciliation", escapeHtml(supply.label), supply.tone),
    metricCard("Workflow steps", escapeHtml(String(steps || "—")), "neutral"),
    metricCard("Artifacts", escapeHtml(String(artifacts.length)), "neutral"),
    metricCard(
      "Evidence bundle",
      escapeHtml(packageStatusLabel),
      packageStatusLabel === "Built" ? "ok" : "neutral"
    ),
  ].join("");
}

function renderOverviewSummary(manifest, artifacts, qaReport) {
  const status = deriveAuditStatus(qaReport, manifest);
  const supply = deriveSupplyReconciliation(qaReport);
  const transfersArt =
    findArtifact(artifacts, { kind: "canonical_transfers", suffix: "canonical_transfers.csv" }) ||
    findArtifact(artifacts, { kind: "transfer_log", suffix: "decoded_transfers.csv" });

  overviewSummaryCardsEl.innerHTML = [
    summaryCard("Audit status", escapeHtml(status.label), status.tone),
    summaryCard("Chain / window", escapeHtml(formatChainsWindows(manifest, qaReport)), "neutral"),
    summaryCard("Transfer rows", escapeHtml(rowCountLabel(transfersArt)), "neutral"),
    summaryCard("Supply reconciliation", escapeHtml(supply.label), supply.tone),
  ].join("");
}

function bundleZipBasename(pkg) {
  if (!pkg) return "stablecoin_map_package.zip";
  const raw = pkg.package_zip_path || "";
  const base = raw.split(/[/\\]/).pop();
  return base || "stablecoin_map_package.zip";
}

function bundleArtifactCount(pkg, fallback) {
  if (pkg?.artifacts?.length != null) return pkg.artifacts.length;
  if (fallback != null) return fallback;
  return "—";
}

function bundleVerificationState() {
  if (packageStatusLabel !== "Built") return "not_built";
  if (packageVerifiedLabel === "verified") return "pass";
  if (packageVerifiedLabel === "verification failed") return "failed";
  return "not_verified";
}

function bundleVerificationStatusLabel() {
  const state = bundleVerificationState();
  if (state === "pass") return "Pass";
  if (state === "failed") return "Failed";
  if (state === "not_verified") return "Not verified";
  return "—";
}

function bundleVerificationStatusClass() {
  const state = bundleVerificationState();
  if (state === "pass") return "status-pass";
  if (state === "failed") return "status-fail";
  return "status-neutral";
}

function renderBundleVerificationConflictAlert() {
  if (!currentManifest) return "";
  const audit = deriveAuditStatus(currentQaReport, currentManifest);
  if (audit.label !== "PASS" || bundleVerificationState() !== "failed") return "";
  return `
    <div class="bundle-verify-alert" role="status">
      <p class="bundle-verify-alert-status">
        <span><strong>Audit status:</strong> PASS</span>
        <span><strong>Bundle verification:</strong> Failed</span>
      </p>
      <p class="bundle-verify-alert-hint">
        The audit run passed, but the evidence bundle does not match the current manifest.
        Rebuild the bundle and verify again.
      </p>
    </div>
  `;
}

function syncBundleVerifyAlertSlot() {
  if (!bundleVerifyAlertSlotEl) return;
  bundleVerifyAlertSlotEl.innerHTML = renderBundleVerificationConflictAlert();
}

function bundleGeneratedAt(pkg) {
  const iso = pkg?.created_at || pkg?.generated_at;
  return iso ? formatDate(iso) : "—";
}

function renderBundleStatusCardHtml(pkg, artifactCount) {
  const built = packageStatusLabel === "Built" && pkg;
  const checksum = pkg?.package_checksum_sha256 || "";
  if (!built) {
    return `
      <dl class="bundle-status-card">
        <dt>Status</dt><dd class="status-not-built">Not built</dd>
        <dt>Verification</dt><dd>—</dd>
        <dt>Artifacts in bundle</dt><dd>—</dd>
        <dt>Bundle file</dt><dd>—</dd>
        <dt>Checksum</dt><dd>—</dd>
        <dt>Generated at</dt><dd>—</dd>
      </dl>
    `;
  }
  return `
    <dl class="bundle-status-card">
      <dt>Status</dt><dd class="status-built">Built</dd>
      <dt>Verification</dt><dd class="${bundleVerificationStatusClass()}">${escapeHtml(bundleVerificationStatusLabel())}</dd>
      <dt>Artifacts in bundle</dt><dd>${escapeHtml(String(bundleArtifactCount(pkg, artifactCount)))}</dd>
      <dt>Bundle file</dt><dd>${escapeHtml(bundleZipBasename(pkg))}</dd>
      <dt>Checksum</dt><dd><code class="checksum-short" title="${escapeHtml(checksum)}">${escapeHtml(shortChecksum(checksum))}</code></dd>
      <dt>Generated at</dt><dd>${escapeHtml(bundleGeneratedAt(pkg))}</dd>
    </dl>
  `;
}

function overviewBundleHeadlineClass() {
  const state = bundleVerificationState();
  if (state === "pass") return "status-pass";
  if (state === "failed") return "status-fail";
  if (state === "not_built") return "status-not-built";
  return "status-built";
}

function overviewBundleHeadlineText() {
  const state = bundleVerificationState();
  if (state === "not_built") return "Not built";
  if (state === "pass") return "Verified";
  if (state === "failed") return "Verification failed";
  return "Built";
}

function renderOverviewPackageStrip() {
  if (!overviewPackageStripEl) return;
  const built = packageStatusLabel === "Built" && currentBundleManifest;
  const pkg = currentBundleManifest;
  const checksum = pkg?.package_checksum_sha256 || "";
  const count = bundleArtifactCount(pkg, currentArtifacts.length);
  const verifyState = bundleVerificationState();
  const conflictAlert =
    verifyState === "failed" ? renderBundleVerificationConflictAlert() : "";

  let bodyHtml;
  if (!built) {
    bodyHtml = `
      <p class="evidence-bundle-desc">
        An Evidence Bundle is a portable zip of this run&rsquo;s manifest, canonical artifacts, QA reports,
        provenance, execution logs, and checksums.
      </p>
      <div class="btn-row evidence-bundle-actions">
        <button type="button" class="btn btn-primary btn-sm" data-bundle-action="build" ${selectedRun ? "" : "disabled"}>Build bundle</button>
      </div>
    `;
  } else if (verifyState === "pass") {
    bodyHtml = `
      <dl class="evidence-bundle-meta evidence-bundle-meta-compact">
        <dt>Artifacts</dt><dd>${escapeHtml(String(count))}</dd>
        <dt>Checksum</dt><dd><code class="checksum-short" title="${escapeHtml(checksum)}">${escapeHtml(shortChecksum(checksum))}</code></dd>
      </dl>
      <div class="btn-row evidence-bundle-actions">
        <button type="button" class="btn btn-primary btn-sm" data-bundle-action="download" ${selectedRun ? "" : "disabled"}>Download zip</button>
        <button type="button" class="btn btn-secondary btn-sm" data-bundle-action="verify-again" ${selectedRun ? "" : "disabled"}>Verify again</button>
      </div>
    `;
  } else if (verifyState === "failed") {
    bodyHtml = `
      ${conflictAlert}
      <p class="evidence-bundle-desc">Rebuild the bundle to match the current manifest.</p>
      <div class="btn-row evidence-bundle-actions">
        <button type="button" class="btn btn-primary btn-sm" data-bundle-action="rebuild" ${selectedRun ? "" : "disabled"}>Rebuild bundle</button>
        <button type="button" class="btn btn-secondary btn-sm" data-bundle-action="verify-again" ${selectedRun ? "" : "disabled"}>Verify again</button>
      </div>
    `;
  } else {
    bodyHtml = `
      <dl class="evidence-bundle-meta evidence-bundle-meta-compact">
        <dt>Artifacts</dt><dd>${escapeHtml(String(count))}</dd>
        <dt>Checksum</dt><dd><code class="checksum-short" title="${escapeHtml(checksum)}">${escapeHtml(shortChecksum(checksum))}</code></dd>
      </dl>
      <div class="btn-row evidence-bundle-actions">
        <button type="button" class="btn btn-primary btn-sm" data-bundle-action="download" ${selectedRun ? "" : "disabled"}>Download zip</button>
        <button type="button" class="btn btn-secondary btn-sm" data-bundle-action="verify" ${selectedRun ? "" : "disabled"}>Verify bundle</button>
      </div>
    `;
  }

  overviewPackageStripEl.innerHTML = `
    <div class="evidence-bundle-card">
      <h3>Evidence Bundle: <span class="${overviewBundleHeadlineClass()}">${escapeHtml(overviewBundleHeadlineText())}</span></h3>
      ${bodyHtml}
    </div>
  `;
}

function renderBundleTabPanel() {
  const artifactCount = currentArtifacts.length;
  syncBundleVerifyAlertSlot();
  if (!selectedRun) {
    packageContentEl.innerHTML =
      '<p class="panel-status">Select a completed run to build an evidence bundle.</p>';
    return;
  }
  packageContentEl.innerHTML = renderBundleStatusCardHtml(currentBundleManifest, artifactCount);
  updateBundleActionButtons();
}

function claimIds(claims) {
  return new Set((claims || []).map((c) => c.claim));
}

function buildClaimsCompactHtml(manifest) {
  const supportedIds = claimIds(manifest.supported_claims);
  const unsupportedIds = claimIds(manifest.unsupported_claims);

  const supportedLines = SUPPORTED_SUMMARY.filter((g) =>
    g.ids.some((id) => supportedIds.has(id))
  ).map((g) => `<li>${escapeHtml(g.label)}</li>`);

  const byCategory = new Map();
  for (const g of UNSUPPORTED_SUMMARY) {
    if (!g.ids.some((id) => unsupportedIds.has(id))) continue;
    const cat = g.category || "Other";
    if (!byCategory.has(cat)) byCategory.set(cat, []);
    byCategory.get(cat).push(g.label);
  }

  const unsupportedBlocks = [...byCategory.entries()]
    .map(
      ([cat, labels]) => `
      <div class="claim-category-block">
        <h4>${escapeHtml(cat)}</h4>
        <ul>${labels.map((l) => `<li>${escapeHtml(l)}</li>`).join("")}</ul>
      </div>`
    )
    .join("");

  return `
    <div class="claims-compact-grid">
      <div class="claims-compact-block supported">
        <strong>Supported</strong>
        <ul>${supportedLines.length ? supportedLines.join("") : "<li class='muted'>None listed</li>"}</ul>
      </div>
      <div class="claims-compact-block unsupported-groups">
        <strong>Out of scope</strong>
        ${unsupportedBlocks || "<p class='muted'>None listed</p>"}
      </div>
    </div>
  `;
}

function renderClaimsCompact(manifest) {
  const html = buildClaimsCompactHtml(manifest);
  claimsCompactEl.innerHTML = html;
}

function renderClaimsTab(manifest) {
  const html = buildClaimsCompactHtml(manifest);
  claimsTabBodyEl.innerHTML = `
    <div class="claims-tab-panels">
      <div class="claims-tab-panel supported">
        <h3>Supported / conditional</h3>
        ${claimsCompactEl.querySelector(".claims-compact-block.supported")?.innerHTML || html}
      </div>
      <div class="claims-tab-panel unsupported">
        <h3>Unsupported (grouped)</h3>
        ${document.createElement("div").innerHTML = html}
      </div>
    </div>
  `;
  // Rebuild claims tab with full grouped layout
  const supportedIds = claimIds(manifest.supported_claims);
  const unsupportedIds = claimIds(manifest.unsupported_claims);
  const supportedLines = SUPPORTED_SUMMARY.filter((g) =>
    g.ids.some((id) => supportedIds.has(id))
  )
    .map((g) => `<li>${escapeHtml(g.label)}</li>`)
    .join("");

  const byCategory = new Map();
  for (const g of UNSUPPORTED_SUMMARY) {
    if (!g.ids.some((id) => unsupportedIds.has(id))) continue;
    const cat = g.category || "Other";
    if (!byCategory.has(cat)) byCategory.set(cat, []);
    byCategory.get(cat).push(g.label);
  }
  const unsupportedBlocks = [...byCategory.entries()]
    .map(
      ([cat, labels]) => `
      <div class="claim-category-block">
        <h4>${escapeHtml(cat)}</h4>
        <ul>${labels.map((l) => `<li>${escapeHtml(l)}</li>`).join("")}</ul>
      </div>`
    )
    .join("");

  claimsTabBodyEl.innerHTML = `
    <div class="claims-tab-panels">
      <div class="claims-tab-panel supported">
        <h3>Supported / conditional</h3>
        <ul>${supportedLines || "<li class='muted'>None listed in manifest</li>"}</ul>
      </div>
      <div class="claims-tab-panel unsupported">
        <h3>Unsupported (out of scope)</h3>
        ${unsupportedBlocks || "<p class='muted'>None listed in manifest</p>"}
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
      ${evidence || warnings ? `<div class="claim-meta muted">${escapeHtml([evidence, warnings].filter(Boolean).join(" · "))}</div>` : ""}
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

/** User-facing labels for manifest artifacts (suffix match first). */
const ARTIFACT_EVIDENCE_ENTRIES = [
  {
    suffix: "canonical_transfers.csv",
    evidence: "Transfer table",
    description:
      "Canonical reconstructed ERC-20 Transfer events for the configured window",
    downloadLabel: "Download CSV",
  },
  {
    suffix: "decoded_transfers.csv",
    evidence: "Transfer table",
    description: "Decoded ERC-20 Transfer events (pre-canonical export)",
    downloadLabel: "Download CSV",
  },
  {
    suffix: "supply_snapshots.csv",
    evidence: "Supply snapshots",
    description: "totalSupply snapshots at block-window boundaries",
    downloadLabel: "Download CSV",
  },
  {
    suffix: "supply_audit.csv",
    evidence: "Supply audit",
    description: "Per-chain supply reconciliation inputs and gate flags",
    downloadLabel: "Download CSV",
  },
  {
    suffix: "qa_report.json",
    kind: "qa_report",
    evidence: "QA report",
    description: "Decode, deduplication, and supply invariant gate results",
    downloadLabel: "Download JSON",
  },
  {
    suffix: "provenance.json",
    evidence: "Provenance",
    description: "Source window, asset, chain, and workflow metadata",
    downloadLabel: "Download JSON",
  },
  {
    suffix: "execution_log.ndjson",
    evidence: "Execution log",
    description: "Local audit execution trace",
    downloadLabel: "Download NDJSON",
  },
  {
    suffix: "artifact_manifest.json",
    evidence: "Artifact manifest",
    description:
      "Product contract listing artifacts, claims, checksums, and workflow steps",
    downloadLabel: "Download JSON",
  },
  {
    suffix: "audit_plan.json",
    evidence: "Audit plan",
    description: "Declared audit scope, chains, and evidence requirements",
    downloadLabel: "Download JSON",
  },
  {
    suffix: "chain_windows.json",
    evidence: "Chain windows",
    description: "Per-chain block spans used for this run",
    downloadLabel: "Download JSON",
  },
  {
    suffix: "deployment_registry.json",
    evidence: "Deployment registry",
    description: "Token contract deployments referenced by this run",
    downloadLabel: "Download JSON",
  },
  {
    suffix: "evidence_sources.json",
    evidence: "Evidence sources",
    description: "RPC and data sources used to produce artifacts",
    downloadLabel: "Download JSON",
  },
  {
    suffix: "summary.md",
    evidence: "Run summary",
    description: "Human-readable audit summary for this run",
    downloadLabel: "Download Markdown",
  },
  {
    suffix: "supply_audit.md",
    evidence: "Supply audit report",
    description: "Markdown supply reconciliation narrative",
    downloadLabel: "Download Markdown",
  },
  {
    suffix: "package_manifest.json",
    evidence: "Bundle manifest",
    description: "Evidence bundle sidecar manifest and included artifact checksums",
    downloadLabel: "Download JSON",
  },
  {
    suffix: "stablecoin_map_package.zip",
    evidence: "Evidence bundle zip",
    description: "Portable zip of manifest-listed artifacts for offline review",
    downloadLabel: "Download zip",
  },
];

function artifactEvidenceMeta(art) {
  const path = String(art.path || "");
  for (const entry of ARTIFACT_EVIDENCE_ENTRIES) {
    if (entry.suffix && path.endsWith(entry.suffix)) {
      return {
        evidence: entry.evidence,
        description: entry.description,
        downloadLabel: entry.downloadLabel,
      };
    }
    if (entry.kind && art.kind === entry.kind) {
      return {
        evidence: entry.evidence,
        description: entry.description,
        downloadLabel: entry.downloadLabel,
      };
    }
  }
  const fmt = String(art.format || "file").toUpperCase();
  const name = basename(path);
  const evidence = art.kind
    ? String(art.kind).replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase())
    : name;
  return {
    evidence,
    description: "Artifact listed in the run manifest.",
    downloadLabel: `Download ${fmt}`,
  };
}

function artifactCategory(art) {
  const kind = String(art.kind || "").toLowerCase();
  const path = String(art.path || "").toLowerCase();
  if (kind.includes("package") || path.includes("package")) return "package";
  if (path.includes("artifact_manifest") || kind.includes("manifest")) return "manifest";
  if (path.includes("execution_log") || kind.includes("execution")) return "logs";
  if (
    path.includes("provenance") ||
    path.includes("chain_windows") ||
    path.includes("deployment_registry") ||
    path.includes("audit_plan")
  ) {
    return "provenance";
  }
  if (kind.includes("qa") || path.includes("qa_report")) return "qa";
  if (
    kind.includes("canonical") ||
    kind.includes("transfer") ||
    path.includes("canonical_transfers") ||
    path.includes("decoded_transfers")
  ) {
    return "canonical";
  }
  return "other";
}

function renderArtifactDetailsRow(art, rowId) {
  const checksum = art.checksum_sha256 || "";
  const manifestJson = escapeHtml(JSON.stringify(art, null, 2));
  return `
    <tr class="artifact-details-row" id="artifact-detail-${rowId}" hidden>
      <td colspan="6">
        <div class="artifact-details-panel">
          <dl class="artifact-details-dl">
            <dt>Kind</dt><dd><code>${escapeHtml(art.kind || "—")}</code></dd>
            <dt>Filename</dt><dd><code>${escapeHtml(basename(art.path || ""))}</code></dd>
            <dt>Relative path</dt><dd><code class="artifact-path-code">${escapeHtml(art.path || "—")}</code></dd>
            <dt>Checksum (full)</dt><dd><code class="artifact-checksum-full">${escapeHtml(checksum || "—")}</code></dd>
          </dl>
          <details class="collapsible artifact-manifest-json">
            <summary>Manifest entry JSON</summary>
            <pre class="json-preview json-preview-sm">${manifestJson}</pre>
          </details>
        </div>
      </td>
    </tr>
  `;
}

function renderArtifacts(artifacts) {
  artifactsBodyEl.innerHTML = "";
  const filtered =
    artifactFilter === "all"
      ? artifacts
      : artifacts.filter((a) => artifactCategory(a) === artifactFilter);

  if (!artifacts?.length) {
    artifactsStatusEl.textContent = "No artifacts in manifest.";
    return;
  }
  artifactsStatusEl.textContent =
    artifactFilter === "all"
      ? `${artifacts.length} evidence file(s) in this run`
      : `${filtered.length} of ${artifacts.length} evidence file(s)`;

  if (!filtered.length) {
    artifactsBodyEl.innerHTML = `<tr><td colspan="6" class="muted">No artifacts match this filter.</td></tr>`;
    return;
  }

  const rows = [];
  filtered.forEach((art, index) => {
    const meta = artifactEvidenceMeta(art);
    const href = `/api/artifacts/${encodeURI(art.path)}`;
    const rowId = `a${index}`;
    const checksum = art.checksum_sha256 || "";
    rows.push(`
      <tr class="artifact-summary-row">
        <td class="artifact-evidence-cell"><span class="artifact-evidence-name">${escapeHtml(meta.evidence)}</span></td>
        <td class="artifact-desc-cell">${escapeHtml(meta.description)}</td>
        <td>${escapeHtml(art.format || "—")}</td>
        <td>${art.row_count != null ? art.row_count.toLocaleString() : "—"}</td>
        <td class="checksum-cell"><code class="checksum-short" title="${escapeHtml(checksum)}">${escapeHtml(shortChecksum(checksum))}</code></td>
        <td class="artifact-action-cell">
          <a class="download-link" href="${href}" download target="_blank" rel="noopener">${escapeHtml(meta.downloadLabel)}</a>
          <button type="button" class="btn-link artifact-details-toggle" data-artifact-toggle="${rowId}" aria-expanded="false">Details</button>
        </td>
      </tr>
      ${renderArtifactDetailsRow(art, rowId)}
    `);
  });
  artifactsBodyEl.innerHTML = rows.join("");
}

let artifactsTableListenersBound = false;

function bindArtifactsTableActions() {
  if (artifactsTableListenersBound) return;
  artifactsTableListenersBound = true;
  artifactsBodyEl.addEventListener("click", (e) => {
    const toggle = e.target.closest("[data-artifact-toggle]");
    if (!toggle) return;
    const rowId = toggle.dataset.artifactToggle;
    const detail = document.getElementById(`artifact-detail-${rowId}`);
    if (!detail) return;
    const open = detail.hidden;
    detail.hidden = !open;
    toggle.setAttribute("aria-expanded", String(open));
    toggle.textContent = open ? "Hide details" : "Details";
  });
}

function setPackageJson(text) {
  packageResultEl.textContent = text;
}

function updateBundleActionButtons() {
  const hasRun = !!selectedRun;
  const hasBundle = packageStatusLabel === "Built";
  const verifyFailed = bundleVerificationState() === "failed";
  btnBuildPackage.disabled = !hasRun;
  btnBuildPackage.textContent =
    hasBundle && verifyFailed ? "Rebuild bundle" : "Build bundle";
  btnDownloadPackage.disabled = !hasBundle;
  btnVerifyPackage.disabled = !hasBundle;
  if (bundleActionHintEl) bundleActionHintEl.hidden = hasBundle;
}

function setPackageButtons(hasRun) {
  if (!hasRun) {
    btnBuildPackage.disabled = true;
    btnDownloadPackage.disabled = true;
    btnVerifyPackage.disabled = true;
    btnBuildPackage.textContent = "Build bundle";
    if (bundleActionHintEl) bundleActionHintEl.hidden = false;
    return;
  }
  updateBundleActionButtons();
}

function refreshRunView() {
  if (!currentManifest) return;
  renderRunHeader(currentManifest, currentArtifacts, currentQaReport);
  renderMetricCards(currentManifest, currentArtifacts, currentQaReport);
  renderOverviewSummary(currentManifest, currentArtifacts, currentQaReport);
  renderClaimsCompact(currentManifest);
  renderClaimsTab(currentManifest);
  renderClaimsFull(currentManifest);
  renderArtifacts(currentArtifacts);
  refreshLogPanels();
  renderOverviewPackageStrip();
  syncBundleVerifyAlertSlot();
}

async function loadPackagePanel(run) {
  setPackageButtons(!!run);
  packageStatusLabel = run ? "Not built" : "—";
  packageVerifiedLabel = "";
  currentBundleManifest = null;

  if (!run) {
    renderBundleTabPanel();
    setPackageJson("No bundle manifest JSON loaded yet.");
    return;
  }

  try {
    const pkg = await apiFetch(
      `/api/runs/${encodeURIComponent(run.run_id)}/package${assetQuery(run)}`
    );
    currentBundleManifest = pkg;
    setPackageJson(JSON.stringify(pkg, null, 2));
    packageStatusLabel = "Built";
    setPackageButtons(true);
  } catch (err) {
    setPackageJson("No bundle manifest JSON loaded yet.");
    currentBundleManifest = null;
    if (err.status === 404) {
      packageStatusLabel = "Not built";
      renderBundleTabPanel();
    } else {
      packageStatusLabel = "Error";
      packageContentEl.innerHTML = `<p class="panel-status error">${escapeHtml(err.message)}</p>`;
    }
    setPackageButtons(true);
  }

  if (packageStatusLabel !== "Error") {
    renderBundleTabPanel();
  }
  refreshRunView();
}

async function buildEvidenceBundle() {
  if (!selectedRun) return;
  btnBuildPackage.disabled = true;
  try {
    const pkg = await apiFetch(
      `/api/runs/${encodeURIComponent(selectedRun.run_id)}/package${assetQuery(selectedRun)}`,
      { method: "POST" }
    );
    currentBundleManifest = pkg;
    setPackageJson(JSON.stringify(pkg, null, 2));
    packageStatusLabel = "Built";
    packageVerifiedLabel = "";
    setPackageButtons(true);
    renderBundleTabPanel();
    refreshRunView();
  } catch (err) {
    setPackageJson(`Error: ${err.message}`);
  } finally {
    updateBundleActionButtons();
  }
}

function downloadEvidenceBundle() {
  if (!selectedRun || packageStatusLabel !== "Built") return;
  window.open(
    `/api/runs/${encodeURIComponent(selectedRun.run_id)}/package/download${assetQuery(selectedRun)}`,
    "_blank"
  );
}

async function verifyEvidenceBundle() {
  if (!selectedRun || packageStatusLabel !== "Built") return;
  btnVerifyPackage.disabled = true;
  try {
    const report = await apiFetch(
      `/api/runs/${encodeURIComponent(selectedRun.run_id)}/package/verify${assetQuery(selectedRun)}`,
      { method: "POST" }
    );
    setPackageJson(JSON.stringify(report, null, 2));
    packageVerifiedLabel = report.package_valid ? "verified" : "verification failed";
    renderBundleTabPanel();
    refreshRunView();
  } catch (err) {
    setPackageJson(`Error: ${err.message}`);
  } finally {
    updateBundleActionButtons();
  }
}

function handleBundleOverviewAction(action) {
  if (action === "build" || action === "rebuild") {
    buildEvidenceBundle();
    return;
  }
  if (action === "download") {
    downloadEvidenceBundle();
    return;
  }
  if (action === "verify" || action === "verify-again") {
    verifyEvidenceBundle();
    return;
  }
  if (action === "open-tab") {
    switchTab("package");
  }
}

function showRunDetail(hasSelection) {
  runDetailEl.hidden = !hasSelection;
  emptyStateEl.hidden = hasSelection;
}

function showLandingView() {
  if (viewLandingEl) viewLandingEl.hidden = false;
  if (viewConsoleEl) viewConsoleEl.hidden = true;
  if (location.hash) {
    history.replaceState(null, "", `${location.pathname}${location.search}`);
  }
}

function showConsoleView(options = {}) {
  if (viewLandingEl) viewLandingEl.hidden = true;
  if (viewConsoleEl) viewConsoleEl.hidden = false;
  if (location.hash !== "#console") {
    location.hash = "console";
  }
  if (!consoleBootstrapped) {
    consoleBootstrapped = true;
    loadRuns(options.selectAsset || null, options.selectRunId || null);
  } else if (options.reload) {
    loadRuns(options.selectAsset || null, options.selectRunId || null);
  }
  if (options.openAudit) {
    openAuditModal();
  }
}

function openCleanModal() {
  if (!cleanModalEl) return;
  cleanModalEl.hidden = false;
  cleanModalEl.setAttribute("aria-hidden", "false");
  if (cleanModalStatusEl) cleanModalStatusEl.textContent = "";
}

function closeCleanModal() {
  if (!cleanModalEl) return;
  cleanModalEl.hidden = true;
  cleanModalEl.setAttribute("aria-hidden", "true");
}

async function confirmCleanDemoHistory() {
  if (cleanModalStatusEl) cleanModalStatusEl.textContent = "Cleaning…";
  try {
    const resp = await apiFetch("/api/demo/clean-history", { method: "POST" });
    const n = resp.removed_count ?? 0;
    selectedRun = null;
    currentManifest = null;
    currentArtifacts = [];
    closeCleanModal();
    if (consoleBootstrapped) {
      await loadRuns();
      showRunDetail(false);
    }
    if (cleanModalStatusEl) {
      cleanModalStatusEl.textContent = "";
    }
    setFormHint(`Removed ${n} local run(s).`);
  } catch (err) {
    if (cleanModalStatusEl) {
      cleanModalStatusEl.textContent = err.message;
      cleanModalStatusEl.classList.add("error");
    }
  }
}

async function loadDemoRun() {
  showConsoleView({ reload: true });
  const prefer = allRuns.find(
    (r) => r.run_id === "demo_001" && r.asset.toUpperCase() === "USDC"
  );
  const target = prefer || allRuns[0];
  if (target) {
    await selectRun(target);
    return;
  }
  showRunDetail(false);
  if (runListStatusEl) {
    runListStatusEl.textContent = "No demo run found — start a local audit or seed artifacts under out/.";
  }
}

function parseBlockInput(el) {
  const raw = String(el.value ?? "").trim();
  if (raw === "") return { missing: true };
  if (!/^\d+$/.test(raw)) return { invalid: true };
  const n = Number(raw);
  if (!Number.isSafeInteger(n)) return { invalid: true };
  return { value: n };
}

function validateRequestBuilderInput() {
  const asset = (reqAssetEl.value || "").trim().toUpperCase();
  const runId = (reqRunIdEl.value || "").trim();
  const chain = (reqChainEl.value || "").trim().toLowerCase();
  const identifierPattern = /^[A-Za-z0-9_-]+$/;

  if (!asset || !ASSET_CHAINS[asset]) return { error: "Select an asset." };
  if (!chain) return { error: "Select a chain." };
  if (!ASSET_CHAINS[asset].includes(chain)) {
    return { error: `${chain} is not configured for ${asset}.` };
  }
  if (!runId) return { error: "run_id is required." };
  if (!identifierPattern.test(runId)) {
    return { error: "run_id must use letters, numbers, underscore, or hyphen only." };
  }

  const fromParsed = parseBlockInput(reqFromBlockEl);
  const toParsed = parseBlockInput(reqToBlockEl);
  if (fromParsed.missing || toParsed.missing) {
    return { error: "Enter from_block and to_block." };
  }
  if (fromParsed.invalid || toParsed.invalid) {
    return { error: "Block numbers must be whole integers (no decimals)." };
  }
  const fromBlock = fromParsed.value;
  const toBlock = toParsed.value;

  if (fromBlock < 1) {
    return { error: "from_block must be at least 1." };
  }
  if (toBlock < fromBlock) {
    return { error: "to_block must be greater than or equal to from_block." };
  }

  return { asset, runId, chain, fromBlock, toBlock, fresh: !!reqFreshEl.checked };
}

function setFormHint(message, isError = false) {
  reqMessageEl.textContent = message;
  reqMessageEl.classList.toggle("error", isError);
}

function buildRequestCommand() {
  const parsed = validateRequestBuilderInput();
  const running = !!pollTimer;
  if (parsed.error) {
    generatedRequestCommand = "";
    setFormHint(parsed.error, true);
    reqCommandEl.textContent = EXAMPLE_CLI;
    btnCopyRequestCommand.disabled = true;
    if (!running) btnRunLocalAudit.disabled = true;
    return;
  }

  const windowArg = `${parsed.chain}:${parsed.fromBlock}:${parsed.toBlock}`;
  let command =
    `cargo run -- transfer-audit --asset ${shellArg(parsed.asset)} --run-id ${shellArg(parsed.runId)} \\\n` +
    `  --window ${shellArg(windowArg)}`;
  if (parsed.fresh) command += " \\\n  --fresh";

  generatedRequestCommand = command;
  reqCommandEl.textContent = command;
  setFormHint(
    `Ready — ${parsed.asset} on ${parsed.chain}, blocks ${parsed.fromBlock.toLocaleString()} → ${parsed.toBlock.toLocaleString()}.`
  );
  btnCopyRequestCommand.disabled = false;
  if (!running) btnRunLocalAudit.disabled = false;
}

async function deriveRunListStatus(run) {
  try {
    const artResp = await apiFetch(
      `/api/runs/${encodeURIComponent(run.run_id)}/artifacts${assetQuery(run)}`
    );
    const qa = await fetchQaReport(artResp.artifacts || []);
    const status = deriveAuditStatus(qa, null);
    return {
      label: status.label,
      tone: status.tone,
      chains: chainSummaryForRun(null, qa),
    };
  } catch {
    return { label: "Recorded", tone: "neutral", chains: "—" };
  }
}

function renderRunListItem(run, statusInfo) {
  const li = document.createElement("li");
  li.className = "run-item";
  const btn = document.createElement("button");
  btn.type = "button";
  btn.dataset.runId = run.run_id;
  btn.dataset.asset = run.asset;
  btn.innerHTML = `
    <div class="run-asset">${escapeHtml(run.asset)}</div>
    <div class="run-id">${escapeHtml(run.run_id)}</div>
    <div class="run-status-row">
      <span class="status-pill tone-${statusInfo.tone}">${escapeHtml(statusInfo.label)}</span>
      <span class="muted">${escapeHtml(statusInfo.chains)}</span>
    </div>
    <div class="run-meta">${escapeHtml(formatDate(run.generated_at))}</div>
  `;
  btn.addEventListener("click", () => selectRun(run));
  li.appendChild(btn);
  return li;
}

function applyRunFilter() {
  const q = (runFilterEl?.value || "").trim().toLowerCase();
  runListEl.innerHTML = "";
  const filtered = q
    ? allRuns.filter(
        (r) =>
          r.run_id.toLowerCase().includes(q) ||
          r.asset.toLowerCase().includes(q) ||
          (r.command || "").toLowerCase().includes(q)
      )
    : allRuns;

  if (!allRuns.length) {
    runListStatusEl.textContent = "";
    sidebarEmptyEl.hidden = false;
    return;
  }

  sidebarEmptyEl.hidden = true;

  if (!filtered.length) {
    runListStatusEl.textContent = "No runs match filter.";
    return;
  }

  runListStatusEl.textContent = `${filtered.length} run(s)`;
  for (const run of filtered) {
    runListEl.appendChild(
      renderRunListItem(run, run._statusInfo || { label: "…", tone: "neutral", chains: "—" })
    );
  }

  if (selectedRun) {
    document.querySelectorAll(".run-item button").forEach((btn) => {
      btn.classList.toggle(
        "active",
        btn.dataset.runId === selectedRun.run_id &&
          btn.dataset.asset?.toUpperCase() === selectedRun.asset.toUpperCase()
      );
    });
  }
}

async function loadRuns(selectAsset = null, selectRunId = null) {
  runListStatusEl.textContent = "Loading runs…";
  runListEl.innerHTML = "";
  try {
    const data = await apiFetch("/api/runs");
    allRuns = data.runs || [];

    if (allRuns.length === 0) {
      sidebarEmptyEl.hidden = false;
      runListStatusEl.textContent = "";
      showRunDetail(false);
      return;
    }

    sidebarEmptyEl.hidden = true;
    await Promise.all(
      allRuns.map(async (run) => {
        run._statusInfo = await deriveRunListStatus(run);
      })
    );

    applyRunFilter();

    let target = allRuns[0];
    if (selectAsset && selectRunId) {
      const match = allRuns.find(
        (r) =>
          r.run_id === selectRunId &&
          r.asset.toUpperCase() === selectAsset.toUpperCase()
      );
      if (match) target = match;
    }
    await selectRun(target);
  } catch (err) {
    runListStatusEl.textContent = `Failed to load runs: ${err.message}`;
    runListStatusEl.classList.add("error");
  }
}

async function pollRunUntilDone(asset, runId) {
  if (pollTimer) clearInterval(pollTimer);
  const q = `?asset=${encodeURIComponent(asset)}`;
  localRunBannerEl.hidden = false;
  localRunBannerEl.textContent = "Local audit in progress — logs update automatically.";
  switchTab("logs");
  renderRunProgress("queued", []);
  setRunningUi(true);

  const poll = async () => {
    try {
      const status = await apiFetch(
        `/api/runs/${encodeURIComponent(runId)}/status${q}`
      );
      const logs = await apiFetch(
        `/api/runs/${encodeURIComponent(runId)}/logs${q}`
      );
      executionLogEntries = logs.entries || [];
      renderExecutionLog(status, logs);
      refreshLogPanels();
      renderRunProgress(status.status, executionLogEntries);

      if (status.status === "running" || status.status === "queued") {
        return;
      }
      clearInterval(pollTimer);
      pollTimer = null;
      setRunningUi(false);
      localRunBannerEl.hidden = true;

      if (status.status === "succeeded") {
        renderRunProgress("succeeded", executionLogEntries);
        setFormHint("Audit completed — loading evidence…");
        await loadRuns(asset, runId);
        closeAuditModal();
        switchTab("overview");
      } else {
        renderRunProgress("failed", executionLogEntries);
        let msg = `Audit failed: ${status.error || "see execution log"}`;
        if (status.has_manifest) {
          msg += " A prior artifact_manifest.json may still be on disk.";
        }
        setFormHint(msg, true);
        localRunBannerEl.hidden = false;
        localRunBannerEl.textContent = msg;
      }
    } catch (err) {
      setFormHint(`Poll error: ${err.message}`, true);
    }
  };
  await poll();
  pollTimer = setInterval(poll, 2000);
}

async function runLocalAudit() {
  const parsed = validateRequestBuilderInput();
  if (parsed.error) {
    setFormHint(parsed.error, true);
    return;
  }
  showConsoleView();
  setRunningUi(true);
  runExecutionPanel.hidden = false;
  runExecutionLogEl.innerHTML = "";
  renderRunProgress("queued", []);
  startElapsedTimer();
  switchTab("logs");
  try {
    const asset = parsed.asset.toUpperCase();
    await apiFetch("/api/runs", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        asset,
        run_id: parsed.runId,
        window: {
          chain: parsed.chain,
          from_block: parsed.fromBlock,
          to_block: parsed.toBlock,
        },
        fresh: parsed.fresh,
      }),
    });
    setFormHint("Transfer-audit started. Watch progress and logs below.");
    await pollRunUntilDone(asset, parsed.runId);
  } catch (err) {
    setFormHint(`Failed to start: ${err.message}`, true);
    setRunningUi(false);
    renderRunProgress("failed", []);
  }
}

async function copyRequestCommand() {
  const text = generatedRequestCommand || EXAMPLE_CLI;
  try {
    await navigator.clipboard.writeText(text);
    setFormHint("CLI command copied to clipboard.");
  } catch {
    const temp = document.createElement("textarea");
    temp.value = text;
    document.body.appendChild(temp);
    temp.select();
    document.execCommand("copy");
    document.body.removeChild(temp);
    setFormHint("CLI command copied to clipboard.");
  }
}

function setRequestBuilderFromRun(manifest, qaReport) {
  if (!manifest) return;
  const asset = (manifest.asset || "USDC").toUpperCase();
  reqAssetEl.value = ASSET_CHAINS[asset] ? asset : "USDC";
  populateChainOptions(reqAssetEl.value);
  reqRunIdEl.value = manifest.run_id || "";
  const firstChain = qaReport?.chains?.[0];
  if (firstChain?.chain && [...reqChainEl.options].some((o) => o.value === firstChain.chain)) {
    reqChainEl.value = firstChain.chain;
  }
  if (firstChain?.from_block != null) reqFromBlockEl.value = firstChain.from_block;
  if (firstChain?.resolved_to_block != null) reqToBlockEl.value = firstChain.resolved_to_block;
  buildRequestCommand();
}

async function selectRun(run) {
  selectedRun = run;
  showRunDetail(true);
  switchTab(activeTab === "overview" ? "overview" : activeTab);

  document.querySelectorAll(".run-item button").forEach((btn) => {
    btn.classList.toggle(
      "active",
      btn.dataset.runId === run.run_id &&
        btn.dataset.asset?.toUpperCase() === run.asset.toUpperCase()
    );
  });

  try {
    currentManifest = await apiFetch(
      `/api/runs/${encodeURIComponent(run.run_id)}/manifest${assetQuery(run)}`
    );
    const artResp = await apiFetch(
      `/api/runs/${encodeURIComponent(run.run_id)}/artifacts${assetQuery(run)}`
    );
    currentArtifacts = artResp.artifacts || [];
    currentQaReport = await fetchQaReport(currentArtifacts);
    await loadExecutionLogEntries(currentArtifacts);
    setRequestBuilderFromRun(currentManifest, currentQaReport);
    refreshRunView();
    await loadPackagePanel(run);
  } catch (err) {
    runHeaderEl.innerHTML = `<p class="panel-status error">${escapeHtml(err.message)}</p>`;
    metricCardsEl.innerHTML = "";
  }
}

btnBuildPackage.addEventListener("click", () => buildEvidenceBundle());
btnDownloadPackage.addEventListener("click", () => downloadEvidenceBundle());
btnVerifyPackage.addEventListener("click", () => verifyEvidenceBundle());

if (overviewPackageStripEl) {
  overviewPackageStripEl.addEventListener("click", (e) => {
    const btn = e.target.closest("[data-bundle-action]");
    if (!btn || btn.disabled) return;
    handleBundleOverviewAction(btn.dataset.bundleAction);
  });
}

document.querySelectorAll(".tab").forEach((btn) => {
  btn.addEventListener("click", () => switchTab(btn.dataset.tab));
});

document.querySelectorAll(".filter-chip").forEach((chip) => {
  chip.addEventListener("click", () => {
    artifactFilter = chip.dataset.filter || "all";
    document.querySelectorAll(".filter-chip").forEach((c) => {
      c.classList.toggle("active", c === chip);
    });
    renderArtifacts(currentArtifacts);
  });
});

[
  reqAssetEl,
  reqRunIdEl,
  reqChainEl,
  reqFromBlockEl,
  reqToBlockEl,
  reqFreshEl,
].forEach((el) => {
  el.addEventListener("input", buildRequestCommand);
  el.addEventListener("change", buildRequestCommand);
});
reqAssetEl.addEventListener("change", () => {
  populateChainOptions(reqAssetEl.value);
  buildRequestCommand();
});

btnCopyRequestCommand.addEventListener("click", copyRequestCommand);
btnRunLocalAudit.addEventListener("click", runLocalAudit);

document.getElementById("btn-open-audit-modal").addEventListener("click", openAuditModal);
document.getElementById("btn-sidebar-new-audit").addEventListener("click", openAuditModal);
document.getElementById("btn-empty-new-audit").addEventListener("click", openAuditModal);
document.getElementById("btn-empty-copy-cli").addEventListener("click", async () => {
  generatedRequestCommand = EXAMPLE_CLI;
  try {
    await navigator.clipboard.writeText(EXAMPLE_CLI);
  } catch {
    /* ignore */
  }
});
document.getElementById("btn-modal-close").addEventListener("click", closeAuditModal);
document.getElementById("btn-modal-cancel").addEventListener("click", closeAuditModal);
document.getElementById("modal-backdrop").addEventListener("click", closeAuditModal);

if (runFilterEl) {
  runFilterEl.addEventListener("input", applyRunFilter);
}

async function checkHealth() {
  const okText = (v) => `API ok · toolkit ${v}`;
  const badText = "API unreachable";
  try {
    const health = await apiFetch("/health");
    const text = okText(health.toolkit_version);
    healthStatusEl.textContent = text;
    if (landingHealthEl) landingHealthEl.textContent = text;
    document.querySelectorAll(".mode-badge").forEach((badge) => {
      badge.textContent = "Local mode";
    });
  } catch {
    healthStatusEl.textContent = badText;
    if (landingHealthEl) landingHealthEl.textContent = badText;
  }
}

function routeFromHash() {
  if (location.hash === "#console") {
    showConsoleView();
  } else {
    showLandingView();
  }
}

checkHealth();
initAssetChainSelects();
reqRunIdEl.value = "article_ui_demo";
reqFromBlockEl.value = "24000000";
reqToBlockEl.value = "24000100";
if (emptyCliExampleEl) emptyCliExampleEl.textContent = EXAMPLE_CLI;
reqCommandEl.textContent = EXAMPLE_CLI;
buildRequestCommand();
bindArtifactsTableActions();

document.getElementById("btn-landing-start-audit")?.addEventListener("click", () => {
  openAuditModal();
});
document.getElementById("btn-landing-open-console")?.addEventListener("click", () => {
  showConsoleView();
});
document.getElementById("btn-landing-load-demo")?.addEventListener("click", () => {
  loadDemoRun();
});
document.getElementById("btn-back-landing")?.addEventListener("click", showLandingView);
document.getElementById("btn-empty-back-landing")?.addEventListener("click", showLandingView);
document.getElementById("btn-clean-history")?.addEventListener("click", openCleanModal);
document.getElementById("btn-clean-cancel")?.addEventListener("click", closeCleanModal);
document.getElementById("btn-clean-modal-close")?.addEventListener("click", closeCleanModal);
document.getElementById("clean-modal-backdrop")?.addEventListener("click", closeCleanModal);
document.getElementById("btn-clean-confirm")?.addEventListener("click", confirmCleanDemoHistory);

window.addEventListener("hashchange", routeFromHash);
routeFromHash();
