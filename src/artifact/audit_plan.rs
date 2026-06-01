//! `audit_plan.json` — explicit audit scope and requested checks for a run.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "audit-plan-v0";
pub const AUDIT_PLAN_FILENAME: &str = "audit_plan.json";

/// Machine-readable audit scope contract for a toolkit run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditPlan {
    pub schema: String,
    pub asset: String,
    pub run_id: String,
    pub audit_window: AuditWindow,
    pub deployments: Vec<DeploymentScope>,
    pub requested_checks: Vec<String>,
    pub out_of_scope: Vec<String>,
    pub data_sources: Vec<DataSourceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditWindow {
    pub per_chain_spans: bool,
    pub chains: Vec<ChainWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChainWindow {
    pub chain: String,
    pub from_block: u64,
    pub to_block_requested: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeploymentScope {
    pub chain: String,
    pub contract_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataSourceRef {
    pub source_name: String,
    pub source_type: String,
}

/// Parse and validate audit plan JSON (schema id, required fields via serde).
pub fn parse_audit_plan_json(text: &str) -> Result<AuditPlan> {
    let plan: AuditPlan = serde_json::from_str(text).context("deserialize audit plan JSON")?;
    anyhow::ensure!(
        plan.schema == SCHEMA,
        "audit plan schema must be {SCHEMA}, got {:?}",
        plan.schema
    );
    Ok(plan)
}

/// Read `audit_plan.json` from `out_dir`.
pub fn load_audit_plan(out_dir: &Path) -> Result<AuditPlan> {
    let path = out_dir.join(AUDIT_PLAN_FILENAME);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    parse_audit_plan_json(&text).with_context(|| format!("parse {}", path.display()))
}

/// Write `audit_plan.json` into `out_dir`.
pub fn write_audit_plan(out_dir: &Path, plan: &AuditPlan) -> Result<()> {
    anyhow::ensure!(
        plan.schema == SCHEMA,
        "audit plan schema must be {SCHEMA}, got {:?}",
        plan.schema
    );
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("create audit plan dir {}", out_dir.display()))?;
    let path = out_dir.join(AUDIT_PLAN_FILENAME);
    std::fs::write(
        &path,
        serde_json::to_string_pretty(plan).context("serialize audit plan")?,
    )
    .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plan() -> AuditPlan {
        AuditPlan {
            schema: SCHEMA.to_string(),
            asset: "USDC".into(),
            run_id: "run_plan_test".into(),
            audit_window: AuditWindow {
                per_chain_spans: true,
                chains: vec![ChainWindow {
                    chain: "ethereum".into(),
                    from_block: 100,
                    to_block_requested: "200".into(),
                    window_start: Some("2026-05-01T00:00:00Z".into()),
                    window_end: Some("2026-05-08T00:00:00Z".into()),
                }],
            },
            deployments: vec![DeploymentScope {
                chain: "ethereum".into(),
                contract_address: "0xabc".into(),
            }],
            requested_checks: vec![
                "transfer_log_fetch".into(),
                "supply_invariant_per_chain".into(),
            ],
            out_of_scope: vec![crate::audit::CLAIM_FIAT_RESERVE_NOT_VERIFIED.into()],
            data_sources: vec![DataSourceRef {
                source_name: "rpc:ethereum".into(),
                source_type: "evm_rpc".into(),
            }],
        }
    }

    #[test]
    fn audit_plan_serializes_and_deserializes() {
        let plan = sample_plan();
        let json = serde_json::to_string_pretty(&plan).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema"], SCHEMA);
        let back: AuditPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);
    }

    #[test]
    fn parse_rejects_wrong_schema() {
        let json = r#"{
          "schema": "wrong-schema",
          "asset": "USDC",
          "run_id": "run_x",
          "audit_window": { "per_chain_spans": false, "chains": [] },
          "deployments": [],
          "requested_checks": [],
          "out_of_scope": [],
          "data_sources": []
        }"#;
        assert!(parse_audit_plan_json(json).is_err());
    }

    #[test]
    fn write_and_load_roundtrip() {
        let dir =
            std::env::temp_dir().join(format!("stablecoin_audit_plan_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plan = sample_plan();
        write_audit_plan(&dir, &plan).unwrap();
        assert!(dir.join(AUDIT_PLAN_FILENAME).is_file());
        let loaded = load_audit_plan(&dir).unwrap();
        assert_eq!(loaded, plan);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
