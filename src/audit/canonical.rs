//! Write canonical audit table artifacts from transfer-audit run data.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use crate::config::load_single_token_config;
use crate::decode::TransferEvent;

use super::contracts::{
    total_supply_source_id, transfer_logs_source_id, CanonicalTransferRecord, ChainWindowEntry,
    ChainWindowsDocument, DeploymentEntry, DeploymentRegistry, DeploymentRole, EvidenceSource,
    EvidenceSourcesDocument, SourceType, SupplySnapshotRecord, TimestampRange, TransferEventType,
    CANONICAL_TRANSFERS_FILENAME, CHAIN_WINDOWS_FILENAME, CHAIN_WINDOWS_SCHEMA,
    DEPLOYMENT_REGISTRY_FILENAME, DEPLOYMENT_REGISTRY_SCHEMA, EVIDENCE_SOURCES_FILENAME,
    EVIDENCE_SOURCES_SCHEMA, SUPPLY_SNAPSHOTS_FILENAME,
};
use super::supply::SupplyAuditRow;

pub struct CanonicalWriteParams<'a> {
    pub asset: &'a str,
    pub run_id: &'a str,
    pub captured_at: &'a str,
    pub events: &'a [TransferEvent],
    pub supply_rows: &'a [SupplyAuditRow],
}

pub fn write_canonical_audit_tables(
    out_dir: &Path,
    params: &CanonicalWriteParams<'_>,
) -> Result<()> {
    let captured_at = parse_captured_at(params.captured_at)?;
    let sources = build_evidence_sources(params, captured_at)?;
    let deployments = build_deployment_registry(params, &sources)?;
    let windows = build_chain_windows(params, &sources)?;
    let transfers = build_canonical_transfers(params, &sources)?;
    let snapshots = build_supply_snapshots(params, &sources)?;

    write_evidence_sources(out_dir, &sources)?;
    write_deployment_registry(out_dir, &deployments)?;
    write_chain_windows(out_dir, &windows)?;
    write_canonical_transfers_csv(out_dir, &transfers)?;
    write_supply_snapshots_csv(out_dir, &snapshots)?;
    Ok(())
}

fn parse_captured_at(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .with_context(|| format!("parse captured_at {:?}", s))
}

fn build_evidence_sources(
    params: &CanonicalWriteParams<'_>,
    captured_at: DateTime<Utc>,
) -> Result<EvidenceSourcesDocument> {
    let mut sources = Vec::new();
    for row in params.supply_rows {
        let end_block = row.resolved_to_block.unwrap_or(row.from_block);
        let logs_id = transfer_logs_source_id(params.run_id, &row.chain);
        sources.push(EvidenceSource {
            source_id: logs_id.clone(),
            source_type: SourceType::Rpc,
            chain: row.chain.clone(),
            chain_id: row.chain_id,
            provider_name: provider_name_for_chain(params.asset, &row.chain).ok(),
            rpc_url_redacted: None,
            block_range: Some(super::contracts::BlockRange {
                start_block: row.from_block,
                end_block,
            }),
            timestamp_range: match (
                row.window_start_block_timestamp_rfc3339.as_deref(),
                row.window_end_block_timestamp_rfc3339.as_deref(),
            ) {
                (Some(start), Some(end)) => Some(TimestampRange {
                    start: start.to_string(),
                    end: end.to_string(),
                }),
                _ => None,
            },
            captured_at,
            notes: Some("eth_getLogs Transfer events in audit window".into()),
        });
        sources.push(EvidenceSource {
            source_id: total_supply_source_id(params.run_id, &row.chain),
            source_type: SourceType::ContractCall,
            chain: row.chain.clone(),
            chain_id: row.chain_id,
            provider_name: provider_name_for_chain(params.asset, &row.chain).ok(),
            rpc_url_redacted: None,
            block_range: None,
            timestamp_range: None,
            captured_at,
            notes: Some("totalSupply() at pinned window boundaries".into()),
        });
    }
    Ok(EvidenceSourcesDocument {
        schema: EVIDENCE_SOURCES_SCHEMA.to_string(),
        sources,
    })
}

fn provider_name_for_chain(asset: &str, chain: &str) -> Result<String> {
    let cfg = load_single_token_config(asset, chain)?;
    Ok(cfg.rpc_url_env.replace("_URL", "").to_lowercase())
}

fn build_deployment_registry(
    params: &CanonicalWriteParams<'_>,
    sources: &EvidenceSourcesDocument,
) -> Result<DeploymentRegistry> {
    let mut deployments = Vec::new();
    for row in params.supply_rows {
        let cfg = load_single_token_config(params.asset, &row.chain)?;
        let logs_id = transfer_logs_source_id(params.run_id, &row.chain);
        let supply_id = total_supply_source_id(params.run_id, &row.chain);
        let evidence_source_ids: Vec<String> = sources
            .sources
            .iter()
            .filter(|s| s.source_id == logs_id || s.source_id == supply_id)
            .map(|s| s.source_id.clone())
            .collect();
        deployments.push(DeploymentEntry {
            chain: row.chain.clone(),
            chain_id: row.chain_id,
            address: row.contract_address.clone(),
            token_standard: token_standard_from_config(&cfg),
            decimals: cfg.decimals,
            symbol: cfg.asset.to_uppercase(),
            role: DeploymentRole::Canonical,
            evidence_source_ids,
        });
    }
    Ok(DeploymentRegistry {
        schema: DEPLOYMENT_REGISTRY_SCHEMA.to_string(),
        asset: params.asset.to_uppercase(),
        run_id: params.run_id.to_string(),
        deployments,
    })
}

fn token_standard_from_config(_cfg: &crate::config::TokenConfig) -> String {
    "ERC-20".to_string()
}

fn build_chain_windows(
    params: &CanonicalWriteParams<'_>,
    _sources: &EvidenceSourcesDocument,
) -> Result<ChainWindowsDocument> {
    let windows = params
        .supply_rows
        .iter()
        .filter_map(|row| {
            let end_block = row.resolved_to_block?;
            Some(ChainWindowEntry {
                chain: row.chain.clone(),
                chain_id: row.chain_id,
                start_block: row.from_block,
                end_block,
                start_timestamp: row.window_start_block_timestamp_rfc3339.clone(),
                end_timestamp: row.window_end_block_timestamp_rfc3339.clone(),
                evidence_source_ids: vec![transfer_logs_source_id(params.run_id, &row.chain)],
            })
        })
        .collect();
    Ok(ChainWindowsDocument {
        schema: CHAIN_WINDOWS_SCHEMA.to_string(),
        asset: params.asset.to_uppercase(),
        run_id: params.run_id.to_string(),
        windows,
    })
}

fn build_canonical_transfers(
    params: &CanonicalWriteParams<'_>,
    _sources: &EvidenceSourcesDocument,
) -> Result<Vec<CanonicalTransferRecord>> {
    let chain_ids: std::collections::HashMap<&str, u64> = params
        .supply_rows
        .iter()
        .map(|r| (r.chain.as_str(), r.chain_id))
        .collect();
    let decimals: std::collections::HashMap<&str, u8> = params
        .supply_rows
        .iter()
        .filter_map(|r| {
            load_single_token_config(params.asset, &r.chain)
                .ok()
                .map(|c| (r.chain.as_str(), c.decimals))
        })
        .collect();

    Ok(params
        .events
        .iter()
        .map(|ev| {
            let chain_id = chain_ids.get(ev.chain.as_str()).copied().unwrap_or(0);
            let dec = decimals.get(ev.chain.as_str()).copied().unwrap_or(0);
            CanonicalTransferRecord {
                chain: ev.chain.clone(),
                chain_id,
                block_number: ev.block_number,
                block_timestamp: None,
                tx_hash: ev.tx_hash.clone(),
                log_index: ev.log_index,
                contract_address: ev.contract_address.clone(),
                from_address: ev.from.clone(),
                to_address: ev.to.clone(),
                raw_amount: ev.value_raw.clone(),
                normalized_amount: ev.value_decimal.clone(),
                decimals: dec,
                event_type: match ev.kind.as_str() {
                    "mint" => TransferEventType::Mint,
                    "burn" => TransferEventType::Burn,
                    "transfer" => TransferEventType::Transfer,
                    _ => TransferEventType::Unknown,
                },
                evidence_source_id: Some(transfer_logs_source_id(params.run_id, &ev.chain)),
            }
        })
        .collect())
}

fn build_supply_snapshots(
    params: &CanonicalWriteParams<'_>,
    _sources: &EvidenceSourcesDocument,
) -> Result<Vec<SupplySnapshotRecord>> {
    let mut snapshots = Vec::new();
    for row in params.supply_rows {
        let supply_source = total_supply_source_id(params.run_id, &row.chain);
        let dec = load_single_token_config(params.asset, &row.chain)
            .map(|c| c.decimals)
            .unwrap_or(0);
        if let (Some(raw), Some(norm)) = (
            row.total_supply_start_raw.as_deref(),
            row.total_supply_at_start_minus_1.as_deref(),
        ) {
            if row.from_block > 0 {
                snapshots.push(SupplySnapshotRecord {
                    chain: row.chain.clone(),
                    chain_id: row.chain_id,
                    contract_address: row.contract_address.clone(),
                    block_number: row.from_block.saturating_sub(1),
                    block_timestamp: row.total_supply_start_block_timestamp_rfc3339.clone(),
                    raw_total_supply: raw.to_string(),
                    normalized_total_supply: norm.to_string(),
                    decimals: dec,
                    method: "totalSupply".into(),
                    evidence_source_id: Some(supply_source.clone()),
                });
            }
        }
        if let (Some(raw), Some(norm), Some(end_block)) = (
            row.total_supply_end_raw.as_deref(),
            row.total_supply_at_end.as_deref(),
            row.resolved_to_block,
        ) {
            snapshots.push(SupplySnapshotRecord {
                chain: row.chain.clone(),
                chain_id: row.chain_id,
                contract_address: row.contract_address.clone(),
                block_number: end_block,
                block_timestamp: row.window_end_block_timestamp_rfc3339.clone(),
                raw_total_supply: raw.to_string(),
                normalized_total_supply: norm.to_string(),
                decimals: dec,
                method: "totalSupply".into(),
                evidence_source_id: Some(supply_source),
            });
        }
    }
    Ok(snapshots)
}

fn write_evidence_sources(out_dir: &Path, doc: &EvidenceSourcesDocument) -> Result<()> {
    std::fs::write(
        out_dir.join(EVIDENCE_SOURCES_FILENAME),
        serde_json::to_string_pretty(doc).context("serialize evidence_sources.json")?,
    )
    .context("write evidence_sources.json")
}

fn write_deployment_registry(out_dir: &Path, doc: &DeploymentRegistry) -> Result<()> {
    std::fs::write(
        out_dir.join(DEPLOYMENT_REGISTRY_FILENAME),
        serde_json::to_string_pretty(doc).context("serialize deployment_registry.json")?,
    )
    .context("write deployment_registry.json")
}

fn write_chain_windows(out_dir: &Path, doc: &ChainWindowsDocument) -> Result<()> {
    std::fs::write(
        out_dir.join(CHAIN_WINDOWS_FILENAME),
        serde_json::to_string_pretty(doc).context("serialize chain_windows.json")?,
    )
    .context("write chain_windows.json")
}

fn write_canonical_transfers_csv(out_dir: &Path, rows: &[CanonicalTransferRecord]) -> Result<()> {
    let path = out_dir.join(CANONICAL_TRANSFERS_FILENAME);
    let mut wtr = csv::Writer::from_path(&path)?;
    for row in rows {
        wtr.serialize(row)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_supply_snapshots_csv(out_dir: &Path, rows: &[SupplySnapshotRecord]) -> Result<()> {
    let path = out_dir.join(SUPPLY_SNAPSHOTS_FILENAME);
    let mut wtr = csv::Writer::from_path(&path)?;
    for row in rows {
        wtr.serialize(row)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn canonical_transfer_csv_headers() -> Vec<&'static str> {
    vec![
        "chain",
        "chain_id",
        "block_number",
        "block_timestamp",
        "tx_hash",
        "log_index",
        "contract_address",
        "from_address",
        "to_address",
        "raw_amount",
        "normalized_amount",
        "decimals",
        "event_type",
        "evidence_source_id",
    ]
}

pub fn supply_snapshot_csv_headers() -> Vec<&'static str> {
    vec![
        "chain",
        "chain_id",
        "contract_address",
        "block_number",
        "block_timestamp",
        "raw_total_supply",
        "normalized_total_supply",
        "decimals",
        "method",
        "evidence_source_id",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::TransferEvent;
    use alloy::primitives::U256;

    fn sample_row() -> SupplyAuditRow {
        SupplyAuditRow {
            chain: "ethereum".into(),
            chain_id: 1,
            contract_address: "0xabc".into(),
            from_block: 100,
            resolved_to_block: Some(200),
            to_block_requested: "200".into(),
            chunk_size: 500,
            transfer_event_count: 1,
            active_senders: 1,
            active_recipients: 1,
            mint_count: 1,
            burn_count: 0,
            plain_transfer_count: 0,
            sum_mints_raw: "1000000".into(),
            sum_burns_raw: "0".into(),
            net_mint_raw: Some("1000000".into()),
            total_supply_at_start_minus_1: Some("1.000000".into()),
            total_supply_at_start_minus_1_provenance: "on-chain".into(),
            total_supply_at_end: Some("2.000000".into()),
            onchain_delta_raw: Some("1000000".into()),
            discrepancy_raw: Some("0".into()),
            metadata_call_pass: true,
            historical_supply_pass: true,
            no_duplicate_logs_pass: Some(true),
            transfer_decode_pass: Some(true),
            supply_invariant_pass: Some(true),
            duplicate_count: 0,
            full_decode_error_count: 0,
            total_supply_start_raw: Some("1000000".into()),
            total_supply_end_raw: Some("2000000".into()),
            total_supply_start_block_timestamp_rfc3339: Some("2026-05-01T00:00:00Z".into()),
            window_start_block_timestamp_rfc3339: Some("2026-05-08T00:00:00Z".into()),
            window_end_block_timestamp_rfc3339: Some("2026-05-08T00:00:00Z".into()),
        }
    }

    #[test]
    fn start_supply_snapshot_uses_boundary_block_timestamp() {
        let row = SupplyAuditRow {
            chain: "ethereum".into(),
            chain_id: 1,
            contract_address: "0xabc".into(),
            from_block: 101,
            resolved_to_block: Some(200),
            to_block_requested: "200".into(),
            chunk_size: 500,
            transfer_event_count: 0,
            active_senders: 0,
            active_recipients: 0,
            mint_count: 0,
            burn_count: 0,
            plain_transfer_count: 0,
            sum_mints_raw: "0".into(),
            sum_burns_raw: "0".into(),
            net_mint_raw: None,
            total_supply_at_start_minus_1: Some("1.000000".into()),
            total_supply_at_start_minus_1_provenance: "on-chain".into(),
            total_supply_at_end: None,
            onchain_delta_raw: None,
            discrepancy_raw: None,
            metadata_call_pass: true,
            historical_supply_pass: true,
            no_duplicate_logs_pass: None,
            transfer_decode_pass: None,
            supply_invariant_pass: None,
            duplicate_count: 0,
            full_decode_error_count: 0,
            total_supply_start_raw: Some("1000000".into()),
            total_supply_end_raw: None,
            total_supply_start_block_timestamp_rfc3339: Some("2026-05-01T00:00:00Z".into()),
            window_start_block_timestamp_rfc3339: Some("2026-05-02T00:00:00Z".into()),
            window_end_block_timestamp_rfc3339: None,
        };
        let snaps = build_supply_snapshots(
            &CanonicalWriteParams {
                asset: "USDC",
                run_id: "run_ts_test",
                captured_at: "2026-05-15T08:00:00+00:00",
                events: &[],
                supply_rows: &[row],
            },
            &EvidenceSourcesDocument {
                schema: EVIDENCE_SOURCES_SCHEMA.to_string(),
                sources: vec![],
            },
        )
        .unwrap();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].block_number, 100);
        assert_eq!(
            snaps[0].block_timestamp.as_deref(),
            Some("2026-05-01T00:00:00Z")
        );
    }

    #[test]
    fn canonical_transfers_csv_has_expected_columns() {
        let out = std::env::temp_dir().join(format!("stablecoin_canon_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let _row = sample_row();
        let event = TransferEvent {
            chain: "ethereum".into(),
            contract_address: "0xabc".into(),
            block_number: 150,
            tx_hash: "0xtx".into(),
            log_index: 0,
            from: "0x0000000000000000000000000000000000000000".into(),
            to: "0xdef".into(),
            value_raw: "1000000".into(),
            value_decimal: "1.000000".into(),
            kind: "mint".into(),
            value_u256: U256::from(1_000_000u64),
        };
        write_canonical_transfers_csv(
            &out,
            &[CanonicalTransferRecord {
                chain: "ethereum".into(),
                chain_id: 1,
                block_number: 150,
                block_timestamp: None,
                tx_hash: "0xtx".into(),
                log_index: 0,
                contract_address: "0xabc".into(),
                from_address: event.from.clone(),
                to_address: event.to.clone(),
                raw_amount: event.value_raw.clone(),
                normalized_amount: event.value_decimal.clone(),
                decimals: 6,
                event_type: TransferEventType::Mint,
                evidence_source_id: Some("run:rpc:ethereum:transfer_logs".into()),
            }],
        )
        .unwrap();
        let header = std::fs::read_to_string(out.join(CANONICAL_TRANSFERS_FILENAME))
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_string();
        for col in canonical_transfer_csv_headers() {
            assert!(header.contains(col), "missing column {col}");
        }
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn supply_snapshots_csv_has_expected_columns() {
        let out = std::env::temp_dir().join(format!("stablecoin_snap_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        write_supply_snapshots_csv(
            &out,
            &[SupplySnapshotRecord {
                chain: "ethereum".into(),
                chain_id: 1,
                contract_address: "0xabc".into(),
                block_number: 200,
                block_timestamp: None,
                raw_total_supply: "2000000".into(),
                normalized_total_supply: "2.000000".into(),
                decimals: 6,
                method: "totalSupply".into(),
                evidence_source_id: Some("run:contract_call:ethereum:totalSupply".into()),
            }],
        )
        .unwrap();
        let header = std::fs::read_to_string(out.join(SUPPLY_SNAPSHOTS_FILENAME))
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_string();
        for col in supply_snapshot_csv_headers() {
            assert!(header.contains(col), "missing column {col}");
        }
        let _ = std::fs::remove_dir_all(&out);
    }
}
