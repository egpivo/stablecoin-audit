use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::report::ensure_out_dir;

#[derive(Debug, Deserialize)]
struct SupplyAuditRow {
    #[serde(default)]
    asset: String,
    chain: String,
    chain_id: u64,
    #[serde(default)]
    contract_address: String,
    #[serde(default)]
    rpc_provider_alias: String,
    #[serde(alias = "from_block")]
    start_block: u64,
    #[serde(alias = "resolved_to_block")]
    end_block: Option<u64>,
    #[serde(alias = "transfer_event_count")]
    transfer_count: usize,
    active_senders: usize,
    active_recipients: usize,
    mint_count: usize,
    burn_count: usize,
    #[serde(alias = "sum_mints_raw")]
    mint_sum_raw: String,
    #[serde(alias = "sum_burns_raw")]
    burn_sum_raw: String,
    #[allow(dead_code)]
    net_mint_raw: Option<String>,
    #[allow(dead_code)]
    total_supply_start_raw: Option<String>,
    #[allow(dead_code)]
    total_supply_end_raw: Option<String>,
    #[allow(dead_code)]
    total_supply_delta_raw: Option<String>,
    discrepancy_raw: Option<String>,
    #[serde(default)]
    qa_status: String,
    #[serde(default)]
    metadata_call_pass: Option<bool>,
    #[serde(default)]
    historical_supply_pass: Option<bool>,
    #[serde(default)]
    no_duplicate_logs_pass: Option<bool>,
    #[serde(default)]
    transfer_decode_pass: Option<bool>,
    #[serde(default)]
    supply_invariant_pass: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct QaReport {
    generated_at: String,
}

#[derive(Debug, Deserialize)]
struct ProvenanceReport {
    data_source: String,
    simulated_data: bool,
}

#[derive(Serialize)]
struct StressSummaryRow<'a> {
    asset: &'a str,
    chain: &'a str,
    chain_id: u64,
    start_block: u64,
    end_block: Option<u64>,
    transfer_count: usize,
    active_senders: usize,
    active_recipients: usize,
    mint_count: usize,
    burn_count: usize,
    mint_sum_raw: &'a str,
    burn_sum_raw: &'a str,
    qa_status: &'a str,
    accounting_stress: bool,
    accounting_signals: String,
    activity_stress: bool,
    activity_signals: String,
}

pub fn run(asset: &str) -> Result<()> {
    let out_dir = ensure_out_dir(asset)?;
    let supply_rows = read_supply_audit(&out_dir)?;
    if supply_rows.is_empty() {
        anyhow::bail!("supply_audit.csv has no rows");
    }
    let supply_rows = normalize_rows(asset, supply_rows)?;

    let qa = read_qa_report(&out_dir)?;
    let provenance = read_provenance(&out_dir)?;
    if provenance.simulated_data {
        anyhow::bail!("provenance.json indicates simulated_data=true; refusing v0.1.5 summary");
    }

    let med_transfer = median_usize(supply_rows.iter().map(|r| r.transfer_count).collect());
    let med_senders = median_usize(supply_rows.iter().map(|r| r.active_senders).collect());
    let med_recipients = median_usize(supply_rows.iter().map(|r| r.active_recipients).collect());
    let med_mints = median_usize(supply_rows.iter().map(|r| r.mint_count).collect());
    let med_burns = median_usize(supply_rows.iter().map(|r| r.burn_count).collect());

    let mut summary_rows = Vec::new();
    for row in &supply_rows {
        let mut accounting_signals = Vec::new();
        let mut activity_signals = Vec::new();

        if row.qa_status != "PASS" {
            accounting_signals.push(format!("qa_status={}", row.qa_status));
        }
        if row
            .discrepancy_raw
            .as_deref()
            .is_some_and(|v| !is_zero_int(v))
        {
            accounting_signals.push(format!(
                "nonzero_discrepancy={}",
                row.discrepancy_raw.as_deref().unwrap_or("")
            ));
        }
        if is_spike(row.mint_count, med_mints, 2.0, 100) {
            accounting_signals.push(format!("mint_count_spike={}", row.mint_count));
        }
        if is_spike(row.burn_count, med_burns, 2.0, 100) {
            accounting_signals.push(format!("burn_count_spike={}", row.burn_count));
        }

        if is_spike(row.transfer_count, med_transfer, 2.0, 1_000) {
            activity_signals.push(format!("transfer_count_spike={}", row.transfer_count));
        }
        if is_spike(row.active_senders, med_senders, 2.0, 500) {
            activity_signals.push(format!("active_senders_spike={}", row.active_senders));
        }
        if is_spike(row.active_recipients, med_recipients, 2.0, 500) {
            activity_signals.push(format!(
                "active_recipients_spike={}",
                row.active_recipients
            ));
        }

        summary_rows.push(StressSummaryRow {
            asset: &row.asset,
            chain: &row.chain,
            chain_id: row.chain_id,
            start_block: row.start_block,
            end_block: row.end_block,
            transfer_count: row.transfer_count,
            active_senders: row.active_senders,
            active_recipients: row.active_recipients,
            mint_count: row.mint_count,
            burn_count: row.burn_count,
            mint_sum_raw: &row.mint_sum_raw,
            burn_sum_raw: &row.burn_sum_raw,
            qa_status: row.qa_status.as_str(),
            accounting_stress: !accounting_signals.is_empty(),
            accounting_signals: join_signals(&accounting_signals),
            activity_stress: !activity_signals.is_empty(),
            activity_signals: join_signals(&activity_signals),
        });
    }

    write_stress_summary_csv(&out_dir, &summary_rows)?;
    write_stress_summary_md(
        &out_dir,
        asset,
        &supply_rows,
        &summary_rows,
        &qa.generated_at,
        &provenance.data_source,
    )?;

    println!("\n=== v0.1.5 stress summary ({}) ===", asset.to_uppercase());
    println!(
        "Wrote: {}/v0_1_5_stress_summary.csv, {}/v0_1_5_summary.md",
        out_dir.display(),
        out_dir.display()
    );
    Ok(())
}

fn read_supply_audit(out_dir: &Path) -> Result<Vec<SupplyAuditRow>> {
    let path = out_dir.join("supply_audit.csv");
    if !path.exists() {
        anyhow::bail!(
            "supply_audit.csv not found at {}; run transfer-audit first",
            path.display()
        );
    }
    let mut rdr = csv::Reader::from_path(path)?;
    let mut rows = Vec::new();
    for rec in rdr.deserialize::<SupplyAuditRow>() {
        rows.push(rec?);
    }
    Ok(rows)
}

fn read_qa_report(out_dir: &Path) -> Result<QaReport> {
    let path = out_dir.join("qa_report.json");
    if !path.exists() {
        anyhow::bail!(
            "qa_report.json not found at {}; run transfer-audit first",
            path.display()
        );
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn read_provenance(out_dir: &Path) -> Result<ProvenanceReport> {
    let path = out_dir.join("provenance.json");
    if !path.exists() {
        anyhow::bail!(
            "provenance.json not found at {}; run transfer-audit first",
            path.display()
        );
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn normalize_rows(asset: &str, mut rows: Vec<SupplyAuditRow>) -> Result<Vec<SupplyAuditRow>> {
    let expected = asset.to_uppercase();
    for row in &mut rows {
        if row.asset.trim().is_empty() {
            row.asset = expected.clone();
        } else if row.asset.to_uppercase() != expected {
            anyhow::bail!(
                "asset mismatch in supply_audit.csv: requested {}, row has {} for chain {}",
                expected,
                row.asset,
                row.chain
            );
        }
        if row.qa_status.trim().is_empty() {
            row.qa_status = derive_qa_status(row);
        }
    }
    Ok(rows)
}

fn derive_qa_status(row: &SupplyAuditRow) -> String {
    if row.metadata_call_pass != Some(true)
        || row.historical_supply_pass != Some(true)
        || row.no_duplicate_logs_pass != Some(true)
        || row.transfer_decode_pass != Some(true)
    {
        return "FAIL".into();
    }
    match row.supply_invariant_pass {
        Some(true) => "PASS".into(),
        Some(false) => "FAIL".into(),
        None => "UNAVAILABLE".into(),
    }
}

fn median_usize(mut values: Vec<usize>) -> usize {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    values[values.len() / 2]
}

fn is_spike(value: usize, median: usize, multiplier: f64, absolute_floor: usize) -> bool {
    if value < absolute_floor {
        return false;
    }
    if median == 0 {
        return value >= absolute_floor;
    }
    (value as f64) > (median as f64 * multiplier)
}

fn is_zero_int(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }
    let digits = if let Some(rest) = trimmed.strip_prefix('-') {
        rest
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        rest
    } else {
        trimmed
    };
    !digits.is_empty() && digits.chars().all(|c| c == '0')
}

fn join_signals(signals: &[String]) -> String {
    if signals.is_empty() {
        "".into()
    } else {
        signals.join("; ")
    }
}

fn write_stress_summary_csv(out_dir: &Path, rows: &[StressSummaryRow<'_>]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(out_dir.join("v0_1_5_stress_summary.csv"))?;
    for row in rows {
        wtr.serialize(row)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_stress_summary_md(
    out_dir: &Path,
    asset: &str,
    source_rows: &[SupplyAuditRow],
    summary_rows: &[StressSummaryRow<'_>],
    qa_generated_at: &str,
    data_source: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let mut md = String::new();
    md.push_str(&format!("# {} v0.1.5 Stress Summary\n\n", asset.to_uppercase()));
    md.push_str(&format!("Generated at: {}\n\n", now));
    md.push_str(&format!("Source QA generated_at: {}\n", qa_generated_at));
    md.push_str(&format!("Data source: {}\n\n", data_source));
    md.push_str("This report summarizes **accounting stress** and **activity stress** from v0.1 artifacts only.\n");
    md.push_str("It does not claim purchasing power, peg, reserve adequacy, or dilution detection.\n\n");

    md.push_str("## Chain Summary\n\n");
    md.push_str("| Chain | Window | QA | Accounting Stress | Activity Stress |\n");
    md.push_str("|---|---|---|---|---|\n");
    for row in summary_rows {
        let window = format!(
            "{} -> {}",
            row.start_block,
            row.end_block
                .map(|v| v.to_string())
                .unwrap_or_else(|| "latest-unresolved".into())
        );
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            row.chain,
            window,
            row.qa_status,
            if row.accounting_stress { "YES" } else { "NO" },
            if row.activity_stress { "YES" } else { "NO" },
        ));
    }
    md.push('\n');

    md.push_str("## Details\n\n");
    for (src, row) in source_rows.iter().zip(summary_rows.iter()) {
        md.push_str(&format!("### {}\n\n", row.chain));
        md.push_str(&format!(
            "- Window: {} -> {}\n",
            src.start_block,
            src.end_block
                .map(|v| v.to_string())
                .unwrap_or_else(|| "latest-unresolved".into())
        ));
        md.push_str(&format!("- Contract: `{}`\n", src.contract_address));
        md.push_str(&format!("- RPC provider alias: `{}`\n", src.rpc_provider_alias));
        md.push_str(&format!(
            "- transfer_count={}, active_senders={}, active_recipients={}\n",
            src.transfer_count, src.active_senders, src.active_recipients
        ));
        md.push_str(&format!(
            "- mint_count={}, burn_count={}, mint_sum_raw={}, burn_sum_raw={}\n",
            src.mint_count, src.burn_count, src.mint_sum_raw, src.burn_sum_raw
        ));
        md.push_str(&format!(
            "- discrepancy_raw={}\n",
            src.discrepancy_raw.as_deref().unwrap_or("")
        ));
        md.push_str(&format!(
            "- accounting_stress={} ({})\n",
            row.accounting_stress,
            if row.accounting_signals.is_empty() {
                "none".to_string()
            } else {
                row.accounting_signals.clone()
            }
        ));
        md.push_str(&format!(
            "- activity_stress={} ({})\n\n",
            row.activity_stress,
            if row.activity_signals.is_empty() {
                "none".to_string()
            } else {
                row.activity_signals.clone()
            }
        ));
    }

    md.push_str("---\n");
    md.push_str("Canonical artifacts remain `supply_audit.csv`, `qa_report.json`, and `provenance.json`.\n");

    std::fs::write(out_dir.join("v0_1_5_summary.md"), md)?;
    Ok(())
}
