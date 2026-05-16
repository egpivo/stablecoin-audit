use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy::rpc::types::BlockId;
use alloy::sol;
use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::str::FromStr;

use crate::config::{load_single_token_config, TokenConfig};
use crate::report::{ensure_out_dir, format_token_amount, format_token_amount_pretty};
use crate::rpc::build_provider;

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
struct ChainMetadata {
    chain: String,
    chain_id: u64,
    contract_address: String,
    issuer: String,
    form: String,
    expected_interfaces: Vec<String>,
    name: Option<String>,
    symbol: Option<String>,
    decimals: Option<u8>,
    // Live totalSupply call — not pinned to a specific block. Used for the metadata_call_pass
    // gate only. Do not use this value in supply invariant calculations; use
    // total_supply_at_end (pinned to resolved_end_block) instead.
    total_supply_live_probe: Option<String>,
    total_supply_live_probe_note: String,
    total_supply_at_start_minus_1: Option<String>,
    total_supply_at_start_minus_1_provenance: String,
    total_supply_at_end: Option<String>,
    start_block: u64,
    end_block: Option<u64>,
    resolved_end_block: Option<u64>,
    metadata_call_pass: bool,
    historical_supply_pass: bool,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct MetadataReport {
    asset: String,
    generated_at: String,
    chains: Vec<ChainMetadata>,
}

// Build a fully-failed ChainMetadata when a TokenConfig is available but a later
// step (env lookup, provider init, address parse) failed before any RPC calls.
pub(crate) fn build_failed_chain(
    config: &TokenConfig,
    from_block: u64,
    to_block: Option<u64>,
    errors: Vec<String>,
    reason: &str,
) -> ChainMetadata {
    let note = format!("skipped: {reason}");
    ChainMetadata {
        chain: config.chain.clone(),
        chain_id: config.chain_id,
        contract_address: config.contract_address.clone(),
        issuer: config.issuer.clone(),
        form: config.form.clone(),
        expected_interfaces: config.expected_interfaces.clone(),
        name: None,
        symbol: None,
        decimals: None,
        total_supply_live_probe: None,
        total_supply_live_probe_note: note.clone(),
        total_supply_at_start_minus_1: None,
        total_supply_at_start_minus_1_provenance: note,
        total_supply_at_end: None,
        start_block: from_block,
        end_block: to_block,
        resolved_end_block: None,
        metadata_call_pass: false,
        historical_supply_pass: false,
        errors,
    }
}

// Build a fully-failed ChainMetadata when config loading itself failed,
// so no TokenConfig fields are available.
pub(crate) fn build_config_failed_chain(
    chain: &str,
    from_block: u64,
    to_block: Option<u64>,
    errors: Vec<String>,
) -> ChainMetadata {
    ChainMetadata {
        chain: chain.to_string(),
        chain_id: 0,
        contract_address: "unknown".into(),
        issuer: "unknown".into(),
        form: "unknown".into(),
        expected_interfaces: vec![],
        name: None,
        symbol: None,
        decimals: None,
        total_supply_live_probe: None,
        total_supply_live_probe_note: "skipped: config load failed".into(),
        total_supply_at_start_minus_1: None,
        total_supply_at_start_minus_1_provenance: "skipped: config load failed".into(),
        total_supply_at_end: None,
        start_block: from_block,
        end_block: to_block,
        resolved_end_block: None,
        metadata_call_pass: false,
        historical_supply_pass: false,
        errors,
    }
}

pub async fn run(
    asset: &str,
    chains: &[String],
    from_block: u64,
    to_block: Option<u64>,
) -> Result<()> {
    let mut chain_results: Vec<ChainMetadata> = Vec::new();
    let mut any_hard_error = false;

    for chain in chains {
        // Load config per-chain so a missing or malformed YAML does not abort the whole
        // run. The partial results from successful chains are still written to the report.
        let config = match load_single_token_config(asset, chain) {
            Ok(c) => c,
            Err(e) => {
                let errors = vec![format!("config: {e}")];
                let cm = build_config_failed_chain(chain, from_block, to_block, errors);
                print_chain_summary(&cm, None, None, None, 0, &None, "skipped: config load failed");
                chain_results.push(cm);
                any_hard_error = true;
                continue;
            }
        };

        // Missing env var or malformed URL.
        let rpc_url = match config.rpc_url() {
            Ok(u) => u,
            Err(e) => {
                let errors = vec![format!("env {}: {e}", config.rpc_url_env)];
                let cm = build_failed_chain(&config, from_block, to_block, errors, "env var not set or invalid");
                print_chain_summary(&cm, None, None, None, cm.decimals.unwrap_or(0), &None, &cm.total_supply_at_start_minus_1_provenance.clone());
                chain_results.push(cm);
                any_hard_error = true;
                continue;
            }
        };

        let provider = match build_provider(&rpc_url) {
            Ok(p) => p,
            Err(e) => {
                let errors = vec![format!("build_provider: {e}")];
                let cm = build_failed_chain(&config, from_block, to_block, errors, "provider initialization failed");
                print_chain_summary(&cm, None, None, None, cm.decimals.unwrap_or(0), &None, &cm.total_supply_at_start_minus_1_provenance.clone());
                chain_results.push(cm);
                any_hard_error = true;
                continue;
            }
        };

        let addr = match Address::from_str(&config.contract_address) {
            Ok(a) => a,
            Err(e) => {
                let errors = vec![format!("contract_address '{}': {e}", config.contract_address)];
                let cm = build_failed_chain(&config, from_block, to_block, errors, "malformed contract address");
                print_chain_summary(&cm, None, None, None, cm.decimals.unwrap_or(0), &None, &cm.total_supply_at_start_minus_1_provenance.clone());
                chain_results.push(cm);
                any_hard_error = true;
                continue;
            }
        };
        let mut errors: Vec<String> = Vec::new();
        let mut skip_rpc = false;

        // Verify chain identity before any contract calls. A miswired RPC URL can
        // silently audit the wrong chain if contract addresses happen to exist there.
        match provider.get_chain_id().await {
            Ok(reported_id) if reported_id != config.chain_id => {
                errors.push(format!(
                    "chain_id mismatch: RPC returned {reported_id}, config expects {} for {}; \
                     check .env URL for {}",
                    config.chain_id, config.chain, config.rpc_url_env
                ));
                skip_rpc = true;
                any_hard_error = true;
            }
            Err(e) => {
                errors.push(format!("eth_chainId failed: {e}"));
                skip_rpc = true;
                any_hard_error = true;
            }
            Ok(_) => {}
        }

        // Resolve end block. Failure is a hard per-chain precondition: we cannot define
        // the audit window. We capture the error, write a partial result for this chain,
        // and continue with remaining chains. The command exits nonzero at the end.
        let resolved_end_block: Option<u64> = if skip_rpc {
            None
        } else {
            match to_block {
                Some(b) => Some(b),
                None => match provider.get_block_number().await {
                    Ok(n) => Some(n),
                    Err(e) => {
                        errors.push(format!(
                            "get_block_number failed: {e}; cannot determine audit end block"
                        ));
                        any_hard_error = true;
                        None
                    }
                },
            }
        };

        let contract = IERC20::new(addr, &provider);

        // Live totalSupply call — not pinned to a block. Used only for metadata_call_pass.
        let (supply_live, live_probe_note) = if skip_rpc {
            (None, "skipped: chain verification failed".into())
        } else {
            match contract.totalSupply().call().await {
                Ok(r) => (
                    Some(r._0),
                    "live call at provider latest block; not pinned to window end block".into(),
                ),
                Err(e) => {
                    errors.push(format!("totalSupply() live: {e}"));
                    (None, "rpc-error".into())
                }
            }
        };

        let name_val = if skip_rpc {
            None
        } else {
            match contract.name().call().await {
                Ok(r) => Some(r._0),
                Err(e) => {
                    errors.push(format!("name(): {e}"));
                    None
                }
            }
        };

        let symbol_val = if skip_rpc {
            None
        } else {
            match contract.symbol().call().await {
                Ok(r) => Some(r._0),
                Err(e) => {
                    errors.push(format!("symbol(): {e}"));
                    None
                }
            }
        };

        let decimals_val = if skip_rpc {
            None
        } else {
            match contract.decimals().call().await {
                Ok(r) => Some(r._0),
                Err(e) => {
                    errors.push(format!("decimals(): {e}"));
                    None
                }
            }
        };

        // Historical totalSupply at start_block - 1.
        // If start_minus_1 is before the contract's deployment_block, the contract doesn't
        // exist at that block; querying it returns an execution error on most RPCs.
        // The correct opening supply is definitionally 0 — use that and record provenance
        // explicitly so the gate can [PASS] without a misleading on-chain call.
        let start_minus_1 = from_block.saturating_sub(1);
        let (supply_start, supply_start_provenance): (Option<U256>, String) = if skip_rpc {
            (None, "skipped: chain verification failed".into())
        } else if start_minus_1 == 0 {
            (Some(U256::ZERO), "genesis (block 0)".into())
        } else if config.deployment_block.is_some_and(|d| start_minus_1 < d) {
            let deploy = config.deployment_block.unwrap();
            (
                Some(U256::ZERO),
                format!(
                    "pre-deployment zero: block {start_minus_1} < deployment_block {deploy}"
                ),
            )
        } else {
            let block_id = BlockId::number(start_minus_1);
            match contract.totalSupply().block(block_id).call().await {
                Ok(r) => (Some(r._0), "on-chain".into()),
                Err(e) => {
                    errors.push(format!("totalSupply() at block {start_minus_1}: {e}"));
                    (None, "rpc-error".into())
                }
            }
        };

        // Historical totalSupply at end block — pinned, used in supply invariant.
        let supply_end = match resolved_end_block {
            None => None,
            Some(end_blk) => {
                let block_id = BlockId::number(end_blk);
                match contract.totalSupply().block(block_id).call().await {
                    Ok(r) => Some(r._0),
                    Err(e) => {
                        errors.push(format!("totalSupply() at block {end_blk}: {e}"));
                        None
                    }
                }
            }
        };

        let effective_decimals = decimals_val.unwrap_or(config.decimals);

        let metadata_call_pass = name_val.is_some()
            && symbol_val.is_some()
            && decimals_val.is_some()
            && supply_live.is_some();

        let historical_supply_pass = supply_start.is_some() && supply_end.is_some();

        let to_decimal_str =
            |v: Option<U256>| -> Option<String> { v.map(|u| format_token_amount(u, effective_decimals)) };

        let cm = ChainMetadata {
            chain: config.chain.clone(),
            chain_id: config.chain_id,
            contract_address: config.contract_address.clone(),
            issuer: config.issuer.clone(),
            form: config.form.clone(),
            expected_interfaces: config.expected_interfaces.clone(),
            name: name_val.clone(),
            symbol: symbol_val.clone(),
            decimals: decimals_val,
            total_supply_live_probe: to_decimal_str(supply_live),
            total_supply_live_probe_note: live_probe_note,
            total_supply_at_start_minus_1: to_decimal_str(supply_start),
            total_supply_at_start_minus_1_provenance: supply_start_provenance.clone(),
            total_supply_at_end: to_decimal_str(supply_end),
            start_block: from_block,
            end_block: to_block,
            resolved_end_block,
            metadata_call_pass,
            historical_supply_pass,
            errors: errors.clone(),
        };

        print_chain_summary(
            &cm,
            supply_live,
            supply_start,
            supply_end,
            effective_decimals,
            &symbol_val,
            &supply_start_provenance,
        );

        chain_results.push(cm);
    }

    let report = MetadataReport {
        asset: asset.to_uppercase(),
        generated_at: Utc::now().to_rfc3339(),
        chains: chain_results,
    };

    let out_dir = ensure_out_dir(asset)?;
    let out_path = out_dir.join("metadata.json");
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&out_path, &json)?;
    println!("\nReport written to {}", out_path.display());

    // Always write the report first so partial results are available for inspection,
    // then signal failure so the caller sees a nonzero exit.
    if any_hard_error {
        anyhow::bail!(
            "one or more chains had hard errors (config load failed, env var missing, \
             malformed contract address, chain_id mismatch, or block resolution failure); \
             partial report at {}",
            out_path.display()
        );
    }

    Ok(())
}

pub(crate) fn print_chain_summary(
    cm: &ChainMetadata,
    supply_live: Option<U256>,
    supply_start: Option<U256>,
    supply_end: Option<U256>,
    decimals: u8,
    symbol: &Option<String>,
    supply_start_provenance: &str,
) {
    let sym = symbol.as_deref().unwrap_or("?");
    println!(
        "\n=== {} (chain_id: {}) ===",
        cm.chain.to_uppercase(),
        cm.chain_id
    );
    println!("  Contract : {}", cm.contract_address);
    println!("  Issuer   : {} ({})", cm.issuer, cm.form);
    println!("  Name     : {}", cm.name.as_deref().unwrap_or("<error>"));
    println!("  Symbol   : {}", sym);
    println!(
        "  Decimals : {}",
        cm.decimals
            .map(|d| d.to_string())
            .unwrap_or_else(|| "<error>".into())
    );
    println!(
        "  Window   : block {} → block {}",
        cm.start_block,
        cm.resolved_end_block
            .map(|b| b.to_string())
            .unwrap_or_else(|| "?".into())
    );

    let fmt = |v: Option<U256>| -> String {
        match v {
            Some(u) => format_token_amount_pretty(u, decimals, sym),
            None => "<error>".into(),
        }
    };

    println!(
        "  totalSupply (live probe, unpinned) : {}",
        fmt(supply_live)
    );
    println!(
        "  totalSupply (block {})  : {}  [{}]",
        cm.start_block.saturating_sub(1),
        fmt(supply_start),
        supply_start_provenance,
    );
    println!(
        "  totalSupply (block {})  : {}",
        cm.resolved_end_block.unwrap_or(0),
        fmt(supply_end)
    );

    let gate = |pass: bool| if pass { "[PASS]" } else { "[FAIL]" };
    println!("  metadata_call_pass     : {}", gate(cm.metadata_call_pass));
    println!(
        "  historical_supply_pass : {}",
        gate(cm.historical_supply_pass)
    );

    if !cm.errors.is_empty() {
        println!("  Errors:");
        for e in &cm.errors {
            println!("    - {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U256;

    #[test]
    fn build_failed_chain_sets_skip_note() {
        let cfg = crate::config::load_single_token_config("USDC", "ethereum").unwrap();
        let cm = build_failed_chain(&cfg, 100, Some(200), vec!["rpc down".into()], "test");
        assert!(!cm.metadata_call_pass);
        assert!(cm.total_supply_live_probe_note.contains("skipped"));
    }

    #[test]
    fn build_config_failed_chain_unknown_contract() {
        let cm = build_config_failed_chain("badchain", 1, None, vec!["x".into()]);
        assert_eq!(cm.contract_address, "unknown");
        assert_eq!(cm.chain_id, 0);
    }

    #[test]
    fn print_chain_summary_runs() {
        let cfg = crate::config::load_single_token_config("USDC", "ethereum").unwrap();
        let cm = build_failed_chain(&cfg, 100, Some(200), vec![], "test");
        print_chain_summary(
            &cm,
            Some(U256::from(1_000_000u64)),
            Some(U256::ZERO),
            Some(U256::from(2_000_000u64)),
            6,
            &Some("USDC".into()),
            "on-chain",
        );
    }

    #[tokio::test]
    async fn metadata_run_partial_on_bad_chain() {
        let err = run("USDC", &["not_a_chain_xyz".into()], 100, Some(200)).await;
        assert!(err.is_err());
        let path = crate::report::ensure_out_dir("USDC").unwrap().join("metadata.json");
        assert!(path.is_file());
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn metadata_run_unreachable_rpc() {
        let _lock = crate::rpc::RPC_ENV_LOCK.lock().unwrap();
        let key = "ALCHEMY_ETHEREUM_URL";
        let saved = std::env::var(key).ok();
        std::env::set_var(key, "http://127.0.0.1:1");
        let err = run("USDC", &["ethereum".into()], 24_000_000, Some(24_001_000)).await;
        assert!(err.is_err());
        if let Some(v) = saved {
            std::env::set_var(key, v);
        } else {
            std::env::remove_var(key);
        }
    }
}
