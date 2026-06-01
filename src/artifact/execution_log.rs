//! `execution_log.ndjson` — local run execution trace (API / dev mode).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::checksum::sha256_file_hex;
use super::manifest::{ArtifactFormat, ArtifactKind, ArtifactManifest, ArtifactRef};
use super::writer::{load_artifact_manifest, write_manifest, MANIFEST_FILENAME};

pub const EXECUTION_LOG_FILENAME: &str = "execution_log.ndjson";
pub const EXECUTION_LOG_SCHEMA: &str = "execution-log-v0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLogEntry {
    pub schema: String,
    pub timestamp: String,
    pub level: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
}

#[derive(Clone)]
pub struct ExecutionLogWriter {
    path: std::path::PathBuf,
    file: Arc<Mutex<File>>,
}

/// Remove any prior execution log (used when `fresh=true` replaces a run).
pub fn reset_execution_log(run_dir: &Path) -> Result<()> {
    let path = run_dir.join(EXECUTION_LOG_FILENAME);
    if path.is_file() {
        std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

impl ExecutionLogWriter {
    /// Open the NDJSON log. When `fresh` is true, truncate any prior file so reruns
    /// do not retain traces from an earlier attempt.
    pub fn open(run_dir: &Path, fresh: bool) -> Result<Self> {
        std::fs::create_dir_all(run_dir)
            .with_context(|| format!("create run dir {}", run_dir.display()))?;
        let path = run_dir.join(EXECUTION_LOG_FILENAME);
        let file = if fresh {
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
        } else {
            OpenOptions::new().create(true).append(true).open(&path)
        }
        .with_context(|| format!("open {}", path.display()))?;
        Ok(Self {
            path,
            file: Arc::new(Mutex::new(file)),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, level: &str, message: impl AsRef<str>) -> Result<()> {
        self.emit(level, None, None, None, message)
    }

    /// Structured execution trace line for progress UI (`event` + `stage` drive the stepper).
    pub fn emit(
        &self,
        level: &str,
        event: Option<&str>,
        stage: Option<&str>,
        chain: Option<&str>,
        message: impl AsRef<str>,
    ) -> Result<()> {
        let entry = ExecutionLogEntry {
            schema: EXECUTION_LOG_SCHEMA.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            level: level.to_string(),
            message: message.as_ref().to_string(),
            chain: chain.map(str::to_string),
            event: event.map(str::to_string),
            stage: stage.map(str::to_string),
        };
        self.write_entry(&entry)
    }

    fn write_entry(&self, entry: &ExecutionLogEntry) -> Result<()> {
        let line = serde_json::to_string(entry).context("serialize execution log entry")?;
        let mut file = self
            .file
            .lock()
            .map_err(|e| anyhow::anyhow!("log lock: {e}"))?;
        writeln!(file, "{line}").context("append execution log")?;
        file.flush().ok();
        Ok(())
    }
}

/// Append `execution_log.ndjson` to an existing product manifest when the file is present.
pub fn upsert_execution_log_in_manifest(run_dir: &Path) -> Result<()> {
    let log_path = run_dir.join(EXECUTION_LOG_FILENAME);
    if !log_path.is_file() {
        return Ok(());
    }
    let manifest_path = run_dir.join(MANIFEST_FILENAME);
    if !manifest_path.is_file() {
        return Ok(());
    }
    let mut manifest: ArtifactManifest = load_artifact_manifest(&manifest_path)?;
    manifest
        .artifacts
        .retain(|a| a.path != EXECUTION_LOG_FILENAME);
    let row_count = std::fs::read_to_string(&log_path)
        .ok()
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count() as u64);
    manifest.artifacts.push(ArtifactRef {
        kind: ArtifactKind::Other,
        path: EXECUTION_LOG_FILENAME.to_string(),
        format: ArtifactFormat::Other,
        row_count,
        checksum_sha256: Some(sha256_file_hex(&log_path)?),
        description: "Local transfer-audit execution trace (NDJSON)".to_string(),
        schema: Some(EXECUTION_LOG_SCHEMA.to_string()),
    });
    write_manifest(&manifest_path, &manifest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_fresh_truncates_prior_log_lines() {
        let dir = std::env::temp_dir().join(format!("stablecoin_exec_log_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let writer = ExecutionLogWriter::open(&dir, false).unwrap();
        writer.append("info", "first attempt").unwrap();
        drop(writer);

        let path = dir.join(EXECUTION_LOG_FILENAME);
        let before = std::fs::read_to_string(&path).unwrap();
        assert!(before.contains("first attempt"));

        reset_execution_log(&dir).unwrap();
        let writer = ExecutionLogWriter::open(&dir, true).unwrap();
        writer.append("info", "second attempt").unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("first attempt"));
        assert!(text.contains("second attempt"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_writes_structured_fields() {
        let dir = std::env::temp_dir().join(format!("stablecoin_exec_emit_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let writer = ExecutionLogWriter::open(&dir, true).unwrap();
        writer
            .emit(
                "info",
                Some("chain_fetch_start"),
                Some("fetching_logs"),
                Some("ethereum"),
                "fetching Transfer logs",
            )
            .unwrap();
        let text = std::fs::read_to_string(dir.join(EXECUTION_LOG_FILENAME)).unwrap();
        assert!(text.contains("\"event\":\"chain_fetch_start\""));
        assert!(text.contains("\"stage\":\"fetching_logs\""));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
