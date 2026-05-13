use alloy::primitives::{Address, I256, U256};
use alloy::providers::Provider;
use alloy::rpc::types::BlockId;
use alloy::sol;
use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::collections::HashSet;
use std::str::FromStr;

use crate::config::load_single_token_config;
use crate::decode::{decode_transfer_log, dedup_transfer_events};
use crate::fetch::{fetch_transfer_logs_adaptive, ChunkingStats, FetchParams, TRANSFER_SIGNATURE_HASH};
use crate::report::ensure_out_dir;
use crate::rpc::build_provider;

const DEFAULT_CHUNK_SIZE: u64 = 500;
const MIN_CHUNK_SIZE: u64 = 25;
const MAX_RETRIES_PER_CHUNK: u32 = 3;
const ZERO_ADDR: &str = "0x0000000000000000000000000000000000000000";

#[derive(Debug, Clone, Copy, Serialize)]
pub enum EndBlock {
    Number(u64),
    Latest,
}

#[derive(Debug, Clone, Serialize)]
pub struct WindowSpec {
    pub chain: String,
    pub start_block: u64,
    pub end_block: EndBlock,
}

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function name() external view returns (string memory);
        function symbol() external view returns (string memory);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
    }
}

#[derive(Clone)]
struct ChainRun {
    chain: String,
    chain_id: u64,
    contract_address: String,
    rpc_provider_alias: String,
    start_block: u64,
    end_block: Option<u64>,
    transfer_count: usize,
    active_senders: usize,
    active_recipients: usize,
    mint_count: usize,
    burn_count: usize,
    mint_sum_raw: String,
    burn_sum_raw: String,
    net_mint_raw: Option<String>,
    total_supply_start_raw: Option<String>,
    total_supply_end_raw: Option<String>,
    total_supply_delta_raw: Option<String>,
    discrepancy_raw: Option<String>,
    metadata_calls_pass: bool,
    historical_total_supply_pass: bool,
    transfer_logs_fetched_pass: bool,
    no_duplicate_logs_pass: Option<bool>,
    transfer_decode_pass: Option<bool>,
    supply_invariant_pass: Option<bool>,
    provenance_stamped_pass: bool,
    no_simulated_data_pass: bool,
    duplicate_count: usize,
    decode_error_count: usize,
    chunking: ChunkingStats,
    topics: Vec<String>,
    data_source: String,
    simulated_data: bool,
    fetched_at: String,
    generated_at: String,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct SupplyAuditRow<'a> {
    asset: &'a str,
    chain: &'a str,
    chain_id: u64,
    contract_address: &'a str,
    rpc_provider_alias: &'a str,
    start_block: u64,
    end_block: Option<u64>,
    transfer_count: usize,
    active_senders: usize,
    active_recipients: usize,
    mint_count: usize,
    burn_count: usize,
    mint_sum_raw: &'a str,
    burn_sum_raw: &'a str,
    net_mint_raw: Option<&'a str>,
    total_supply_start_raw: Option<&'a str>,
    total_supply_end_raw: Option<&'a str>,
    total_supply_delta_raw: Option<&'a str>,
    discrepancy_raw: Option<&'a str>,
    qa_status: &'a str,
    generated_at: &'a str,
}

#[derive(Serialize)]
struct MintBurnSummaryRow<'a> {
    asset: &'a str,
    chain: &'a str,
    start_block: u64,
    end_block: Option<u64>,
    mint_count: usize,
    burn_count: usize,
    mint_sum_raw: &'a str,
    burn_sum_raw: &'a str,
    net_mint_raw: Option<&'a str>,
}

#[derive(Serialize)]
struct TransferSummaryRow<'a> {
    asset: &'a str,
    chain: &'a str,
    start_block: u64,
    end_block: Option<u64>,
    transfer_count: usize,
    active_senders: usize,
    active_recipients: usize,
}

#[derive(Serialize)]
struct QaReport<'a> {
    asset: &'a str,
    generated_at: &'a str,
    chains: Vec<QaChain<'a>>,
}

#[derive(Serialize)]
struct QaChain<'a> {
    chain: &'a str,
    chain_id: u64,
    contract_address: &'a str,
    rpc_provider_alias: &'a str,
    start_block: u64,
    end_block: Option<u64>,
    gates: QaGates,
    chunking: ChunkingStats,
    duplicate_count: usize,
    decode_error_count: usize,
    errors: &'a [String],
}

#[derive(Serialize)]
struct QaGates {
    metadata_calls_pass: String,
    historical_total_supply_calls_pass: String,
    transfer_logs_fetched_pass: String,
    no_duplicate_logs_pass: String,
    transfer_decode_pass: String,
    supply_invariant_pass: String,
    provenance_stamped_pass: String,
    no_simulated_data_pass: String,
}

#[derive(Serialize)]
struct ProvenanceReport<'a> {
    asset: &'a str,
    generated_at: &'a str,
    data_source: &'a str,
    simulated_data: bool,
    chains: Vec<ProvenanceRow<'a>>,
}

#[derive(Serialize)]
struct ProvenanceRow<'a> {
    asset: &'a str,
    chain: &'a str,
    chain_id: u64,
    contract_address: &'a str,
    rpc_provider_alias: &'a str,
    start_block: u64,
    end_block: Option<u64>,
    fetched_at: &'a str,
    generated_at: &'a str,
    topics: &'a [String],
    data_source: &'a str,
    simulated_data: bool,
    chunking: ChunkingStats,
}

fn gate_bool(pass: bool) -> String {
    if pass { "PASS".into() } else { "FAIL".into() }
}

fn gate_opt(pass: Option<bool>) -> String {
    match pass {
        Some(true) => "PASS".into(),
        Some(false) => "FAIL".into(),
        None => "UNAVAILABLE".into(),
    }
}

fn qa_status(row: &ChainRun) -> &'static str {
    if !row.metadata_calls_pass
        || !row.historical_total_supply_pass
        || !row.transfer_logs_fetched_pass
        || !matches!(row.no_duplicate_logs_pass, Some(true))
        || !matches!(row.transfer_decode_pass, Some(true))
        || !row.provenance_stamped_pass
        || !row.no_simulated_data_pass
    {
        return "FAIL";
    }
    match row.supply_invariant_pass {
        Some(true) => "PASS",
        Some(false) => "FAIL",
        None => "UNAVAILABLE",
    }
}

pub async fn run(asset: &str, windows: &[WindowSpec], chunk_size: Option<u64>) -> Result<()> {
    if windows.is_empty() {
        anyhow::bail!("at least one --window is required");
    }
    let chunk_size = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE);
    let generated_at = Utc::now().to_rfc3339();
    let out_dir = ensure_out_dir(asset)?;

    let mut all_events = Vec::new();
    let mut runs = Vec::new();
    let mut any_hard_error = false;

    for win in windows {
        let (events, run, hard_error) = process_chain_window(asset, win, chunk_size, &generated_at).await;
        if hard_error {
            any_hard_error = true;
        }
        all_events.extend(events);
        runs.push(run);
    }

    write_decoded_transfers_csv(&out_dir, &all_events)?;
    write_supply_audit_csv(&out_dir, asset, &runs)?;
    write_mint_burn_summary_csv(&out_dir, asset, &runs)?;
    write_transfer_summary_csv(&out_dir, asset, &runs)?;
    write_qa_report_json(&out_dir, asset, &generated_at, &runs)?;
    write_provenance_json(&out_dir, asset, &generated_at, &runs)?;
    write_summary_md(&out_dir, asset, &generated_at, &runs)?;

    println!("\nOutputs written under {}:", out_dir.display());
    println!("  decoded_transfers.csv, supply_audit.csv, mint_burn_summary.csv");
    println!("  transfer_summary.csv, qa_report.json, provenance.json, summary.md");

    if any_hard_error {
        anyhow::bail!("one or more chains had hard errors; partial outputs were written");
    }
    Ok(())
}

async fn process_chain_window(
    asset: &str,
    win: &WindowSpec,
    chunk_size: u64,
    generated_at: &str,
) -> (Vec<crate::decode::TransferEvent>, ChainRun, bool) {
    let chain = win.chain.as_str();
    let mut errors = Vec::<String>::new();
    let mut hard_error = false;
    let mut chunking = ChunkingStats {
        initial_chunk: chunk_size,
        final_chunk: chunk_size,
        chunks_total: 0,
        retries_total: 0,
        backoffs_total: 0,
    };

    let config = match load_single_token_config(asset, chain) {
        Ok(c) => c,
        Err(e) => {
            errors.push(format!("config: {e:#}"));
            return (Vec::new(), failed_row(chain, win, generated_at, errors, chunking), true);
        }
    };

    let rpc_provider_alias = config.rpc_url_env.clone();
    let rpc_url = match config.rpc_url() {
        Ok(u) => u,
        Err(e) => {
            errors.push(format!("{e:#}"));
            return (
                Vec::new(),
                failed_row_with_config(&config, win, generated_at, errors, chunking),
                true,
            );
        }
    };
    let provider = match build_provider(&rpc_url) {
        Ok(p) => p,
        Err(e) => {
            errors.push(format!("provider: {e:#}"));
            return (
                Vec::new(),
                failed_row_with_config(&config, win, generated_at, errors, chunking),
                true,
            );
        }
    };

    match provider.get_chain_id().await {
        Ok(id) if id != config.chain_id => {
            errors.push(format!("chain_id mismatch: rpc={id} config={}", config.chain_id));
            hard_error = true;
        }
        Err(e) => {
            errors.push(format!("eth_chainId failed: {e:#}"));
            hard_error = true;
        }
        Ok(_) => {}
    }
    if hard_error {
        return (
            Vec::new(),
            failed_row_with_config(&config, win, generated_at, errors, chunking),
            true,
        );
    }

    let addr = match Address::from_str(&config.contract_address) {
        Ok(a) => a,
        Err(e) => {
            errors.push(format!("contract address invalid: {e:#}"));
            return (
                Vec::new(),
                failed_row_with_config(&config, win, generated_at, errors, chunking),
                true,
            );
        }
    };

    let end_block = match win.end_block {
        EndBlock::Number(n) => Some(n),
        EndBlock::Latest => match provider.get_block_number().await {
            Ok(n) => Some(n),
            Err(e) => {
                errors.push(format!("get_block_number failed: {e:#}"));
                hard_error = true;
                None
            }
        },
    };
    if end_block.is_none() {
        return (
            Vec::new(),
            failed_row_with_config(&config, win, generated_at, errors, chunking),
            true,
        );
    }

    let contract = IERC20::new(addr, &provider);
    let (name_ok, symbol_ok, decimals_val, live_supply_ok) = {
        let name_ok = contract.name().call().await.is_ok();
        let symbol_ok = contract.symbol().call().await.is_ok();
        let decimals_val = contract.decimals().call().await.ok().map(|r| r._0);
        let live_supply_ok = contract.totalSupply().call().await.is_ok();
        (name_ok, symbol_ok, decimals_val, live_supply_ok)
    };
    let metadata_calls_pass = name_ok && symbol_ok && decimals_val.is_some() && live_supply_ok;
    let decimals = decimals_val.unwrap_or(config.decimals);

    let start_minus_1 = win.start_block.saturating_sub(1);
    let (supply_start, supply_start_raw) = if start_minus_1 == 0
        || config
            .deployment_block
            .is_some_and(|d| start_minus_1 < d)
    {
        (Some(U256::ZERO), Some("0".to_string()))
    } else {
        match contract
            .totalSupply()
            .block(BlockId::number(start_minus_1))
            .call()
            .await
        {
            Ok(r) => (Some(r._0), Some(r._0.to_string())),
            Err(e) => {
                errors.push(format!("totalSupply(start-1) failed: {e:#}"));
                (None, None)
            }
        }
    };

    let supply_end = match end_block {
        Some(end) => match contract.totalSupply().block(BlockId::number(end)).call().await {
            Ok(r) => Some(r._0),
            Err(e) => {
                errors.push(format!("totalSupply(end) failed: {e:#}"));
                None
            }
        },
        _ => None,
    };
    let supply_end_raw = supply_end.map(|v| v.to_string());
    let historical_total_supply_pass = supply_start.is_some() && supply_end.is_some();

    let mut transfer_logs_fetched_pass = false;
    let mut decoded = Vec::new();
    let mut duplicate_count = 0usize;
    let mut decode_error_count = 0usize;
    let mut active_senders = 0usize;
    let mut active_recipients = 0usize;
    let mut mint_count = 0usize;
    let mut burn_count = 0usize;
    let mut mint_sum_raw = "0".to_string();
    let mut burn_sum_raw = "0".to_string();
    let mut net_mint_raw = None;
    let mut total_supply_delta_raw = None;
    let mut discrepancy_raw = None;
    let mut no_duplicate_logs_pass = None;
    let mut transfer_decode_pass = None;
    let mut supply_invariant_pass = None;
    let mut fetched_at = Utc::now().to_rfc3339();

    if let Some(end) = end_block {
        let params = FetchParams {
            contract_address: addr,
            from_block: win.start_block,
            to_block: end,
            chunk_size,
        };
        match fetch_transfer_logs_adaptive(&provider, &params, MIN_CHUNK_SIZE, MAX_RETRIES_PER_CHUNK).await {
            Ok((raw_logs, stats)) => {
                fetched_at = Utc::now().to_rfc3339();
                chunking = stats;
                transfer_logs_fetched_pass = true;

                let mut events = Vec::with_capacity(raw_logs.len());
                for log in &raw_logs {
                    match decode_transfer_log(log, chain, &config.contract_address, decimals) {
                        Ok(ev) => events.push(ev),
                        Err(e) => {
                            decode_error_count += 1;
                            if decode_error_count <= 5 {
                                errors.push(format!("decode: {e:#}"));
                            }
                        }
                    }
                }
                if decode_error_count > 5 {
                    errors.push(format!("... and {} more decode errors", decode_error_count - 5));
                }

                let (deduped, dups) = dedup_transfer_events(events);
                duplicate_count = dups;
                no_duplicate_logs_pass = Some(dups == 0);
                transfer_decode_pass = Some(decode_error_count == 0);

                let mut senders = HashSet::new();
                let mut recipients = HashSet::new();
                let mut sum_mints = U256::ZERO;
                let mut sum_burns = U256::ZERO;
                for ev in &deduped {
                    if ev.from != ZERO_ADDR {
                        senders.insert(ev.from.clone());
                    }
                    if ev.to != ZERO_ADDR {
                        recipients.insert(ev.to.clone());
                    }
                    if ev.kind == "mint" {
                        mint_count += 1;
                        sum_mints += ev.value_u256;
                    } else if ev.kind == "burn" {
                        burn_count += 1;
                        sum_burns += ev.value_u256;
                    }
                }
                active_senders = senders.len();
                active_recipients = recipients.len();
                mint_sum_raw = sum_mints.to_string();
                burn_sum_raw = sum_burns.to_string();
                decoded = deduped;

                if decode_error_count == 0 {
                    if let (Some(start), Some(end_supply)) = (supply_start, supply_end) {
                        let net_mint = I256::from_raw(sum_mints) - I256::from_raw(sum_burns);
                        let delta = I256::from_raw(end_supply) - I256::from_raw(start);
                        let discrepancy = net_mint - delta;
                        net_mint_raw = Some(net_mint.to_string());
                        total_supply_delta_raw = Some(delta.to_string());
                        discrepancy_raw = Some(discrepancy.to_string());
                        supply_invariant_pass = Some(discrepancy == I256::ZERO);
                    }
                }
            }
            Err(e) => {
                errors.push(format!("transfer logs fetch failed: {e:#}"));
                hard_error = true;
            }
        }
    }

    let provenance_stamped_pass = end_block.is_some()
        && !rpc_provider_alias.is_empty()
        && !generated_at.is_empty()
        && !fetched_at.is_empty();
    let no_simulated_data_pass = true;
    let row = ChainRun {
        chain: chain.to_string(),
        chain_id: config.chain_id,
        contract_address: config.contract_address.clone(),
        rpc_provider_alias,
        start_block: win.start_block,
        end_block,
        transfer_count: decoded.len(),
        active_senders,
        active_recipients,
        mint_count,
        burn_count,
        mint_sum_raw,
        burn_sum_raw,
        net_mint_raw,
        total_supply_start_raw: supply_start_raw,
        total_supply_end_raw: supply_end_raw,
        total_supply_delta_raw,
        discrepancy_raw,
        metadata_calls_pass,
        historical_total_supply_pass,
        transfer_logs_fetched_pass,
        no_duplicate_logs_pass,
        transfer_decode_pass,
        supply_invariant_pass,
        provenance_stamped_pass,
        no_simulated_data_pass,
        duplicate_count,
        decode_error_count,
        chunking,
        topics: vec![format!("{TRANSFER_SIGNATURE_HASH:#x}")],
        data_source: "onchain_rpc".into(),
        simulated_data: false,
        fetched_at,
        generated_at: generated_at.to_string(),
        errors,
    };

    (decoded, row, hard_error)
}

fn failed_row(chain: &str, win: &WindowSpec, generated_at: &str, errors: Vec<String>, chunking: ChunkingStats) -> ChainRun {
    ChainRun {
        chain: chain.to_string(),
        chain_id: 0,
        contract_address: "unknown".into(),
        rpc_provider_alias: "unknown".into(),
        start_block: win.start_block,
        end_block: None,
        transfer_count: 0,
        active_senders: 0,
        active_recipients: 0,
        mint_count: 0,
        burn_count: 0,
        mint_sum_raw: "0".into(),
        burn_sum_raw: "0".into(),
        net_mint_raw: None,
        total_supply_start_raw: None,
        total_supply_end_raw: None,
        total_supply_delta_raw: None,
        discrepancy_raw: None,
        metadata_calls_pass: false,
        historical_total_supply_pass: false,
        transfer_logs_fetched_pass: false,
        no_duplicate_logs_pass: None,
        transfer_decode_pass: None,
        supply_invariant_pass: None,
        provenance_stamped_pass: false,
        no_simulated_data_pass: true,
        duplicate_count: 0,
        decode_error_count: 0,
        chunking,
        topics: vec![format!("{TRANSFER_SIGNATURE_HASH:#x}")],
        data_source: "onchain_rpc".into(),
        simulated_data: false,
        fetched_at: Utc::now().to_rfc3339(),
        generated_at: generated_at.to_string(),
        errors,
    }
}

fn failed_row_with_config(
    config: &crate::config::TokenConfig,
    win: &WindowSpec,
    generated_at: &str,
    errors: Vec<String>,
    chunking: ChunkingStats,
) -> ChainRun {
    let mut row = failed_row(&config.chain, win, generated_at, errors, chunking);
    row.chain_id = config.chain_id;
    row.contract_address = config.contract_address.clone();
    row.rpc_provider_alias = config.rpc_url_env.clone();
    row
}

fn write_decoded_transfers_csv(out_dir: &std::path::Path, events: &[crate::decode::TransferEvent]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(out_dir.join("decoded_transfers.csv"))?;
    for ev in events {
        wtr.serialize(ev)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_supply_audit_csv(out_dir: &std::path::Path, asset: &str, rows: &[ChainRun]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(out_dir.join("supply_audit.csv"))?;
    for row in rows {
        let rec = SupplyAuditRow {
            asset,
            chain: &row.chain,
            chain_id: row.chain_id,
            contract_address: &row.contract_address,
            rpc_provider_alias: &row.rpc_provider_alias,
            start_block: row.start_block,
            end_block: row.end_block,
            transfer_count: row.transfer_count,
            active_senders: row.active_senders,
            active_recipients: row.active_recipients,
            mint_count: row.mint_count,
            burn_count: row.burn_count,
            mint_sum_raw: &row.mint_sum_raw,
            burn_sum_raw: &row.burn_sum_raw,
            net_mint_raw: row.net_mint_raw.as_deref(),
            total_supply_start_raw: row.total_supply_start_raw.as_deref(),
            total_supply_end_raw: row.total_supply_end_raw.as_deref(),
            total_supply_delta_raw: row.total_supply_delta_raw.as_deref(),
            discrepancy_raw: row.discrepancy_raw.as_deref(),
            qa_status: qa_status(row),
            generated_at: &row.generated_at,
        };
        wtr.serialize(rec)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_mint_burn_summary_csv(out_dir: &std::path::Path, asset: &str, rows: &[ChainRun]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(out_dir.join("mint_burn_summary.csv"))?;
    for row in rows {
        wtr.serialize(MintBurnSummaryRow {
            asset,
            chain: &row.chain,
            start_block: row.start_block,
            end_block: row.end_block,
            mint_count: row.mint_count,
            burn_count: row.burn_count,
            mint_sum_raw: &row.mint_sum_raw,
            burn_sum_raw: &row.burn_sum_raw,
            net_mint_raw: row.net_mint_raw.as_deref(),
        })?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_transfer_summary_csv(out_dir: &std::path::Path, asset: &str, rows: &[ChainRun]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(out_dir.join("transfer_summary.csv"))?;
    for row in rows {
        wtr.serialize(TransferSummaryRow {
            asset,
            chain: &row.chain,
            start_block: row.start_block,
            end_block: row.end_block,
            transfer_count: row.transfer_count,
            active_senders: row.active_senders,
            active_recipients: row.active_recipients,
        })?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_qa_report_json(out_dir: &std::path::Path, asset: &str, generated_at: &str, rows: &[ChainRun]) -> Result<()> {
    let report = QaReport {
        asset,
        generated_at,
        chains: rows
            .iter()
            .map(|row| QaChain {
                chain: &row.chain,
                chain_id: row.chain_id,
                contract_address: &row.contract_address,
                rpc_provider_alias: &row.rpc_provider_alias,
                start_block: row.start_block,
                end_block: row.end_block,
                gates: QaGates {
                    metadata_calls_pass: gate_bool(row.metadata_calls_pass),
                    historical_total_supply_calls_pass: gate_bool(row.historical_total_supply_pass),
                    transfer_logs_fetched_pass: gate_bool(row.transfer_logs_fetched_pass),
                    no_duplicate_logs_pass: gate_opt(row.no_duplicate_logs_pass),
                    transfer_decode_pass: gate_opt(row.transfer_decode_pass),
                    supply_invariant_pass: gate_opt(row.supply_invariant_pass),
                    provenance_stamped_pass: gate_bool(row.provenance_stamped_pass),
                    no_simulated_data_pass: gate_bool(row.no_simulated_data_pass),
                },
                chunking: row.chunking,
                duplicate_count: row.duplicate_count,
                decode_error_count: row.decode_error_count,
                errors: &row.errors,
            })
            .collect(),
    };
    std::fs::write(
        out_dir.join("qa_report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    Ok(())
}

fn write_provenance_json(out_dir: &std::path::Path, asset: &str, generated_at: &str, rows: &[ChainRun]) -> Result<()> {
    let report = ProvenanceReport {
        asset,
        generated_at,
        data_source: "onchain_rpc",
        simulated_data: false,
        chains: rows
            .iter()
            .map(|row| ProvenanceRow {
                asset,
                chain: &row.chain,
                chain_id: row.chain_id,
                contract_address: &row.contract_address,
                rpc_provider_alias: &row.rpc_provider_alias,
                start_block: row.start_block,
                end_block: row.end_block,
                fetched_at: &row.fetched_at,
                generated_at: &row.generated_at,
                topics: &row.topics,
                data_source: &row.data_source,
                simulated_data: row.simulated_data,
                chunking: row.chunking,
            })
            .collect(),
    };
    std::fs::write(
        out_dir.join("provenance.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    Ok(())
}

fn write_summary_md(out_dir: &std::path::Path, asset: &str, generated_at: &str, rows: &[ChainRun]) -> Result<()> {
    let mut md = String::new();
    md.push_str(&format!("# {} transfer-audit summary\n\n", asset.to_uppercase()));
    md.push_str(&format!("Generated at: {}\n\n", generated_at));
    md.push_str("Canonical artifacts: supply_audit.csv, qa_report.json, provenance.json.\n\n");
    md.push_str("> Active senders/recipients are unique non-zero addresses in this window only; they are not holder counts.\n\n");

    for row in rows {
        md.push_str(&format!("## {}\n\n", row.chain));
        md.push_str(&format!(
            "- window: {} -> {}\n",
            row.start_block,
            row.end_block.map(|v| v.to_string()).unwrap_or_else(|| "unknown".into())
        ));
        md.push_str(&format!(
            "- transfer_count={}, active_senders={}, active_recipients={}\n",
            row.transfer_count, row.active_senders, row.active_recipients
        ));
        md.push_str(&format!(
            "- mint_count={}, burn_count={}, mint_sum_raw={}, burn_sum_raw={}\n",
            row.mint_count, row.burn_count, row.mint_sum_raw, row.burn_sum_raw
        ));
        md.push_str(&format!(
            "- totalSupply_start_raw={}, totalSupply_end_raw={}, totalSupply_delta_raw={}, discrepancy_raw={}\n",
            row.total_supply_start_raw.as_deref().unwrap_or(""),
            row.total_supply_end_raw.as_deref().unwrap_or(""),
            row.total_supply_delta_raw.as_deref().unwrap_or(""),
            row.discrepancy_raw.as_deref().unwrap_or("")
        ));
        md.push_str(&format!("- qa_status={}\n\n", qa_status(row)));
    }
    std::fs::write(out_dir.join("summary.md"), md)?;
    Ok(())
}
