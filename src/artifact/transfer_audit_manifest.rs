//! Build `artifact_manifest.json` for completed `transfer-audit` runs.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use super::audit_plan::{
    load_audit_plan, write_audit_plan, AuditPlan, AuditWindow, ChainWindow, DataSourceRef,
    DeploymentScope, AUDIT_PLAN_FILENAME, SCHEMA as AUDIT_PLAN_SCHEMA,
};
use super::checksum::sha256_file_hex;
use super::manifest::{
    ArtifactFormat, ArtifactKind, ArtifactManifest, ArtifactRef, InputRef, SourceSnapshot,
    WorkflowStep, SCHEMA,
};
use super::writer::write_manifest;
use crate::audit::claims::{
    instantiate_claims, transfer_audit_supported_claim_ids, transfer_audit_unsupported_claim_ids,
};
use crate::audit::contracts::{
    CANONICAL_TRANSFERS_FILENAME, CHAIN_WINDOWS_FILENAME, DEPLOYMENT_REGISTRY_FILENAME,
    EVIDENCE_SOURCES_FILENAME, SUPPLY_SNAPSHOTS_FILENAME,
};

const COMMAND: &str = "transfer-audit";

const REQUESTED_CHECKS: &[&str] = &[
    "transfer_log_fetch",
    "transfer_decode",
    "mint_burn_aggregation",
    "supply_invariant_per_chain",
    "qa_gates",
];

/// Per-chain window and source metadata for manifest construction.
#[derive(Debug, Clone)]
pub struct ManifestChainInput {
    pub chain: String,
    pub contract_address: String,
    pub from_block: u64,
    pub to_block_requested: String,
    pub window_start_rfc3339: Option<String>,
    pub window_end_rfc3339: Option<String>,
    pub errors: Vec<String>,
}

/// Inputs required to build a transfer-audit product manifest.
#[derive(Debug, Clone)]
pub struct TransferAuditManifestParams {
    pub asset: String,
    pub run_id: String,
    pub generated_at: String,
    pub per_chain_spans: bool,
    pub provenance_from_block: u64,
    pub provenance_to_block_requested: Option<String>,
    pub chains: Vec<ManifestChainInput>,
    pub warnings: Vec<String>,
}

/// Write or accept `audit_plan.json` for the run (before evidence artifacts).
pub fn ensure_audit_plan(out_dir: &Path, params: &TransferAuditManifestParams) -> Result<()> {
    let plan = resolve_audit_plan(out_dir, params)?;
    write_audit_plan(out_dir, &plan)
}

/// Build manifest JSON value from run parameters and files present in `out_dir`.
pub fn build_transfer_audit_manifest(
    out_dir: &Path,
    params: &TransferAuditManifestParams,
) -> Result<ArtifactManifest> {
    ensure_audit_plan(out_dir, params)?;

    let generated_at = parse_generated_at(&params.generated_at)?;
    let mut inputs = vec![
        InputRef {
            name: "asset".into(),
            value: params.asset.to_uppercase(),
        },
        InputRef {
            name: "per_chain_spans".into(),
            value: params.per_chain_spans.to_string(),
        },
    ];
    if !params.per_chain_spans {
        inputs.push(InputRef {
            name: "from_block".into(),
            value: params.provenance_from_block.to_string(),
        });
        if let Some(ref to) = params.provenance_to_block_requested {
            inputs.push(InputRef {
                name: "to_block_requested".into(),
                value: to.clone(),
            });
        }
    }
    for chain in &params.chains {
        inputs.push(InputRef {
            name: "window".into(),
            value: format!(
                "{}:{}:{}",
                chain.chain, chain.from_block, chain.to_block_requested
            ),
        });
    }

    let artifacts = collect_artifacts(out_dir)?;
    let available: std::collections::HashSet<&str> =
        artifacts.iter().map(|a| a.path.as_str()).collect();
    let supported_claims = instantiate_claims(transfer_audit_supported_claim_ids(), &available);
    let unsupported_claims = instantiate_claims(transfer_audit_unsupported_claim_ids(), &available);
    let source_snapshots = params
        .chains
        .iter()
        .map(|c| SourceSnapshot {
            source_name: format!("rpc:{}", c.chain),
            source_url: None,
            retrieved_at: Some(generated_at),
            window_start: parse_optional_ts(c.window_start_rfc3339.as_deref()),
            window_end: parse_optional_ts(c.window_end_rfc3339.as_deref()),
        })
        .collect();
    let step_artifacts: Vec<String> = artifacts.iter().map(|a| a.path.clone()).collect();
    let workflow_steps = vec![WorkflowStep {
        command: COMMAND.to_string(),
        completed_at: generated_at,
        artifacts: step_artifacts,
        warnings: params.warnings.clone(),
    }];

    Ok(ArtifactManifest {
        schema: SCHEMA.to_string(),
        toolkit_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at,
        command: COMMAND.to_string(),
        run_id: Some(params.run_id.clone()),
        package_id: None,
        asset: Some(params.asset.to_uppercase()),
        inputs,
        artifacts,
        source_snapshots,
        supported_claims,
        unsupported_claims,
        warnings: params.warnings.clone(),
        workflow_steps,
    })
}

/// Write `artifact_manifest.json` after transfer-audit artifacts are on disk.
pub fn write_transfer_audit_manifest(
    out_dir: &Path,
    params: &TransferAuditManifestParams,
) -> Result<()> {
    let manifest = build_transfer_audit_manifest(out_dir, params)?;
    write_manifest(out_dir, &manifest)
}

fn resolve_audit_plan(out_dir: &Path, params: &TransferAuditManifestParams) -> Result<AuditPlan> {
    let path = out_dir.join(AUDIT_PLAN_FILENAME);
    if path.is_file() {
        let plan = load_audit_plan(out_dir)?;
        validate_audit_plan_matches_run(&plan, params)?;
        return Ok(plan);
    }
    Ok(build_audit_plan(params))
}

fn validate_audit_plan_matches_run(
    plan: &AuditPlan,
    params: &TransferAuditManifestParams,
) -> Result<()> {
    anyhow::ensure!(
        plan.asset.eq_ignore_ascii_case(&params.asset),
        "audit plan asset {:?} does not match run asset {:?}",
        plan.asset,
        params.asset
    );
    anyhow::ensure!(
        plan.run_id == params.run_id,
        "audit plan run_id {:?} does not match run {:?}",
        plan.run_id,
        params.run_id
    );
    Ok(())
}

pub(crate) fn build_audit_plan(params: &TransferAuditManifestParams) -> AuditPlan {
    AuditPlan {
        schema: AUDIT_PLAN_SCHEMA.to_string(),
        asset: params.asset.to_uppercase(),
        run_id: params.run_id.clone(),
        audit_window: AuditWindow {
            per_chain_spans: params.per_chain_spans,
            chains: params
                .chains
                .iter()
                .map(|c| ChainWindow {
                    chain: c.chain.clone(),
                    from_block: c.from_block,
                    to_block_requested: c.to_block_requested.clone(),
                    window_start: c.window_start_rfc3339.clone(),
                    window_end: c.window_end_rfc3339.clone(),
                })
                .collect(),
        },
        deployments: params
            .chains
            .iter()
            .map(|c| DeploymentScope {
                chain: c.chain.clone(),
                contract_address: c.contract_address.clone(),
            })
            .collect(),
        requested_checks: REQUESTED_CHECKS.iter().map(|s| (*s).to_string()).collect(),
        out_of_scope: crate::audit::audit_plan_out_of_scope_ids()
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        data_sources: params
            .chains
            .iter()
            .map(|c| DataSourceRef {
                source_name: format!("rpc:{}", c.chain),
                source_type: "evm_rpc".into(),
            })
            .collect(),
    }
}

fn parse_generated_at(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .with_context(|| format!("parse generated_at {:?}", s))
}

fn parse_optional_ts(s: Option<&str>) -> Option<DateTime<Utc>> {
    s.and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn collect_artifacts(out_dir: &Path) -> Result<Vec<ArtifactRef>> {
    let candidates: &[(&str, ArtifactKind, ArtifactFormat, &str, Option<&str>)] = &[
        (
            AUDIT_PLAN_FILENAME,
            ArtifactKind::AuditPlan,
            ArtifactFormat::Json,
            "Declared audit scope, requested checks, and out-of-scope boundaries",
            Some(AUDIT_PLAN_SCHEMA),
        ),
        (
            EVIDENCE_SOURCES_FILENAME,
            ArtifactKind::EvidenceSources,
            ArtifactFormat::Json,
            "Evidence source registry for this run",
            Some(crate::audit::contracts::EVIDENCE_SOURCES_SCHEMA),
        ),
        (
            DEPLOYMENT_REGISTRY_FILENAME,
            ArtifactKind::DeploymentRegistry,
            ArtifactFormat::Json,
            "In-scope token deployments and evidence lineage",
            Some(crate::audit::contracts::DEPLOYMENT_REGISTRY_SCHEMA),
        ),
        (
            CHAIN_WINDOWS_FILENAME,
            ArtifactKind::ChainWindows,
            ArtifactFormat::Json,
            "Per-chain audit block windows",
            Some(crate::audit::contracts::CHAIN_WINDOWS_SCHEMA),
        ),
        (
            CANONICAL_TRANSFERS_FILENAME,
            ArtifactKind::CanonicalTransfers,
            ArtifactFormat::Csv,
            "Canonical transfer log table (preferred contract)",
            Some(crate::audit::contracts::CANONICAL_TRANSFERS_SCHEMA),
        ),
        (
            SUPPLY_SNAPSHOTS_FILENAME,
            ArtifactKind::SupplySnapshots,
            ArtifactFormat::Csv,
            "Pinned totalSupply snapshots per chain",
            Some(crate::audit::contracts::SUPPLY_SNAPSHOTS_SCHEMA),
        ),
        (
            "qa_report.json",
            ArtifactKind::QaReport,
            ArtifactFormat::Json,
            "Per-chain QA gates (PASS/FAIL)",
            None,
        ),
        (
            "provenance.json",
            ArtifactKind::Provenance,
            ArtifactFormat::Json,
            "Per-chain block windows and contract addresses",
            None,
        ),
        (
            "supply_audit.csv",
            ArtifactKind::SupplyAudit,
            ArtifactFormat::Csv,
            "Mint/burn aggregate vs totalSupply delta per chain",
            None,
        ),
        (
            "supply_audit.md",
            ArtifactKind::SupplyAudit,
            ArtifactFormat::Markdown,
            "Human-readable supply invariant report",
            None,
        ),
        (
            "summary.md",
            ArtifactKind::Summary,
            ArtifactFormat::Markdown,
            "Transfer-audit run summary",
            None,
        ),
        (
            "decoded_transfers.csv",
            ArtifactKind::TransferLog,
            ArtifactFormat::Csv,
            "Decoded Transfer events in window (legacy workflow output)",
            None,
        ),
        (
            crate::artifact::execution_log::EXECUTION_LOG_FILENAME,
            ArtifactKind::Other,
            ArtifactFormat::Other,
            "Local transfer-audit execution trace (NDJSON)",
            Some(crate::artifact::execution_log::EXECUTION_LOG_SCHEMA),
        ),
    ];

    let mut artifacts = Vec::new();
    for (file, kind, format, description, schema) in candidates {
        let path = out_dir.join(file);
        if path.is_file() {
            artifacts.push(ArtifactRef {
                kind: *kind,
                path: file.to_string(),
                format: *format,
                row_count: csv_row_count_if_applicable(&path, *format),
                checksum_sha256: Some(sha256_file_hex(&path)?),
                description: (*description).to_string(),
                schema: schema.map(str::to_string),
            });
        }
    }
    Ok(artifacts)
}

pub(crate) fn csv_row_count_if_applicable(path: &Path, format: ArtifactFormat) -> Option<u64> {
    if format != ArtifactFormat::Csv {
        return None;
    }
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut lines = 0u64;
    for line in reader.lines() {
        if line.ok().as_ref().is_some_and(|l| !l.is_empty()) {
            lines += 1;
        }
    }
    if lines == 0 {
        Some(0)
    } else {
        Some(lines.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn minimal_out_dir(label: &str) -> (std::path::PathBuf, TransferAuditManifestParams) {
        let out = std::env::temp_dir().join(format!(
            "stablecoin_ta_manifest_{}_{}",
            label,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        for (name, body) in [
            ("qa_report.json", b"{}".as_slice()),
            ("provenance.json", b"{}".as_slice()),
            ("supply_audit.md", b"# audit".as_slice()),
            ("summary.md", b"# summary".as_slice()),
        ] {
            std::fs::write(out.join(name), body).unwrap();
        }
        let mut w = std::fs::File::create(out.join("supply_audit.csv")).unwrap();
        writeln!(w, "chain,chain_id").unwrap();
        writeln!(w, "ethereum,1").unwrap();
        let mut w = std::fs::File::create(out.join("decoded_transfers.csv")).unwrap();
        writeln!(w, "chain,block_number").unwrap();

        seed_canonical_stub_artifacts(&out);

        let params = TransferAuditManifestParams {
            asset: "USDC".into(),
            run_id: "run_manifest_test".into(),
            generated_at: "2026-05-15T08:03:31.695921+00:00".into(),
            per_chain_spans: true,
            provenance_from_block: 100,
            provenance_to_block_requested: None,
            chains: vec![ManifestChainInput {
                chain: "ethereum".into(),
                contract_address: "0xabc".into(),
                from_block: 100,
                to_block_requested: "200".into(),
                window_start_rfc3339: Some("2026-05-01T00:00:00Z".into()),
                window_end_rfc3339: Some("2026-05-08T00:00:00Z".into()),
                errors: vec![],
            }],
            warnings: vec![],
        };
        (out, params)
    }

    fn seed_canonical_stub_artifacts(out: &std::path::Path) {
        std::fs::write(
            out.join(EVIDENCE_SOURCES_FILENAME),
            r#"{"schema":"evidence-sources-v0","sources":[]}"#,
        )
        .unwrap();
        std::fs::write(
            out.join(DEPLOYMENT_REGISTRY_FILENAME),
            r#"{"schema":"deployment-registry-v0","asset":"USDC","run_id":"run_manifest_test","deployments":[]}"#,
        )
        .unwrap();
        std::fs::write(
            out.join(CHAIN_WINDOWS_FILENAME),
            r#"{"schema":"chain-windows-v0","asset":"USDC","run_id":"run_manifest_test","windows":[]}"#,
        )
        .unwrap();
        std::fs::write(out.join(CANONICAL_TRANSFERS_FILENAME), "chain,chain_id\n").unwrap();
        std::fs::write(out.join(SUPPLY_SNAPSHOTS_FILENAME), "chain,chain_id\n").unwrap();
    }

    #[test]
    fn build_and_roundtrip_json() {
        let (out, params) = minimal_out_dir("roundtrip");
        let manifest = build_transfer_audit_manifest(&out, &params).unwrap();
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let back: ArtifactManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.command, COMMAND);
        assert_eq!(back.run_id.as_deref(), Some("run_manifest_test"));
        assert_eq!(back.asset.as_deref(), Some("USDC"));
        assert!(!back.artifacts.is_empty());
        assert_eq!(back.supported_claims.len(), 3);
        assert!(back
            .unsupported_claims
            .iter()
            .any(|c| c.claim == "circulating_supply_not_verified"));
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn write_transfer_audit_manifest_creates_file() {
        let (out, params) = minimal_out_dir("write");
        write_transfer_audit_manifest(&out, &params).unwrap();
        assert!(out.join(super::super::writer::MANIFEST_FILENAME).is_file());
        assert!(out.join(AUDIT_PLAN_FILENAME).is_file());
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn manifest_includes_canonical_artifacts_with_checksums() {
        let (out, params) = minimal_out_dir("canonical_manifest");
        write_transfer_audit_manifest(&out, &params).unwrap();
        let manifest = super::super::writer::load_artifact_manifest(&out).unwrap();
        for path in [
            EVIDENCE_SOURCES_FILENAME,
            DEPLOYMENT_REGISTRY_FILENAME,
            CHAIN_WINDOWS_FILENAME,
            CANONICAL_TRANSFERS_FILENAME,
            SUPPLY_SNAPSHOTS_FILENAME,
        ] {
            let artifact = manifest
                .artifacts
                .iter()
                .find(|a| a.path == path)
                .unwrap_or_else(|| panic!("missing manifest artifact {path}"));
            assert!(artifact.checksum_sha256.is_some());
            assert!(artifact.schema.is_some());
        }
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn manifest_includes_audit_plan_artifact_with_checksum() {
        let (out, params) = minimal_out_dir("audit_plan_artifact");
        write_transfer_audit_manifest(&out, &params).unwrap();
        let manifest = super::super::writer::load_artifact_manifest(&out).unwrap();
        let audit_plan = manifest
            .artifacts
            .iter()
            .find(|a| a.path == AUDIT_PLAN_FILENAME)
            .expect("audit_plan.json listed in manifest");
        assert_eq!(audit_plan.kind, ArtifactKind::AuditPlan);
        let expected =
            super::super::checksum::sha256_file_hex(&out.join(AUDIT_PLAN_FILENAME)).unwrap();
        assert_eq!(
            audit_plan.checksum_sha256.as_deref(),
            Some(expected.as_str())
        );
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn supported_claims_include_evidence_paths_and_limitations() {
        let (out, params) = minimal_out_dir("claims");
        let manifest = build_transfer_audit_manifest(&out, &params).unwrap();
        let claim = manifest
            .supported_claims
            .iter()
            .find(|c| c.claim == "transfer_activity_reconstructible")
            .unwrap();
        assert!(!claim.statement.is_empty());
        assert!(claim
            .evidence_artifacts
            .contains(&"decoded_transfers.csv".to_string()));
        assert!(!claim.limitations.is_empty());
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn rejects_wrong_audit_plan_schema_when_present() {
        let (out, params) = minimal_out_dir("bad_plan");
        std::fs::write(
            out.join(AUDIT_PLAN_FILENAME),
            r#"{"schema":"wrong","asset":"USDC","run_id":"run_manifest_test","audit_window":{"per_chain_spans":true,"chains":[]},"deployments":[],"requested_checks":[],"out_of_scope":[],"data_sources":[]}"#,
        )
        .unwrap();
        assert!(build_transfer_audit_manifest(&out, &params).is_err());
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn accepts_existing_valid_audit_plan() {
        let (out, params) = minimal_out_dir("accept_plan");
        let plan = build_audit_plan(&params);
        write_audit_plan(&out, &plan).unwrap();
        let manifest = build_transfer_audit_manifest(&out, &params).unwrap();
        assert!(manifest
            .artifacts
            .iter()
            .any(|a| a.path == AUDIT_PLAN_FILENAME));
        let loaded = load_audit_plan(&out).unwrap();
        assert_eq!(loaded, plan);
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn artifact_checksums_match_file_contents() {
        let (out, params) = minimal_out_dir("checksums");
        let manifest = build_transfer_audit_manifest(&out, &params).unwrap();
        for artifact in &manifest.artifacts {
            let file = out.join(&artifact.path);
            let expected = super::super::checksum::sha256_file_hex(&file).unwrap();
            assert_eq!(artifact.checksum_sha256.as_deref(), Some(expected.as_str()));
            assert_eq!(expected.len(), 64);
        }
        let _ = std::fs::remove_dir_all(&out);
    }
}
