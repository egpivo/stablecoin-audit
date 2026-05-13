//! Transfer-log audit: chunked `eth_getLogs`, decode, dedup, supply reconciliation.

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
use crate::decode::{decode_transfer_log, dedup_transfer_events, sample_decode_qa};
use crate::fetch::{fetch_transfer_logs, FetchParams};
use crate::report::{ensure_out_dir, format_token_amount};
use crate::rpc::build_provider;

const DEFAULT_CHUNK_SIZE: u64 = 500;
const QA_SAMPLE_SIZE: usize = 100;
const ZERO_ADDR: &str = "0x0000000000000000000000000000000000000000";

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function name() external view returns (string memory);
        function symbol() external view returns (string memory);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
    }
}

#[derive(Serialize)]
struct QaReport {
    asset: String,
    generated_at: String,
    provenance: ProvenanceBlock,
    chains: Vec<QaChain>,
}

#[derive(Serialize)]
struct ProvenanceBlock {
    from_block: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_block_requested: Option<String>,
    generated_at: String,
}

#[derive(Serialize)]
struct ProvenanceReport {
    asset: String,
    generated_at: String,
    data_source: String,
    simulated_data: bool,
    chains: Vec<ProvenanceChain>,
}

#[derive(Serialize)]
struct ProvenanceChain {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    resolved_to_block: Option<u64>,
}

#[derive(Serialize)]
struct QaChain {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    resolved_to_block: Option<u64>,
    gates: QaGates,
    duplicate_count: usize,
    full_decode_error_count: usize,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct QaGates {
    metadata_call_pass: String,
    historical_supply_pass: String,
    no_duplicate_logs_pass: String,
    transfer_decode_pass: String,
    supply_invariant_pass: String,
    provenance_stamped: String,
}

#[derive(Clone, Serialize)]
struct SupplyAuditRow {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    resolved_to_block: Option<u64>,
    to_block_requested: String,
    chunk_size: u64,
    transfer_event_count: usize,
    active_senders: usize,
    active_recipients: usize,
    mint_count: usize,
    burn_count: usize,
    plain_transfer_count: usize,
    sum_mints_raw: String,
    sum_burns_raw: String,
    net_mint_raw: Option<String>,
    total_supply_at_start_minus_1: Option<String>,
    total_supply_at_start_minus_1_provenance: String,
    total_supply_at_end: Option<String>,
    onchain_delta_raw: Option<String>,
    discrepancy_raw: Option<String>,
    metadata_call_pass: bool,
    historical_supply_pass: bool,
    no_duplicate_logs_pass: Option<bool>,
    transfer_decode_pass: Option<bool>,
    supply_invariant_pass: Option<bool>,
    duplicate_count: usize,
    full_decode_error_count: usize,
}

fn gate_csv(pass: bool) -> &'static str {
    if pass {
        "PASS"
    } else {
        "FAIL"
    }
}

fn gate_opt_csv(pass: Option<bool>) -> &'static str {
    match pass {
        Some(true) => "PASS",
        Some(false) => "FAIL",
        None => "UNAVAILABLE",
    }
}

pub async fn run(
    asset: &str,
    chains: &[String],
    from_block: u64,
    to_block_raw: &str,
    chunk_size: Option<u64>,
) -> Result<()> {
    let chunk_size = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE);
    let to_block_requested = if to_block_raw.eq_ignore_ascii_case("latest") {
        None
    } else {
        Some(to_block_raw.parse::<u64>().map_err(|_| {
            anyhow::anyhow!(
                "--to-block must be a positive integer or 'latest'; got {:?}",
                to_block_raw
            )
        })?)
    };

    let generated_at = Utc::now().to_rfc3339();
    let mut all_events: Vec<crate::decode::TransferEvent> = Vec::new();
    let mut supply_rows: Vec<SupplyAuditRow> = Vec::new();
    let mut qa_chains: Vec<QaChain> = Vec::new();
    let mut any_hard_error = false;

    for chain in chains {
        let (events, supply, qa, hard_err) = process_chain(
            asset,
            chain,
            from_block,
            to_block_requested,
            chunk_size,
            to_block_raw,
            &generated_at,
        )
        .await;
        if hard_err {
            any_hard_error = true;
        }
        all_events.extend(events);
        qa_chains.push(qa);
        supply_rows.push(supply);
    }

    let out_dir = ensure_out_dir(asset)?;
    write_decoded_transfers_csv(&out_dir, &all_events)?;
    write_supply_audit_csv(&out_dir, &supply_rows)?;

    write_supply_audit_md(
        &out_dir,
        asset,
        &generated_at,
        from_block,
        to_block_raw,
        &supply_rows,
        &qa_chains,
    )?;

    let provenance = ProvenanceBlock {
        from_block,
        to_block_requested: if to_block_raw.eq_ignore_ascii_case("latest") {
            Some("latest".into())
        } else {
            Some(to_block_raw.to_string())
        },
        generated_at: generated_at.clone(),
    };

    let qa_report = QaReport {
        asset: asset.to_uppercase(),
        generated_at: generated_at.clone(),
        provenance,
        chains: qa_chains,
    };
    std::fs::write(
        out_dir.join("qa_report.json"),
        serde_json::to_string_pretty(&qa_report)?,
    )?;
    write_provenance_json(&out_dir, asset, &generated_at, &supply_rows)?;

    println!(
        "\nOutputs written under {}:",
        out_dir.display()
    );
    println!("  decoded_transfers.csv, supply_audit.csv, qa_report.json, provenance.json, supply_audit.md");

    if any_hard_error {
        anyhow::bail!(
            "one or more chains had hard errors; partial outputs written under {}",
            out_dir.display()
        );
    }

    Ok(())
}

async fn process_chain(
    asset: &str,
    chain: &str,
    from_block: u64,
    to_block: Option<u64>,
    chunk_size: u64,
    to_block_raw: &str,
    generated_at: &str,
) -> (
    Vec<crate::decode::TransferEvent>,
    SupplyAuditRow,
    QaChain,
    bool,
) {
    let mut hard_error = false;
    let empty_events = Vec::new();

    let config = match load_single_token_config(asset, chain) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{}] config load failed: {e:#}", chain.to_uppercase());
            let supply = failed_supply_row(chain, from_block, to_block_raw, chunk_size);
            let qa = build_qa_chain(&supply, generated_at, &[format!("config: {e:#}")]);
            return (empty_events, supply, qa, true);
        }
    };

    let rpc_url = match config.rpc_url() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("[{}] env var missing: {e:#}", chain.to_uppercase());
            let supply = failed_supply_row(chain, from_block, to_block_raw, chunk_size);
            let qa = build_qa_chain(&supply, generated_at, &[format!("{e:#}")]);
            return (empty_events, supply, qa, true);
        }
    };

    let provider = match build_provider(&rpc_url) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[{}] provider init failed: {e:#}", chain.to_uppercase());
            let supply = failed_supply_row(chain, from_block, to_block_raw, chunk_size);
            let qa = build_qa_chain(&supply, generated_at, &[format!("{e:#}")]);
            return (empty_events, supply, qa, true);
        }
    };

    let addr = match Address::from_str(&config.contract_address) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[{}] bad contract address: {e:#}", chain.to_uppercase());
            let supply = failed_supply_row(chain, from_block, to_block_raw, chunk_size);
            let qa = build_qa_chain(&supply, generated_at, &[format!("contract_address: {e:#}")]);
            return (empty_events, supply, qa, true);
        }
    };

    let mut errors: Vec<String> = Vec::new();
    let mut skip_rpc = false;

    match provider.get_chain_id().await {
        Ok(id) if id != config.chain_id => {
            let msg = format!(
                "chain_id mismatch: RPC returned {id}, config expects {}; check {}",
                config.chain_id, config.rpc_url_env
            );
            eprintln!("[{}] {msg}", chain.to_uppercase());
            errors.push(msg);
            skip_rpc = true;
            hard_error = true;
        }
        Err(e) => {
            let msg = format!("eth_chainId failed: {e:#}");
            eprintln!("[{}] {msg}", chain.to_uppercase());
            errors.push(msg);
            skip_rpc = true;
            hard_error = true;
        }
        Ok(_) => {}
    }

    let resolved_to_block: Option<u64> = if skip_rpc {
        None
    } else {
        match to_block {
            Some(b) => Some(b),
            None => match provider.get_block_number().await {
                Ok(n) => Some(n),
                Err(e) => {
                    let msg = format!("get_block_number failed: {e:#}");
                    errors.push(msg.clone());
                    eprintln!("[{}] {msg}", chain.to_uppercase());
                    hard_error = true;
                    None
                }
            },
        }
    };

    let contract = IERC20::new(addr, &provider);

    let (name_val, symbol_val, decimals_val, supply_live) = if skip_rpc {
        (None, None, None, None)
    } else {
        let name_val = match contract.name().call().await {
            Ok(r) => Some(r._0),
            Err(e) => {
                errors.push(format!("name(): {e:#}"));
                None
            }
        };
        let symbol_val = match contract.symbol().call().await {
            Ok(r) => Some(r._0),
            Err(e) => {
                errors.push(format!("symbol(): {e:#}"));
                None
            }
        };
        let decimals_val = match contract.decimals().call().await {
            Ok(r) => Some(r._0),
            Err(e) => {
                errors.push(format!("decimals(): {e:#}"));
                None
            }
        };
        let supply_live = match contract.totalSupply().call().await {
            Ok(r) => Some(r._0),
            Err(e) => {
                errors.push(format!("totalSupply() live: {e:#}"));
                None
            }
        };
        (name_val, symbol_val, decimals_val, supply_live)
    };

    let metadata_call_pass = name_val.is_some()
        && symbol_val.is_some()
        && decimals_val.is_some()
        && supply_live.is_some();

    let effective_decimals = decimals_val.unwrap_or(config.decimals);
    let decimals_for_decode = effective_decimals;

    let Some(end_blk) = resolved_to_block else {
        let supply = SupplyAuditRow {
            chain: chain.to_string(),
            chain_id: config.chain_id,
            contract_address: config.contract_address.clone(),
            from_block,
            resolved_to_block: None,
            to_block_requested: to_block_raw.to_string(),
            chunk_size,
            transfer_event_count: 0,
            active_senders: 0,
            active_recipients: 0,
            mint_count: 0,
            burn_count: 0,
            plain_transfer_count: 0,
            sum_mints_raw: "0".into(),
            sum_burns_raw: "0".into(),
            net_mint_raw: None,
            total_supply_at_start_minus_1: None,
            total_supply_at_start_minus_1_provenance: "skipped".into(),
            total_supply_at_end: None,
            onchain_delta_raw: None,
            discrepancy_raw: None,
            metadata_call_pass,
            historical_supply_pass: false,
            no_duplicate_logs_pass: None,
            transfer_decode_pass: None,
            supply_invariant_pass: None,
            duplicate_count: 0,
            full_decode_error_count: 0,
        };
        let qa = build_qa_chain(&supply, generated_at, &errors);
        return (empty_events, supply, qa, hard_error);
    };

    // Historical totalSupply @ start-1 and end (aligned with metadata command).
    let start_minus_1 = from_block.saturating_sub(1);
    let (supply_start, supply_start_provenance): (Option<U256>, String) = if skip_rpc {
        (None, "skipped".into())
    } else if start_minus_1 == 0 {
        (Some(U256::ZERO), "genesis (block 0)".into())
    } else if config.deployment_block.is_some_and(|d| start_minus_1 < d) {
        let deploy = config.deployment_block.unwrap();
        (
            Some(U256::ZERO),
            format!("pre-deployment zero: block {start_minus_1} < deployment_block {deploy}"),
        )
    } else {
        match contract
            .totalSupply()
            .block(BlockId::number(start_minus_1))
            .call()
            .await
        {
            Ok(r) => (Some(r._0), "on-chain".into()),
            Err(e) => {
                errors.push(format!("totalSupply() at block {start_minus_1}: {e:#}"));
                (None, "rpc-error".into())
            }
        }
    };

    let supply_end: Option<U256> = if skip_rpc {
        None
    } else {
        match contract
            .totalSupply()
            .block(BlockId::number(end_blk))
            .call()
            .await
        {
            Ok(r) => Some(r._0),
            Err(e) => {
                errors.push(format!("totalSupply() at block {end_blk}: {e:#}"));
                None
            }
        }
    };

    let historical_supply_pass = supply_start.is_some() && supply_end.is_some();

    println!(
        "[{}] fetching Transfer logs block {} → {} (chunk {})",
        chain.to_uppercase(),
        from_block,
        end_blk,
        chunk_size
    );

    let params = FetchParams {
        contract_address: addr,
        from_block,
        to_block: end_blk,
        chunk_size,
    };

    let raw_logs = match fetch_transfer_logs(&provider, &params).await {
        Ok(logs) => logs,
        Err(e) => {
            let msg = format!("eth_getLogs failed: {e:#}");
            eprintln!("[{}] {msg}", chain.to_uppercase());
            errors.push(msg);
            let supply = SupplyAuditRow {
                chain: chain.to_string(),
                chain_id: config.chain_id,
                contract_address: config.contract_address.clone(),
                from_block,
                resolved_to_block: Some(end_blk),
                to_block_requested: to_block_raw.to_string(),
                chunk_size,
                transfer_event_count: 0,
                active_senders: 0,
                active_recipients: 0,
                mint_count: 0,
                burn_count: 0,
                plain_transfer_count: 0,
                sum_mints_raw: "0".into(),
                sum_burns_raw: "0".into(),
                net_mint_raw: None,
                total_supply_at_start_minus_1: supply_start.map(|u| format_token_amount(u, effective_decimals)),
                total_supply_at_start_minus_1_provenance: supply_start_provenance.clone(),
                total_supply_at_end: supply_end.map(|u| format_token_amount(u, effective_decimals)),
                onchain_delta_raw: None,
                discrepancy_raw: None,
                metadata_call_pass,
                historical_supply_pass,
                no_duplicate_logs_pass: None,
                transfer_decode_pass: None,
                supply_invariant_pass: None,
                duplicate_count: 0,
                full_decode_error_count: 0,
            };
            let qa = build_qa_chain(&supply, generated_at, &errors);
            return (empty_events, supply, qa, true);
        }
    };

    let raw_count = raw_logs.len();
    println!("[{}] received {} raw logs", chain.to_uppercase(), raw_count);

    let contract_str = config.contract_address.clone();
    let (_qa_n, _qa_fails, qa_errors) = sample_decode_qa(
        &raw_logs,
        chain,
        &contract_str,
        decimals_for_decode,
        QA_SAMPLE_SIZE,
    );
    for e in qa_errors {
        errors.push(format!("decode QA: {e}"));
    }

    let mut events = Vec::with_capacity(raw_count);
    let mut decode_errors = 0usize;
    for log in &raw_logs {
        match decode_transfer_log(log, chain, &contract_str, decimals_for_decode) {
            Ok(ev) => events.push(ev),
            Err(e) => {
                decode_errors += 1;
                if decode_errors <= 5 {
                    errors.push(format!("decode: {e:#}"));
                }
            }
        }
    }
    if decode_errors > 5 {
        errors.push(format!(
            "... and {} more decode errors",
            decode_errors - 5
        ));
    }

    let (deduped, dup_count) = dedup_transfer_events(events);

    let mint_count = deduped.iter().filter(|e| e.kind == "mint").count();
    let burn_count = deduped.iter().filter(|e| e.kind == "burn").count();
    let plain_transfer_count = deduped.iter().filter(|e| e.kind == "transfer").count();

    let mut senders: HashSet<String> = HashSet::new();
    let mut recipients: HashSet<String> = HashSet::new();
    for e in &deduped {
        if e.from != ZERO_ADDR {
            senders.insert(e.from.clone());
        }
        if e.to != ZERO_ADDR {
            recipients.insert(e.to.clone());
        }
    }

    let sum_mints: U256 = deduped
        .iter()
        .filter(|e| e.kind == "mint")
        .fold(U256::ZERO, |acc, e| acc + e.value_u256);
    let sum_burns: U256 = deduped
        .iter()
        .filter(|e| e.kind == "burn")
        .fold(U256::ZERO, |acc, e| acc + e.value_u256);

    let (net_mint_opt, onchain_delta_opt, discrepancy_opt, invariant_pass) =
        if decode_errors > 0 {
            (None, None, None, None)
        } else {
            match (supply_start, supply_end) {
                (Some(start), Some(end)) => {
                    let net_mint = I256::from_raw(sum_mints) - I256::from_raw(sum_burns);
                    let onchain_delta = I256::from_raw(end) - I256::from_raw(start);
                    let discrepancy = net_mint - onchain_delta;
                    let pass = discrepancy == I256::ZERO;
                    (Some(net_mint), Some(onchain_delta), Some(discrepancy), Some(pass))
                }
                _ => (None, None, None, None),
            }
        };

    let no_dup_pass = dup_count == 0;
    let all_decode_pass = decode_errors == 0;

    let gate = |b: bool| if b { "[PASS]" } else { "[FAIL]" };
    let inv_label = match invariant_pass {
        Some(true) => "[PASS]",
        Some(false) => "[FAIL]",
        None => "[UNAVAILABLE]",
    };
    println!(
        "[{}] {} logs → {} unique (dup: {}) | mint: {} burn: {} plain_transfer: {}",
        chain.to_uppercase(),
        raw_count,
        deduped.len(),
        dup_count,
        mint_count,
        burn_count,
        plain_transfer_count,
    );
    println!(
        "[{}] no_dup: {}  all_decode: {}  supply_invariant: {}",
        chain.to_uppercase(),
        gate(no_dup_pass),
        gate(all_decode_pass),
        inv_label,
    );
    if let (Some(nm), Some(od), Some(disc)) = (net_mint_opt, onchain_delta_opt, discrepancy_opt) {
        println!(
            "[{}]   net_mint={} onchain_delta={} discrepancy={}",
            chain.to_uppercase(),
            nm,
            od,
            disc,
        );
    }

    let fmt_u256 = |v: U256| format_token_amount(v, effective_decimals);

    let supply = SupplyAuditRow {
        chain: chain.to_string(),
        chain_id: config.chain_id,
        contract_address: config.contract_address.clone(),
        from_block,
        resolved_to_block: Some(end_blk),
        to_block_requested: to_block_raw.to_string(),
        chunk_size,
        transfer_event_count: deduped.len(),
        active_senders: senders.len(),
        active_recipients: recipients.len(),
        mint_count,
        burn_count,
        plain_transfer_count,
        sum_mints_raw: sum_mints.to_string(),
        sum_burns_raw: sum_burns.to_string(),
        net_mint_raw: net_mint_opt.map(|v| v.to_string()),
        total_supply_at_start_minus_1: supply_start.map(fmt_u256),
        total_supply_at_start_minus_1_provenance: supply_start_provenance.clone(),
        total_supply_at_end: supply_end.map(fmt_u256),
        onchain_delta_raw: onchain_delta_opt.map(|v| v.to_string()),
        discrepancy_raw: discrepancy_opt.map(|v| v.to_string()),
        metadata_call_pass,
        historical_supply_pass,
        no_duplicate_logs_pass: Some(no_dup_pass),
        transfer_decode_pass: Some(all_decode_pass),
        supply_invariant_pass: invariant_pass,
        duplicate_count: dup_count,
        full_decode_error_count: decode_errors,
    };

    let qa = build_qa_chain(&supply, generated_at, &errors);

    (deduped, supply, qa, hard_error)
}

fn failed_supply_row(
    chain: &str,
    from_block: u64,
    to_block_raw: &str,
    chunk_size: u64,
) -> SupplyAuditRow {
    SupplyAuditRow {
        chain: chain.to_string(),
        chain_id: 0,
        contract_address: "unknown".into(),
        from_block,
        resolved_to_block: None,
        to_block_requested: to_block_raw.to_string(),
        chunk_size,
        transfer_event_count: 0,
        active_senders: 0,
        active_recipients: 0,
        mint_count: 0,
        burn_count: 0,
        plain_transfer_count: 0,
        sum_mints_raw: "0".into(),
        sum_burns_raw: "0".into(),
        net_mint_raw: None,
        total_supply_at_start_minus_1: None,
        total_supply_at_start_minus_1_provenance: "skipped".into(),
        total_supply_at_end: None,
        onchain_delta_raw: None,
        discrepancy_raw: None,
        metadata_call_pass: false,
        historical_supply_pass: false,
        no_duplicate_logs_pass: None,
        transfer_decode_pass: None,
        supply_invariant_pass: None,
        duplicate_count: 0,
        full_decode_error_count: 0,
    }
}

fn build_qa_chain(supply: &SupplyAuditRow, generated_at: &str, errors: &[String]) -> QaChain {
    let prov = !supply.to_block_requested.is_empty() && supply.from_block > 0 && !generated_at.is_empty();
    QaChain {
        chain: supply.chain.clone(),
        chain_id: supply.chain_id,
        contract_address: supply.contract_address.clone(),
        from_block: supply.from_block,
        resolved_to_block: supply.resolved_to_block,
        gates: QaGates {
            metadata_call_pass: gate_csv(supply.metadata_call_pass).to_string(),
            historical_supply_pass: gate_csv(supply.historical_supply_pass).to_string(),
            no_duplicate_logs_pass: gate_opt_csv(supply.no_duplicate_logs_pass).to_string(),
            transfer_decode_pass: gate_opt_csv(supply.transfer_decode_pass).to_string(),
            supply_invariant_pass: gate_opt_csv(supply.supply_invariant_pass).to_string(),
            provenance_stamped: gate_csv(prov).to_string(),
        },
        duplicate_count: supply.duplicate_count,
        full_decode_error_count: supply.full_decode_error_count,
        errors: errors.to_vec(),
    }
}

fn write_decoded_transfers_csv(
    out_dir: &std::path::Path,
    events: &[crate::decode::TransferEvent],
) -> Result<()> {
    let path = out_dir.join("decoded_transfers.csv");
    let mut wtr = csv::Writer::from_path(path)?;
    for ev in events {
        wtr.serialize(ev)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_supply_audit_csv(out_dir: &std::path::Path, rows: &[SupplyAuditRow]) -> Result<()> {
    let path = out_dir.join("supply_audit.csv");
    let mut wtr = csv::Writer::from_path(path)?;
    for row in rows {
        wtr.serialize(row)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_supply_audit_md(
    out_dir: &std::path::Path,
    asset: &str,
    generated_at: &str,
    from_block: u64,
    to_block_raw: &str,
    rows: &[SupplyAuditRow],
    qa: &[QaChain],
) -> Result<()> {
    let mut md = String::new();
    md.push_str(&format!("# {} supply audit\n\n", asset.to_uppercase()));
    md.push_str(&format!("**Generated:** {}\n\n", generated_at));
    md.push_str("## Provenance\n\n");
    md.push_str(&format!(
        "- **from_block:** {}\n- **to_block (requested):** {}\n\n",
        from_block, to_block_raw
    ));

    md.push_str(
        "> Active sender/recipient counts count addresses that appear in Transfer events **within this block window only**. \
They are **not** estimates of total token holders or a full holder reconstruction.\n\n",
    );

    md.push_str("## Per-chain results\n\n");
    for (row, q) in rows.iter().zip(qa.iter()) {
        md.push_str(&format!("### {}\n\n", row.chain));
        md.push_str(&format!(
            "- **Resolved to block:** {}\n",
            row.resolved_to_block
                .map(|b| b.to_string())
                .unwrap_or_else(|| "—".into())
        ));
        md.push_str(&format!(
            "- **Contract:** `{}`\n",
            row.contract_address
        ));
        md.push_str(&format!(
            "- **Transfer events (deduped):** {}\n",
            row.transfer_event_count
        ));
        md.push_str(&format!(
            "- **Active senders / recipients (window):** {} / {}\n",
            row.active_senders, row.active_recipients
        ));
        md.push_str(&format!(
            "- **Mints / burns / plain transfers:** {} / {} / {}\n",
            row.mint_count, row.burn_count, row.plain_transfer_count
        ));
        md.push_str(&format!(
            "- **Sum mints / burns (raw):** {} / {}\n",
            row.sum_mints_raw, row.sum_burns_raw
        ));
        md.push_str(&format!(
            "- **totalSupply @ start−1:** {}  _( {} )_\n",
            row.total_supply_at_start_minus_1.as_deref().unwrap_or("—"),
            row.total_supply_at_start_minus_1_provenance
        ));
        md.push_str(&format!(
            "- **totalSupply @ end:** {}\n",
            row.total_supply_at_end.as_deref().unwrap_or("—")
        ));
        md.push_str(&format!(
            "- **On-chain Δ / net mint / discrepancy (raw int):** {} / {} / {}\n",
            row.onchain_delta_raw.as_deref().unwrap_or("—"),
            row.net_mint_raw.as_deref().unwrap_or("—"),
            row.discrepancy_raw.as_deref().unwrap_or("—"),
        ));

        md.push_str("\n**QA gates:**\n\n");
        md.push_str(&format!(
            "| metadata | historical totalSupply | no dup logs | decode | supply invariant |\n|---|---|---|---|---|\n| {} | {} | {} | {} | {} |\n\n",
            gate_csv(row.metadata_call_pass),
            gate_csv(row.historical_supply_pass),
            gate_opt_csv(row.no_duplicate_logs_pass),
            gate_opt_csv(row.transfer_decode_pass),
            gate_opt_csv(row.supply_invariant_pass),
        ));

        if !q.errors.is_empty() {
            md.push_str("**Errors:**\n\n");
            for e in &q.errors {
                md.push_str(&format!("- {}\n", e));
            }
            md.push('\n');
        }
    }

    md.push_str("---\n\n");
    md.push_str(
        "_Window-limited Transfer audit. Not a reserve audit, price analysis, or full-history holder census._\n",
    );

    std::fs::write(out_dir.join("supply_audit.md"), md)?;
    Ok(())
}

fn write_provenance_json(
    out_dir: &std::path::Path,
    asset: &str,
    generated_at: &str,
    rows: &[SupplyAuditRow],
) -> Result<()> {
    let report = ProvenanceReport {
        asset: asset.to_uppercase(),
        generated_at: generated_at.to_string(),
        data_source: "onchain_rpc".into(),
        simulated_data: false,
        chains: rows
            .iter()
            .map(|r| ProvenanceChain {
                chain: r.chain.clone(),
                chain_id: r.chain_id,
                contract_address: r.contract_address.clone(),
                from_block: r.from_block,
                resolved_to_block: r.resolved_to_block,
            })
            .collect(),
    };
    std::fs::write(
        out_dir.join("provenance.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    Ok(())
}
