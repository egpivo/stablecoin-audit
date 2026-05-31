use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::artifact::{ArtifactManifest, ArtifactRef, MANIFEST_FILENAME};
use crate::domain::asset::validate_identifier;
use crate::report::validate_run_id;

use super::error::ApiError;

#[derive(Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunDescriptor {
    pub asset: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    pub manifest_path: String,
}

#[derive(Debug, Clone)]
struct RunLocation {
    asset_dir: String,
    asset_display: String,
    run_id: String,
    manifest_abs: PathBuf,
}

impl ArtifactStore {
    pub fn open(artifact_root: impl AsRef<Path>) -> Result<Self, ApiError> {
        let root = artifact_root.as_ref().to_path_buf();
        if !root.exists() {
            fs::create_dir_all(&root)?;
        }
        root.canonicalize()
            .map_err(|e| ApiError::io_error(format!("canonicalize artifact root: {e}")))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn list_runs(&self) -> Result<Vec<RunDescriptor>, ApiError> {
        let mut runs = Vec::new();
        if !self.root.is_dir() {
            return Ok(runs);
        }
        for asset_entry in fs::read_dir(&self.root)? {
            let asset_entry = asset_entry?;
            if !asset_entry.file_type()?.is_dir() {
                continue;
            }
            let asset_dir = asset_entry.file_name().to_string_lossy().into_owned();
            let runs_dir = asset_entry.path().join("runs");
            if !runs_dir.is_dir() {
                continue;
            }
            for run_entry in fs::read_dir(&runs_dir)? {
                let run_entry = run_entry?;
                if !run_entry.file_type()?.is_dir() {
                    continue;
                }
                let run_id = run_entry.file_name().to_string_lossy().into_owned();
                let manifest_abs = runs_dir.join(&run_id).join(MANIFEST_FILENAME);
                if !manifest_abs.is_file() {
                    continue;
                }
                let loc = RunLocation {
                    asset_dir: asset_dir.clone(),
                    asset_display: asset_dir.to_uppercase(),
                    run_id,
                    manifest_abs,
                };
                runs.push(self.descriptor_from_location(&loc)?);
            }
        }
        runs.sort_by(|a, b| {
            (a.asset.as_str(), a.run_id.as_str()).cmp(&(b.asset.as_str(), b.run_id.as_str()))
        });
        Ok(runs)
    }

    pub fn load_manifest(
        &self,
        run_id: &str,
        asset: Option<&str>,
    ) -> Result<ArtifactManifest, ApiError> {
        let loc = self.resolve_run(run_id, asset)?;
        let text = fs::read_to_string(&loc.manifest_abs).map_err(|e| {
            ApiError::manifest_not_found(format!(
                "failed to read manifest for run_id {run_id}: {e}"
            ))
        })?;
        serde_json::from_str(&text)
            .map_err(|e| ApiError::io_error(format!("invalid manifest JSON: {e}")))
    }

    pub fn list_run_artifacts(
        &self,
        run_id: &str,
        asset: Option<&str>,
    ) -> Result<(String, String, Vec<ArtifactRefResponse>), ApiError> {
        let loc = self.resolve_run(run_id, asset)?;
        let manifest = self.load_manifest(run_id, asset)?;
        let prefix = format!("{}/{}/{}", loc.asset_dir, "runs", loc.run_id);
        let artifacts = manifest
            .artifacts
            .iter()
            .map(|a| ArtifactRefResponse::from_manifest_ref(a, &prefix))
            .collect();
        Ok((loc.run_id, loc.asset_display, artifacts))
    }

    fn resolve_run(&self, run_id: &str, asset: Option<&str>) -> Result<RunLocation, ApiError> {
        validate_run_id(run_id).map_err(|e| ApiError::invalid_path(e.to_string()))?;
        if let Some(a) = asset {
            validate_identifier(a, "asset").map_err(|e| ApiError::invalid_path(e.to_string()))?;
        }

        let matches: Vec<RunLocation> = self.find_run_locations(run_id, asset)?;

        match matches.len() {
            0 => Err(ApiError::manifest_not_found(format!(
                "no {MANIFEST_FILENAME} for run_id {run_id} under {}",
                self.root.display()
            ))),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => Err(ApiError::ambiguous_run_id(format!(
                "run_id {run_id} exists under multiple assets; pass ?asset=USDC"
            ))),
        }
    }

    fn find_run_locations(
        &self,
        run_id: &str,
        asset: Option<&str>,
    ) -> Result<Vec<RunLocation>, ApiError> {
        let mut matches = Vec::new();
        if !self.root.is_dir() {
            return Ok(matches);
        }

        let asset_filter: Option<String> = asset.map(|a| a.to_lowercase());

        for asset_entry in fs::read_dir(&self.root)? {
            let asset_entry = asset_entry?;
            if !asset_entry.file_type()?.is_dir() {
                continue;
            }
            let asset_dir = asset_entry.file_name().to_string_lossy().into_owned();
            if let Some(ref want) = asset_filter {
                if asset_dir != *want {
                    continue;
                }
            }
            let manifest_abs = asset_entry
                .path()
                .join("runs")
                .join(run_id)
                .join(MANIFEST_FILENAME);
            if manifest_abs.is_file() {
                matches.push(RunLocation {
                    asset_dir: asset_dir.clone(),
                    asset_display: asset_dir.to_uppercase(),
                    run_id: run_id.to_string(),
                    manifest_abs,
                });
            }
        }
        Ok(matches)
    }

    fn descriptor_from_location(&self, loc: &RunLocation) -> Result<RunDescriptor, ApiError> {
        let manifest_path = loc
            .manifest_abs
            .strip_prefix(&self.root)
            .map_err(|_| ApiError::io_error("manifest path not under artifact root"))?
            .to_string_lossy()
            .replace('\\', "/");

        let (command, generated_at) = match fs::read_to_string(&loc.manifest_abs) {
            Ok(text) => match serde_json::from_str::<ArtifactManifest>(&text) {
                Ok(m) => (Some(m.command), Some(m.generated_at.to_rfc3339())),
                Err(_) => (None, None),
            },
            Err(_) => (None, None),
        };

        Ok(RunDescriptor {
            asset: loc.asset_display.clone(),
            run_id: loc.run_id.clone(),
            command,
            generated_at,
            manifest_path,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactRefResponse {
    pub kind: crate::artifact::ArtifactKind,
    pub path: String,
    pub format: crate::artifact::ArtifactFormat,
    pub row_count: Option<u64>,
    pub checksum_sha256: Option<String>,
    pub description: String,
}

impl ArtifactRefResponse {
    fn from_manifest_ref(a: &ArtifactRef, prefix: &str) -> Self {
        let path = format!("{prefix}/{}", a.path);
        Self {
            kind: a.kind,
            path,
            format: a.format,
            row_count: a.row_count,
            checksum_sha256: a.checksum_sha256.clone(),
            description: a.description.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RunsResponse {
    pub runs: Vec<RunDescriptor>,
}

#[derive(Debug, Serialize)]
pub struct RunArtifactsResponse {
    pub run_id: String,
    pub asset: String,
    pub artifacts: Vec<ArtifactRefResponse>,
}
