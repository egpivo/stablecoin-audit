//! Manifest-driven stablecoin-map evidence package generation.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::read::ZipArchive;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use super::checksum::sha256_file_hex;
use super::manifest::{ArtifactFormat, ArtifactKind, ArtifactManifest, ArtifactRef};
use super::writer::{load_artifact_manifest, resolve_artifact_under_root, MANIFEST_FILENAME};

pub const PACKAGE_MANIFEST_FILENAME: &str = "package_manifest.json";
pub const PACKAGE_ZIP_FILENAME: &str = "stablecoin_map_package.zip";
pub const PACKAGE_KIND: &str = "stablecoin-map-package";

/// Fixed-width placeholder while building the zip; replaced before returning.
const PACKAGE_CHECKSUM_PLACEHOLDER: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// One artifact entry recorded in `package_manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageIncludedArtifact {
    pub path: String,
    pub kind: ArtifactKind,
    pub format: ArtifactFormat,
    pub checksum_sha256: Option<String>,
}

/// Sidecar manifest for a generated stablecoin-map package zip.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageManifest {
    pub package_kind: String,
    pub run_id: String,
    pub asset: String,
    pub created_at: DateTime<Utc>,
    pub source_manifest_path: String,
    pub artifacts: Vec<PackageIncludedArtifact>,
    /// Lowercase hex SHA-256 of `stablecoin_map_package.zip` as stored on disk.
    ///
    /// Computed over zip entry bytes excluding `package_manifest.json` so the digest stays
    /// stable while the sidecar manifest is embedded in the archive.
    pub package_checksum_sha256: String,
    pub package_zip_path: String,
}

/// Load `package_manifest.json` from a run directory.
pub fn load_package_manifest(run_dir: &Path) -> Result<PackageManifest> {
    let path = run_dir.join(PACKAGE_MANIFEST_FILENAME);
    if !path.is_file() {
        anyhow::bail!(
            "{PACKAGE_MANIFEST_FILENAME} not found at {}",
            path.display()
        );
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// Build or replace `stablecoin_map_package.zip` and `package_manifest.json` from `artifact_manifest.json`.
///
/// Uses only manifest-listed artifacts (no directory scanning). Validates each artifact exists on disk
/// before packaging. Re-running overwrites prior package outputs in the run directory.
pub fn generate_stablecoin_map_package(
    run_dir: &Path,
    source_manifest_path: &str,
) -> Result<PackageManifest> {
    let artifact_manifest = load_artifact_manifest(run_dir).with_context(|| {
        format!(
            "{MANIFEST_FILENAME} not found or invalid in {}",
            run_dir.display()
        )
    })?;

    let run_id = artifact_manifest
        .run_id
        .clone()
        .context("artifact manifest missing run_id")?;
    let asset = artifact_manifest
        .asset
        .clone()
        .context("artifact manifest missing asset")?;

    validate_manifest_artifacts_on_disk(run_dir, &artifact_manifest)?;

    let included: Vec<PackageIncludedArtifact> = artifact_manifest
        .artifacts
        .iter()
        .map(package_included_from_ref)
        .collect();

    let zip_members = zip_member_paths(&artifact_manifest.artifacts);
    let mut zip_members_with_manifest = zip_members.clone();
    zip_members_with_manifest.push(PACKAGE_MANIFEST_FILENAME.to_string());

    let mut package_manifest = PackageManifest {
        package_kind: PACKAGE_KIND.to_string(),
        run_id,
        asset,
        created_at: artifact_manifest.generated_at,
        source_manifest_path: source_manifest_path.to_string(),
        artifacts: included,
        package_checksum_sha256: PACKAGE_CHECKSUM_PLACEHOLDER.to_string(),
        package_zip_path: PACKAGE_ZIP_FILENAME.to_string(),
    };

    finalize_package_outputs(run_dir, &mut package_manifest, &zip_members_with_manifest)
}

/// Write sidecar manifest and zip so both contain the same `package_manifest.json` bytes.
fn finalize_package_outputs(
    run_dir: &Path,
    manifest: &mut PackageManifest,
    zip_members: &[String],
) -> Result<PackageManifest> {
    manifest.package_checksum_sha256 = PACKAGE_CHECKSUM_PLACEHOLDER.to_string();
    sync_package_outputs(run_dir, manifest, zip_members)?;
    manifest.package_checksum_sha256 =
        package_content_checksum(&run_dir.join(PACKAGE_ZIP_FILENAME))?;
    sync_package_outputs(run_dir, manifest, zip_members)?;
    Ok(manifest.clone())
}

fn sync_package_outputs(
    run_dir: &Path,
    manifest: &PackageManifest,
    zip_members: &[String],
) -> Result<()> {
    write_package_manifest_file(run_dir, manifest)?;
    write_zip_members(run_dir, zip_members, Some(PACKAGE_ZIP_FILENAME))?;
    Ok(())
}

/// Read and parse `package_manifest.json` from inside a package zip.
pub fn read_package_manifest_from_zip(zip_path: &Path) -> Result<PackageManifest> {
    let file =
        File::open(zip_path).with_context(|| format!("open package zip {}", zip_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("read package zip {}", zip_path.display()))?;
    let mut entry = archive
        .by_name(PACKAGE_MANIFEST_FILENAME)
        .with_context(|| {
            format!(
                "{PACKAGE_MANIFEST_FILENAME} missing from package zip {}",
                zip_path.display()
            )
        })?;
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .with_context(|| format!("read {PACKAGE_MANIFEST_FILENAME} from zip"))?;
    serde_json::from_str(&text)
        .with_context(|| format!("parse {PACKAGE_MANIFEST_FILENAME} from zip"))
}

fn package_included_from_ref(a: &ArtifactRef) -> PackageIncludedArtifact {
    PackageIncludedArtifact {
        path: a.path.clone(),
        kind: a.kind,
        format: a.format,
        checksum_sha256: a.checksum_sha256.clone(),
    }
}

fn validate_manifest_artifacts_on_disk(run_dir: &Path, manifest: &ArtifactManifest) -> Result<()> {
    for artifact in &manifest.artifacts {
        resolve_artifact_under_root(run_dir, &artifact.path, true).with_context(|| {
            format!(
                "manifest lists {:?} but file is missing on disk under {}",
                artifact.path,
                run_dir.display()
            )
        })?;
    }
    resolve_artifact_under_root(run_dir, MANIFEST_FILENAME, true).with_context(|| {
        format!(
            "{MANIFEST_FILENAME} missing on disk under {}",
            run_dir.display()
        )
    })?;
    Ok(())
}

fn zip_member_paths(artifacts: &[ArtifactRef]) -> Vec<String> {
    let mut paths = vec![MANIFEST_FILENAME.to_string()];
    paths.extend(artifacts.iter().map(|a| a.path.clone()));
    paths
}

fn write_package_manifest_file(run_dir: &Path, manifest: &PackageManifest) -> Result<()> {
    let path = run_dir.join(PACKAGE_MANIFEST_FILENAME);
    std::fs::write(
        &path,
        serde_json::to_string_pretty(manifest).context("serialize package manifest")?,
    )
    .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Write a zip containing `members` (paths relative to `run_dir`). Returns SHA-256 hex of the zip bytes.
fn write_zip_members(
    run_dir: &Path,
    members: &[String],
    zip_filename: Option<&str>,
) -> Result<String> {
    let zip_name = zip_filename.unwrap_or(PACKAGE_ZIP_FILENAME);
    let zip_path = run_dir.join(zip_name);
    if zip_path.exists() {
        std::fs::remove_file(&zip_path)
            .with_context(|| format!("remove existing {}", zip_path.display()))?;
    }

    let file =
        File::create(&zip_path).with_context(|| format!("create zip {}", zip_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    for member in members {
        let source = run_dir.join(member);
        anyhow::ensure!(
            source.is_file(),
            "zip member {:?} is not a file at {}",
            member,
            source.display()
        );
        zip.start_file(member, options)
            .with_context(|| format!("zip start file {:?}", member))?;
        let mut input = File::open(&source)
            .with_context(|| format!("open zip member source {}", source.display()))?;
        std::io::copy(&mut input, &mut zip)
            .with_context(|| format!("write zip member {:?}", member))?;
    }

    zip.finish()
        .with_context(|| format!("finalize zip {}", zip_path.display()))?;
    sha256_file_hex(&zip_path).with_context(|| format!("checksum zip {}", zip_path.display()))
}

/// SHA-256 of zip member bytes excluding `package_manifest.json` (sorted by entry path).
pub fn package_content_checksum(zip_path: &Path) -> Result<String> {
    let file =
        File::open(zip_path).with_context(|| format!("open package zip {}", zip_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("read package zip {}", zip_path.display()))?;
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| {
            archive
                .by_index(i)
                .map(|entry| entry.name().to_string())
                .with_context(|| format!("read zip entry index {i}"))
        })
        .collect::<Result<_>>()?;
    names.sort();

    let mut hasher = Sha256::new();
    for name in names {
        if name == PACKAGE_MANIFEST_FILENAME {
            continue;
        }
        let mut entry = archive
            .by_name(&name)
            .with_context(|| format!("open zip entry {:?}", name))?;
        std::io::copy(&mut entry, &mut hasher)
            .with_context(|| format!("hash zip entry {:?}", name))?;
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::transfer_audit_manifest::{
        build_transfer_audit_manifest, ManifestChainInput, TransferAuditManifestParams,
    };
    use crate::artifact::{
        upsert_cross_chain_summary_manifest, write_manifest, CrossChainSummaryManifestParams,
    };
    use zip::ZipArchive;

    fn seed_transfer_audit_run(out: &Path, run_id: &str) -> Result<()> {
        for (name, body) in [
            ("qa_report.json", b"{}".as_slice()),
            ("provenance.json", b"{}".as_slice()),
            ("supply_audit.md", b"# audit".as_slice()),
            ("summary.md", b"# summary".as_slice()),
        ] {
            std::fs::write(out.join(name), body)?;
        }
        std::fs::write(out.join("supply_audit.csv"), "chain\neth\nbase\n")?;
        std::fs::write(out.join("decoded_transfers.csv"), "chain\n")?;
        let params = TransferAuditManifestParams {
            asset: "USDC".into(),
            run_id: run_id.to_string(),
            generated_at: "2026-05-15T08:03:31.695921+00:00".into(),
            per_chain_spans: true,
            provenance_from_block: 100,
            provenance_to_block_requested: None,
            chains: vec![ManifestChainInput {
                chain: "ethereum".into(),
                from_block: 100,
                to_block_requested: "200".into(),
                window_start_rfc3339: Some("2026-05-01T00:00:00Z".into()),
                window_end_rfc3339: Some("2026-05-08T00:00:00Z".into()),
                errors: vec![],
            }],
            warnings: vec![],
        };
        let manifest = build_transfer_audit_manifest(out, &params)?;
        write_manifest(out, &manifest)
    }

    fn seed_transfer_audit_and_cross_chain(out: &Path, run_id: &str) -> Result<()> {
        seed_transfer_audit_run(out, run_id)?;
        std::fs::write(out.join("cross_chain_summary.json"), r#"{"chains":[]}"#)?;
        std::fs::write(out.join("cross_chain_summary.md"), "# cross-chain")?;
        upsert_cross_chain_summary_manifest(
            out,
            &CrossChainSummaryManifestParams {
                completed_at: "2026-05-16T10:00:00+00:00".into(),
                warnings: vec![],
            },
        )
    }

    fn zip_entry_names(zip_path: &Path) -> Vec<String> {
        let file = File::open(zip_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect()
    }

    #[test]
    fn generate_succeeds_from_transfer_audit_and_cross_chain_run() {
        let out = std::env::temp_dir().join(format!(
            "stablecoin_pkg_gen_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_and_cross_chain(&out, "pkg_run").unwrap();

        let pkg = generate_stablecoin_map_package(&out, "usdc/runs/pkg_run/artifact_manifest.json")
            .unwrap();

        assert_eq!(pkg.package_kind, PACKAGE_KIND);
        assert_eq!(pkg.run_id, "pkg_run");
        assert_eq!(pkg.asset, "USDC");
        assert!(out.join(PACKAGE_MANIFEST_FILENAME).is_file());
        assert!(out.join(PACKAGE_ZIP_FILENAME).is_file());
        assert!(pkg
            .artifacts
            .iter()
            .any(|a| a.path == "cross_chain_summary.json"));
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn package_manifest_contains_expected_checksums() {
        let out = std::env::temp_dir().join(format!(
            "stablecoin_pkg_cs_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_run(&out, "pkg_cs").unwrap();

        generate_stablecoin_map_package(&out, "usdc/runs/pkg_cs/artifact_manifest.json").unwrap();
        let pkg = load_package_manifest(&out).unwrap();
        let artifact_manifest = load_artifact_manifest(&out).unwrap();

        for included in &pkg.artifacts {
            let expected = artifact_manifest
                .artifacts
                .iter()
                .find(|a| a.path == included.path)
                .expect("artifact in source manifest");
            assert_eq!(included.checksum_sha256, expected.checksum_sha256);
            assert_eq!(
                included.checksum_sha256.as_deref(),
                Some(sha256_file_hex(&out.join(&included.path)).unwrap().as_str())
            );
        }
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn package_zip_contains_expected_files() {
        let out = std::env::temp_dir().join(format!(
            "stablecoin_pkg_zip_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_and_cross_chain(&out, "pkg_zip").unwrap();

        generate_stablecoin_map_package(&out, "usdc/runs/pkg_zip/artifact_manifest.json").unwrap();
        let names = zip_entry_names(&out.join(PACKAGE_ZIP_FILENAME));
        assert!(names.contains(&MANIFEST_FILENAME.to_string()));
        assert!(names.contains(&PACKAGE_MANIFEST_FILENAME.to_string()));
        assert!(names.contains(&"cross_chain_summary.json".to_string()));
        assert!(names.contains(&"supply_audit.csv".to_string()));
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn generate_fails_without_artifact_manifest() {
        let out = std::env::temp_dir().join(format!("stablecoin_pkg_noman_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let err = generate_stablecoin_map_package(&out, "usdc/runs/x/artifact_manifest.json")
            .unwrap_err();
        assert!(err.to_string().contains("artifact_manifest.json"));
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn generate_fails_when_manifest_artifact_missing_on_disk() {
        let out = std::env::temp_dir().join(format!("stablecoin_pkg_miss_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_run(&out, "pkg_miss").unwrap();
        std::fs::remove_file(out.join("qa_report.json")).unwrap();

        let err =
            generate_stablecoin_map_package(&out, "usdc/runs/pkg_miss/artifact_manifest.json")
                .unwrap_err();
        assert!(err.to_string().contains("qa_report.json"));
        assert!(err.to_string().contains("missing on disk"));
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn package_checksum_matches_zip_content_excluding_manifest() {
        let out = std::env::temp_dir().join(format!("stablecoin_pkg_hash_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_run(&out, "pkg_hash").unwrap();

        let pkg =
            generate_stablecoin_map_package(&out, "usdc/runs/pkg_hash/artifact_manifest.json")
                .unwrap();
        let zip_path = out.join(PACKAGE_ZIP_FILENAME);
        let expected = package_content_checksum(&zip_path).unwrap();
        assert_eq!(pkg.package_checksum_sha256, expected);
        let loaded = load_package_manifest(&out).unwrap();
        assert_eq!(loaded.package_checksum_sha256, expected);
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn embedded_package_manifest_matches_sidecar() {
        let out = std::env::temp_dir().join(format!("stablecoin_pkg_embed_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_and_cross_chain(&out, "pkg_embed").unwrap();

        let returned =
            generate_stablecoin_map_package(&out, "usdc/runs/pkg_embed/artifact_manifest.json")
                .unwrap();
        let sidecar = load_package_manifest(&out).unwrap();
        let embedded = read_package_manifest_from_zip(&out.join(PACKAGE_ZIP_FILENAME)).unwrap();

        assert_eq!(returned, sidecar);
        assert_eq!(embedded, sidecar);
        assert_eq!(
            embedded.package_checksum_sha256,
            package_content_checksum(&out.join(PACKAGE_ZIP_FILENAME)).unwrap()
        );
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn regenerate_is_idempotent() {
        let out = std::env::temp_dir().join(format!("stablecoin_pkg_idem_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_and_cross_chain(&out, "pkg_idem").unwrap();
        let source = "usdc/runs/pkg_idem/artifact_manifest.json";

        let pkg1 = generate_stablecoin_map_package(&out, source).unwrap();
        let zip1 = sha256_file_hex(&out.join(PACKAGE_ZIP_FILENAME)).unwrap();
        let pkg2 = generate_stablecoin_map_package(&out, source).unwrap();
        let zip2 = sha256_file_hex(&out.join(PACKAGE_ZIP_FILENAME)).unwrap();

        assert_eq!(pkg1.artifacts.len(), pkg2.artifacts.len());
        assert_eq!(zip1, zip2);
        assert_eq!(
            load_artifact_manifest(&out).unwrap().artifacts.len(),
            pkg2.artifacts.len()
        );
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn legacy_package_manifest_without_optional_fields_still_loads() {
        let json = r#"{
          "package_kind": "stablecoin-map-package",
          "run_id": "legacy",
          "asset": "USDC",
          "created_at": "2026-05-15T08:00:00+00:00",
          "source_manifest_path": "usdc/runs/legacy/artifact_manifest.json",
          "artifacts": [{
            "path": "qa_report.json",
            "kind": "qa_report",
            "format": "json"
          }],
          "package_checksum_sha256": "abc123",
          "package_zip_path": "stablecoin_map_package.zip"
        }"#;
        let pkg: PackageManifest = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.artifacts[0].checksum_sha256, None);
    }
}
