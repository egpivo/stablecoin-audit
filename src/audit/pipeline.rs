//! v0 audit product pipeline — stage validation for completed transfer-audit runs.

use std::path::Path;

use anyhow::{Context, Result};

use crate::artifact::{
    audit_plan::{parse_audit_plan_json, AUDIT_PLAN_FILENAME, SCHEMA as AUDIT_PLAN_SCHEMA},
    load_artifact_manifest, ArtifactManifest, ClaimStatus,
};

use super::claims::{
    lookup_claim, transfer_audit_forbidden_supported_claim_ids, transfer_audit_supported_claim_ids,
    transfer_audit_unsupported_claim_ids, CLAIM_TRANSFER_ACTIVITY_RECONSTRUCTIBLE,
};
use super::contracts::{
    CANONICAL_TRANSFERS_FILENAME, CANONICAL_TRANSFERS_SCHEMA, CHAIN_WINDOWS_FILENAME,
    CHAIN_WINDOWS_SCHEMA, DEPLOYMENT_REGISTRY_FILENAME, DEPLOYMENT_REGISTRY_SCHEMA,
    EVIDENCE_SOURCES_FILENAME, EVIDENCE_SOURCES_SCHEMA, SUPPLY_SNAPSHOTS_FILENAME,
    SUPPLY_SNAPSHOTS_SCHEMA,
};

/// Pipeline stage ids (documentation + tests).
pub const STAGE_AUDIT_PLAN: &str = "audit_plan";
pub const STAGE_EVIDENCE_COLLECTION: &str = "evidence_collection";
pub const STAGE_CANONICAL_TABLES: &str = "canonical_audit_tables";
pub const STAGE_AUDIT_ENGINE: &str = "audit_engine";
pub const STAGE_CLAIM_REGISTRY: &str = "claim_registry";
pub const STAGE_ARTIFACT_MANIFEST: &str = "artifact_manifest";
pub const STAGE_PRODUCT_DELIVERY: &str = "product_delivery";

const CANONICAL_PRODUCT_ARTIFACTS: &[(&str, &str)] = &[
    (EVIDENCE_SOURCES_FILENAME, EVIDENCE_SOURCES_SCHEMA),
    (DEPLOYMENT_REGISTRY_FILENAME, DEPLOYMENT_REGISTRY_SCHEMA),
    (CHAIN_WINDOWS_FILENAME, CHAIN_WINDOWS_SCHEMA),
    (CANONICAL_TRANSFERS_FILENAME, CANONICAL_TRANSFERS_SCHEMA),
    (SUPPLY_SNAPSHOTS_FILENAME, SUPPLY_SNAPSHOTS_SCHEMA),
];

/// Validate that a completed transfer-audit run satisfies the v0 product pipeline contract.
pub fn validate_transfer_audit_product_run(out_dir: &Path) -> Result<()> {
    validate_audit_plan_stage(out_dir)?;
    validate_canonical_tables_stage(out_dir)?;
    let manifest = load_artifact_manifest(out_dir).context("artifact manifest stage")?;
    validate_manifest_stage(out_dir, &manifest)?;
    validate_claim_registry_stage(&manifest)?;
    Ok(())
}

fn validate_audit_plan_stage(out_dir: &Path) -> Result<()> {
    let path = out_dir.join(AUDIT_PLAN_FILENAME);
    anyhow::ensure!(
        path.is_file(),
        "missing {AUDIT_PLAN_FILENAME} ({STAGE_AUDIT_PLAN})"
    );
    let text = std::fs::read_to_string(&path)?;
    let plan = parse_audit_plan_json(&text)?;
    anyhow::ensure!(
        plan.schema == AUDIT_PLAN_SCHEMA,
        "audit plan schema must be {AUDIT_PLAN_SCHEMA}"
    );
    Ok(())
}

fn validate_canonical_tables_stage(out_dir: &Path) -> Result<()> {
    for (file, _schema) in CANONICAL_PRODUCT_ARTIFACTS {
        anyhow::ensure!(
            out_dir.join(file).is_file(),
            "missing canonical artifact {file} ({STAGE_CANONICAL_TABLES})"
        );
    }
    Ok(())
}

fn validate_manifest_stage(out_dir: &Path, manifest: &ArtifactManifest) -> Result<()> {
    anyhow::ensure!(
        manifest.schema == crate::artifact::SCHEMA,
        "artifact manifest schema mismatch"
    );
    for (file, schema_id) in CANONICAL_PRODUCT_ARTIFACTS {
        let artifact = manifest
            .artifacts
            .iter()
            .find(|a| a.path == *file)
            .with_context(|| format!("manifest missing declared artifact {file}"))?;
        anyhow::ensure!(
            artifact
                .checksum_sha256
                .as_ref()
                .is_some_and(|s| !s.is_empty()),
            "manifest artifact {file} missing checksum_sha256"
        );
        anyhow::ensure!(
            artifact.schema.as_deref() == Some(*schema_id),
            "manifest artifact {file} schema must be {schema_id}"
        );
        anyhow::ensure!(
            out_dir.join(file).is_file(),
            "manifest artifact {file} not on disk"
        );
    }
    Ok(())
}

fn validate_claim_registry_stage(manifest: &ArtifactManifest) -> Result<()> {
    for id in transfer_audit_supported_claim_ids() {
        let def = lookup_claim(id).with_context(|| format!("unknown supported claim {id}"))?;
        anyhow::ensure!(
            def.produced_by == "transfer-audit",
            "supported claim {id} must be produced by transfer-audit"
        );
        let claim = manifest
            .supported_claims
            .iter()
            .find(|c| c.claim == *id)
            .with_context(|| format!("manifest missing supported claim {id}"))?;
        anyhow::ensure!(
            !claim.statement.is_empty(),
            "supported claim {id} must use catalog statement"
        );
        anyhow::ensure!(
            claim.status == ClaimStatus::Supported || claim.status == ClaimStatus::Conditional,
            "supported claim {id} has wrong status"
        );
    }

    for id in transfer_audit_unsupported_claim_ids() {
        let claim = manifest
            .unsupported_claims
            .iter()
            .find(|c| c.claim == *id)
            .with_context(|| format!("manifest missing unsupported claim {id}"))?;
        anyhow::ensure!(
            claim.status == ClaimStatus::Unsupported,
            "claim {id} must be unsupported"
        );
        anyhow::ensure!(
            claim.evidence_artifacts.is_empty(),
            "unsupported claim {id} must not cite evidence"
        );
    }

    for id in transfer_audit_forbidden_supported_claim_ids() {
        anyhow::ensure!(
            !manifest.supported_claims.iter().any(|c| c.claim == *id),
            "forbidden claim {id} must not appear in supported_claims"
        );
    }

    for claim in &manifest.supported_claims {
        let def = lookup_claim(&claim.claim).with_context(|| {
            format!(
                "supported claim {:?} not in central catalog — do not hand-write claims",
                claim.claim
            )
        })?;
        for evidence in &claim.evidence_artifacts {
            anyhow::ensure!(
                manifest.artifacts.iter().any(|a| a.path == *evidence),
                "claim {:?} evidence {:?} not listed in manifest.artifacts",
                claim.claim,
                evidence
            );
        }
        if claim.claim == CLAIM_TRANSFER_ACTIVITY_RECONSTRUCTIBLE {
            anyhow::ensure!(
                !claim.evidence_artifacts.is_empty(),
                "transfer_activity_reconstructible requires run-specific evidence paths"
            );
        }
        if !def.limitations.is_empty() {
            anyhow::ensure!(
                !claim.limitations.is_empty(),
                "claim {:?} must include catalog limitations",
                claim.claim
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::transfer_audit_manifest::{
        write_transfer_audit_manifest, TransferAuditManifestParams,
    };

    fn seed_pipeline_run(out: &Path) -> TransferAuditManifestParams {
        use std::io::Write;

        for (name, body) in [
            ("qa_report.json", b"{}".as_slice()),
            ("provenance.json", b"{}".as_slice()),
            ("supply_audit.md", b"#".as_slice()),
            ("summary.md", b"#".as_slice()),
        ] {
            std::fs::write(out.join(name), body).unwrap();
        }
        let mut w = std::fs::File::create(out.join("supply_audit.csv")).unwrap();
        writeln!(w, "chain,chain_id").unwrap();
        writeln!(w, "ethereum,1").unwrap();
        let mut w = std::fs::File::create(out.join("decoded_transfers.csv")).unwrap();
        writeln!(w, "chain,block_number").unwrap();

        for (file, body) in [
            (
                EVIDENCE_SOURCES_FILENAME,
                r#"{"schema":"evidence-sources-v0","sources":[]}"#,
            ),
            (
                DEPLOYMENT_REGISTRY_FILENAME,
                r#"{"schema":"deployment-registry-v0","asset":"USDC","run_id":"run_pipeline","deployments":[]}"#,
            ),
            (
                CHAIN_WINDOWS_FILENAME,
                r#"{"schema":"chain-windows-v0","asset":"USDC","run_id":"run_pipeline","windows":[]}"#,
            ),
        ] {
            std::fs::write(out.join(file), body).unwrap();
        }
        std::fs::write(out.join(CANONICAL_TRANSFERS_FILENAME), "chain,chain_id\n").unwrap();
        std::fs::write(out.join(SUPPLY_SNAPSHOTS_FILENAME), "chain,chain_id\n").unwrap();

        TransferAuditManifestParams {
            asset: "USDC".into(),
            run_id: "run_pipeline".into(),
            generated_at: "2026-05-15T08:03:31.695921+00:00".into(),
            per_chain_spans: true,
            provenance_from_block: 100,
            provenance_to_block_requested: None,
            chains: vec![
                crate::artifact::transfer_audit_manifest::ManifestChainInput {
                    chain: "ethereum".into(),
                    contract_address: "0xabc".into(),
                    from_block: 100,
                    to_block_requested: "200".into(),
                    window_start_rfc3339: Some("2026-05-01T00:00:00Z".into()),
                    window_end_rfc3339: Some("2026-05-08T00:00:00Z".into()),
                    errors: vec![],
                },
            ],
            warnings: vec![],
        }
    }

    #[test]
    fn validate_transfer_audit_product_run_accepts_complete_pipeline() {
        let out = std::env::temp_dir().join(format!("stablecoin_pipeline_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let params = seed_pipeline_run(&out);
        write_transfer_audit_manifest(&out, &params).unwrap();
        validate_transfer_audit_product_run(&out).unwrap();
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn validate_rejects_missing_canonical_artifact() {
        let out =
            std::env::temp_dir().join(format!("stablecoin_pipeline_bad_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let params = seed_pipeline_run(&out);
        write_transfer_audit_manifest(&out, &params).unwrap();
        std::fs::remove_file(out.join(EVIDENCE_SOURCES_FILENAME)).unwrap();
        assert!(validate_transfer_audit_product_run(&out).is_err());
        let _ = std::fs::remove_dir_all(&out);
    }
}
