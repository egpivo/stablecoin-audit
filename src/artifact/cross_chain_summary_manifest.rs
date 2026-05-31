//! Upsert `artifact_manifest.json` after `cross-chain-summary` completes.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use super::checksum::sha256_file_hex;
use super::manifest::{
    ArtifactFormat, ArtifactKind, ArtifactRef, ClaimBoundary, ClaimStatus, WorkflowStep,
};
use super::transfer_audit_manifest::csv_row_count_if_applicable;
use super::writer::{load_artifact_manifest, write_manifest, MANIFEST_FILENAME};

const COMMAND: &str = "cross-chain-summary";

/// Parameters for the cross-chain-summary manifest upsert.
#[derive(Debug, Clone)]
pub struct CrossChainSummaryManifestParams {
    pub completed_at: String,
    pub warnings: Vec<String>,
}

/// Load existing manifest, merge cross-chain summary artifacts and workflow step, write back.
pub fn upsert_cross_chain_summary_manifest(
    out_dir: &Path,
    params: &CrossChainSummaryManifestParams,
) -> Result<()> {
    let path = out_dir.join(MANIFEST_FILENAME);
    if !path.is_file() {
        anyhow::bail!(
            "{} not found at {}; run a successful transfer-audit for this run_id first (artifact_manifest.json is required)",
            MANIFEST_FILENAME,
            path.display()
        );
    }

    let mut manifest = load_artifact_manifest(out_dir)?;
    let completed_at = parse_completed_at(&params.completed_at)?;

    let cross_chain_artifacts = collect_cross_chain_artifacts(out_dir)?;
    manifest.artifacts = upsert_artifact_refs(manifest.artifacts, cross_chain_artifacts);

    let step_paths: Vec<String> = manifest
        .artifacts
        .iter()
        .filter(|a| a.path == "cross_chain_summary.json" || a.path == "cross_chain_summary.md")
        .map(|a| a.path.clone())
        .collect();

    let step = WorkflowStep {
        command: COMMAND.to_string(),
        completed_at,
        artifacts: step_paths,
        warnings: params.warnings.clone(),
    };
    upsert_workflow_step(&mut manifest.workflow_steps, step);

    merge_cross_chain_claims(&mut manifest.supported_claims);
    merge_cross_chain_unsupported_claims(&mut manifest.unsupported_claims);
    merge_top_level_warnings(&mut manifest.warnings, &params.warnings);
    manifest.generated_at = completed_at;

    write_manifest(out_dir, &manifest)
}

fn parse_completed_at(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .with_context(|| format!("parse completed_at {:?}", s))
}

fn collect_cross_chain_artifacts(out_dir: &Path) -> Result<Vec<ArtifactRef>> {
    let candidates: &[(&str, ArtifactFormat, &str)] = &[
        (
            "cross_chain_summary.json",
            ArtifactFormat::Json,
            "Cross-chain rollup of per-chain transfer-audit metrics",
        ),
        (
            "cross_chain_summary.md",
            ArtifactFormat::Markdown,
            "Human-readable cross-chain summary",
        ),
    ];

    let mut artifacts = Vec::new();
    for (file, format, description) in candidates {
        let path = out_dir.join(file);
        if path.is_file() {
            artifacts.push(ArtifactRef {
                kind: ArtifactKind::CrossChainSummary,
                path: file.to_string(),
                format: *format,
                row_count: csv_row_count_if_applicable(&path, *format),
                checksum_sha256: Some(sha256_file_hex(&path)?),
                description: (*description).to_string(),
            });
        }
    }
    Ok(artifacts)
}

fn upsert_artifact_refs(existing: Vec<ArtifactRef>, updates: Vec<ArtifactRef>) -> Vec<ArtifactRef> {
    let update_paths: HashSet<&str> = updates.iter().map(|a| a.path.as_str()).collect();
    let mut merged: Vec<ArtifactRef> = existing
        .into_iter()
        .filter(|a| !update_paths.contains(a.path.as_str()))
        .collect();
    merged.extend(updates);
    merged
}

fn upsert_workflow_step(steps: &mut Vec<WorkflowStep>, step: WorkflowStep) {
    if let Some(i) = steps.iter().position(|s| s.command == step.command) {
        steps[i] = step;
    } else {
        steps.push(step);
    }
}

fn merge_cross_chain_claims(supported: &mut Vec<ClaimBoundary>) {
    upsert_claims(supported, cross_chain_supported_claims());
}

fn merge_cross_chain_unsupported_claims(unsupported: &mut Vec<ClaimBoundary>) {
    upsert_claims(unsupported, cross_chain_unsupported_claims());
}

fn upsert_claims(target: &mut Vec<ClaimBoundary>, updates: Vec<ClaimBoundary>) {
    for claim in updates {
        if let Some(i) = target.iter().position(|c| c.claim == claim.claim) {
            target[i] = claim;
        } else {
            target.push(claim);
        }
    }
}

fn merge_top_level_warnings(warnings: &mut Vec<String>, step_warnings: &[String]) {
    let mut seen: HashSet<String> = warnings.iter().cloned().collect();
    for w in step_warnings {
        if seen.insert(w.clone()) {
            warnings.push(w.clone());
        }
    }
}

fn cross_chain_supported_claims() -> Vec<ClaimBoundary> {
    vec![
        ClaimBoundary::new(
            "cross_chain_per_deployment_comparison",
            ClaimStatus::Conditional,
            "Per-deployment transfer-audit metrics are rolled up for cross-chain comparison on one asset schema.",
            vec![
                "cross_chain_summary.json".into(),
                "supply_audit.csv".into(),
            ],
            vec![
                "Compares per-chain deployments on one schema; bridged inventory double-counts if summed as circulating supply.".into(),
            ],
            vec![],
        ),
        ClaimBoundary::new(
            "per_chain_totalSupply_not_circulating_supply",
            ClaimStatus::Conditional,
            "Per-chain totalSupply(end) values are reported separately and must not be read as consolidated circulating supply.",
            vec!["cross_chain_summary.json".into(), "supply_audit.csv".into()],
            vec![
                "Summing per-chain totalSupply(end) double-counts bridged or custodied inventory.".into(),
            ],
            vec![],
        ),
    ]
}

fn cross_chain_unsupported_claims() -> Vec<ClaimBoundary> {
    vec![ClaimBoundary::new(
        "bridge_backing_not_verified_without_bridge_collateral",
        ClaimStatus::Unsupported,
        "Bridge collateral, mint authority, and reserve backing are not verified without bridge-specific collateral evidence.",
        vec![],
        vec![
            "Cross-chain summary compares on-chain totals only; bridge attestations and reserve data are out of scope.".into(),
        ],
        vec![],
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::transfer_audit_manifest::{
        build_transfer_audit_manifest, TransferAuditManifestParams,
    };

    fn seed_transfer_audit_manifest(out: &Path) -> Result<()> {
        for (name, body) in [
            ("qa_report.json", b"{}".as_slice()),
            ("provenance.json", b"{}".as_slice()),
            ("supply_audit.md", b"#".as_slice()),
            ("summary.md", b"#".as_slice()),
            ("decoded_transfers.csv", b"chain\n".as_slice()),
        ] {
            std::fs::write(out.join(name), body)?;
        }
        std::fs::write(out.join("supply_audit.csv"), "chain\neth\nbase\n")?;
        let params = TransferAuditManifestParams {
            asset: "USDC".into(),
            run_id: "run_cc".into(),
            generated_at: "2026-05-15T08:00:00+00:00".into(),
            per_chain_spans: false,
            provenance_from_block: 100,
            provenance_to_block_requested: Some("200".into()),
            chains: vec![],
            warnings: vec![],
        };
        let manifest = build_transfer_audit_manifest(out, &params)?;
        write_manifest(out, &manifest)
    }

    #[test]
    fn upsert_adds_cross_chain_artifacts_and_step() {
        let out =
            std::env::temp_dir().join(format!("stablecoin_cc_manifest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_manifest(&out).unwrap();
        std::fs::write(out.join("cross_chain_summary.json"), "{}").unwrap();
        std::fs::write(out.join("cross_chain_summary.md"), "# cc").unwrap();

        upsert_cross_chain_summary_manifest(
            &out,
            &CrossChainSummaryManifestParams {
                completed_at: "2026-05-16T09:00:00+00:00".into(),
                warnings: vec!["test warning".into()],
            },
        )
        .unwrap();

        let m = load_artifact_manifest(&out).unwrap();
        assert_eq!(m.generated_at.to_rfc3339(), "2026-05-16T09:00:00+00:00");
        assert_eq!(m.command, "transfer-audit");
        assert!(m
            .artifacts
            .iter()
            .any(|a| a.path == "cross_chain_summary.json"));
        assert!(m
            .workflow_steps
            .iter()
            .any(|s| s.command == "transfer-audit"));
        assert!(m.workflow_steps.iter().any(|s| s.command == COMMAND));
        for artifact in m
            .artifacts
            .iter()
            .filter(|a| a.path.starts_with("cross_chain_summary"))
        {
            let expected = crate::artifact::sha256_file_hex(&out.join(&artifact.path)).unwrap();
            assert_eq!(artifact.checksum_sha256.as_deref(), Some(expected.as_str()));
        }
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn upsert_formal_claims_idempotently() {
        let out = std::env::temp_dir().join(format!("stablecoin_cc_claims_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_manifest(&out).unwrap();
        std::fs::write(out.join("cross_chain_summary.json"), "{}").unwrap();
        std::fs::write(out.join("cross_chain_summary.md"), "#").unwrap();

        let params = CrossChainSummaryManifestParams {
            completed_at: "2026-05-16T09:00:00+00:00".into(),
            warnings: vec![],
        };
        upsert_cross_chain_summary_manifest(&out, &params).unwrap();
        let m1 = load_artifact_manifest(&out).unwrap();
        let claim = m1
            .supported_claims
            .iter()
            .find(|c| c.claim == "cross_chain_per_deployment_comparison")
            .unwrap();
        assert!(!claim.statement.is_empty());
        assert!(!claim.limitations.is_empty());
        assert!(m1
            .unsupported_claims
            .iter()
            .any(|c| c.claim == "bridge_backing_not_verified_without_bridge_collateral"));

        upsert_cross_chain_summary_manifest(&out, &params).unwrap();
        let m2 = load_artifact_manifest(&out).unwrap();
        assert_eq!(
            m2.supported_claims
                .iter()
                .filter(|c| {
                    c.claim == "cross_chain_per_deployment_comparison"
                        || c.claim == "per_chain_totalSupply_not_circulating_supply"
                })
                .count(),
            2
        );
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn upsert_is_idempotent() {
        let out = std::env::temp_dir().join(format!("stablecoin_cc_idem_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_manifest(&out).unwrap();
        std::fs::write(out.join("cross_chain_summary.json"), "{}").unwrap();
        std::fs::write(out.join("cross_chain_summary.md"), "#").unwrap();

        let params = CrossChainSummaryManifestParams {
            completed_at: "2026-05-16T09:00:00+00:00".into(),
            warnings: vec![],
        };
        upsert_cross_chain_summary_manifest(&out, &params).unwrap();
        let m1 = load_artifact_manifest(&out).unwrap();
        let n1 = m1.artifacts.len();
        upsert_cross_chain_summary_manifest(&out, &params).unwrap();
        let m2 = load_artifact_manifest(&out).unwrap();
        assert_eq!(m2.artifacts.len(), n1);
        assert_eq!(
            m2.artifacts
                .iter()
                .filter(|a| a.path.starts_with("cross_chain_summary"))
                .count(),
            2
        );
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn upsert_replaces_checksum_when_artifact_regenerated() {
        let out = std::env::temp_dir().join(format!(
            "stablecoin_cc_checksum_regen_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        seed_transfer_audit_manifest(&out).unwrap();
        std::fs::write(out.join("cross_chain_summary.json"), r#"{"v":1}"#).unwrap();
        std::fs::write(out.join("cross_chain_summary.md"), "# v1").unwrap();

        let params = CrossChainSummaryManifestParams {
            completed_at: "2026-05-16T09:00:00+00:00".into(),
            warnings: vec![],
        };
        upsert_cross_chain_summary_manifest(&out, &params).unwrap();
        let checksum_v1 = load_artifact_manifest(&out)
            .unwrap()
            .artifacts
            .into_iter()
            .find(|a| a.path == "cross_chain_summary.json")
            .unwrap()
            .checksum_sha256;

        std::fs::write(out.join("cross_chain_summary.json"), r#"{"v":2}"#).unwrap();
        upsert_cross_chain_summary_manifest(&out, &params).unwrap();
        let checksum_v2 = load_artifact_manifest(&out)
            .unwrap()
            .artifacts
            .into_iter()
            .find(|a| a.path == "cross_chain_summary.json")
            .unwrap()
            .checksum_sha256;

        assert_ne!(checksum_v1, checksum_v2);
        let expected =
            crate::artifact::sha256_file_hex(&out.join("cross_chain_summary.json")).unwrap();
        assert_eq!(checksum_v2.as_deref(), Some(expected.as_str()));
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn upsert_fails_without_existing_manifest() {
        let out =
            std::env::temp_dir().join(format!("stablecoin_cc_nomanifest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("cross_chain_summary.json"), "{}").unwrap();
        assert!(upsert_cross_chain_summary_manifest(
            &out,
            &CrossChainSummaryManifestParams {
                completed_at: "2026-05-16T09:00:00+00:00".into(),
                warnings: vec![],
            },
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn merge_top_level_warnings_dedupes_within_step_warnings() {
        let mut warnings = vec!["existing".into()];
        merge_top_level_warnings(
            &mut warnings,
            &["dup".into(), "dup".into(), "existing".into(), "new".into()],
        );
        assert_eq!(
            warnings,
            vec!["existing".to_string(), "dup".to_string(), "new".to_string(),]
        );
    }
}
