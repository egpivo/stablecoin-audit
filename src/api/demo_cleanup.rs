use serde::Serialize;
use std::fs;

use crate::artifact::MANIFEST_FILENAME;
use crate::report::validate_run_id;

use super::artifact_store::ArtifactStore;
use super::error::ApiError;

#[derive(Debug, Clone, Serialize)]
pub struct CleanedRunEntry {
    pub asset: String,
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanHistoryResponse {
    pub removed: Vec<CleanedRunEntry>,
    pub removed_count: usize,
}

impl ArtifactStore {
    /// Remove product run directories under the jailed artifact root.
    ///
    /// Only deletes `{asset}/runs/{run_id}/` when:
    /// - paths stay under the canonical artifact root (no traversal, no symlinks)
    /// - `artifact_manifest.json` exists (completed product run)
    /// - asset and run_id pass the same validators used elsewhere
    pub fn clean_product_runs(&self) -> Result<CleanHistoryResponse, ApiError> {
        let root = self
            .root()
            .canonicalize()
            .map_err(|e| ApiError::io_error(format!("canonicalize artifact root: {e}")))?;

        if !root.is_dir() {
            return Ok(CleanHistoryResponse {
                removed: Vec::new(),
                removed_count: 0,
            });
        }

        let mut removed = Vec::new();

        for asset_entry in fs::read_dir(&root).map_err(|e| ApiError::io_error(e.to_string()))? {
            let asset_entry = asset_entry.map_err(|e| ApiError::io_error(e.to_string()))?;
            if !is_safe_dir_entry(&asset_entry)? {
                continue;
            }
            let asset_dir = asset_entry.file_name().to_string_lossy().into_owned();
            if !is_safe_asset_dir_name(&asset_dir) {
                continue;
            }
            let runs_dir = asset_entry.path().join("runs");
            if !runs_dir.is_dir() {
                continue;
            }
            let runs_dir = runs_dir
                .canonicalize()
                .map_err(|e| ApiError::io_error(e.to_string()))?;
            if !runs_dir.starts_with(&root) {
                continue;
            }

            for run_entry in
                fs::read_dir(&runs_dir).map_err(|e| ApiError::io_error(e.to_string()))?
            {
                let run_entry = run_entry.map_err(|e| ApiError::io_error(e.to_string()))?;
                if !is_safe_dir_entry(&run_entry)? {
                    continue;
                }
                let run_id = run_entry.file_name().to_string_lossy().into_owned();
                if validate_run_id(&run_id).is_err() {
                    continue;
                }

                let run_dir = run_entry.path();
                let run_dir = run_dir
                    .canonicalize()
                    .map_err(|e| ApiError::io_error(e.to_string()))?;
                if !run_dir.starts_with(&root) {
                    continue;
                }

                let manifest = run_dir.join(MANIFEST_FILENAME);
                if !manifest.is_file() {
                    continue;
                }

                fs::remove_dir_all(&run_dir).map_err(|e| {
                    ApiError::io_error(format!("remove {}: {e}", run_dir.display()))
                })?;

                eprintln!("clean_product_runs: removed {}/runs/{}", asset_dir, run_id);

                removed.push(CleanedRunEntry {
                    asset: asset_dir.to_uppercase(),
                    run_id,
                });
            }
        }

        removed.sort_by(|a, b| {
            (a.asset.as_str(), a.run_id.as_str()).cmp(&(b.asset.as_str(), b.run_id.as_str()))
        });
        let removed_count = removed.len();
        Ok(CleanHistoryResponse {
            removed,
            removed_count,
        })
    }
}

fn is_safe_asset_dir_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(unix)]
fn is_safe_dir_entry(entry: &fs::DirEntry) -> Result<bool, ApiError> {
    let meta = entry
        .metadata()
        .map_err(|e| ApiError::io_error(e.to_string()))?;
    Ok(meta.is_dir() && !meta.file_type().is_symlink())
}

#[cfg(not(unix))]
fn is_safe_dir_entry(entry: &fs::DirEntry) -> Result<bool, ApiError> {
    let meta = entry
        .metadata()
        .map_err(|e| ApiError::io_error(e.to_string()))?;
    Ok(meta.is_dir())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::artifact::{
        transfer_audit_manifest::{ManifestChainInput, TransferAuditManifestParams},
        write_transfer_audit_manifest,
    };

    fn seed_run(root: &Path, asset: &str, run_id: &str) {
        let run_dir = root.join(asset).join("runs").join(run_id);
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("qa_report.json"), r#"{"asset":"USDC"}"#).unwrap();
        std::fs::write(
            run_dir.join("provenance.json"),
            r#"{"schema":"transfer-audit-provenance-v1"}"#,
        )
        .unwrap();
        write_transfer_audit_manifest(
            &run_dir,
            &TransferAuditManifestParams {
                asset: asset.to_uppercase(),
                run_id: run_id.to_string(),
                generated_at: "2026-05-15T08:03:31.695921+00:00".into(),
                per_chain_spans: true,
                provenance_from_block: 1,
                provenance_to_block_requested: None,
                chains: vec![ManifestChainInput {
                    chain: "ethereum".into(),
                    contract_address: "0xabc".into(),
                    from_block: 1,
                    to_block_requested: "2".into(),
                    window_start_rfc3339: None,
                    window_end_rfc3339: None,
                    errors: vec![],
                }],
                warnings: vec![],
            },
        )
        .unwrap();
    }

    #[test]
    fn clean_removes_manifest_runs_only() {
        let root = std::env::temp_dir().join(format!(
            "stablecoin_clean_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        seed_run(&root, "usdc", "demo_a");
        let partial = root.join("usdc/runs/partial_no_manifest");
        std::fs::create_dir_all(&partial).unwrap();
        std::fs::write(partial.join("qa_report.json"), "{}").unwrap();

        let store = ArtifactStore::open(&root).unwrap();
        let resp = store.clean_product_runs().unwrap();
        assert_eq!(resp.removed_count, 1);
        assert_eq!(resp.removed[0].run_id, "demo_a");
        assert!(partial.is_dir());
        assert!(!root.join("usdc/runs/demo_a").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn clean_skips_symlinked_run_dir() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "stablecoin_clean_symlink_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let outside =
            std::env::temp_dir().join(format!("stablecoin_clean_outside_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
        seed_run(&outside, "usdc", "evil");
        std::fs::create_dir_all(root.join("usdc/runs")).unwrap();
        symlink(outside.join("usdc/runs/evil"), root.join("usdc/runs/evil")).unwrap();

        let store = ArtifactStore::open(&root).unwrap();
        let resp = store.clean_product_runs().unwrap();
        assert_eq!(resp.removed_count, 0);
        assert!(outside.join("usdc/runs/evil").is_dir());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
