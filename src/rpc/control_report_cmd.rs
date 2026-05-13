use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::control_events::ControlEventRecord;
use crate::report::ensure_out_dir;

#[derive(Debug, Deserialize)]
struct ControlQaReport {
    asset: String,
    generated_at: String,
    chains: Vec<ControlQaChain>,
}

#[derive(Debug, Deserialize)]
struct ControlQaChain {
    chain: String,
    chain_id: u64,
    #[allow(dead_code)]
    contract_address: String,
    #[allow(dead_code)]
    rpc_provider_alias: String,
    from_block: u64,
    to_block: Option<u64>,
    control_event_count: usize,
    control_event_query_status: String,
    control_decode_error_count: usize,
    gates: ControlQaGates,
    #[allow(dead_code)]
    errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ControlQaGates {
    control_event_query_pass: String,
    control_decode_pass: String,
    provenance_stamped_pass: String,
    no_simulated_data_pass: String,
}

#[derive(Debug, Deserialize)]
struct ControlProvChainRow {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    to_block: Option<u64>,
    #[serde(default)]
    simulated_data: bool,
}

#[derive(Debug, Deserialize)]
struct ControlProvenanceReport {
    asset: String,
    generated_at: String,
    data_source: String,
    simulated_data: bool,
    chains: Vec<ControlProvChainRow>,
}

#[derive(Serialize)]
struct BenchmarkRow<'a> {
    asset: &'a str,
    chain: &'a str,
    chain_id: u64,
    from_block: u64,
    to_block: Option<u64>,
    control_event_count: usize,
    pause_event_count: usize,
    blacklist_event_count: usize,
    minter_admin_event_count: usize,
    upgrade_ownership_event_count: usize,
    query_status: &'a str,
    decode_error_count: usize,
    benchmark_status: &'a str,
    benchmark_signals: String,
}

pub fn run(asset: &str) -> Result<()> {
    let out_dir = ensure_out_dir(asset)?;
    let qa = read_control_qa(&out_dir)?;
    let prov = read_control_provenance(&out_dir)?;
    if prov.simulated_data {
        anyhow::bail!("control_provenance.json indicates simulated_data=true; refusing benchmark");
    }
    if prov.data_source != "onchain_rpc" {
        anyhow::bail!(
            "control_provenance.json data_source must be \"onchain_rpc\"; got {:?}",
            prov.data_source
        );
    }
    if qa.asset.to_uppercase() != asset.to_uppercase() || prov.asset.to_uppercase() != asset.to_uppercase() {
        anyhow::bail!("asset mismatch between CLI and control artifacts");
    }
    validate_control_provenance_vs_qa(&qa, &prov)?;

    let mut rows = Vec::new();
    for chain in &qa.chains {
        let counts = validate_and_count_control_events_csv(&out_dir, chain)?;
        let mut signals = Vec::new();
        if chain.gates.control_event_query_pass != "PASS" {
            signals.push("query_not_pass".to_string());
        }
        if chain.gates.control_decode_pass != "PASS" {
            signals.push("decode_not_pass".to_string());
        }
        if chain.gates.provenance_stamped_pass != "PASS" {
            signals.push("provenance_not_pass".to_string());
        }
        if chain.gates.no_simulated_data_pass != "PASS" {
            signals.push("simulated_data_gate_not_pass".to_string());
        }
        if counts.pause > 0 {
            signals.push(format!("pause_events={}", counts.pause));
        }
        if counts.blacklist > 0 {
            signals.push(format!("blacklist_events={}", counts.blacklist));
        }
        if counts.minter_admin > 0 {
            signals.push(format!("minter_admin_events={}", counts.minter_admin));
        }
        if counts.upgrade_ownership > 0 {
            signals.push(format!("upgrade_or_ownership_events={}", counts.upgrade_ownership));
        }

        let benchmark_status = if signals.is_empty() { "PASS" } else { "WARN" };
        rows.push(BenchmarkRow {
            asset,
            chain: &chain.chain,
            chain_id: chain.chain_id,
            from_block: chain.from_block,
            to_block: chain.to_block,
            control_event_count: chain.control_event_count,
            pause_event_count: counts.pause,
            blacklist_event_count: counts.blacklist,
            minter_admin_event_count: counts.minter_admin,
            upgrade_ownership_event_count: counts.upgrade_ownership,
            query_status: &chain.control_event_query_status,
            decode_error_count: chain.control_decode_error_count,
            benchmark_status,
            benchmark_signals: signals.join("; "),
        });
    }

    write_benchmark_csv(&out_dir, &rows)?;
    write_benchmark_md(asset, &qa, &prov, &rows, &out_dir)?;

    println!("\n=== v0.2 control-surface benchmark ({}) ===", asset.to_uppercase());
    println!(
        "Wrote: {}/v0_2_control_benchmark.csv, {}/v0_2_control_benchmark.md",
        out_dir.display(),
        out_dir.display()
    );

    Ok(())
}

fn read_control_qa(out_dir: &std::path::Path) -> Result<ControlQaReport> {
    let path = out_dir.join("control_qa_report.json");
    if !path.exists() {
        anyhow::bail!("control_qa_report.json not found; run control-audit first");
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn read_control_provenance(out_dir: &std::path::Path) -> Result<ControlProvenanceReport> {
    let path = out_dir.join("control_provenance.json");
    if !path.exists() {
        anyhow::bail!("control_provenance.json not found; run control-audit first");
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn addr_eq(a: &str, b: &str) -> bool {
    let strip = |x: &str| x.trim_start_matches("0x").to_lowercase();
    strip(a) == strip(b)
}

/// Full bundle: same chain set in QA and provenance, matching fingerprints, no simulated rows.
fn validate_control_provenance_vs_qa(qa: &ControlQaReport, prov: &ControlProvenanceReport) -> Result<()> {
    if qa.chains.is_empty() {
        anyhow::bail!("control_qa_report.json has no chains");
    }
    if prov.chains.is_empty() {
        anyhow::bail!(
            "control_provenance.json has no chains; re-run control-audit (no chain reached a provenance stamp)"
        );
    }

    let mut qa_by: HashMap<&str, &ControlQaChain> = HashMap::new();
    for c in &qa.chains {
        if qa_by.insert(c.chain.as_str(), c).is_some() {
            anyhow::bail!("duplicate chain {:?} in control_qa_report.json", c.chain);
        }
    }

    let mut seen = HashSet::new();
    for p in &prov.chains {
        if !seen.insert(p.chain.as_str()) {
            anyhow::bail!("duplicate chain {:?} in control_provenance.json", p.chain);
        }
        if p.simulated_data {
            anyhow::bail!(
                "control_provenance.json chain {:?} has simulated_data=true",
                p.chain
            );
        }
        let Some(q) = qa_by.get(p.chain.as_str()) else {
            anyhow::bail!(
                "control_provenance.json lists chain {:?} not present in control_qa_report.json (stale or mismatched bundle)",
                p.chain
            );
        };
        if q.chain_id != p.chain_id {
            anyhow::bail!(
                "chain_id mismatch for {:?}: qa {} vs control_provenance {}",
                p.chain,
                q.chain_id,
                p.chain_id
            );
        }
        if !addr_eq(&q.contract_address, &p.contract_address) {
            anyhow::bail!(
                "contract_address mismatch for {:?}: qa {} vs control_provenance {}",
                p.chain,
                q.contract_address,
                p.contract_address
            );
        }
        if q.from_block != p.from_block {
            anyhow::bail!(
                "from_block mismatch for {:?}: qa {} vs control_provenance {}",
                p.chain,
                q.from_block,
                p.from_block
            );
        }
        if q.to_block != p.to_block {
            anyhow::bail!(
                "to_block mismatch for {:?}: qa {:?} vs control_provenance {:?}",
                p.chain,
                q.to_block,
                p.to_block
            );
        }
    }

    for q in &qa.chains {
        if !seen.contains(q.chain.as_str()) {
            anyhow::bail!(
                "control_provenance.json must list every QA chain; missing {:?}. Re-run control-audit so all chains share one coherent bundle.",
                q.chain
            );
        }
    }

    Ok(())
}

struct ControlCounts {
    pause: usize,
    blacklist: usize,
    minter_admin: usize,
    upgrade_ownership: usize,
}

/// Ensures `control_events_<chain>.csv` matches this QA row (row count, chain column, block window)
/// before deriving pause/blacklist/minter/upgrade bucket counts.
fn validate_and_count_control_events_csv(
    out_dir: &Path,
    qa_chain: &ControlQaChain,
) -> Result<ControlCounts> {
    let chain = qa_chain.chain.as_str();
    let path = out_dir.join(format!("control_events_{chain}.csv"));
    if !path.exists() {
        anyhow::bail!(
            "control_events_{}.csv not found at {}; expected for every QA chain after a full control-audit",
            chain,
            path.display()
        );
    }
    let to_block = qa_chain.to_block.ok_or_else(|| {
        anyhow::anyhow!(
            "QA chain {:?} has no resolved to_block; cannot validate control_events CSV",
            qa_chain.chain
        )
    })?;

    let mut rdr = csv::Reader::from_path(&path)
        .with_context(|| format!("open {}", path.display()))?;
    let mut counts = ControlCounts {
        pause: 0,
        blacklist: 0,
        minter_admin: 0,
        upgrade_ownership: 0,
    };
    let mut row_count: usize = 0;
    for (idx, rec) in rdr.deserialize::<ControlEventRecord>().enumerate() {
        let row = rec.with_context(|| format!("parse row {} in {}", idx.saturating_add(1), path.display()))?;
        row_count += 1;
        if row.chain != chain {
            anyhow::bail!(
                "control_events_{}.csv row {}: chain column {:?} does not match filename/QA chain {:?}",
                chain,
                idx.saturating_add(1),
                row.chain,
                chain
            );
        }
        if row.block_number < qa_chain.from_block || row.block_number > to_block {
            anyhow::bail!(
                "control_events_{}.csv row {}: block_number {} outside QA inclusive window {}–{}",
                chain,
                idx.saturating_add(1),
                row.block_number,
                qa_chain.from_block,
                to_block
            );
        }
        match row.event_name.as_str() {
            "Pause" | "Unpause" => counts.pause += 1,
            "Blacklisted" | "UnBlacklisted" => counts.blacklist += 1,
            "MinterConfigured" | "MinterRemoved" | "MasterMinterChanged" => counts.minter_admin += 1,
            "OwnershipTransferred" | "Upgraded" => counts.upgrade_ownership += 1,
            _ => {}
        }
    }

    if row_count != qa_chain.control_event_count {
        anyhow::bail!(
            "control_events_{}.csv has {} data rows but control_qa_report.json control_event_count is {}; stale or mismatched CSV",
            chain,
            row_count,
            qa_chain.control_event_count
        );
    }

    Ok(counts)
}

fn write_benchmark_csv(out_dir: &std::path::Path, rows: &[BenchmarkRow<'_>]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(out_dir.join("v0_2_control_benchmark.csv"))?;
    for row in rows {
        wtr.serialize(row)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_benchmark_md(
    asset: &str,
    qa: &ControlQaReport,
    prov: &ControlProvenanceReport,
    rows: &[BenchmarkRow<'_>],
    out_dir: &std::path::Path,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let mut md = String::new();
    md.push_str(&format!("# {} v0.2 Control-Surface Benchmark\n\n", asset.to_uppercase()));
    md.push_str(&format!("Generated at: {}\n\n", now));
    md.push_str(&format!("Control QA generated_at: {}\n", qa.generated_at));
    md.push_str(&format!("Control provenance generated_at: {}\n", prov.generated_at));
    md.push_str(&format!("Data source: {}\n\n", prov.data_source));
    md.push_str("Scope: issuer-side control actions only.\n");
    md.push_str("Non-scope: wallet attribution, AML scoring, or intent inference.\n\n");
    md.push_str("| Chain | Window | Events | Pause | Blacklist | Minter/Admin | Upgrade/Ownership | Status |\n");
    md.push_str("|---|---|---:|---:|---:|---:|---:|---|\n");
    for row in rows {
        md.push_str(&format!(
            "| {} | {} -> {} | {} | {} | {} | {} | {} | {} |\n",
            row.chain,
            row.from_block,
            row.to_block
                .map(|v| v.to_string())
                .unwrap_or_else(|| "unavailable".to_string()),
            row.control_event_count,
            row.pause_event_count,
            row.blacklist_event_count,
            row.minter_admin_event_count,
            row.upgrade_ownership_event_count,
            row.benchmark_status,
        ));
    }
    md.push_str("\n---\n");
    md.push_str("Canonical v0.2 artifacts remain control_events_<chain>.csv, control_qa_report.json, control_provenance.json.\n");
    std::fs::write(out_dir.join("v0_2_control_benchmark.md"), md)?;
    Ok(())
}
