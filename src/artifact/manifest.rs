use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "artifact-manifest-v0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactManifest {
    pub schema: String,
    pub toolkit_version: String,
    pub generated_at: DateTime<Utc>,
    pub command: String,
    pub run_id: Option<String>,
    pub package_id: Option<String>,
    pub asset: Option<String>,
    pub inputs: Vec<InputRef>,
    pub artifacts: Vec<ArtifactRef>,
    pub source_snapshots: Vec<SourceSnapshot>,
    pub supported_claims: Vec<ClaimBoundary>,
    pub unsupported_claims: Vec<ClaimBoundary>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub workflow_steps: Vec<WorkflowStep>,
}

/// One completed toolkit command contributing to a run's evidence package.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowStep {
    pub command: String,
    pub completed_at: DateTime<Utc>,
    /// Relative artifact paths produced by this step.
    pub artifacts: Vec<String>,
    pub warnings: Vec<String>,
}

impl ArtifactManifest {
    pub fn new(command: impl Into<String>, toolkit_version: impl Into<String>) -> Self {
        Self {
            schema: SCHEMA.to_string(),
            toolkit_version: toolkit_version.into(),
            generated_at: Utc::now(),
            command: command.into(),
            run_id: None,
            package_id: None,
            asset: None,
            inputs: Vec::new(),
            artifacts: Vec::new(),
            source_snapshots: Vec::new(),
            supported_claims: Vec::new(),
            unsupported_claims: Vec::new(),
            warnings: Vec::new(),
            workflow_steps: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputRef {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactRef {
    pub kind: ArtifactKind,
    pub path: String,
    pub format: ArtifactFormat,
    pub row_count: Option<u64>,
    pub checksum_sha256: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Provenance,
    QaReport,
    SupplyAudit,
    TransferLog,
    Summary,
    CrossChainSummary,
    Checkpoint,
    Metadata,
    MapPackage,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactFormat {
    Csv,
    Json,
    Markdown,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSnapshot {
    pub source_name: String,
    pub source_url: Option<String>,
    pub retrieved_at: Option<DateTime<Utc>>,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimBoundary {
    pub claim: String,
    pub status: ClaimStatus,
    pub evidence_artifacts: Vec<String>,
    pub caveat: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Supported,
    Unsupported,
    Conditional,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_serializes_with_schema_field() {
        let manifest = ArtifactManifest::new("transfer-audit", "0.1.0");
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema"], SCHEMA);
        assert_eq!(parsed["command"], "transfer-audit");
        assert!(parsed["package_id"].is_null());
        assert!(parsed["run_id"].is_null());
        assert!(parsed["asset"].is_null());
        let roundtrip: ArtifactManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.schema, SCHEMA);
        assert_eq!(roundtrip.command, "transfer-audit");
    }

    #[test]
    fn artifact_ref_serializes_null_optional_fields() {
        let artifact = ArtifactRef {
            kind: ArtifactKind::QaReport,
            path: "qa_report.json".into(),
            format: ArtifactFormat::Json,
            row_count: None,
            checksum_sha256: None,
            description: "QA gates".into(),
        };
        let v: serde_json::Value = serde_json::to_value(&artifact).unwrap();
        assert!(v["row_count"].is_null());
        assert!(v["checksum_sha256"].is_null());
    }

    #[test]
    fn manifest_deserializes_and_preserves_required_fields() {
        let manifest = ArtifactManifest {
            run_id: Some("run_abc".into()),
            asset: Some("USDC".into()),
            inputs: vec![InputRef {
                name: "window".into(),
                value: "ethereum:1:2".into(),
            }],
            ..ArtifactManifest::new("transfer-audit", "0.2.0")
        };
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let back: ArtifactManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.schema, SCHEMA);
        assert_eq!(back.toolkit_version, "0.2.0");
        assert_eq!(back.command, "transfer-audit");
        assert_eq!(back.run_id.as_deref(), Some("run_abc"));
        assert_eq!(back.asset.as_deref(), Some("USDC"));
        assert_eq!(back.inputs.len(), 1);
    }

    #[test]
    fn claim_boundary_roundtrip() {
        let claim = ClaimBoundary {
            claim: "supply_invariant_per_chain".into(),
            status: ClaimStatus::Conditional,
            evidence_artifacts: vec!["supply_audit.csv".into()],
            caveat: "FAIL is not proof of fraud.".into(),
        };
        let manifest = ArtifactManifest {
            unsupported_claims: vec![ClaimBoundary {
                claim: "reserve_backing".into(),
                status: ClaimStatus::Unsupported,
                evidence_artifacts: vec![],
                caveat: "Out of scope.".into(),
            }],
            supported_claims: vec![claim.clone()],
            ..ArtifactManifest::new("transfer-audit", "0.1.0")
        };
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let back: ArtifactManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.supported_claims, vec![claim]);
    }

    #[test]
    fn legacy_manifest_without_checksum_deserializes() {
        let json = r#"{
          "schema": "artifact-manifest-v0",
          "toolkit_version": "0.1.0",
          "generated_at": "2026-05-15T08:00:00+00:00",
          "command": "transfer-audit",
          "run_id": "legacy_run",
          "package_id": null,
          "asset": "USDC",
          "inputs": [],
          "artifacts": [{
            "kind": "qa_report",
            "path": "qa_report.json",
            "format": "json",
            "row_count": null,
            "description": "QA gates"
          }],
          "source_snapshots": [],
          "supported_claims": [],
          "unsupported_claims": [],
          "warnings": []
        }"#;
        let m: ArtifactManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.artifacts.len(), 1);
        assert_eq!(m.artifacts[0].checksum_sha256, None);
    }
}
