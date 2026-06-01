//! Stable audit-domain contracts (canonical tables and evidence metadata).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const EVIDENCE_SOURCES_SCHEMA: &str = "evidence-sources-v0";
pub const DEPLOYMENT_REGISTRY_SCHEMA: &str = "deployment-registry-v0";
pub const CHAIN_WINDOWS_SCHEMA: &str = "chain-windows-v0";
pub const CANONICAL_TRANSFERS_SCHEMA: &str = "canonical-transfers-v0";
pub const SUPPLY_SNAPSHOTS_SCHEMA: &str = "supply-snapshots-v0";

pub const EVIDENCE_SOURCES_FILENAME: &str = "evidence_sources.json";
pub const DEPLOYMENT_REGISTRY_FILENAME: &str = "deployment_registry.json";
pub const CHAIN_WINDOWS_FILENAME: &str = "chain_windows.json";
pub const CANONICAL_TRANSFERS_FILENAME: &str = "canonical_transfers.csv";
pub const SUPPLY_SNAPSHOTS_FILENAME: &str = "supply_snapshots.csv";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Rpc,
    ExplorerApi,
    ContractCall,
    Derived,
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentRole {
    Canonical,
    Bridged,
    Wrapped,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferEventType {
    Transfer,
    Mint,
    Burn,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockRange {
    pub start_block: u64,
    pub end_block: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimestampRange {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceSource {
    pub source_id: String,
    pub source_type: SourceType,
    pub chain: String,
    pub chain_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpc_url_redacted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_range: Option<BlockRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_range: Option<TimestampRange>,
    pub captured_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceSourcesDocument {
    pub schema: String,
    pub sources: Vec<EvidenceSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeploymentEntry {
    pub chain: String,
    pub chain_id: u64,
    pub address: String,
    pub token_standard: String,
    pub decimals: u8,
    pub symbol: String,
    pub role: DeploymentRole,
    pub evidence_source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeploymentRegistry {
    pub schema: String,
    pub asset: String,
    pub run_id: String,
    pub deployments: Vec<DeploymentEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChainWindowEntry {
    pub chain: String,
    pub chain_id: u64,
    pub start_block: u64,
    pub end_block: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_timestamp: Option<String>,
    pub evidence_source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChainWindowsDocument {
    pub schema: String,
    pub asset: String,
    pub run_id: String,
    pub windows: Vec<ChainWindowEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalTransferRecord {
    pub chain: String,
    pub chain_id: u64,
    pub block_number: u64,
    #[serde(default)]
    pub block_timestamp: Option<String>,
    pub tx_hash: String,
    pub log_index: u64,
    pub contract_address: String,
    pub from_address: String,
    pub to_address: String,
    pub raw_amount: String,
    pub normalized_amount: String,
    pub decimals: u8,
    pub event_type: TransferEventType,
    #[serde(default)]
    pub evidence_source_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SupplySnapshotRecord {
    pub chain: String,
    pub chain_id: u64,
    pub contract_address: String,
    pub block_number: u64,
    #[serde(default)]
    pub block_timestamp: Option<String>,
    pub raw_total_supply: String,
    pub normalized_total_supply: String,
    pub decimals: u8,
    pub method: String,
    #[serde(default)]
    pub evidence_source_id: Option<String>,
}

pub fn transfer_logs_source_id(run_id: &str, chain: &str) -> String {
    format!("{run_id}:rpc:{chain}:transfer_logs")
}

pub fn total_supply_source_id(run_id: &str, chain: &str) -> String {
    format!("{run_id}:contract_call:{chain}:totalSupply")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_sources_document_roundtrip() {
        let doc = EvidenceSourcesDocument {
            schema: EVIDENCE_SOURCES_SCHEMA.to_string(),
            sources: vec![EvidenceSource {
                source_id: "run:rpc:ethereum:transfer_logs".into(),
                source_type: SourceType::Rpc,
                chain: "ethereum".into(),
                chain_id: 1,
                provider_name: Some("alchemy".into()),
                rpc_url_redacted: None,
                block_range: Some(BlockRange {
                    start_block: 100,
                    end_block: 200,
                }),
                timestamp_range: None,
                captured_at: Utc::now(),
                notes: None,
            }],
        };
        let json = serde_json::to_string_pretty(&doc).unwrap();
        let back: EvidenceSourcesDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(back.schema, EVIDENCE_SOURCES_SCHEMA);
        assert_eq!(back.sources.len(), 1);
    }

    #[test]
    fn deployment_registry_schema_field() {
        let reg = DeploymentRegistry {
            schema: DEPLOYMENT_REGISTRY_SCHEMA.to_string(),
            asset: "USDC".into(),
            run_id: "run_test".into(),
            deployments: vec![DeploymentEntry {
                chain: "ethereum".into(),
                chain_id: 1,
                address: "0xabc".into(),
                token_standard: "ERC-20".into(),
                decimals: 6,
                symbol: "USDC".into(),
                role: DeploymentRole::Canonical,
                evidence_source_ids: vec!["run:rpc:ethereum:transfer_logs".into()],
            }],
        };
        let json = serde_json::to_string_pretty(&reg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema"], DEPLOYMENT_REGISTRY_SCHEMA);
    }

    #[test]
    fn chain_windows_schema_field() {
        let doc = ChainWindowsDocument {
            schema: CHAIN_WINDOWS_SCHEMA.to_string(),
            asset: "USDC".into(),
            run_id: "run_test".into(),
            windows: vec![ChainWindowEntry {
                chain: "ethereum".into(),
                chain_id: 1,
                start_block: 100,
                end_block: 200,
                start_timestamp: Some("2026-05-01T00:00:00Z".into()),
                end_timestamp: Some("2026-05-08T00:00:00Z".into()),
                evidence_source_ids: vec!["run:rpc:ethereum:transfer_logs".into()],
            }],
        };
        let json = serde_json::to_string_pretty(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema"], CHAIN_WINDOWS_SCHEMA);
    }
}
