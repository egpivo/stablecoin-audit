use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::control_events::ControlEventRecord;
use crate::report::ensure_out_dir;

// ─── Deserialisation of fetch_report.json ─────────────────────────────────

#[derive(Deserialize)]
struct FetchReport {
    #[allow(dead_code)]
    asset: String,
    generated_at: String,
    chains: Vec<ChainFetchResult>,
}

#[derive(Deserialize)]
struct ChainFetchResult {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    to_block: u64,
    mint_count: usize,
    burn_count: usize,
    transfer_count: usize,
    #[serde(default)]
    no_duplicate_logs_pass: Option<bool>,
    #[serde(default)]
    transfer_decode_sample_pass: Option<bool>,
    #[serde(default)]
    full_decode_error_count: usize,
    #[serde(default)]
    all_transfer_decode_pass: Option<bool>,
    #[serde(default)]
    control_event_count: Option<usize>,
    #[serde(default)]
    control_event_query_status: Option<String>,
    sum_mints_raw: String,
    sum_burns_raw: String,
    total_supply_at_start_minus_1: Option<String>,
    total_supply_at_end: Option<String>,
    net_mint_raw: Option<String>,
    onchain_delta_raw: Option<String>,
    discrepancy_raw: Option<String>,
    supply_invariant_pass: Option<bool>,
    errors: Vec<String>,
}

// ─── Local CSV row (only fields present in the file) ──────────────────────

#[derive(Deserialize)]
struct TransferRow {
    // chain: String,  // present but unused for address counting
    from: String,
    to: String,
    // other columns not needed for address counting
}

// ─── Per-chain data after CSV enrichment ──────────────────────────────────

struct ChainReport<'a> {
    result: &'a ChainFetchResult,
    unique_senders: usize,
    unique_recipients: usize,
    csv_missing: bool,
    control_events: Vec<ControlEventRecord>,
    ctrl_csv_missing: bool,
}

// ─── Helpers ──────────────────────────────────────────────────────────────

const ZERO_ADDR: &str = "0x0000000000000000000000000000000000000000";

fn opt_bool_csv(opt: Option<bool>) -> &'static str {
    match opt {
        Some(true) => "true",
        Some(false) => "false",
        None => "unavailable",
    }
}

fn supply_invariant_gate(pass: Option<bool>) -> &'static str {
    match pass {
        Some(true) => "PASS",
        Some(false) => "FAIL",
        None => "UNAVAILABLE",
    }
}

// ─── Public entry point ───────────────────────────────────────────────────

pub fn run(asset: &str) -> Result<()> {
    let out_dir = ensure_out_dir(asset)?;

    // 1. Read fetch_report.json
    let report_path = out_dir.join("fetch_report.json");
    if !report_path.exists() {
        anyhow::bail!(
            "fetch_report.json not found at {}; run `fetch` first",
            report_path.display()
        );
    }
    let fetch_report: FetchReport = serde_json::from_str(&std::fs::read_to_string(&report_path)?)?;

    let report_generated_at = Utc::now().to_rfc3339();

    // 2. For each chain read the CSV and compute address counts
    let mut chain_reports: Vec<ChainReport> = Vec::new();
    for result in &fetch_report.chains {
        let csv_path = out_dir.join(format!("transfers_{}.csv", result.chain));
        let (unique_senders, unique_recipients, csv_missing) = if csv_path.exists() {
            let (s, r) = count_unique_addresses(&csv_path)?;
            (s, r, false)
        } else {
            eprintln!(
                "[WARN] CSV not found for chain {}: {}",
                result.chain,
                csv_path.display()
            );
            (0, 0, true)
        };

        let ctrl_csv_path = out_dir.join(format!("control_events_{}.csv", result.chain));
        let (control_events, ctrl_csv_missing) = if ctrl_csv_path.exists() {
            match load_control_events_csv(&ctrl_csv_path) {
                Ok(evs) => (evs, false),
                Err(e) => {
                    eprintln!(
                        "[WARN] Failed to read control_events CSV for chain {}: {e:#}",
                        result.chain
                    );
                    (Vec::new(), false)
                }
            }
        } else {
            (Vec::new(), true)
        };

        chain_reports.push(ChainReport {
            result,
            unique_senders,
            unique_recipients,
            csv_missing,
            control_events,
            ctrl_csv_missing,
        });
    }

    // 3. Write all output files
    write_provenance_json(
        &out_dir,
        asset,
        &report_generated_at,
        &fetch_report.generated_at,
        &fetch_report.chains,
    )?;

    write_chain_summary_csv(&out_dir, &chain_reports)?;
    write_cross_chain_supply_delta_csv(&out_dir, &chain_reports)?;
    write_mint_burn_by_chain_csv(&out_dir, &chain_reports)?;
    write_transfer_activity_by_chain_csv(&out_dir, &chain_reports)?;
    write_qa_report_json(&out_dir, asset, &report_generated_at, &chain_reports)?;
    write_risk_flags_md(&out_dir, asset, &report_generated_at, &chain_reports)?;
    write_summary_md(&out_dir, asset, &report_generated_at, &chain_reports)?;

    // 4. Print brief summary
    println!("\n=== Report for {} ===", asset.to_uppercase());
    println!("Chains audited: {}", fetch_report.chains.len());
    for cr in &chain_reports {
        let r = cr.result;
        println!(
            "  {} | blocks {}–{} | mints: {} burns: {} transfers: {} | senders: {} recipients: {}",
            r.chain,
            r.from_block,
            r.to_block,
            r.mint_count,
            r.burn_count,
            r.transfer_count,
            cr.unique_senders,
            cr.unique_recipients,
        );
    }
    println!("\nOutput files written to {}/", out_dir.display());
    println!(
        "  provenance.json, chain_summary.csv, cross_chain_supply_delta.csv,\n  \
         mint_burn_by_chain.csv, transfer_activity_by_chain.csv,\n  \
         qa_report.json, risk_flags.md, summary.md"
    );

    Ok(())
}

// ─── Address counting ─────────────────────────────────────────────────────

fn count_unique_addresses(csv_path: &Path) -> Result<(usize, usize)> {
    let mut rdr = csv::Reader::from_path(csv_path)?;
    let mut senders: HashSet<String> = HashSet::new();
    let mut recipients: HashSet<String> = HashSet::new();

    for record in rdr.deserialize::<TransferRow>() {
        let row = record?;
        if row.from != ZERO_ADDR {
            senders.insert(row.from);
        }
        if row.to != ZERO_ADDR {
            recipients.insert(row.to);
        }
    }

    Ok((senders.len(), recipients.len()))
}

fn load_control_events_csv(csv_path: &Path) -> Result<Vec<ControlEventRecord>> {
    let mut rdr = csv::Reader::from_path(csv_path)?;
    let mut records = Vec::new();
    for result in rdr.deserialize::<ControlEventRecord>() {
        records.push(result?);
    }
    Ok(records)
}

// ─── 1. provenance.json ───────────────────────────────────────────────────

#[derive(Serialize)]
struct ProvenanceJson {
    asset: String,
    report_generated_at: String,
    fetch_report_generated_at: String,
    chains: Vec<ProvenanceChain>,
}

#[derive(Serialize)]
struct ProvenanceChain {
    chain: String,
    from_block: u64,
    to_block: u64,
    contract_address: String,
    chain_id: u64,
}

fn write_provenance_json(
    out_dir: &Path,
    asset: &str,
    report_generated_at: &str,
    fetch_report_generated_at: &str,
    chains: &[ChainFetchResult],
) -> Result<()> {
    let prov = ProvenanceJson {
        asset: asset.to_uppercase(),
        report_generated_at: report_generated_at.to_string(),
        fetch_report_generated_at: fetch_report_generated_at.to_string(),
        chains: chains
            .iter()
            .map(|c| ProvenanceChain {
                chain: c.chain.clone(),
                from_block: c.from_block,
                to_block: c.to_block,
                contract_address: c.contract_address.clone(),
                chain_id: c.chain_id,
            })
            .collect(),
    };
    let path = out_dir.join("provenance.json");
    std::fs::write(&path, serde_json::to_string_pretty(&prov)?)?;
    println!("Written: {}", path.display());
    Ok(())
}

// ─── 2. chain_summary.csv ─────────────────────────────────────────────────

fn write_chain_summary_csv(out_dir: &Path, chain_reports: &[ChainReport]) -> Result<()> {
    let path = out_dir.join("chain_summary.csv");
    let mut wtr = csv::Writer::from_path(&path)?;

    wtr.write_record([
        "chain",
        "chain_id",
        "from_block",
        "to_block",
        "total_supply_start_decimal",
        "total_supply_end_decimal",
        "onchain_delta_raw",
        "mint_count",
        "sum_mints_raw",
        "burn_count",
        "sum_burns_raw",
        "net_mint_raw",
        "transfer_count",
        "unique_senders",
        "unique_recipients",
        "no_dup_pass",
        "decode_sample_pass",
        "all_transfer_decode_pass",
        "supply_invariant_pass",
    ])?;

    for cr in chain_reports {
        let r = cr.result;
        let mut errors_extra: Vec<String> = Vec::new();
        if cr.csv_missing {
            errors_extra.push("CSV not found".into());
        }

        wtr.write_record([
            &r.chain,
            &r.chain_id.to_string(),
            &r.from_block.to_string(),
            &r.to_block.to_string(),
            r.total_supply_at_start_minus_1.as_deref().unwrap_or(""),
            r.total_supply_at_end.as_deref().unwrap_or(""),
            r.onchain_delta_raw.as_deref().unwrap_or(""),
            &r.mint_count.to_string(),
            &r.sum_mints_raw,
            &r.burn_count.to_string(),
            &r.sum_burns_raw,
            r.net_mint_raw.as_deref().unwrap_or(""),
            &r.transfer_count.to_string(),
            &cr.unique_senders.to_string(),
            &cr.unique_recipients.to_string(),
            opt_bool_csv(r.no_duplicate_logs_pass),
            opt_bool_csv(r.transfer_decode_sample_pass),
            opt_bool_csv(r.all_transfer_decode_pass),
            opt_bool_csv(r.supply_invariant_pass),
        ])?;

        // suppress unused warning
        let _ = errors_extra;
    }

    wtr.flush()?;
    println!("Written: {}", path.display());
    Ok(())
}

// ─── 3. cross_chain_supply_delta.csv ─────────────────────────────────────

fn write_cross_chain_supply_delta_csv(out_dir: &Path, chain_reports: &[ChainReport]) -> Result<()> {
    let path = out_dir.join("cross_chain_supply_delta.csv");
    let mut wtr = csv::Writer::from_path(&path)?;

    wtr.write_record([
        "chain",
        "from_block",
        "to_block",
        "total_supply_start_decimal",
        "total_supply_end_decimal",
        "onchain_delta_raw",
    ])?;

    for cr in chain_reports {
        let r = cr.result;
        wtr.write_record([
            &r.chain,
            &r.from_block.to_string(),
            &r.to_block.to_string(),
            r.total_supply_at_start_minus_1.as_deref().unwrap_or(""),
            r.total_supply_at_end.as_deref().unwrap_or(""),
            r.onchain_delta_raw.as_deref().unwrap_or(""),
        ])?;
    }

    wtr.flush()?;
    println!("Written: {}", path.display());
    Ok(())
}

// ─── 4. mint_burn_by_chain.csv ────────────────────────────────────────────

fn write_mint_burn_by_chain_csv(out_dir: &Path, chain_reports: &[ChainReport]) -> Result<()> {
    let path = out_dir.join("mint_burn_by_chain.csv");
    let mut wtr = csv::Writer::from_path(&path)?;

    wtr.write_record([
        "chain",
        "from_block",
        "to_block",
        "mint_count",
        "sum_mints_raw",
        "burn_count",
        "sum_burns_raw",
        "net_mint_raw",
    ])?;

    for cr in chain_reports {
        let r = cr.result;
        wtr.write_record([
            &r.chain,
            &r.from_block.to_string(),
            &r.to_block.to_string(),
            &r.mint_count.to_string(),
            &r.sum_mints_raw,
            &r.burn_count.to_string(),
            &r.sum_burns_raw,
            r.net_mint_raw.as_deref().unwrap_or(""),
        ])?;
    }

    wtr.flush()?;
    println!("Written: {}", path.display());
    Ok(())
}

// ─── 5. transfer_activity_by_chain.csv ────────────────────────────────────

fn write_transfer_activity_by_chain_csv(
    out_dir: &Path,
    chain_reports: &[ChainReport],
) -> Result<()> {
    let path = out_dir.join("transfer_activity_by_chain.csv");
    let mut wtr = csv::Writer::from_path(&path)?;

    wtr.write_record([
        "chain",
        "from_block",
        "to_block",
        "total_logs",
        "unique_senders",
        "unique_recipients",
    ])?;

    for cr in chain_reports {
        let r = cr.result;
        let total_logs = r.mint_count + r.burn_count + r.transfer_count;
        wtr.write_record([
            &r.chain,
            &r.from_block.to_string(),
            &r.to_block.to_string(),
            &total_logs.to_string(),
            &cr.unique_senders.to_string(),
            &cr.unique_recipients.to_string(),
        ])?;
    }

    wtr.flush()?;
    println!("Written: {}", path.display());
    Ok(())
}

// ─── 6. qa_report.json ────────────────────────────────────────────────────

#[derive(Serialize)]
struct QaReport {
    asset: String,
    generated_at: String,
    chains: Vec<QaChain>,
}

#[derive(Serialize)]
struct QaChain {
    chain: String,
    gates: QaGates,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct QaGates {
    no_duplicate_logs: String,
    transfer_decode_sample: String,
    all_transfer_decode: String,
    supply_invariant: String,
    control_event_query: String,
}

fn write_qa_report_json(
    out_dir: &Path,
    asset: &str,
    generated_at: &str,
    chain_reports: &[ChainReport],
) -> Result<()> {
    let mut qa_chains = Vec::new();
    for cr in chain_reports {
        let r = cr.result;
        let mut errors = r.errors.clone();
        if cr.csv_missing {
            errors.push("CSV not found".into());
        }
        let ctrl_status = r.control_event_query_status.as_deref().unwrap_or("skipped");
        let ctrl_gate = match ctrl_status {
            "pass" => "PASS",
            "partial" => "WARN",
            "skipped" => "UNAVAILABLE",
            _ => "WARN", // error: <msg>
        };
        qa_chains.push(QaChain {
            chain: r.chain.clone(),
            gates: QaGates {
                no_duplicate_logs: supply_invariant_gate(r.no_duplicate_logs_pass).to_string(),
                transfer_decode_sample: supply_invariant_gate(r.transfer_decode_sample_pass)
                    .to_string(),
                all_transfer_decode: supply_invariant_gate(r.all_transfer_decode_pass).to_string(),
                supply_invariant: supply_invariant_gate(r.supply_invariant_pass).to_string(),
                control_event_query: ctrl_gate.to_string(),
            },
            errors,
        });
    }

    let report = QaReport {
        asset: asset.to_uppercase(),
        generated_at: generated_at.to_string(),
        chains: qa_chains,
    };

    let path = out_dir.join("qa_report.json");
    std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
    println!("Written: {}", path.display());
    Ok(())
}

// ─── 7. risk_flags.md ─────────────────────────────────────────────────────

fn write_risk_flags_md(
    out_dir: &Path,
    asset: &str,
    report_generated_at: &str,
    chain_reports: &[ChainReport],
) -> Result<()> {
    let mut md = String::new();

    md.push_str("# Risk Flags\n\n");
    md.push_str(&format!(
        "## {} — Report generated {}\n\n",
        asset.to_uppercase(),
        report_generated_at
    ));

    for cr in chain_reports {
        let r = cr.result;
        md.push_str(&format!(
            "### {} (block {} → {})\n",
            r.chain, r.from_block, r.to_block
        ));

        // no_duplicate_logs gate
        match r.no_duplicate_logs_pass {
            Some(true) => md.push_str("- [PASS] No duplicate logs\n"),
            Some(false) => md.push_str("- [FAIL] Duplicate logs detected\n"),
            None => md.push_str("- [SKIP] Duplicate log check: not evaluated (chain hard error)\n"),
        }

        // transfer_decode_sample gate
        match r.transfer_decode_sample_pass {
            Some(true) => md.push_str("- [PASS] Transfer decode sample pass\n"),
            Some(false) => md.push_str("- [FAIL] Transfer decode sample failed\n"),
            None => {
                md.push_str("- [SKIP] Transfer decode sample: not evaluated (chain hard error)\n")
            }
        }

        // all_transfer_decode gate
        match r.all_transfer_decode_pass {
            Some(true) => md.push_str("- [PASS] All transfer logs decoded without error\n"),
            Some(false) => {
                let n = r.full_decode_error_count;
                md.push_str(&format!(
                    "- [FAIL] Full decode had {n} error(s); CSV and mint/burn sums may be incomplete\n"
                ));
            }
            None => md.push_str("- [SKIP] Full decode check: not evaluated (chain hard error)\n"),
        }

        // supply_invariant gate
        match r.supply_invariant_pass {
            Some(true) => {
                let net = r.net_mint_raw.as_deref().unwrap_or("?");
                let od = r.onchain_delta_raw.as_deref().unwrap_or("?");
                let disc = r.discrepancy_raw.as_deref().unwrap_or("?");
                md.push_str(&format!(
                    "- [PASS] Supply invariant matched: net_mint_raw={net} onchain_delta_raw={od} discrepancy_raw={disc}\n"
                ));
            }
            Some(false) => {
                let net = r.net_mint_raw.as_deref().unwrap_or("?");
                let od = r.onchain_delta_raw.as_deref().unwrap_or("?");
                let disc = r.discrepancy_raw.as_deref().unwrap_or("?");
                md.push_str(&format!(
                    "- [FAIL] Supply invariant mismatch: net_mint_raw={net} onchain_delta_raw={od} discrepancy_raw={disc}\n"
                ));
            }
            None => {
                md.push_str(
                    "- [WARN] Supply invariant unavailable: totalSupply historical call failed or block out of range\n",
                );
            }
        }

        // CSV missing warning
        if cr.csv_missing {
            md.push_str(&format!(
                "- [WARN] transfers_{}.csv not found; address counts unavailable\n",
                r.chain
            ));
        }

        // Per-error warnings
        for err in &r.errors {
            md.push_str(&format!("- [WARN] {err}\n"));
        }

        // Control events section
        let ctrl_status = cr
            .result
            .control_event_query_status
            .as_deref()
            .unwrap_or("skipped");
        if ctrl_status.starts_with("error") {
            md.push_str(&format!(
                "- [WARN] Control event query failed: {ctrl_status}\n"
            ));
        } else if cr.ctrl_csv_missing && ctrl_status == "skipped" {
            md.push_str("- [INFO] Control event data not available (run fetch to collect)\n");
        } else if cr.control_events.is_empty() {
            md.push_str("- [INFO] Control event query succeeded: no events observed in window\n");
        } else {
            if ctrl_status == "partial" {
                md.push_str(
                    "- [WARN] Control event query partial: one or more events failed to decode\n",
                );
            }
            for ev in &cr.control_events {
                let level = if ev.decode_status == "decode_error" {
                    "[WARN]"
                } else {
                    match ev.event_name.as_str() {
                        "MinterConfigured" | "MinterRemoved" => "[INFO]",
                        _ => "[WARN]",
                    }
                };
                md.push_str(&format!(
                    "- {level} Control event detected: {} ({})\n",
                    ev.event_name, ev.args_json
                ));
            }
        }

        md.push('\n');
    }

    md.push_str("---\n");

    let path = out_dir.join("risk_flags.md");
    std::fs::write(&path, &md)?;
    println!("Written: {}", path.display());
    Ok(())
}

// ─── 8. summary.md ────────────────────────────────────────────────────────

fn write_summary_md(
    out_dir: &Path,
    asset: &str,
    report_generated_at: &str,
    chain_reports: &[ChainReport],
) -> Result<()> {
    let mut md = String::new();

    md.push_str(&format!("# {} Audit Report\n\n", asset.to_uppercase()));
    md.push_str(&format!(
        "**Report generated:** {}\n\n",
        report_generated_at
    ));
    md.push_str(&format!(
        "**Chains audited:** {}\n\n",
        chain_reports
            .iter()
            .map(|cr| cr.result.chain.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));

    // ── Chain metadata table ──────────────────────────────────────────────
    md.push_str("## Chain Metadata\n\n");
    md.push_str("| Chain | Chain ID | Contract | Block Window |\n");
    md.push_str("|-------|----------|----------|--------------|\n");
    for cr in chain_reports {
        let r = cr.result;
        md.push_str(&format!(
            "| {} | {} | `{}` | {} → {} |\n",
            r.chain, r.chain_id, r.contract_address, r.from_block, r.to_block
        ));
    }
    md.push('\n');

    // ── Supply delta table ────────────────────────────────────────────────
    md.push_str("## Supply Delta\n\n");
    md.push_str("| Chain | Supply Start (decimal) | Supply End (decimal) | Delta (raw int) |\n");
    md.push_str("|-------|----------------------|--------------------|-----------------|\n");
    for cr in chain_reports {
        let r = cr.result;
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            r.chain,
            r.total_supply_at_start_minus_1.as_deref().unwrap_or("—"),
            r.total_supply_at_end.as_deref().unwrap_or("—"),
            r.onchain_delta_raw.as_deref().unwrap_or("—"),
        ));
    }
    md.push('\n');

    // ── Mint/burn summary table ───────────────────────────────────────────
    md.push_str("## Mint / Burn Summary\n\n");
    md.push_str("| Chain | Mints | Sum Mints (raw) | Burns | Sum Burns (raw) | Net Mint (raw) |\n");
    md.push_str("|-------|-------|-----------------|-------|-----------------|----------------|\n");
    for cr in chain_reports {
        let r = cr.result;
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            r.chain,
            r.mint_count,
            r.sum_mints_raw,
            r.burn_count,
            r.sum_burns_raw,
            r.net_mint_raw.as_deref().unwrap_or("—"),
        ));
    }
    md.push('\n');

    // ── Transfer activity table ───────────────────────────────────────────
    md.push_str("## Transfer Activity\n\n");
    md.push_str("| Chain | Total Transfers | Unique Senders | Unique Recipients |\n");
    md.push_str("|-------|----------------|---------------|------------------|\n");
    for cr in chain_reports {
        let r = cr.result;
        let total = r.mint_count + r.burn_count + r.transfer_count;
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            r.chain, total, cr.unique_senders, cr.unique_recipients,
        ));
    }
    md.push('\n');

    // ── QA gate summary table ─────────────────────────────────────────────
    md.push_str("## QA Gate Summary\n\n");
    md.push_str("| Chain | No Dup Logs | Decode Sample | All Decoded | Supply Invariant |\n");
    md.push_str("|-------|------------|--------------|-------------|------------------|\n");
    for cr in chain_reports {
        let r = cr.result;
        let dup = match r.no_duplicate_logs_pass {
            Some(true) => "✓",
            Some(false) => "✗",
            None => "—",
        };
        let decode = match r.transfer_decode_sample_pass {
            Some(true) => "✓",
            Some(false) => "✗",
            None => "—",
        };
        let all_dec = match r.all_transfer_decode_pass {
            Some(true) => "✓",
            Some(false) => "✗",
            None => "—",
        };
        let inv = match r.supply_invariant_pass {
            Some(true) => "✓",
            Some(false) => "✗",
            None => "—",
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            r.chain, dup, decode, all_dec, inv,
        ));
    }
    md.push('\n');

    // ── Control Events table ──────────────────────────────────────────────
    md.push_str("## Control Events\n\n");
    md.push_str("| Chain | Control Events | Query Status |\n");
    md.push_str("|-------|---------------|-------------- |\n");
    for cr in chain_reports {
        let r = cr.result;
        let count = r.control_event_count.unwrap_or(cr.control_events.len());
        let status = r
            .control_event_query_status
            .as_deref()
            .unwrap_or(if cr.ctrl_csv_missing {
                "skipped"
            } else {
                "pass"
            });
        md.push_str(&format!("| {} | {} | {} |\n", r.chain, count, status));
    }
    md.push('\n');

    // ── Disclaimer ────────────────────────────────────────────────────────
    md.push_str("---\n\n");
    md.push_str(
        "> **Note:** This report covers on-chain Transfer events in the specified window only. \
        It does not constitute a reserve audit, AML assessment, or full historical holder reconstruction.\n",
    );

    let path = out_dir.join("summary.md");
    std::fs::write(&path, &md)?;
    println!("Written: {}", path.display());
    Ok(())
}
