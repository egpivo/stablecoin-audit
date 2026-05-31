//! Build `artifact_manifest.json` for completed `transfer-audit` runs.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use super::checksum::sha256_file_hex;
use super::manifest::{
    ArtifactFormat, ArtifactKind, ArtifactManifest, ArtifactRef, ClaimBoundary, ClaimStatus,
    InputRef, SourceSnapshot, WorkflowStep, SCHEMA,
};
use super::writer::write_manifest;

const COMMAND: &str = "transfer-audit";

/// Per-chain window and source metadata for manifest construction.
#[derive(Debug, Clone)]
pub struct ManifestChainInput {
    pub chain: String,
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

/// Build manifest JSON value from run parameters and files present in `out_dir`.
pub fn build_transfer_audit_manifest(
    out_dir: &Path,
    params: &TransferAuditManifestParams,
) -> Result<ArtifactManifest> {
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

    let supported_claims = transfer_audit_supported_claims();
    let unsupported_claims = transfer_audit_unsupported_claims();
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
    let candidates: &[(&str, ArtifactKind, ArtifactFormat, &str)] = &[
        (
            "qa_report.json",
            ArtifactKind::QaReport,
            ArtifactFormat::Json,
            "Per-chain QA gates (PASS/FAIL)",
        ),
        (
            "provenance.json",
            ArtifactKind::Provenance,
            ArtifactFormat::Json,
            "Per-chain block windows and contract addresses",
        ),
        (
            "supply_audit.csv",
            ArtifactKind::SupplyAudit,
            ArtifactFormat::Csv,
            "Mint/burn aggregate vs totalSupply delta per chain",
        ),
        (
            "supply_audit.md",
            ArtifactKind::SupplyAudit,
            ArtifactFormat::Markdown,
            "Human-readable supply invariant report",
        ),
        (
            "summary.md",
            ArtifactKind::Summary,
            ArtifactFormat::Markdown,
            "Transfer-audit run summary",
        ),
        (
            "decoded_transfers.csv",
            ArtifactKind::TransferLog,
            ArtifactFormat::Csv,
            "Decoded Transfer events in window",
        ),
    ];

    let mut artifacts = Vec::new();
    for (file, kind, format, description) in candidates {
        let path = out_dir.join(file);
        if path.is_file() {
            artifacts.push(ArtifactRef {
                kind: *kind,
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

fn transfer_audit_supported_claims() -> Vec<ClaimBoundary> {
    vec![
        ClaimBoundary {
            claim: "transfer_logs_fetched".into(),
            status: ClaimStatus::Conditional,
            evidence_artifacts: vec!["decoded_transfers.csv".into(), "qa_report.json".into()],
            caveat: "Holds for configured asset, chain, and block window when RPC fetch and decode gates PASS.".into(),
        },
        ClaimBoundary {
            claim: "mint_burn_aggregates_computed".into(),
            status: ClaimStatus::Conditional,
            evidence_artifacts: vec!["supply_audit.csv".into(), "qa_report.json".into()],
            caveat: "Mint/burn sums use toolkit Transfer definitions (zero-address mint/burn); not holder intent.".into(),
        },
        ClaimBoundary {
            claim: "total_supply_boundary_checks_evaluated".into(),
            status: ClaimStatus::Conditional,
            evidence_artifacts: vec!["supply_audit.csv".into(), "qa_report.json".into()],
            caveat: "Compares pinned totalSupply deltas to mint/burn aggregates per chain; FAIL is not proof of fraud.".into(),
        },
        ClaimBoundary {
            claim: "qa_artifacts_generated".into(),
            status: ClaimStatus::Supported,
            evidence_artifacts: vec![
                "qa_report.json".into(),
                "supply_audit.md".into(),
                "summary.md".into(),
            ],
            caveat: "Files exist for this run; gate PASS/FAIL is read from qa_report.json.".into(),
        },
    ]
}

fn transfer_audit_unsupported_claims() -> Vec<ClaimBoundary> {
    const CAVEAT: &str = "Out of scope for transfer-audit; not attested by this toolkit.";
    let unsupported = [
        "reserve_adequacy",
        "peg_stability",
        "redemption_capacity",
        "bridge_backing",
        "user_geography",
        "holder_identity",
        "actual_swap_routing",
        "issuer_intent",
        "stress_transmission",
    ];
    unsupported
        .iter()
        .map(|claim| ClaimBoundary {
            claim: (*claim).to_string(),
            status: ClaimStatus::Unsupported,
            evidence_artifacts: vec![],
            caveat: CAVEAT.to_string(),
        })
        .collect()
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

        let params = TransferAuditManifestParams {
            asset: "USDC".into(),
            run_id: "run_manifest_test".into(),
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
        (out, params)
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
        assert_eq!(back.supported_claims.len(), 4);
        assert_eq!(back.unsupported_claims.len(), 9);
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn write_transfer_audit_manifest_creates_file() {
        let (out, params) = minimal_out_dir("write");
        write_transfer_audit_manifest(&out, &params).unwrap();
        assert!(out.join(super::super::writer::MANIFEST_FILENAME).is_file());
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
