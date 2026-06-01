pub mod application;
pub mod artifact;
pub mod audit;
pub mod cli;
pub mod config;
pub mod decode;
pub mod domain;
pub mod fetch;
pub mod report;
pub mod rpc;
pub mod stablecoin_map;

#[cfg(feature = "api")]
pub mod api;

#[cfg(feature = "experimental")]
pub mod control_events;

pub use artifact::manifest::{
    ArtifactFormat, ArtifactKind, ArtifactManifest, ArtifactRef, ClaimBoundary, ClaimStatus,
    InputRef, SourceSnapshot, SCHEMA as ARTIFACT_MANIFEST_SCHEMA,
};
pub use artifact::{write_artifact_manifest, write_manifest, MANIFEST_FILENAME};
pub use domain::asset::validate_identifier;
pub use report::{default_run_id, ensure_run_out_dir, validate_run_id};

/// CLI entry (used by the binary and integration tests).
pub fn run_cli<I, S>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    cli::run(args)
}

#[cfg(test)]
mod tests {
    use super::run_cli;
    use super::validate_identifier;
    use std::path::Path;

    #[test]
    fn validate_identifier_accepts_usdc() {
        validate_identifier("USDC", "--asset").unwrap();
    }

    #[test]
    fn validate_identifier_rejects_slash() {
        assert!(validate_identifier("a/b", "--asset").is_err());
    }

    #[test]
    fn cli_rejects_chunk_size_zero() {
        let err = run_cli([
            "stablecoin-audit",
            "transfer-audit",
            "--from-block",
            "100",
            "--to-block",
            "200",
            "--chunk-size",
            "0",
        ]);
        assert!(err.is_err());
    }

    #[test]
    fn cli_rejects_window_with_from_block() {
        assert!(run_cli([
            "stablecoin-audit",
            "transfer-audit",
            "--from-block",
            "1",
            "--window",
            "ethereum:100:200",
        ])
        .is_err());
    }

    #[test]
    fn cli_metadata_rejects_zero_from_block() {
        assert!(run_cli([
            "stablecoin-audit",
            "metadata",
            "--from-block",
            "0",
            "--chains",
            "ethereum",
        ])
        .is_err());
    }

    #[test]
    fn cli_resolve_window_rejects_inverted_range() {
        assert!(run_cli([
            "stablecoin-audit",
            "resolve-window",
            "--from",
            "2026-05-08T00:00:00Z",
            "--to",
            "2026-05-01T00:00:00Z",
            "--chains",
            "ethereum",
        ])
        .is_err());
    }

    #[test]
    fn cli_transfer_audit_rejects_from_block_zero() {
        assert!(run_cli([
            "stablecoin-audit",
            "transfer-audit",
            "--from-block",
            "0",
            "--to-block",
            "100",
        ])
        .is_err());
    }

    #[test]
    fn cross_chain_summary_from_benchmark_fixture() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/benchmarks/usdc_7d_20260501_20260508");
        let run_id = format!("itest_{}", std::process::id());
        let out_dir = crate::ensure_run_out_dir("USDC", &run_id).unwrap();
        for name in [
            "supply_audit.csv",
            "provenance.json",
            "summary.md",
            "supply_audit.md",
        ] {
            std::fs::copy(fixture.join(name), out_dir.join(name)).unwrap();
        }
        let mut qa: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(fixture.join("qa_report.json")).unwrap())
                .unwrap();
        qa["run_id"] = serde_json::Value::String(run_id.clone());
        std::fs::write(
            out_dir.join("qa_report.json"),
            serde_json::to_string_pretty(&qa).unwrap(),
        )
        .unwrap();
        std::fs::write(out_dir.join("decoded_transfers.csv"), "chain\n").unwrap();
        crate::artifact::write_transfer_audit_manifest(
            &out_dir,
            &crate::artifact::TransferAuditManifestParams {
                asset: "USDC".into(),
                run_id: run_id.clone(),
                generated_at: "2026-05-15T08:03:31.695921+00:00".into(),
                per_chain_spans: true,
                provenance_from_block: 24996368,
                provenance_to_block_requested: None,
                chains: vec![],
                warnings: vec![],
            },
        )
        .unwrap();
        run_cli([
            "stablecoin-audit",
            "cross-chain-summary",
            "--asset",
            "USDC",
            "--run-id",
            &run_id,
        ])
        .unwrap();
        assert!(out_dir.join("cross_chain_summary.json").is_file());
        assert!(out_dir.join("cross_chain_summary.md").is_file());
        let manifest: crate::artifact::ArtifactManifest = serde_json::from_str(
            &std::fs::read_to_string(out_dir.join("artifact_manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.command, "transfer-audit");
        assert!(manifest
            .artifacts
            .iter()
            .any(|a| a.path == "cross_chain_summary.json"));
        assert!(manifest
            .workflow_steps
            .iter()
            .any(|s| s.command == "cross-chain-summary"));
        let _ = std::fs::remove_dir_all(&out_dir);
    }
}
