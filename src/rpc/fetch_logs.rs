use alloy::primitives::{Address, I256, U256};
use alloy::providers::Provider;
use alloy::rpc::types::BlockId;
use alloy::sol;
use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::path::Path;
use std::str::FromStr;

use crate::config::load_single_token_config;
use crate::decode::{decode_transfer_log, dedup_transfer_events, sample_decode_qa};
use crate::fetch::{fetch_transfer_logs, FetchParams};
use crate::report::ensure_out_dir;
use crate::rpc::build_provider;

const DEFAULT_CHUNK_SIZE: u64 = 500;
const QA_SAMPLE_SIZE: usize = 100;

sol! {
    #[sol(rpc)]
    interface IERC20Supply {
        function totalSupply() external view returns (uint256);
    }
}

#[derive(Serialize)]
struct ChainFetchResult {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    to_block: u64,
    chunk_size: u64,
    // Transfer log counts
    raw_log_count: usize,
    duplicate_count: usize,
    deduped_log_count: usize,
    mint_count: usize,
    burn_count: usize,
    transfer_count: usize,
    // QA gates
    qa_sample_size: usize,
    qa_sample_fail_count: usize,
    no_duplicate_logs_pass: Option<bool>,
    transfer_decode_sample_pass: Option<bool>,
    full_decode_error_count: usize,
    all_transfer_decode_pass: Option<bool>,
    // Control events (M5)
    control_event_count: usize,
    control_event_query_status: String,
    // Supply invariant (M3)
    sum_mints_raw: String,
    sum_burns_raw: String,
    total_supply_at_start_minus_1: Option<String>,
    total_supply_at_start_minus_1_provenance: String,
    total_supply_at_end: Option<String>,
    net_mint_raw: Option<String>,
    onchain_delta_raw: Option<String>,
    discrepancy_raw: Option<String>,
    supply_invariant_pass: Option<bool>,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct FetchReport {
    asset: String,
    generated_at: String,
    chains: Vec<ChainFetchResult>,
}

pub async fn run(
    asset: &str,
    chains: &[String],
    from_block: u64,
    to_block: u64,
    chunk_size: Option<u64>,
) -> Result<()> {
    let chunk_size = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE);
    let mut chain_results: Vec<ChainFetchResult> = Vec::new();
    let mut any_hard_error = false;

    for chain in chains {
        let config = match load_single_token_config(asset, chain) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[{}] config load failed: {e:#}", chain.to_uppercase());
                chain_results.push(failed_chain_result(
                    chain,
                    from_block,
                    to_block,
                    chunk_size,
                    vec![format!("config: {e:#}")],
                ));
                any_hard_error = true;
                continue;
            }
        };

        let rpc_url = match config.rpc_url() {
            Ok(u) => u,
            Err(e) => {
                eprintln!("[{}] env var missing: {e:#}", chain.to_uppercase());
                chain_results.push(failed_chain_result(
                    chain,
                    from_block,
                    to_block,
                    chunk_size,
                    vec![format!("{e:#}")],
                ));
                any_hard_error = true;
                continue;
            }
        };

        let provider = match build_provider(&rpc_url) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[{}] provider init failed: {e:#}", chain.to_uppercase());
                chain_results.push(failed_chain_result(
                    chain,
                    from_block,
                    to_block,
                    chunk_size,
                    vec![format!("{e:#}")],
                ));
                any_hard_error = true;
                continue;
            }
        };

        let addr = match Address::from_str(&config.contract_address) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("[{}] bad contract address: {e:#}", chain.to_uppercase());
                chain_results.push(failed_chain_result(
                    chain,
                    from_block,
                    to_block,
                    chunk_size,
                    vec![format!("contract_address: {e:#}")],
                ));
                any_hard_error = true;
                continue;
            }
        };

        // Chain ID check
        let mut errors: Vec<String> = Vec::new();
        match provider.get_chain_id().await {
            Ok(id) if id != config.chain_id => {
                let msg = format!(
                    "chain_id mismatch: RPC returned {id}, config expects {}; check {}",
                    config.chain_id, config.rpc_url_env
                );
                eprintln!("[{}] {msg}", chain.to_uppercase());
                chain_results.push(failed_chain_result(
                    chain,
                    from_block,
                    to_block,
                    chunk_size,
                    vec![msg],
                ));
                any_hard_error = true;
                continue;
            }
            Err(e) => {
                let msg = format!("eth_chainId failed: {e:#}");
                eprintln!("[{}] {msg}", chain.to_uppercase());
                chain_results.push(failed_chain_result(
                    chain,
                    from_block,
                    to_block,
                    chunk_size,
                    vec![msg],
                ));
                any_hard_error = true;
                continue;
            }
            Ok(_) => {}
        }

        println!(
            "[{}] fetching Transfer logs block {} → {} (chunk {})",
            chain.to_uppercase(),
            from_block,
            to_block,
            chunk_size
        );

        let params = FetchParams {
            contract_address: addr,
            from_block,
            to_block,
            chunk_size,
        };
        let raw_logs = match fetch_transfer_logs(&provider, &params).await {
            Ok(logs) => logs,
            Err(e) => {
                let msg = format!("eth_getLogs failed: {e:#}");
                eprintln!("[{}] {msg}", chain.to_uppercase());
                chain_results.push(failed_chain_result(
                    chain,
                    from_block,
                    to_block,
                    chunk_size,
                    vec![msg],
                ));
                any_hard_error = true;
                continue;
            }
        };

        let raw_count = raw_logs.len();
        println!("[{}] received {} raw logs", chain.to_uppercase(), raw_count);

        // Decode sample QA before dedup
        let (qa_n, qa_fails, qa_errors) = sample_decode_qa(
            &raw_logs,
            chain,
            &config.contract_address,
            config.decimals,
            QA_SAMPLE_SIZE,
        );
        for e in &qa_errors {
            errors.push(format!("decode QA: {e}"));
        }

        // Decode all logs
        let mut events = Vec::with_capacity(raw_count);
        let mut decode_errors = 0usize;
        for log in &raw_logs {
            match decode_transfer_log(log, chain, &config.contract_address, config.decimals) {
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
            errors.push(format!("... and {} more decode errors", decode_errors - 5));
        }

        // Dedup
        let (deduped, dup_count) = dedup_transfer_events(events);

        let mint_count = deduped.iter().filter(|e| e.kind == "mint").count();
        let burn_count = deduped.iter().filter(|e| e.kind == "burn").count();
        let transfer_count = deduped.iter().filter(|e| e.kind == "transfer").count();

        // Supply invariant: sum mint and burn values from decoded events
        let sum_mints: U256 = deduped
            .iter()
            .filter(|e| e.kind == "mint")
            .fold(U256::ZERO, |acc, e| acc + e.value_u256);
        let sum_burns: U256 = deduped
            .iter()
            .filter(|e| e.kind == "burn")
            .fold(U256::ZERO, |acc, e| acc + e.value_u256);

        // Historical totalSupply at start_block - 1 and end_block.
        // Pre-deployment windows synthesise 0 (same logic as metadata subcommand).
        let contract = IERC20Supply::new(addr, &provider);
        let start_minus_1 = from_block.saturating_sub(1);

        let (supply_start, supply_start_provenance): (Option<U256>, String) = if config
            .deployment_block
            .is_some_and(|d| start_minus_1 < d)
        {
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

        let supply_end: Option<U256> = match contract
            .totalSupply()
            .block(BlockId::number(to_block))
            .call()
            .await
        {
            Ok(r) => Some(r._0),
            Err(e) => {
                errors.push(format!("totalSupply() at block {to_block}: {e:#}"));
                None
            }
        };

        // Compute invariant using I256 to handle negative deltas (net burning > minting).
        // All realistic stablecoin supply values are well below I256::MAX so from_raw is safe.
        // If any transfer logs failed to decode, sum_mints/sum_burns are incomplete; suppress
        // the invariant verdict so a coincidental zero discrepancy cannot show as PASS.
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
                        (
                            Some(net_mint),
                            Some(onchain_delta),
                            Some(discrepancy),
                            Some(pass),
                        )
                    }
                    _ => (None, None, None, None),
                }
            };

        let no_dup_pass = dup_count == 0;
        let decode_sample_pass = qa_fails == 0;
        let all_decode_pass = decode_errors == 0;

        // Write CSV
        let out_dir = ensure_out_dir(asset)?;
        let csv_path = out_dir.join(format!("transfers_{chain}.csv"));
        write_transfers_csv(&csv_path, &deduped)?;

        // Fetch control events
        let (ctrl_events, ctrl_status) = crate::control_events::fetch_control_events(
            &provider, addr, from_block, to_block, chain,
        )
        .await;
        let ctrl_count = ctrl_events.len();
        println!(
            "[{}] control events: {} (status: {})",
            chain.to_uppercase(),
            ctrl_count,
            ctrl_status
        );
        let ctrl_path = out_dir.join(format!("control_events_{chain}.csv"));
        write_control_events_csv(&ctrl_path, &ctrl_events)?;

        let gate = |b: bool| if b { "[PASS]" } else { "[FAIL]" };
        let inv_label = match invariant_pass {
            Some(true) => "[PASS]",
            Some(false) => "[FAIL]",
            None => "[UNAVAILABLE]",
        };
        println!(
            "[{}] {} logs → {} unique (dup: {}) | mint: {} burn: {} transfer: {}",
            chain.to_uppercase(),
            raw_count,
            deduped.len(),
            dup_count,
            mint_count,
            burn_count,
            transfer_count,
        );
        println!(
            "[{}] no_dup: {}  decode_qa: {}  all_decode: {}  supply_invariant: {}",
            chain.to_uppercase(),
            gate(no_dup_pass),
            gate(decode_sample_pass),
            gate(all_decode_pass),
            inv_label,
        );
        if let (Some(nm), Some(od), Some(disc)) = (net_mint_opt, onchain_delta_opt, discrepancy_opt)
        {
            println!(
                "[{}]   net_mint={} onchain_delta={} discrepancy={}",
                chain.to_uppercase(),
                nm,
                od,
                disc,
            );
        }
        println!(
            "[{}] transfers written to {}",
            chain.to_uppercase(),
            csv_path.display()
        );

        // fmt_u256 is used only for decimal-scaled supply boundary fields.
        // _raw fields store exact integer strings (no decimal scaling).
        let fmt_u256 = |v: U256| crate::report::format_token_amount(v, config.decimals);

        chain_results.push(ChainFetchResult {
            chain: chain.to_string(),
            chain_id: config.chain_id,
            contract_address: config.contract_address.clone(),
            from_block,
            to_block,
            chunk_size,
            raw_log_count: raw_count,
            duplicate_count: dup_count,
            deduped_log_count: deduped.len(),
            mint_count,
            burn_count,
            transfer_count,
            qa_sample_size: qa_n,
            qa_sample_fail_count: qa_fails,
            no_duplicate_logs_pass: Some(no_dup_pass),
            transfer_decode_sample_pass: Some(decode_sample_pass),
            full_decode_error_count: decode_errors,
            all_transfer_decode_pass: Some(all_decode_pass),
            control_event_count: ctrl_count,
            control_event_query_status: ctrl_status,
            sum_mints_raw: sum_mints.to_string(),
            sum_burns_raw: sum_burns.to_string(),
            total_supply_at_start_minus_1: supply_start.map(fmt_u256),
            total_supply_at_start_minus_1_provenance: supply_start_provenance,
            total_supply_at_end: supply_end.map(fmt_u256),
            net_mint_raw: net_mint_opt.map(|v| v.to_string()),
            onchain_delta_raw: onchain_delta_opt.map(|v| v.to_string()),
            discrepancy_raw: discrepancy_opt.map(|v| v.to_string()),
            supply_invariant_pass: invariant_pass,
            errors,
        });
    }

    let out_dir = ensure_out_dir(asset)?;
    let report = FetchReport {
        asset: asset.to_uppercase(),
        generated_at: Utc::now().to_rfc3339(),
        chains: chain_results,
    };
    let report_path = out_dir.join("fetch_report.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
    println!("\nReport written to {}", report_path.display());

    write_fetch_risk_flags_md(&out_dir, asset, &report.generated_at, &report.chains)?;
    println!("Written: {}", out_dir.join("risk_flags.md").display());

    if any_hard_error {
        anyhow::bail!(
            "one or more chains had hard errors; partial report at {}",
            report_path.display()
        );
    }

    Ok(())
}

fn write_control_events_csv(
    path: &std::path::Path,
    events: &[crate::control_events::ControlEventRecord],
) -> Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    for ev in events {
        wtr.serialize(ev)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_transfers_csv(
    path: &std::path::Path,
    events: &[crate::decode::TransferEvent],
) -> Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    for ev in events {
        wtr.serialize(ev)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_fetch_risk_flags_md(
    out_dir: &Path,
    asset: &str,
    generated_at: &str,
    chains: &[ChainFetchResult],
) -> Result<()> {
    let mut md = String::new();
    md.push_str("# Risk Flags — Fetch (transfers + control)\n\n");
    md.push_str(&format!(
        "## {} — Generated {}\n\n",
        asset.to_uppercase(),
        generated_at
    ));

    for r in chains {
        md.push_str(&format!(
            "### {} (blocks {} → {})\n",
            r.chain, r.from_block, r.to_block
        ));

        match r.no_duplicate_logs_pass {
            Some(true) => md.push_str("- [PASS] No duplicate transfer logs\n"),
            Some(false) => md.push_str("- [FAIL] Duplicate transfer logs detected\n"),
            None => md.push_str("- [SKIP] Duplicate transfer log check not evaluated\n"),
        }
        match r.transfer_decode_sample_pass {
            Some(true) => md.push_str("- [PASS] Transfer decode QA sample pass\n"),
            Some(false) => md.push_str("- [FAIL] Transfer decode QA sample failed\n"),
            None => md.push_str("- [SKIP] Transfer decode QA sample not evaluated\n"),
        }
        match r.all_transfer_decode_pass {
            Some(true) => md.push_str("- [PASS] All transfer logs decoded\n"),
            Some(false) => md.push_str(&format!(
                "- [FAIL] Full transfer decode had {} error(s)\n",
                r.full_decode_error_count
            )),
            None => md.push_str("- [SKIP] Full transfer decode not evaluated\n"),
        }
        match r.supply_invariant_pass {
            Some(true) => md.push_str("- [PASS] Supply invariant matched\n"),
            Some(false) => md.push_str("- [FAIL] Supply invariant mismatch\n"),
            None => md.push_str("- [WARN] Supply invariant unavailable\n"),
        }

        let qs = r.control_event_query_status.as_str();
        if qs.starts_with("error") {
            md.push_str(&format!("- [WARN] Control event query failed: {qs}\n"));
        } else if qs == "skipped" {
            md.push_str("- [INFO] Control event query skipped\n");
        } else if r.control_event_count == 0 {
            md.push_str("- [INFO] No issuer control events in window\n");
        } else {
            if qs == "partial" {
                md.push_str("- [WARN] Control event query partial (decode errors)\n");
            }
            let p = out_dir.join(format!("control_events_{}.csv", r.chain));
            if p.exists() {
                let mut rdr = csv::Reader::from_path(&p)?;
                for rec in rdr.deserialize::<crate::control_events::ControlEventRecord>() {
                    let ev = rec?;
                    let level = if ev.decode_status == "decode_error" {
                        "[WARN]"
                    } else {
                        match ev.event_name.as_str() {
                            "MinterConfigured" | "MinterRemoved" => "[INFO]",
                            _ => "[WARN]",
                        }
                    };
                    md.push_str(&format!(
                        "- {level} {} ({}) [{}]\n",
                        ev.event_name, ev.args_json, ev.decode_status
                    ));
                }
            }
        }

        for e in &r.errors {
            md.push_str(&format!("- [WARN] {e}\n"));
        }
        md.push('\n');
    }

    md.push_str("---\n\n_Window-scoped fetch audit; not reserve or AML attestation._\n");
    std::fs::write(out_dir.join("risk_flags.md"), md)?;
    Ok(())
}

fn failed_chain_result(
    chain: &str,
    from_block: u64,
    to_block: u64,
    chunk_size: u64,
    errors: Vec<String>,
) -> ChainFetchResult {
    ChainFetchResult {
        chain: chain.to_string(),
        chain_id: 0,
        contract_address: "unknown".into(),
        from_block,
        to_block,
        chunk_size,
        raw_log_count: 0,
        duplicate_count: 0,
        deduped_log_count: 0,
        mint_count: 0,
        burn_count: 0,
        transfer_count: 0,
        qa_sample_size: 0,
        qa_sample_fail_count: 0,
        no_duplicate_logs_pass: None,
        transfer_decode_sample_pass: None,
        full_decode_error_count: 0,
        all_transfer_decode_pass: None,
        control_event_count: 0,
        control_event_query_status: "skipped".into(),
        sum_mints_raw: "0".into(),
        sum_burns_raw: "0".into(),
        total_supply_at_start_minus_1: None,
        total_supply_at_start_minus_1_provenance: "skipped".into(),
        total_supply_at_end: None,
        net_mint_raw: None,
        onchain_delta_raw: None,
        discrepancy_raw: None,
        supply_invariant_pass: None,
        errors,
    }
}
