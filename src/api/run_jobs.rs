//! Local-only transfer-audit job runner for the evidence API (developer mode).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::artifact::MANIFEST_FILENAME;
use crate::artifact::{
    reset_execution_log, upsert_execution_log_in_manifest, ExecutionLogWriter,
    EXECUTION_LOG_FILENAME,
};
use crate::config::load_single_token_config;
use crate::domain::asset::validate_identifier;
use crate::report::{ensure_run_out_dir_at, validate_run_id};
use crate::rpc::transfer_audit::run_per_chain_windows_at;

use super::artifact_store::ArtifactStore;
use super::error::ApiError;

/// Maximum inclusive block span per window for API-triggered runs.
pub const MAX_BLOCK_RANGE: u64 = 500_000;

const ALLOWED_ASSETS: &[&str] = &["USDC", "EURC", "XSGD"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

impl JobStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunJobRecord {
    pub asset: String,
    pub run_id: String,
    pub status: JobStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRunRequest {
    pub asset: String,
    pub run_id: String,
    pub window: WindowSpec,
    #[serde(default)]
    pub fresh: bool,
}

#[derive(Debug, Deserialize)]
pub struct WindowSpec {
    pub chain: String,
    pub from_block: u64,
    pub to_block: u64,
}

#[derive(Debug, Serialize)]
pub struct CreateRunResponse {
    pub asset: String,
    pub run_id: String,
    pub status: JobStatus,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RunStatusResponse {
    pub asset: String,
    pub run_id: String,
    pub status: JobStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub has_manifest: bool,
}

#[derive(Debug, Serialize)]
pub struct RunLogsResponse {
    pub asset: String,
    pub run_id: String,
    pub entries: Vec<crate::artifact::ExecutionLogEntry>,
}

#[derive(Clone)]
pub struct RunJobRegistry {
    inner: Arc<Mutex<HashMap<String, RunJobRecord>>>,
}

impl Default for RunJobRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RunJobRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn job_key(asset: &str, run_id: &str) -> String {
        format!("{}/{}", asset.to_uppercase(), run_id)
    }

    pub async fn get(&self, asset: &str, run_id: &str) -> Option<RunJobRecord> {
        let key = Self::job_key(asset, run_id);
        self.inner.lock().await.get(&key).cloned()
    }

    async fn set(&self, record: RunJobRecord) {
        let key = Self::job_key(&record.asset, &record.run_id);
        self.inner.lock().await.insert(key, record);
    }
}

pub fn chains_for_asset(asset: &str) -> Result<Vec<String>, ApiError> {
    let asset_lower = asset.to_lowercase();
    let dir = Path::new("configs/tokens");
    let mut chains = Vec::new();
    if !dir.is_dir() {
        return Err(ApiError::io_error(format!(
            "token config directory missing: {}",
            dir.display()
        )));
    }
    let prefix = format!("{asset_lower}.");
    for entry in std::fs::read_dir(dir).map_err(ApiError::from)? {
        let entry = entry.map_err(ApiError::from)?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) || !name.ends_with(".yml") {
            continue;
        }
        let stem = name.trim_end_matches(".yml");
        let chain = stem
            .strip_prefix(&prefix)
            .ok_or_else(|| ApiError::validation_error("unexpected config filename"))?;
        chains.push(chain.to_string());
    }
    chains.sort();
    chains.dedup();
    Ok(chains)
}

pub fn validate_create_run(req: &CreateRunRequest) -> Result<(), ApiError> {
    let asset = req.asset.trim();
    let run_id = req.run_id.trim();
    let chain = req.window.chain.trim().to_lowercase();

    validate_identifier(asset, "asset").map_err(|e| ApiError::validation_error(e.to_string()))?;
    validate_run_id(run_id).map_err(|e| ApiError::validation_error(e.to_string()))?;
    validate_identifier(&chain, "chain").map_err(|e| ApiError::validation_error(e.to_string()))?;

    let asset_upper = asset.to_uppercase();
    if !ALLOWED_ASSETS.contains(&asset_upper.as_str()) {
        return Err(ApiError::validation_error(format!(
            "asset must be one of: {}",
            ALLOWED_ASSETS.join(", ")
        )));
    }

    let allowed_chains = chains_for_asset(&asset_upper)?;
    if !allowed_chains.iter().any(|c| c == &chain) {
        return Err(ApiError::validation_error(format!(
            "chain {:?} is not configured for {}; allowed: {}",
            chain,
            asset_upper,
            allowed_chains.join(", ")
        )));
    }

    // Ensure token config loads (RPC env etc. validated at run time).
    load_single_token_config(&asset_upper, &chain)
        .map_err(|e| ApiError::validation_error(e.to_string()))?;

    if req.window.from_block == 0 {
        return Err(ApiError::validation_error(
            "from_block 0 is not supported".to_string(),
        ));
    }
    if req.window.to_block < req.window.from_block {
        return Err(ApiError::validation_error(format!(
            "to_block ({}) must be >= from_block ({})",
            req.window.to_block, req.window.from_block
        )));
    }
    let span = req.window.to_block - req.window.from_block;
    if span > MAX_BLOCK_RANGE {
        return Err(ApiError::validation_error(format!(
            "block range {} exceeds maximum {}",
            span, MAX_BLOCK_RANGE
        )));
    }

    Ok(())
}

/// Run directory path without creating directories.
pub fn run_dir_path(store: &ArtifactStore, asset: &str, run_id: &str) -> PathBuf {
    store
        .root()
        .join(asset.to_lowercase())
        .join("runs")
        .join(run_id)
}

pub fn ensure_run_dir(
    store: &ArtifactStore,
    asset: &str,
    run_id: &str,
) -> Result<PathBuf, ApiError> {
    ensure_run_out_dir_at(store.root(), asset, run_id)
        .map_err(|e| ApiError::validation_error(e.to_string()))
}

/// True when an in-memory job, product manifest, or on-disk run artifacts exist.
pub async fn run_record_exists(
    store: &ArtifactStore,
    registry: &RunJobRegistry,
    asset: &str,
    run_id: &str,
) -> bool {
    if registry.get(asset, run_id).await.is_some() {
        return true;
    }
    if store.load_manifest(run_id, Some(asset)).is_ok() {
        return true;
    }
    let dir = run_dir_path(store, asset, run_id);
    if !dir.is_dir() {
        return false;
    }
    dir.join(EXECUTION_LOG_FILENAME).is_file() || dir.join(MANIFEST_FILENAME).is_file()
}

pub async fn start_run(
    store: Arc<ArtifactStore>,
    registry: RunJobRegistry,
    req: CreateRunRequest,
) -> Result<CreateRunResponse, ApiError> {
    validate_create_run(&req)?;
    let asset = req.asset.trim().to_uppercase();
    let run_id = req.run_id.trim().to_string();
    let chain = req.window.chain.trim().to_lowercase();
    let from_block = req.window.from_block;
    let to_block = req.window.to_block;
    let fresh = req.fresh;

    if let Some(existing) = registry.get(&asset, &run_id).await {
        if matches!(existing.status, JobStatus::Queued | JobStatus::Running) {
            return Err(ApiError::validation_error(format!(
                "run {asset}/{run_id} is already {}",
                existing.status.as_str()
            )));
        }
    }

    let out_dir = ensure_run_dir(&store, &asset, &run_id)?;
    if fresh {
        reset_execution_log(&out_dir).map_err(|e| ApiError::io_error(e.to_string()))?;
    }
    let manifest_path = out_dir.join(MANIFEST_FILENAME);
    if manifest_path.is_file() && !fresh {
        return Err(ApiError::validation_error(format!(
            "run {asset}/{run_id} already has artifact_manifest.json; pass fresh=true to replace"
        )));
    }

    let started_at = Utc::now().to_rfc3339();
    registry
        .set(RunJobRecord {
            asset: asset.clone(),
            run_id: run_id.clone(),
            status: JobStatus::Running,
            started_at: Some(started_at.clone()),
            finished_at: None,
            error: None,
        })
        .await;

    let store_bg = store.clone();
    let registry_bg = registry.clone();
    tokio::spawn(async move {
        run_job(RunJobParams {
            store: store_bg,
            registry: registry_bg,
            asset,
            run_id,
            chain,
            from_block,
            to_block,
            fresh,
            started_at,
        })
        .await;
    });

    Ok(CreateRunResponse {
        asset: req.asset.trim().to_uppercase(),
        run_id: req.run_id.trim().to_string(),
        status: JobStatus::Running,
        message: "Local transfer-audit started. Poll /status and /logs for progress.".into(),
    })
}

struct RunJobParams {
    store: Arc<ArtifactStore>,
    registry: RunJobRegistry,
    asset: String,
    run_id: String,
    chain: String,
    from_block: u64,
    to_block: u64,
    fresh: bool,
    started_at: String,
}

async fn run_job(params: RunJobParams) {
    let RunJobParams {
        store,
        registry,
        asset,
        run_id,
        chain,
        from_block,
        to_block,
        fresh,
        started_at,
    } = params;

    let out_dir = match ensure_run_out_dir_at(store.root(), &asset, &run_id) {
        Ok(d) => d,
        Err(e) => {
            registry
                .set(RunJobRecord {
                    asset,
                    run_id,
                    status: JobStatus::Failed,
                    started_at: Some(started_at),
                    finished_at: Some(Utc::now().to_rfc3339()),
                    error: Some(e.to_string()),
                })
                .await;
            return;
        }
    };

    let log = match ExecutionLogWriter::open(&out_dir, fresh) {
        Ok(l) => l,
        Err(e) => {
            registry
                .set(RunJobRecord {
                    asset,
                    run_id,
                    status: JobStatus::Failed,
                    started_at: Some(started_at),
                    finished_at: Some(Utc::now().to_rfc3339()),
                    error: Some(e.to_string()),
                })
                .await;
            return;
        }
    };

    let _ = log.append(
        "info",
        format!(
            "starting transfer-audit asset={asset} run_id={run_id} window={chain}:{from_block}:{to_block} fresh={fresh}"
        ),
    );

    let result = run_per_chain_windows_at(
        store.root(),
        &asset,
        vec![(chain.clone(), from_block, to_block)],
        None,
        Some(run_id.clone()),
        fresh,
    )
    .await;

    match &result {
        Ok(()) => {
            let _ = log.append("info", "transfer-audit completed successfully");
        }
        Err(e) => {
            let _ = log.append("error", format!("transfer-audit failed: {e:#}"));
        }
    }

    if result.is_ok() {
        if let Err(e) = upsert_execution_log_in_manifest(&out_dir) {
            let _ = log.append("warn", format!("manifest execution_log upsert: {e:#}"));
        }
    }

    let finished_at = Utc::now().to_rfc3339();
    registry
        .set(RunJobRecord {
            asset,
            run_id,
            status: if result.is_ok() {
                JobStatus::Succeeded
            } else {
                JobStatus::Failed
            },
            started_at: Some(started_at),
            finished_at: Some(finished_at),
            error: result.err().map(|e| e.to_string()),
        })
        .await;
}

pub async fn get_status(
    store: &ArtifactStore,
    registry: &RunJobRegistry,
    run_id: &str,
    asset: Option<&str>,
) -> Result<RunStatusResponse, ApiError> {
    let asset = resolve_asset_for_run(store, run_id, asset)?;
    let job = registry.get(&asset, run_id).await;
    let has_manifest = store.load_manifest(run_id, Some(&asset)).is_ok();
    if let Some(j) = job {
        return Ok(RunStatusResponse {
            asset: j.asset,
            run_id: j.run_id,
            status: j.status,
            started_at: j.started_at,
            finished_at: j.finished_at,
            error: j.error,
            has_manifest,
        });
    }
    if has_manifest {
        return Ok(RunStatusResponse {
            asset: asset.clone(),
            run_id: run_id.to_string(),
            status: JobStatus::Succeeded,
            started_at: None,
            finished_at: None,
            error: None,
            has_manifest,
        });
    }
    Err(ApiError::not_found(format!(
        "no job or manifest for run {run_id}"
    )))
}

pub fn read_execution_log(run_dir: &Path) -> Result<RunLogsResponse, ApiError> {
    let log_path = run_dir.join(EXECUTION_LOG_FILENAME);
    if !log_path.is_file() {
        return Ok(RunLogsResponse {
            asset: String::new(),
            run_id: String::new(),
            entries: Vec::new(),
        });
    }
    let text = std::fs::read_to_string(&log_path)
        .map_err(|e| ApiError::io_error(format!("read execution log: {e}")))?;
    let mut entries = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<crate::artifact::ExecutionLogEntry>(line) {
            Ok(e) => entries.push(e),
            Err(_) => entries.push(crate::artifact::ExecutionLogEntry {
                schema: crate::artifact::EXECUTION_LOG_SCHEMA.to_string(),
                timestamp: Utc::now().to_rfc3339(),
                level: "raw".into(),
                message: line.to_string(),
            }),
        }
    }
    Ok(RunLogsResponse {
        asset: String::new(),
        run_id: String::new(),
        entries,
    })
}

pub async fn get_logs(
    store: &ArtifactStore,
    registry: &RunJobRegistry,
    run_id: &str,
    asset: Option<&str>,
) -> Result<RunLogsResponse, ApiError> {
    let asset = resolve_asset_for_run(store, run_id, asset)?;
    if !run_record_exists(store, registry, &asset, run_id).await {
        return Err(ApiError::not_found(format!(
            "no job or run for {asset}/{run_id}"
        )));
    }
    let out_dir = run_dir_path(store, &asset, run_id);
    let mut resp = read_execution_log(&out_dir)?;
    resp.asset = asset.clone();
    resp.run_id = run_id.to_string();
    if resp.entries.is_empty() {
        if let Some(job) = registry.get(&asset, run_id).await {
            if let Some(err) = job.error {
                resp.entries.push(crate::artifact::ExecutionLogEntry {
                    schema: crate::artifact::EXECUTION_LOG_SCHEMA.to_string(),
                    timestamp: job.finished_at.unwrap_or_else(|| Utc::now().to_rfc3339()),
                    level: "error".into(),
                    message: err,
                });
            }
        }
    }
    Ok(resp)
}

fn resolve_asset_for_run(
    store: &ArtifactStore,
    run_id: &str,
    asset: Option<&str>,
) -> Result<String, ApiError> {
    if let Some(a) = asset {
        let asset_upper = a.trim().to_uppercase();
        if !ALLOWED_ASSETS.contains(&asset_upper.as_str()) {
            return Err(ApiError::validation_error(format!(
                "asset must be one of: {}",
                ALLOWED_ASSETS.join(", ")
            )));
        }
        validate_run_id(run_id).map_err(|e| ApiError::validation_error(e.to_string()))?;
        return Ok(asset_upper);
    }
    let runs = store.list_runs()?;
    let matches: Vec<_> = runs.into_iter().filter(|r| r.run_id == run_id).collect();
    match matches.len() {
        0 => Err(ApiError::not_found(format!("run {run_id} not found"))),
        1 => Ok(matches[0].asset.clone()),
        _ => Err(ApiError::ambiguous_run_id(format!(
            "multiple assets have run_id {run_id}; pass ?asset="
        ))),
    }
}
