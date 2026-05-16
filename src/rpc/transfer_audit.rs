//! Transfer-log audit: chunked `eth_getLogs`, decode, dedup, supply reconciliation.

use alloy::eips::BlockNumberOrTag;
use alloy::primitives::{Address, I256, U256};
use alloy::providers::Provider;
use alloy::rpc::types::{BlockId, BlockTransactionsKind};
use alloy::sol;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::str::FromStr;

use crate::config::{load_single_token_config, TokenConfig};
use crate::decode::{decode_transfer_log, dedup_transfer_events, sample_decode_qa};
use crate::fetch::{fetch_transfer_logs_incremental, FetchParams};
use crate::report::{default_run_id, ensure_run_out_dir, format_token_amount, validate_run_id};
use crate::rpc::transfer_checkpoint::{
    self, ChainSpecRecord, CheckpointChainBundle, CheckpointManifest, FetchChunkProgress,
};
use crate::rpc::{build_provider, HttpProvider};
use std::path::Path;

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
    run_id: String,
    provenance: ProvenanceBlock,
    chains: Vec<QaChain>,
}

#[derive(Clone, Serialize)]
struct ProvenanceBlock {
    /// When `per_chain_spans` is true, each chain row carries its own `from_block` / `to_block_requested`;
    /// this field is the minimum `from_block` across chains (display / fingerprint hint only).
    from_block: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_block_requested: Option<String>,
    generated_at: String,
    /// True when the run used `--window chain:from:to` (independent block heights per chain).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    per_chain_spans: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaChain {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaGates {
    metadata_call_pass: String,
    historical_supply_pass: String,
    no_duplicate_logs_pass: String,
    transfer_decode_pass: String,
    supply_invariant_pass: String,
    provenance_stamped: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyAuditRow {
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
    /// Block header time for `from_block` (window start); not written to CSV.
    #[serde(skip)]
    window_start_block_timestamp_rfc3339: Option<String>,
    /// Block header time for the resolved end block of the log window; not written to CSV.
    #[serde(skip)]
    window_end_block_timestamp_rfc3339: Option<String>,
}

pub(crate) fn gate_csv(pass: bool) -> &'static str {
    if pass {
        "PASS"
    } else {
        "FAIL"
    }
}

pub(crate) fn gate_opt_csv(pass: Option<bool>) -> &'static str {
    match pass {
        Some(true) => "PASS",
        Some(false) => "FAIL",
        None => "UNAVAILABLE",
    }
}

/// Mint/burn aggregates vs pinned `totalSupply` boundaries (signed I256).
pub(crate) fn compute_supply_invariant(
    sum_mints: U256,
    sum_burns: U256,
    supply_start: U256,
    supply_end: U256,
) -> (I256, I256, I256, bool) {
    let net_mint = I256::from_raw(sum_mints) - I256::from_raw(sum_burns);
    let onchain_delta = I256::from_raw(supply_end) - I256::from_raw(supply_start);
    let discrepancy = net_mint - onchain_delta;
    (
        net_mint,
        onchain_delta,
        discrepancy,
        discrepancy == I256::ZERO,
    )
}

/// Parse `--window chain:from:to` (inclusive end block `to`, same convention as `--to-block`).
pub fn parse_window_arg(s: &str) -> Result<(String, u64, u64)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        anyhow::bail!(
            "expected --window chain:from:to with exactly two ':' separators; got {:?}",
            s
        );
    }
    let chain = parts[0].trim().to_string();
    if chain.is_empty() {
        anyhow::bail!("empty chain in --window {:?}", s);
    }
    if !chain
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "chain in --window {:?} must be alphanumeric / hyphen / underscore",
            s
        );
    }
    let from = parts[1]
        .trim()
        .parse::<u64>()
        .map_err(|_| anyhow::anyhow!("invalid from_block in --window {:?}", s))?;
    let to = parts[2]
        .trim()
        .parse::<u64>()
        .map_err(|_| anyhow::anyhow!("invalid to_block in --window {:?}", s))?;
    if from == 0 {
        anyhow::bail!("--window from_block 0 is not supported");
    }
    if to < from {
        anyhow::bail!("--window to_block ({to}) must be >= from_block ({from})");
    }
    Ok((chain, from, to))
}

async fn block_window_timestamp_rfc3339(
    provider: &HttpProvider,
    block_number: u64,
) -> Option<String> {
    let tag = BlockNumberOrTag::from(block_number);
    match provider
        .get_block_by_number(tag, BlockTransactionsKind::Hashes)
        .await
    {
        Ok(Some(b)) => {
            let ts = b.header.timestamp;
            chrono::DateTime::from_timestamp(ts as i64, 0)
                .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        }
        _ => None,
    }
}

pub async fn run(
    asset: &str,
    chains: &[String],
    from_block: u64,
    to_block_raw: &str,
    chunk_size: Option<u64>,
    run_id: Option<String>,
    fresh: bool,
) -> Result<()> {
    run_inner(
        asset,
        RunMode::Unified {
            chains,
            from_block,
            to_block_raw,
        },
        chunk_size,
        run_id,
        fresh,
    )
    .await
}

pub async fn run_per_chain_windows(
    asset: &str,
    windows: Vec<(String, u64, u64)>,
    chunk_size: Option<u64>,
    run_id: Option<String>,
    fresh: bool,
) -> Result<()> {
    run_inner(asset, RunMode::PerChain(windows), chunk_size, run_id, fresh).await
}

pub(crate) enum RunMode<'a> {
    Unified {
        chains: &'a [String],
        from_block: u64,
        to_block_raw: &'a str,
    },
    PerChain(Vec<(String, u64, u64)>),
}

pub(crate) struct ChainTask {
    pub chain: String,
    pub from_block: u64,
    pub to_block_requested: String,
    /// `None` → resolve `--to-block latest` at run time; `Some(n)` → fixed end block.
    pub fixed_to_block: Option<u64>,
}

pub(crate) struct RunPlan {
    pub per_chain_spans: bool,
    pub provenance_from_block: u64,
    pub provenance_to_block_requested: Option<String>,
    pub chain_tasks: Vec<ChainTask>,
    pub spec_records: Vec<ChainSpecRecord>,
}

pub(crate) fn build_run_plan(mode: RunMode<'_>) -> Result<RunPlan> {
    match mode {
        RunMode::Unified {
            chains,
            from_block,
            to_block_raw,
        } => {
            let fixed_to_block = if to_block_raw.eq_ignore_ascii_case("latest") {
                None
            } else {
                Some(to_block_raw.parse::<u64>().map_err(|_| {
                    anyhow::anyhow!(
                        "--to-block must be a positive integer or 'latest'; got {:?}",
                        to_block_raw
                    )
                })?)
            };
            let to_req = if to_block_raw.eq_ignore_ascii_case("latest") {
                "latest".to_string()
            } else {
                to_block_raw.to_string()
            };
            let chain_tasks: Vec<ChainTask> = chains
                .iter()
                .map(|c| ChainTask {
                    chain: c.clone(),
                    from_block,
                    to_block_requested: to_req.clone(),
                    fixed_to_block,
                })
                .collect();
            let spec_records: Vec<ChainSpecRecord> = chain_tasks
                .iter()
                .map(|t| ChainSpecRecord {
                    chain: t.chain.clone(),
                    from_block: t.from_block,
                    to_block_requested: t.to_block_requested.clone(),
                })
                .collect();
            Ok(RunPlan {
                per_chain_spans: false,
                provenance_from_block: from_block,
                provenance_to_block_requested: Some(to_req),
                chain_tasks,
                spec_records,
            })
        }
        RunMode::PerChain(mut windows) => {
            windows.sort_by(|a, b| a.0.cmp(&b.0));
            let mut seen = HashSet::new();
            for (c, _, _) in &windows {
                if !seen.insert(c.as_str()) {
                    anyhow::bail!("duplicate chain {:?} in --window arguments", c);
                }
            }
            let min_from = windows
                .iter()
                .map(|(_, f, _)| *f)
                .min()
                .expect("windows non-empty");
            let chain_tasks: Vec<ChainTask> = windows
                .into_iter()
                .map(|(chain, from_b, to_b)| {
                    let to_raw = to_b.to_string();
                    ChainTask {
                        chain,
                        from_block: from_b,
                        to_block_requested: to_raw.clone(),
                        fixed_to_block: Some(to_b),
                    }
                })
                .collect();
            let spec_records: Vec<ChainSpecRecord> = chain_tasks
                .iter()
                .map(|t| ChainSpecRecord {
                    chain: t.chain.clone(),
                    from_block: t.from_block,
                    to_block_requested: t.to_block_requested.clone(),
                })
                .collect();
            Ok(RunPlan {
                per_chain_spans: true,
                provenance_from_block: min_from,
                provenance_to_block_requested: Some("per_chain".into()),
                chain_tasks,
                spec_records,
            })
        }
    }
}

pub(crate) fn build_provenance_md_intro(
    per_chain_spans: bool,
    supply_rows: &[SupplyAuditRow],
) -> String {
    if per_chain_spans {
        let mut intro = String::from(
            "**Per-chain block spans** — each L2/L1 uses its own block height; \
numbers are not comparable across chains, but metrics use one schema.\n\n",
        );
        for row in supply_rows {
            intro.push_str(&format!(
                "- **{}:** blocks `{}` → `{}` (resolved end {:?})\n",
                row.chain, row.from_block, row.to_block_requested, row.resolved_to_block
            ));
        }
        intro.push('\n');
        intro
    } else if let Some(r0) = supply_rows.first() {
        format!(
            "- **from_block:** {}\n- **to_block (requested):** {}\n\n",
            r0.from_block, r0.to_block_requested
        )
    } else {
        String::new()
    }
}

async fn run_inner(
    asset: &str,
    mode: RunMode<'_>,
    chunk_size: Option<u64>,
    run_id: Option<String>,
    fresh: bool,
) -> Result<()> {
    let chunk_size = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE);
    let run_id = match run_id {
        Some(r) => {
            validate_run_id(&r)?;
            r
        }
        None => default_run_id(),
    };
    let out_dir = ensure_run_out_dir(asset, &run_id)?;
    let plan = build_run_plan(mode)?;

    if fresh {
        transfer_checkpoint::clear_checkpoint_dir(&out_dir)?;
        println!("--fresh: cleared checkpoint under {}", out_dir.display());
    }

    let existing_manifest = if fresh {
        None
    } else {
        transfer_checkpoint::load_manifest(&out_dir)?
    };

    let generated_at = existing_manifest
        .as_ref()
        .map(|m| m.started_at.clone())
        .unwrap_or_else(|| Utc::now().to_rfc3339());

    let mut manifest = match existing_manifest {
        Some(m) => {
            transfer_checkpoint::validate_manifest_matches(
                &m,
                asset,
                &run_id,
                chunk_size,
                plan.per_chain_spans,
                &plan.spec_records,
            )?;
            if !m.completed_chains.is_empty() {
                println!(
                    "\nResuming run `{run_id}` — skipping checkpointed chains: {}",
                    m.completed_chains.join(", ")
                );
            }
            m
        }
        None => {
            let m = CheckpointManifest::new(
                asset,
                &run_id,
                &generated_at,
                chunk_size,
                plan.per_chain_spans,
                plan.spec_records.clone(),
            );
            transfer_checkpoint::save_manifest(&out_dir, &m)?;
            m
        }
    };

    let completed = transfer_checkpoint::completed_set(&manifest);
    let mut all_events: Vec<crate::decode::TransferEvent> = Vec::new();
    let mut supply_rows: Vec<SupplyAuditRow> = Vec::new();
    let mut qa_chains: Vec<QaChain> = Vec::new();
    let mut any_hard_error = false;

    for task in &plan.chain_tasks {
        if completed.contains(&task.chain) {
            let (events, bundle) =
                transfer_checkpoint::load_completed_chain(&out_dir, &task.chain)?;
            println!(
                "[{}] loaded from checkpoint ({} transfers)",
                task.chain.to_uppercase(),
                events.len()
            );
            all_events.extend(events);
            supply_rows.push(bundle.supply);
            qa_chains.push(bundle.qa);
            continue;
        }

        let (events, supply, qa, hard_err) = process_chain(
            asset,
            &task.chain,
            task.from_block,
            task.fixed_to_block,
            chunk_size,
            &task.to_block_requested,
            &generated_at,
            &out_dir,
        )
        .await;

        if hard_err {
            any_hard_error = true;
            all_events.extend(events);
            supply_rows.push(supply);
            qa_chains.push(qa);
        } else {
            let bundle = CheckpointChainBundle {
                supply: supply.clone(),
                qa: qa.clone(),
            };
            transfer_checkpoint::save_completed_chain(
                &out_dir,
                &mut manifest,
                &task.chain,
                &events,
                &bundle,
            )?;
            all_events.extend(events);
            supply_rows.push(supply);
            qa_chains.push(qa);
        }
    }

    let provenance = ProvenanceBlock {
        from_block: plan.provenance_from_block,
        to_block_requested: plan.provenance_to_block_requested.clone(),
        generated_at: generated_at.clone(),
        per_chain_spans: plan.per_chain_spans,
    };

    let mut pairs: Vec<(SupplyAuditRow, QaChain)> =
        supply_rows.into_iter().zip(qa_chains).collect();
    pairs.sort_by(|a, b| a.0.chain.cmp(&b.0.chain));
    let (supply_rows, qa_chains): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();

    write_run_artifacts(
        &out_dir,
        asset,
        &run_id,
        &generated_at,
        plan.per_chain_spans,
        provenance.from_block,
        provenance.to_block_requested.clone(),
        &all_events,
        &supply_rows,
        &qa_chains,
    )?;

    println!(
        "\nRun id: {}\nOutputs written under {}:",
        run_id,
        out_dir.display()
    );
    println!(
        "  decoded_transfers.csv, supply_audit.csv, supply_audit.md, qa_report.json,\n  provenance.json, summary.md"
    );
    println!(
        "\nNext (≥2 chains): cargo run -- cross-chain-summary --asset {} --run-id {}",
        asset.to_uppercase(),
        run_id
    );

    if any_hard_error {
        println!(
            "\nCheckpoint preserved at {}/checkpoint/ — re-run the same command (same --run-id, same --window) to resume finished chains.",
            out_dir.display()
        );
        anyhow::bail!(
            "one or more chains had hard errors; partial outputs written under {}",
            out_dir.display()
        );
    }

    transfer_checkpoint::remove_checkpoint_dir(&out_dir)?;
    println!("Checkpoint cleared (run completed successfully).");

    Ok(())
}

#[derive(Serialize)]
struct TransferProvenanceJson {
    schema: &'static str,
    asset: String,
    run_id: String,
    generated_at: String,
    per_chain_spans: bool,
    chains: Vec<TransferProvenanceChain>,
}

#[derive(Serialize)]
struct TransferProvenanceChain {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    to_block_requested: String,
    resolved_to_block: Option<u64>,
    /// Block header timestamp (UTC) for `from_block`.
    window_start_block_timestamp_rfc3339: Option<String>,
    /// Block header timestamp (UTC) for the resolved end block of the log window.
    window_end_block_timestamp_rfc3339: Option<String>,
}

fn write_transfer_provenance_json(
    out_dir: &std::path::Path,
    asset: &str,
    run_id: &str,
    generated_at: &str,
    per_chain_spans: bool,
    rows: &[SupplyAuditRow],
) -> Result<()> {
    let chains = rows
        .iter()
        .map(|r| TransferProvenanceChain {
            chain: r.chain.clone(),
            chain_id: r.chain_id,
            contract_address: r.contract_address.clone(),
            from_block: r.from_block,
            to_block_requested: r.to_block_requested.clone(),
            resolved_to_block: r.resolved_to_block,
            window_start_block_timestamp_rfc3339: r.window_start_block_timestamp_rfc3339.clone(),
            window_end_block_timestamp_rfc3339: r.window_end_block_timestamp_rfc3339.clone(),
        })
        .collect();
    let doc = TransferProvenanceJson {
        schema: "transfer-audit-provenance-v1",
        asset: asset.to_uppercase(),
        run_id: run_id.to_string(),
        generated_at: generated_at.to_string(),
        per_chain_spans,
        chains,
    };
    let path = out_dir.join("provenance.json");
    std::fs::write(&path, serde_json::to_string_pretty(&doc)?)?;
    Ok(())
}

fn write_transfer_summary_md(
    out_dir: &std::path::Path,
    asset: &str,
    run_id: &str,
    generated_at: &str,
    per_chain_spans: bool,
    rows: &[SupplyAuditRow],
    qa: &[QaChain],
) -> Result<()> {
    let mut md = String::new();
    md.push_str(&format!(
        "# {} — transfer-audit summary\n\n",
        asset.to_uppercase()
    ));
    md.push_str(&format!("**Run id:** `{}`\n\n", run_id));
    md.push_str(&format!("**Generated:** {}\n\n", generated_at));
    if per_chain_spans {
        md.push_str(
            "**Window:** per-chain block spans (see `provenance.json` or per-chain rows below). \
Block heights are chain-native and not numerically comparable across chains.\n\n",
        );
    } else if let Some(r0) = rows.first() {
        md.push_str(&format!(
            "**Window:** from_block `{}` → to_block_requested `{}` (per-chain resolved end may vary if `latest`).\n\n",
            r0.from_block, r0.to_block_requested
        ));
    }

    md.push_str("## Chain overview\n\n");
    md.push_str("| Chain | Chain ID | Contract | from → requested to | resolved end |\n");
    md.push_str("|-------|---------:|----------|--------------------:|-------------:|\n");
    for row in rows {
        md.push_str(&format!(
            "| {} | {} | `{}` | {} → {} | {} |\n",
            row.chain,
            row.chain_id,
            row.contract_address,
            row.from_block,
            row.to_block_requested,
            row.resolved_to_block
                .map(|b| b.to_string())
                .unwrap_or_else(|| "—".into())
        ));
    }
    md.push('\n');

    md.push_str("## Supply (window)\n\n");
    md.push_str("| Chain | totalSupply @ start−1 | totalSupply @ end | on-chain Δ (signed) |\n");
    md.push_str("|-------|----------------------|-------------------|---------------------|\n");
    for row in rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            row.chain,
            row.total_supply_at_start_minus_1.as_deref().unwrap_or("—"),
            row.total_supply_at_end.as_deref().unwrap_or("—"),
            row.onchain_delta_raw.as_deref().unwrap_or("—"),
        ));
    }
    md.push('\n');

    md.push_str("## Mint / burn / transfers (deduped)\n\n");
    md.push_str("| Chain | Transfers | Mints | Burns | Plain | Net mint (raw) |\n");
    md.push_str("|-------|----------:|------:|------:|------:|---------------:|\n");
    for row in rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            row.chain,
            row.transfer_event_count,
            row.mint_count,
            row.burn_count,
            row.plain_transfer_count,
            row.net_mint_raw.as_deref().unwrap_or("—"),
        ));
    }
    md.push('\n');

    md.push_str("## QA gates\n\n");
    md.push_str(
        "| Chain | metadata | hist_supply | no_dup | decode | supply_inv | provenance_stamp |\n",
    );
    md.push_str(
        "|-------|----------|-------------|--------|--------|------------|------------------|\n",
    );
    for (row, q) in rows.iter().zip(qa.iter()) {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            row.chain,
            q.gates.metadata_call_pass,
            q.gates.historical_supply_pass,
            q.gates.no_duplicate_logs_pass,
            q.gates.transfer_decode_pass,
            q.gates.supply_invariant_pass,
            q.gates.provenance_stamped,
        ));
    }
    md.push('\n');

    md.push_str("---\n\n");
    md.push_str(
        "> **Scope:** On-chain accounting in the declared block window(s) only. \
This is not a reserve audit, peg or purchasing-power analysis, chain safety ranking, \
or holder/identity attribution.\n\n\
> **Comparable claim:** Under one schema, per-deployment supply movement and QA gates \
can be read side-by-side for the same asset symbol.\n",
    );

    std::fs::write(out_dir.join("summary.md"), md)?;
    Ok(())
}

/// Deduped transfer aggregates and supply-invariant fields (no RPC).
pub(crate) struct SupplyMetrics {
    pub deduped: Vec<crate::decode::TransferEvent>,
    pub duplicate_count: usize,
    pub mint_count: usize,
    pub burn_count: usize,
    pub plain_transfer_count: usize,
    pub active_senders: usize,
    pub active_recipients: usize,
    pub sum_mints: U256,
    pub sum_burns: U256,
    pub net_mint: Option<I256>,
    pub onchain_delta: Option<I256>,
    pub discrepancy: Option<I256>,
    pub supply_invariant_pass: Option<bool>,
    pub no_duplicate_logs_pass: bool,
    pub transfer_decode_pass: bool,
}

pub(crate) fn build_supply_metrics_from_events(
    events: Vec<crate::decode::TransferEvent>,
    decode_errors: usize,
    supply_start: Option<U256>,
    supply_end: Option<U256>,
) -> SupplyMetrics {
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

    let (net_mint_opt, onchain_delta_opt, discrepancy_opt, invariant_pass) = if decode_errors > 0 {
        (None, None, None, None)
    } else {
        match (supply_start, supply_end) {
            (Some(start), Some(end)) => {
                let (net_mint, onchain_delta, discrepancy, pass) =
                    compute_supply_invariant(sum_mints, sum_burns, start, end);
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

    SupplyMetrics {
        deduped,
        duplicate_count: dup_count,
        mint_count,
        burn_count,
        plain_transfer_count,
        active_senders: senders.len(),
        active_recipients: recipients.len(),
        sum_mints,
        sum_burns,
        net_mint: net_mint_opt,
        onchain_delta: onchain_delta_opt,
        discrepancy: discrepancy_opt,
        supply_invariant_pass: invariant_pass,
        no_duplicate_logs_pass: dup_count == 0,
        transfer_decode_pass: decode_errors == 0,
    }
}

/// Write transfer-audit artifacts (CSV/MD/JSON) for a completed run.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_run_artifacts(
    out_dir: &Path,
    asset: &str,
    run_id: &str,
    generated_at: &str,
    per_chain_spans: bool,
    provenance_from_block: u64,
    provenance_to_block_requested: Option<String>,
    all_events: &[crate::decode::TransferEvent],
    supply_rows: &[SupplyAuditRow],
    qa_chains: &[QaChain],
) -> Result<()> {
    let provenance = ProvenanceBlock {
        from_block: provenance_from_block,
        to_block_requested: provenance_to_block_requested,
        generated_at: generated_at.to_string(),
        per_chain_spans,
    };
    let provenance_md_intro = build_provenance_md_intro(per_chain_spans, supply_rows);

    write_decoded_transfers_csv(out_dir, all_events)?;
    write_supply_audit_csv(out_dir, supply_rows)?;
    write_supply_audit_md(
        out_dir,
        asset,
        run_id,
        generated_at,
        &provenance_md_intro,
        supply_rows,
        qa_chains,
    )?;

    let qa_report = QaReport {
        asset: asset.to_uppercase(),
        generated_at: generated_at.to_string(),
        run_id: run_id.to_string(),
        provenance: provenance.clone(),
        chains: qa_chains.to_vec(),
    };
    std::fs::write(
        out_dir.join("qa_report.json"),
        serde_json::to_string_pretty(&qa_report)?,
    )?;

    write_transfer_provenance_json(
        out_dir,
        asset,
        run_id,
        generated_at,
        per_chain_spans,
        supply_rows,
    )?;
    write_transfer_summary_md(
        out_dir,
        asset,
        run_id,
        generated_at,
        per_chain_spans,
        supply_rows,
        qa_chains,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn process_chain(
    asset: &str,
    chain: &str,
    from_block: u64,
    to_block: Option<u64>,
    chunk_size: u64,
    to_block_raw: &str,
    generated_at: &str,
    out_dir: &Path,
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
            window_start_block_timestamp_rfc3339: None,
            window_end_block_timestamp_rfc3339: None,
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

    let (win_start_ts, win_end_ts) = if skip_rpc {
        (None, None)
    } else {
        (
            block_window_timestamp_rfc3339(&provider, from_block).await,
            block_window_timestamp_rfc3339(&provider, end_blk).await,
        )
    };

    let params = FetchParams {
        contract_address: addr,
        from_block,
        to_block: end_blk,
        chunk_size,
    };

    let contract_str = config.contract_address.clone();
    let total_chunks = transfer_checkpoint::count_chunks(from_block, end_blk, chunk_size);

    let fetch_progress = match transfer_checkpoint::load_fetch_progress(
        out_dir,
        chain,
        &contract_str,
        from_block,
        end_blk,
        chunk_size,
    ) {
        Ok(p) => p,
        Err(e) => {
            errors.push(format!("fetch checkpoint: {e:#}"));
            hard_error = true;
            let supply = partial_supply_after_fetch_fail(
                chain,
                &config,
                from_block,
                end_blk,
                to_block_raw,
                chunk_size,
                metadata_call_pass,
                historical_supply_pass,
                &supply_start,
                &supply_start_provenance,
                &supply_end,
                effective_decimals,
                &win_start_ts,
                &win_end_ts,
            );
            let qa = build_qa_chain(&supply, generated_at, &errors);
            return (empty_events, supply, qa, hard_error);
        }
    };

    if fetch_progress.is_none() && transfer_checkpoint::fetch_partial_path(out_dir, chain).exists()
    {
        transfer_checkpoint::clear_chain_fetch_progress(out_dir, chain).ok();
    }

    let fetch_already_done = fetch_progress.as_ref().is_some_and(|p| p.is_complete());

    let mut events = if fetch_already_done {
        match transfer_checkpoint::load_fetch_partial_events(out_dir, chain) {
            Ok(e) => e,
            Err(e) => {
                errors.push(format!("load partial transfers: {e:#}"));
                hard_error = true;
                Vec::new()
            }
        }
    } else if let Some(ref p) = fetch_progress {
        match transfer_checkpoint::load_fetch_partial_events(out_dir, chain) {
            Ok(e) => {
                println!(
                    "[{}] resuming log fetch from block {} (chunk {}/{})",
                    chain.to_uppercase(),
                    p.resume_from_block(),
                    p.chunks_done,
                    p.total_chunks
                );
                e
            }
            Err(e) => {
                errors.push(format!("load partial transfers: {e:#}"));
                hard_error = true;
                Vec::new()
            }
        }
    } else {
        println!(
            "[{}] fetching Transfer logs block {} → {} (chunk {}, {} chunks)",
            chain.to_uppercase(),
            from_block,
            end_blk,
            chunk_size,
            total_chunks
        );
        Vec::new()
    };

    if hard_error && events.is_empty() {
        let supply = partial_supply_after_fetch_fail(
            chain,
            &config,
            from_block,
            end_blk,
            to_block_raw,
            chunk_size,
            metadata_call_pass,
            historical_supply_pass,
            &supply_start,
            &supply_start_provenance,
            &supply_end,
            effective_decimals,
            &win_start_ts,
            &win_end_ts,
        );
        let qa = build_qa_chain(&supply, generated_at, &errors);
        return (empty_events, supply, qa, true);
    }

    let resume_from = fetch_progress
        .as_ref()
        .filter(|p| !p.is_complete())
        .map(|p| p.resume_from_block())
        .unwrap_or(from_block);

    let mut logs_fetched: usize = fetch_progress.as_ref().map(|p| p.logs_fetched).unwrap_or(0);
    let mut decode_errors = 0usize;
    let mut qa_sample_done = fetch_already_done || fetch_progress.is_some();

    if !fetch_already_done && !hard_error {
        let chain_upper = chain.to_uppercase();
        let out_owned = out_dir.to_path_buf();
        let chain_owned = chain.to_string();

        let fetch_result = fetch_transfer_logs_incremental(
            &provider,
            &params,
            resume_from,
            |start, end, chunks_done, total_chunks, logs| {
                println!(
                    "[{}] fetch chunk {}/{} blocks {}..{} ({} logs)",
                    chain_upper,
                    chunks_done,
                    total_chunks,
                    start,
                    end,
                    logs.len()
                );

                if !qa_sample_done && !logs.is_empty() {
                    let (_qa_n, _qa_fails, qa_errors) = sample_decode_qa(
                        logs,
                        &chain_owned,
                        &contract_str,
                        decimals_for_decode,
                        QA_SAMPLE_SIZE,
                    );
                    for e in qa_errors {
                        errors.push(format!("decode QA: {e}"));
                    }
                    qa_sample_done = true;
                }

                let mut chunk_events = Vec::with_capacity(logs.len());
                for log in logs {
                    match decode_transfer_log(log, &chain_owned, &contract_str, decimals_for_decode)
                    {
                        Ok(ev) => chunk_events.push(ev),
                        Err(e) => {
                            decode_errors += 1;
                            if decode_errors <= 5 {
                                errors.push(format!("decode: {e:#}"));
                            }
                        }
                    }
                }

                transfer_checkpoint::append_fetch_partial_events(
                    &out_owned,
                    &chain_owned,
                    &chunk_events,
                )?;
                events.extend(chunk_events);
                logs_fetched += logs.len();

                let progress = FetchChunkProgress {
                    schema: FetchChunkProgress::SCHEMA.to_string(),
                    chain: chain_owned.clone(),
                    contract_address: contract_str.clone(),
                    from_block,
                    to_block: end_blk,
                    chunk_size,
                    last_fetched_through: end,
                    chunks_done,
                    total_chunks,
                    logs_fetched,
                };
                transfer_checkpoint::save_fetch_progress(&out_owned, &progress)?;
                Ok(())
            },
        )
        .await;

        if let Err(e) = fetch_result {
            let msg = format!("eth_getLogs failed: {e:#}");
            eprintln!("[{}] {msg}", chain.to_uppercase());
            errors.push(msg);
            let supply = partial_supply_after_fetch_fail(
                chain,
                &config,
                from_block,
                end_blk,
                to_block_raw,
                chunk_size,
                metadata_call_pass,
                historical_supply_pass,
                &supply_start,
                &supply_start_provenance,
                &supply_end,
                effective_decimals,
                &win_start_ts,
                &win_end_ts,
            );
            let qa = build_qa_chain(&supply, generated_at, &errors);
            return (empty_events, supply, qa, true);
        }
    }

    let raw_count = logs_fetched;
    println!(
        "[{}] received {} raw logs ({} decoded rows before dedup)",
        chain.to_uppercase(),
        raw_count,
        events.len()
    );

    if decode_errors > 5 {
        errors.push(format!("... and {} more decode errors", decode_errors - 5));
    }

    let metrics = build_supply_metrics_from_events(events, decode_errors, supply_start, supply_end);
    let deduped = metrics.deduped;
    let dup_count = metrics.duplicate_count;
    let mint_count = metrics.mint_count;
    let burn_count = metrics.burn_count;
    let plain_transfer_count = metrics.plain_transfer_count;
    let net_mint_opt = metrics.net_mint;
    let onchain_delta_opt = metrics.onchain_delta;
    let discrepancy_opt = metrics.discrepancy;
    let invariant_pass = metrics.supply_invariant_pass;
    let no_dup_pass = metrics.no_duplicate_logs_pass;
    let all_decode_pass = metrics.transfer_decode_pass;

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
        active_senders: metrics.active_senders,
        active_recipients: metrics.active_recipients,
        mint_count,
        burn_count,
        plain_transfer_count,
        sum_mints_raw: metrics.sum_mints.to_string(),
        sum_burns_raw: metrics.sum_burns.to_string(),
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
        window_start_block_timestamp_rfc3339: win_start_ts,
        window_end_block_timestamp_rfc3339: win_end_ts,
    };

    let qa = build_qa_chain(&supply, generated_at, &errors);

    (deduped, supply, qa, hard_error)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn partial_supply_after_fetch_fail(
    chain: &str,
    config: &TokenConfig,
    from_block: u64,
    end_blk: u64,
    to_block_raw: &str,
    chunk_size: u64,
    metadata_call_pass: bool,
    historical_supply_pass: bool,
    supply_start: &Option<U256>,
    supply_start_provenance: &str,
    supply_end: &Option<U256>,
    effective_decimals: u8,
    win_start_ts: &Option<String>,
    win_end_ts: &Option<String>,
) -> SupplyAuditRow {
    SupplyAuditRow {
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
        total_supply_at_start_minus_1: supply_start
            .map(|u| format_token_amount(u, effective_decimals)),
        total_supply_at_start_minus_1_provenance: supply_start_provenance.to_string(),
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
        window_start_block_timestamp_rfc3339: win_start_ts.clone(),
        window_end_block_timestamp_rfc3339: win_end_ts.clone(),
    }
}

pub(crate) fn failed_supply_row(
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
        window_start_block_timestamp_rfc3339: None,
        window_end_block_timestamp_rfc3339: None,
    }
}

pub(crate) fn build_qa_chain(
    supply: &SupplyAuditRow,
    generated_at: &str,
    errors: &[String],
) -> QaChain {
    let prov =
        !supply.to_block_requested.is_empty() && supply.from_block > 0 && !generated_at.is_empty();
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
    run_id: &str,
    generated_at: &str,
    provenance_md_intro: &str,
    rows: &[SupplyAuditRow],
    qa: &[QaChain],
) -> Result<()> {
    let mut md = String::new();
    md.push_str(&format!("# {} supply audit\n\n", asset.to_uppercase()));
    md.push_str(&format!("**Run id:** `{}`\n\n", run_id));
    md.push_str(&format!("**Generated:** {}\n\n", generated_at));
    md.push_str("## Provenance\n\n");
    md.push_str(provenance_md_intro);

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
        md.push_str(&format!("- **Contract:** `{}`\n", row.contract_address));
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
        "_v0.1 window-limited supply invariant audit. Not a reserve audit, peg or purchasing-power analysis, or full-history holder census._\n",
    );

    std::fs::write(out_dir.join("supply_audit.md"), md)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{I256, U256};
    use std::path::PathBuf;

    fn tmp_out(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ta_test_{}_{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn sample_event(kind: &str, value: u64, log_index: u64) -> crate::decode::TransferEvent {
        let (from, to) = match kind {
            "mint" => (
                "0x0000000000000000000000000000000000000000".to_string(),
                "0x0000000000000000000000000000000000000001".to_string(),
            ),
            "burn" => (
                "0x0000000000000000000000000000000000000001".to_string(),
                "0x0000000000000000000000000000000000000000".to_string(),
            ),
            _ => (
                "0x00000000000000000000000000000000000000aa".to_string(),
                "0x00000000000000000000000000000000000000bb".to_string(),
            ),
        };
        crate::decode::TransferEvent {
            chain: "ethereum".into(),
            contract_address: "0xabc".into(),
            block_number: 1,
            tx_hash: "0x1".into(),
            log_index,
            from,
            to,
            value_raw: value.to_string(),
            value_decimal: "0".into(),
            kind: kind.into(),
            value_u256: U256::from(value),
        }
    }

    #[test]
    fn parse_window_ok() {
        let (c, a, b) = parse_window_arg("ethereum:24000000:24001000").unwrap();
        assert_eq!(c, "ethereum");
        assert_eq!(a, 24_000_000);
        assert_eq!(b, 24_001_000);
    }

    #[test]
    fn parse_window_rejects_bad_range() {
        assert!(parse_window_arg("base:100:50").is_err());
    }

    #[test]
    fn parse_window_rejects_zero_from() {
        assert!(parse_window_arg("ethereum:0:100").is_err());
    }

    #[test]
    fn parse_window_rejects_malformed() {
        assert!(parse_window_arg("only-two").is_err());
        assert!(parse_window_arg("eth::100").is_err());
    }

    #[test]
    fn supply_invariant_passes_when_mints_match_delta() {
        let start = U256::from(1_000_000u64);
        let end = U256::from(2_500_000u64);
        let sum_mints = U256::from(2_000_000u64);
        let sum_burns = U256::from(500_000u64);
        let (_, _, disc, pass) = compute_supply_invariant(sum_mints, sum_burns, start, end);
        assert!(pass);
        assert_eq!(disc, I256::ZERO);
    }

    #[test]
    fn supply_invariant_fails_on_discrepancy() {
        let (_, _, disc, pass) = compute_supply_invariant(
            U256::from(100u64),
            U256::ZERO,
            U256::ZERO,
            U256::from(50u64),
        );
        assert!(!pass);
        assert_ne!(disc, I256::ZERO);
    }

    #[test]
    fn gate_csv_labels() {
        assert_eq!(gate_csv(true), "PASS");
        assert_eq!(gate_csv(false), "FAIL");
        assert_eq!(gate_opt_csv(None), "UNAVAILABLE");
    }

    #[test]
    fn build_run_plan_unified_latest() {
        let chains = vec!["ethereum".into(), "base".into()];
        let plan = build_run_plan(RunMode::Unified {
            chains: &chains,
            from_block: 100,
            to_block_raw: "latest",
        })
        .unwrap();
        assert!(!plan.per_chain_spans);
        assert_eq!(plan.provenance_from_block, 100);
        assert_eq!(plan.chain_tasks.len(), 2);
        assert!(plan.chain_tasks[0].fixed_to_block.is_none());
    }

    #[test]
    fn build_run_plan_per_chain_rejects_dup() {
        let err = build_run_plan(RunMode::PerChain(vec![
            ("ethereum".into(), 1, 2),
            ("ethereum".into(), 3, 4),
        ]));
        assert!(err.is_err());
    }

    #[test]
    fn build_provenance_intro_per_chain() {
        let rows = vec![SupplyAuditRow {
            chain: "base".into(),
            from_block: 10,
            to_block_requested: "20".into(),
            resolved_to_block: Some(20),
            ..failed_supply_row("base", 10, "20", 500)
        }];
        let md = build_provenance_md_intro(true, &rows);
        assert!(md.contains("Per-chain block spans"));
        assert!(md.contains("base"));
    }

    #[test]
    fn build_supply_metrics_mint_burn_counts() {
        let events = vec![
            sample_event("mint", 1_000_000, 0),
            sample_event("burn", 500_000, 1),
            sample_event("transfer", 1, 2),
        ];
        let m = build_supply_metrics_from_events(
            events,
            0,
            Some(U256::from(1_000_000u64)),
            Some(U256::from(1_500_000u64)),
        );
        assert_eq!(m.mint_count, 1);
        assert_eq!(m.burn_count, 1);
        assert_eq!(m.plain_transfer_count, 1);
        assert_eq!(m.active_senders, 2);
        assert!(m.supply_invariant_pass == Some(true));
    }

    #[test]
    fn write_run_artifacts_creates_expected_files() {
        let out = tmp_out("artifacts");
        let supply = SupplyAuditRow {
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
            sum_mints_raw: "1000".into(),
            sum_burns_raw: "0".into(),
            net_mint_raw: Some("1000".into()),
            total_supply_at_start_minus_1: Some("1.000000".into()),
            total_supply_at_start_minus_1_provenance: "on-chain".into(),
            total_supply_at_end: Some("2.000000".into()),
            onchain_delta_raw: Some("1000".into()),
            discrepancy_raw: Some("0".into()),
            metadata_call_pass: true,
            historical_supply_pass: true,
            no_duplicate_logs_pass: Some(true),
            transfer_decode_pass: Some(true),
            supply_invariant_pass: Some(true),
            duplicate_count: 0,
            full_decode_error_count: 0,
            window_start_block_timestamp_rfc3339: None,
            window_end_block_timestamp_rfc3339: None,
        };
        let qa = build_qa_chain(&supply, "2026-01-01T00:00:00Z", &[]);
        let ev = sample_event("mint", 1000, 0);
        write_run_artifacts(
            &out,
            "USDC",
            "run_test",
            "2026-01-01T00:00:00Z",
            false,
            100,
            Some("200".into()),
            &[ev],
            &[supply],
            &[qa],
        )
        .unwrap();
        for f in [
            "decoded_transfers.csv",
            "supply_audit.csv",
            "supply_audit.md",
            "qa_report.json",
            "provenance.json",
            "summary.md",
        ] {
            assert!(out.join(f).is_file(), "missing {f}");
        }
        let _ = std::fs::remove_dir_all(&out);
    }

    #[tokio::test]
    async fn process_chain_unknown_config_fails_fast() {
        let out = tmp_out("pcfg");
        let (_, supply, _, hard) = process_chain(
            "USDC",
            "not_a_real_chain_xyz",
            100,
            Some(200),
            500,
            "200",
            "2026-01-01T00:00:00Z",
            &out,
        )
        .await;
        assert!(hard);
        assert_eq!(supply.chain, "not_a_real_chain_xyz");
        assert!(!supply.metadata_call_pass);
        let _ = std::fs::remove_dir_all(&out);
    }

    #[tokio::test]
    async fn process_chain_missing_rpc_env() {
        let out = tmp_out("prpc");
        let key = "ALCHEMY_ETHEREUM_URL";
        let saved = std::env::var(key).ok();
        {
            let _lock = crate::rpc::RPC_ENV_LOCK.lock().unwrap();
            std::env::set_var(key, "");
        }
        let (_, supply, qa, hard) = process_chain(
            "USDC",
            "ethereum",
            100,
            Some(200),
            500,
            "200",
            "2026-01-01T00:00:00Z",
            &out,
        )
        .await;
        assert!(hard);
        assert_eq!(supply.chain, "ethereum");
        assert!(!qa.errors.is_empty());
        {
            let _lock = crate::rpc::RPC_ENV_LOCK.lock().unwrap();
            if let Some(v) = saved {
                std::env::set_var(key, v);
            } else {
                std::env::remove_var(key);
            }
        }
        let _ = std::fs::remove_dir_all(&out);
    }

    #[tokio::test]
    async fn process_chain_unreachable_rpc_exercises_metadata_and_fetch_paths() {
        let out = tmp_out("prpcfail");
        let key = "ALCHEMY_ETHEREUM_URL";
        let saved = std::env::var(key).ok();
        {
            let _lock = crate::rpc::RPC_ENV_LOCK.lock().unwrap();
            std::env::set_var(key, "http://127.0.0.1:1");
        }
        let (events, supply, qa, hard) = process_chain(
            "USDC",
            "ethereum",
            24_000_000,
            Some(24_001_000),
            500,
            "24001000",
            "2026-01-01T00:00:00Z",
            &out,
        )
        .await;
        assert!(hard);
        assert_eq!(supply.chain, "ethereum");
        assert!(!qa.errors.is_empty() || !supply.metadata_call_pass);
        let _ = events;
        {
            let _lock = crate::rpc::RPC_ENV_LOCK.lock().unwrap();
            if let Some(v) = saved {
                std::env::set_var(key, v);
            } else {
                std::env::remove_var(key);
            }
        }
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn partial_supply_after_fetch_fail_formats_boundaries() {
        let cfg = crate::config::load_single_token_config("USDC", "ethereum").unwrap();
        let row = partial_supply_after_fetch_fail(
            "ethereum",
            &cfg,
            100,
            200,
            "200",
            500,
            true,
            true,
            &Some(U256::from(1_000_000u64)),
            "on-chain",
            &Some(U256::from(2_000_000u64)),
            6,
            &None,
            &None,
        );
        assert_eq!(row.resolved_to_block, Some(200));
        assert!(row.total_supply_at_end.is_some());
    }

    #[test]
    fn build_qa_chain_provenance_stamp() {
        let row = failed_supply_row("ethereum", 100, "200", 500);
        let qa = build_qa_chain(&row, "2026-01-01T00:00:00Z", &[]);
        assert_eq!(qa.gates.provenance_stamped, "PASS");
    }

    #[tokio::test]
    async fn run_per_chain_windows_resumes_from_checkpoint_without_rpc() {
        let run_id = format!("ckpt_resume_{}", std::process::id());
        let out = crate::report::ensure_run_out_dir("USDC", &run_id).unwrap();
        let specs = vec![
            ChainSpecRecord {
                chain: "ethereum".into(),
                from_block: 100,
                to_block_requested: "200".into(),
            },
            ChainSpecRecord {
                chain: "base".into(),
                from_block: 300,
                to_block_requested: "400".into(),
            },
        ];
        let mut manifest =
            CheckpointManifest::new("USDC", &run_id, "2026-01-01T00:00:00Z", 500, true, specs);
        for (chain, from, to) in [("ethereum", 100u64, 200u64), ("base", 300, 400)] {
            let supply = SupplyAuditRow {
                chain: chain.into(),
                chain_id: if chain == "ethereum" { 1 } else { 8453 },
                contract_address: format!("0x{chain}"),
                from_block: from,
                resolved_to_block: Some(to),
                to_block_requested: to.to_string(),
                chunk_size: 500,
                transfer_event_count: 0,
                active_senders: 0,
                active_recipients: 0,
                mint_count: 0,
                burn_count: 0,
                plain_transfer_count: 0,
                sum_mints_raw: "0".into(),
                sum_burns_raw: "0".into(),
                net_mint_raw: Some("0".into()),
                total_supply_at_start_minus_1: Some("1.000000".into()),
                total_supply_at_start_minus_1_provenance: "on-chain".into(),
                total_supply_at_end: Some("1.000000".into()),
                onchain_delta_raw: Some("0".into()),
                discrepancy_raw: Some("0".into()),
                metadata_call_pass: true,
                historical_supply_pass: true,
                no_duplicate_logs_pass: Some(true),
                transfer_decode_pass: Some(true),
                supply_invariant_pass: Some(true),
                duplicate_count: 0,
                full_decode_error_count: 0,
                window_start_block_timestamp_rfc3339: None,
                window_end_block_timestamp_rfc3339: None,
            };
            let qa = build_qa_chain(&supply, "2026-01-01T00:00:00Z", &[]);
            let bundle = CheckpointChainBundle { supply, qa };
            transfer_checkpoint::save_completed_chain(&out, &mut manifest, chain, &[], &bundle)
                .unwrap();
        }

        run_per_chain_windows(
            "USDC",
            vec![("ethereum".into(), 100, 200), ("base".into(), 300, 400)],
            None,
            Some(run_id.clone()),
            false,
        )
        .await
        .unwrap();

        assert!(out.join("qa_report.json").is_file());
        assert!(out.join("supply_audit.csv").is_file());
        assert!(out.join("summary.md").is_file());
        let _ = std::fs::remove_dir_all(&out);
    }

    #[tokio::test]
    async fn process_chain_mock_rpc_completes_with_empty_logs() {
        let server = crate::rpc::mock_rpc::tests::spawn_usdc_rpc().await;
        let out = tmp_out("mockrpc");
        let key = "ALCHEMY_ETHEREUM_URL";
        let saved = std::env::var(key).ok();
        let uri = server.uri();
        {
            let _lock = crate::rpc::RPC_ENV_LOCK.lock().unwrap();
            std::env::set_var(key, uri);
        }
        let (events, supply, qa, hard) = process_chain(
            "USDC",
            "ethereum",
            24_000_000,
            Some(24_000_100),
            500,
            "24000100",
            "2026-01-01T00:00:00Z",
            &out,
        )
        .await;
        assert!(!hard);
        assert!(events.is_empty());
        assert_eq!(supply.chain, "ethereum");
        assert_eq!(supply.transfer_event_count, 0);
        assert_eq!(supply.supply_invariant_pass, Some(true));
        let _ = qa;
        {
            let _lock = crate::rpc::RPC_ENV_LOCK.lock().unwrap();
            if let Some(v) = saved {
                std::env::set_var(key, v);
            } else {
                std::env::remove_var(key);
            }
        }
        let _ = std::fs::remove_dir_all(&out);
    }
}
