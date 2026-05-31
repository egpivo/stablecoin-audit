use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

use super::manifest::{ArtifactManifest, ClaimBoundary, SCHEMA};

pub const MANIFEST_FILENAME: &str = "artifact_manifest.json";

/// Read `artifact_manifest.json` from `out_dir`.
pub fn load_artifact_manifest(out_dir: &Path) -> Result<ArtifactManifest> {
    let path = out_dir.join(MANIFEST_FILENAME);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    parse_artifact_manifest_json(&text).with_context(|| format!("parse {}", path.display()))
}

/// Parse and validate product manifest JSON (schema id, required fields via serde).
pub fn parse_artifact_manifest_json(text: &str) -> Result<ArtifactManifest> {
    let manifest: ArtifactManifest =
        serde_json::from_str(text).context("deserialize artifact manifest JSON")?;
    anyhow::ensure!(
        manifest.schema == SCHEMA,
        "manifest schema must be {SCHEMA}, got {:?}",
        manifest.schema
    );
    Ok(manifest)
}

/// Write `artifact_manifest.json` into `out_dir` (run or package directory).
pub fn write_manifest(path: impl AsRef<Path>, manifest: &ArtifactManifest) -> Result<()> {
    write_artifact_manifest(path.as_ref(), manifest)
}

/// Alias target: [`write_manifest`].
pub fn write_artifact_manifest(out_dir: &Path, manifest: &ArtifactManifest) -> Result<()> {
    anyhow::ensure!(
        manifest.schema == SCHEMA,
        "manifest schema must be {SCHEMA}, got {:?}",
        manifest.schema
    );
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("create manifest dir {}", out_dir.display()))?;
    validate_manifest_paths(out_dir, manifest, true)?;
    let path = out_dir.join(MANIFEST_FILENAME);
    std::fs::write(
        &path,
        serde_json::to_string_pretty(manifest).context("serialize artifact manifest")?,
    )
    .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Validate artifact and claim-evidence paths before persisting a manifest.
///
/// When `require_existing_files` is true, each declared artifact must exist as a **file**
/// under `manifest_dir`, and `canonicalize` must keep it inside the canonical manifest root.
pub fn validate_manifest_paths(
    manifest_dir: &Path,
    manifest: &ArtifactManifest,
    require_existing_files: bool,
) -> Result<()> {
    let declared: HashSet<&str> = manifest.artifacts.iter().map(|a| a.path.as_str()).collect();

    for artifact in &manifest.artifacts {
        validate_relative_artifact_path(&artifact.path)?;
        resolve_artifact_under_root(manifest_dir, &artifact.path, require_existing_files)
            .with_context(|| format!("artifact path {:?}", artifact.path))?;
    }

    for claim in manifest
        .supported_claims
        .iter()
        .chain(manifest.unsupported_claims.iter())
    {
        validate_claim_evidence(claim, &declared)?;
        for evidence in &claim.evidence_artifacts {
            validate_relative_artifact_path(evidence)?;
            resolve_artifact_under_root(manifest_dir, evidence, require_existing_files)
                .with_context(|| format!("claim {:?} evidence path {:?}", claim.claim, evidence))?;
        }
    }

    for step in &manifest.workflow_steps {
        validate_workflow_step_artifacts(step, &declared, manifest_dir, require_existing_files)?;
    }
    Ok(())
}

fn validate_workflow_step_artifacts(
    step: &super::manifest::WorkflowStep,
    declared: &HashSet<&str>,
    manifest_dir: &Path,
    require_existing_files: bool,
) -> Result<()> {
    for path in &step.artifacts {
        if !declared.contains(path.as_str()) {
            anyhow::bail!(
                "workflow step {:?} references artifact path {:?} which is not listed in manifest.artifacts",
                step.command,
                path
            );
        }
        validate_relative_artifact_path(path)?;
        resolve_artifact_under_root(manifest_dir, path, require_existing_files).with_context(
            || format!("workflow step {:?} artifact path {:?}", step.command, path),
        )?;
    }
    Ok(())
}

fn validate_claim_evidence(claim: &ClaimBoundary, declared: &HashSet<&str>) -> Result<()> {
    for evidence in &claim.evidence_artifacts {
        if !declared.contains(evidence.as_str()) {
            anyhow::bail!(
                "claim {:?} references evidence path {:?} which is not listed in manifest.artifacts",
                claim.claim,
                evidence
            );
        }
    }
    Ok(())
}

fn canonicalize_root(root: &Path) -> Result<PathBuf> {
    root.canonicalize()
        .with_context(|| format!("canonicalize artifact root {}", root.display()))
}

/// Reject paths that could escape a run directory when resolved.
pub fn validate_relative_artifact_path(path: &str) -> Result<()> {
    if path.is_empty() {
        anyhow::bail!("artifact path must not be empty");
    }
    if path.as_bytes().contains(&0) {
        anyhow::bail!("artifact path must not contain NUL");
    }
    if path.contains('\\') {
        anyhow::bail!("artifact path must use forward slashes, got {:?}", path);
    }
    if path.ends_with('/') {
        anyhow::bail!("artifact path must not end with '/': {:?}", path);
    }
    for component in Path::new(path).components() {
        match component {
            Component::ParentDir => {
                anyhow::bail!("artifact path must not contain '..': {:?}", path);
            }
            Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("artifact path must be relative: {:?}", path);
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    let mut normal_segments = 0usize;
    for segment in path.split('/') {
        if segment.is_empty() {
            anyhow::bail!("artifact path must not contain empty segments: {:?}", path);
        }
        if segment == "." {
            anyhow::bail!("artifact path must not contain '.' segments: {:?}", path);
        }
        if segment == ".." {
            anyhow::bail!("artifact path must not contain '..': {:?}", path);
        }
        normal_segments += 1;
    }
    if normal_segments == 0 {
        anyhow::bail!(
            "artifact path must include at least one file segment: {:?}",
            path
        );
    }
    Ok(())
}

/// Join `relative` under `root` and ensure the result cannot escape the canonical root.
///
/// `root` may be non-canonical (e.g. `out/`); it is canonicalized inside this function.
/// When `must_exist` is true, the target must exist as a regular **file** (not a directory).
pub fn resolve_artifact_under_root(
    root: &Path,
    relative: &str,
    must_exist: bool,
) -> Result<PathBuf> {
    validate_relative_artifact_path(relative)?;
    let root = canonicalize_root(root)?;
    let resolved = normalize_under_root(&root, relative)?;
    if must_exist {
        if !resolved.exists() {
            anyhow::bail!(
                "artifact file does not exist under manifest directory: {}",
                resolved.display()
            );
        }
        let canonical = resolved
            .canonicalize()
            .with_context(|| format!("canonicalize artifact {}", resolved.display()))?;
        if !canonical.starts_with(&root) {
            anyhow::bail!(
                "artifact path escapes manifest directory: {}",
                canonical.display()
            );
        }
        if !canonical.is_file() {
            anyhow::bail!(
                "artifact path must refer to a file, not a directory: {}",
                canonical.display()
            );
        }
        return Ok(canonical);
    }
    if !resolved.starts_with(&root) {
        anyhow::bail!(
            "artifact path escapes manifest directory: {}",
            resolved.display()
        );
    }
    Ok(resolved)
}

fn normalize_under_root(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut out = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("invalid path component in {:?}", relative);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::manifest::{
        ArtifactFormat, ArtifactKind, ArtifactManifest, ArtifactRef, ClaimBoundary, ClaimStatus,
        WorkflowStep,
    };
    use chrono::Utc;
    use std::io::Write;

    #[test]
    fn rejects_parent_segment() {
        assert!(validate_relative_artifact_path("../qa_report.json").is_err());
    }

    #[test]
    fn rejects_backslash() {
        assert!(validate_relative_artifact_path("a\\b.csv").is_err());
    }

    #[test]
    fn rejects_nul() {
        assert!(validate_relative_artifact_path("qa\0report.json").is_err());
    }

    #[test]
    fn rejects_dot_only_paths() {
        assert!(validate_relative_artifact_path(".").is_err());
        assert!(validate_relative_artifact_path("foo/.").is_err());
        assert!(validate_relative_artifact_path("./qa.json").is_err());
    }

    #[test]
    fn rejects_trailing_slash() {
        assert!(validate_relative_artifact_path("qa_report.json/").is_err());
    }

    #[test]
    fn rejects_directory_target_when_must_exist() {
        let dir = std::env::temp_dir().join(format!("stablecoin_audit_dir_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        assert!(resolve_artifact_under_root(&dir, "nested", true).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolves_with_non_canonical_root() {
        let base =
            std::env::temp_dir().join(format!("stablecoin_audit_root_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let run_dir = base.join("run");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("artifact.json"), "{}").unwrap();
        let sloppy_root = base.join(".").join("run");
        let resolved = resolve_artifact_under_root(&sloppy_root, "artifact.json", true).unwrap();
        assert!(resolved.is_file());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn writes_manifest_file() {
        let dir =
            std::env::temp_dir().join(format!("stablecoin_audit_manifest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let manifest = ArtifactManifest::new("transfer-audit", "0.1.0");
        write_artifact_manifest(&dir, &manifest).unwrap();
        assert!(dir.join(MANIFEST_FILENAME).is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_evidence_not_in_artifacts_list() {
        let dir = std::env::temp_dir().join(format!(
            "stablecoin_audit_manifest_claim_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let supply = dir.join("supply_audit.csv");
        let mut f = std::fs::File::create(&supply).unwrap();
        writeln!(f, "chain").unwrap();

        let manifest = ArtifactManifest {
            artifacts: vec![ArtifactRef {
                kind: ArtifactKind::SupplyAudit,
                path: "supply_audit.csv".into(),
                format: ArtifactFormat::Csv,
                row_count: None,
                checksum_sha256: None,
                description: "test".into(),
            }],
            supported_claims: vec![ClaimBoundary::new(
                "supply_invariant",
                ClaimStatus::Conditional,
                "Supply invariant evaluated.",
                vec!["qa_report.json".into()],
                vec!["test".into()],
                vec![],
            )],
            ..ArtifactManifest::new("transfer-audit", "0.1.0")
        };
        assert!(write_artifact_manifest(&dir, &manifest).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn accepts_claim_evidence_when_file_exists_and_declared() {
        let dir = std::env::temp_dir().join(format!(
            "stablecoin_audit_manifest_ok_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("qa_report.json"), "{}").unwrap();

        let manifest = ArtifactManifest {
            artifacts: vec![ArtifactRef {
                kind: ArtifactKind::QaReport,
                path: "qa_report.json".into(),
                format: ArtifactFormat::Json,
                row_count: None,
                checksum_sha256: None,
                description: "test".into(),
            }],
            supported_claims: vec![ClaimBoundary::new(
                "gates",
                ClaimStatus::Conditional,
                "QA gates evaluated.",
                vec!["qa_report.json".into()],
                vec!["test".into()],
                vec![],
            )],
            ..ArtifactManifest::new("transfer-audit", "0.1.0")
        };
        write_artifact_manifest(&dir, &manifest).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_workflow_step_path_not_in_artifacts_list() {
        let dir = std::env::temp_dir().join(format!(
            "stablecoin_audit_manifest_workflow_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("supply_audit.csv"), "chain\n").unwrap();

        let manifest = ArtifactManifest {
            artifacts: vec![ArtifactRef {
                kind: ArtifactKind::SupplyAudit,
                path: "supply_audit.csv".into(),
                format: ArtifactFormat::Csv,
                row_count: None,
                checksum_sha256: None,
                description: "test".into(),
            }],
            workflow_steps: vec![WorkflowStep {
                command: "cross-chain-summary".into(),
                completed_at: Utc::now(),
                artifacts: vec!["cross_chain_summary.json".into()],
                warnings: vec![],
            }],
            ..ArtifactManifest::new("transfer-audit", "0.1.0")
        };
        assert!(write_artifact_manifest(&dir, &manifest).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_rejects_wrong_schema() {
        let json = r#"{
          "schema": "wrong-schema",
          "toolkit_version": "0.1.0",
          "generated_at": "2026-05-15T08:00:00+00:00",
          "command": "transfer-audit",
          "run_id": null,
          "package_id": null,
          "asset": null,
          "inputs": [],
          "artifacts": [],
          "source_snapshots": [],
          "supported_claims": [],
          "unsupported_claims": [],
          "warnings": []
        }"#;
        assert!(parse_artifact_manifest_json(json).is_err());
    }
}
