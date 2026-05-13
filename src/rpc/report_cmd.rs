//! v0.1.5 stress summary from `transfer-audit` artifacts (`qa_report.json`, `provenance.json`, `supply_audit.csv`).
//!
//! Accounting stress: supply-invariant QA lane plus mint/burn **event** spike vs sibling chains.
//! Activity stress: transfer count and active-address **touches** (`senders + recipients` in-window) vs siblings.
//! Cohort medians exclude the chain under test; single-chain runs yield `UNAVAILABLE` spike tiers.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::report::ensure_out_dir;

/// Multiplicative line for HIGH vs cohort median of **other** chains (same run).
const SPIKE_HIGH_MULT: f64 = 3.0;
/// Multiplicative line for ELEVATED tier.
const SPIKE_ELEVATED_MULT: f64 = 1.75;
/// Floor so tiny medians do not explode ratios; tuned per metric below.
const SPIKE_TRANSFER_MIN_ABS: usize = 500;
const SPIKE_MINT_BURN_MIN_ABS: usize = 50;
const SPIKE_ACTIVE_TOUCH_MIN_ABS: usize = 200;

// ─── Inputs (match `transfer_audit` JSON/CSV shapes) ───────────────────────

#[derive(Debug, Deserialize)]
struct QaReportFile {
    asset: String,
    generated_at: String,
    #[serde(default)]
    provenance: Option<QaProvenanceBlock>,
    chains: Vec<QaChainFile>,
}

#[derive(Debug, Deserialize)]
struct QaProvenanceBlock {
    from_block: u64,
    #[serde(default)]
    to_block_requested: Option<String>,
    generated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct QaChainFile {
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

#[derive(Debug, Clone, Deserialize)]
struct QaGates {
    metadata_call_pass: String,
    historical_supply_pass: String,
    no_duplicate_logs_pass: String,
    transfer_decode_pass: String,
    supply_invariant_pass: String,
    provenance_stamped: String,
}

#[derive(Debug, Deserialize)]
struct ProvenanceFile {
    asset: String,
    generated_at: String,
    data_source: String,
    simulated_data: bool,
    chains: Vec<ProvChain>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProvChain {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    resolved_to_block: Option<u64>,
}

/// Row shape matches `transfer_audit` `supply_audit.csv`; only a subset is used in the stress table.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
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

#[derive(Serialize)]
struct StressSummaryRow {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    resolved_to_block: Option<u64>,
    /// All QA gates `PASS`.
    stress_qa_gates: String,
    /// Supply invariant lane from QA (`PASS` / `FAIL` / other → `UNAVAILABLE`).
    accounting_supply_invariant: String,
    /// Cohort-relative mint+burn **event** intensity vs other chains (`CALM` / `ELEVATED` / `HIGH` / `UNAVAILABLE`).
    accounting_mint_burn_spike: String,
    /// Cohort-relative deduped transfer event count.
    activity_transfer_spike: String,
    /// Cohort-relative `active_senders + active_recipients` (window touch volume; not unique holders).
    activity_active_address_spike: String,
    /// Worst of supply invariant + mint/burn spike (`PASS` / `WARN` / `FAIL`).
    stress_accounting: String,
    /// Transfer + active-address spike lanes (`PASS` / `WARN` / `FAIL` / `UNAVAILABLE`).
    stress_activity: String,
    /// QA gates + accounting + activity composite (`PASS` / `WARN` / `FAIL`).
    stress_overall: String,
    metadata_call_pass: String,
    historical_supply_pass: String,
    no_duplicate_logs_pass: String,
    transfer_decode_pass: String,
    supply_invariant_pass: String,
    provenance_stamped: String,
    duplicate_count: usize,
    full_decode_error_count: usize,
    transfer_event_count: usize,
    active_senders: usize,
    active_recipients: usize,
    mint_burn_event_intensity: usize,
    active_address_touches: usize,
    cohort_peer_count: usize,
    cohort_median_mint_burn_events: String,
    cohort_median_transfer_events: String,
    cohort_median_active_address_touches: String,
    mint_count: usize,
    burn_count: usize,
    plain_transfer_count: usize,
    sum_mints_raw: String,
    sum_burns_raw: String,
    net_mint_raw: String,
    onchain_delta_raw: String,
    discrepancy_raw: String,
    qa_error_count: usize,
}

// ─── Helpers ──────────────────────────────────────────────────────────────

fn norm_asset(s: &str) -> String {
    s.trim().to_uppercase()
}

fn addr_eq(a: &str, b: &str) -> bool {
    let strip = |x: &str| x.trim_start_matches("0x").to_lowercase();
    strip(a) == strip(b)
}

fn gate_is_pass(s: &str) -> bool {
    s == "PASS"
}

fn chain_stress_pass(gates: &QaGates) -> bool {
    gate_is_pass(&gates.metadata_call_pass)
        && gate_is_pass(&gates.historical_supply_pass)
        && gate_is_pass(&gates.no_duplicate_logs_pass)
        && gate_is_pass(&gates.transfer_decode_pass)
        && gate_is_pass(&gates.supply_invariant_pass)
        && gate_is_pass(&gates.provenance_stamped)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpikeTier {
    Calm,
    Elevated,
    High,
    Unavailable,
}

fn spike_tier_label(t: SpikeTier) -> &'static str {
    match t {
        SpikeTier::Calm => "CALM",
        SpikeTier::Elevated => "ELEVATED",
        SpikeTier::High => "HIGH",
        SpikeTier::Unavailable => "UNAVAILABLE",
    }
}

fn median_sorted(sorted: &[usize]) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    if n % 2 == 1 {
        sorted[n / 2] as f64
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) as f64 / 2.0
    }
}

/// Median of **other** chains (same run), for cohort-relative spikes.
fn peer_median_excluding(chain: &str, counts: &HashMap<String, usize>) -> Option<f64> {
    let mut v: Vec<usize> = counts
        .iter()
        .filter(|(c, _)| *c != chain)
        .map(|(_, x)| *x)
        .collect();
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(median_sorted(&v))
}

fn spike_tier(
    value: usize,
    median_peers: Option<f64>,
    min_abs: usize,
    high_mult: f64,
    elevated_mult: f64,
) -> SpikeTier {
    let Some(m_raw) = median_peers else {
        return SpikeTier::Unavailable;
    };
    let m = m_raw.max(0.0);
    let high_line = f64::max(min_abs as f64, high_mult * m);
    let elevated_line = f64::max((min_abs / 2).max(1) as f64, elevated_mult * m);

    if value == 0 && m == 0.0 {
        return SpikeTier::Calm;
    }
    if value as f64 >= high_line {
        SpikeTier::High
    } else if value as f64 >= elevated_line {
        SpikeTier::Elevated
    } else {
        SpikeTier::Calm
    }
}

fn supply_invariant_lane(gates: &QaGates) -> &'static str {
    match gates.supply_invariant_pass.as_str() {
        "PASS" => "PASS",
        "FAIL" => "FAIL",
        _ => "UNAVAILABLE",
    }
}

fn stress_accounting_lane(supply_lane: &str, mint_spike: SpikeTier) -> &'static str {
    if supply_lane == "FAIL" || mint_spike == SpikeTier::High {
        "FAIL"
    } else if supply_lane == "UNAVAILABLE" || mint_spike == SpikeTier::Elevated {
        "WARN"
    } else {
        "PASS"
    }
}

fn stress_activity_lane(transfer: SpikeTier, active: SpikeTier) -> &'static str {
    use SpikeTier::*;
    if matches!(transfer, Unavailable) && matches!(active, Unavailable) {
        return "UNAVAILABLE";
    }
    if matches!(transfer, High) || matches!(active, High) {
        return "FAIL";
    }
    if matches!(transfer, Elevated) || matches!(active, Elevated) {
        return "WARN";
    }
    "PASS"
}

fn stress_overall_lane(qa_all_pass: bool, accounting: &str, activity: &str) -> &'static str {
    if !qa_all_pass || accounting == "FAIL" || activity == "FAIL" {
        "FAIL"
    } else if accounting == "WARN" || activity == "WARN" || activity == "UNAVAILABLE" {
        "WARN"
    } else {
        "PASS"
    }
}

fn fmt_opt_median(m: Option<f64>) -> String {
    match m {
        Some(x) if x.is_finite() => format!("{x:.2}"),
        _ => String::new(),
    }
}

fn load_supply_csv(path: &Path) -> Result<HashMap<String, SupplyAuditRow>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut m: HashMap<String, SupplyAuditRow> = HashMap::new();
    for res in rdr.deserialize::<SupplyAuditRow>() {
        let row = res.with_context(|| format!("parse row in {}", path.display()))?;
        let chain = row.chain.clone();
        if m.insert(chain.clone(), row).is_some() {
            anyhow::bail!("duplicate chain {chain:?} in {}", path.display());
        }
    }
    Ok(m)
}

fn validate_cross_artifacts(
    asset_cli: &str,
    qa: &QaReportFile,
    prov: &ProvenanceFile,
    supply: &HashMap<String, SupplyAuditRow>,
) -> Result<HashMap<String, QaChainFile>> {
    let want = norm_asset(asset_cli);
    if norm_asset(&qa.asset) != want {
        anyhow::bail!(
            "qa_report.json asset {:?} does not match --asset {:?}",
            qa.asset,
            asset_cli
        );
    }
    if norm_asset(&prov.asset) != want {
        anyhow::bail!(
            "provenance.json asset {:?} does not match --asset {:?}",
            prov.asset,
            asset_cli
        );
    }
    if prov.simulated_data {
        anyhow::bail!(
            "provenance.json indicates simulated_data=true; refusing v0.1.5 report (non-canonical bundle)"
        );
    }
    if prov.data_source != "onchain_rpc" {
        anyhow::bail!(
            "provenance.json data_source must be \"onchain_rpc\" (canonical transfer-audit); got {:?}",
            prov.data_source
        );
    }

    let mut qa_seen: HashSet<&str> = HashSet::new();
    for c in &qa.chains {
        if !qa_seen.insert(c.chain.as_str()) {
            anyhow::bail!("duplicate chain {:?} in qa_report.json", c.chain);
        }
    }
    let mut pr_seen: HashSet<&str> = HashSet::new();
    for c in &prov.chains {
        if !pr_seen.insert(c.chain.as_str()) {
            anyhow::bail!("duplicate chain {:?} in provenance.json", c.chain);
        }
    }

    let qa_chains: HashMap<String, QaChainFile> = qa
        .chains
        .iter()
        .map(|c| (c.chain.clone(), c.clone()))
        .collect::<HashMap<_, _>>();
    let prov_chains: HashMap<String, ProvChain> = prov
        .chains
        .iter()
        .map(|c| (c.chain.clone(), c.clone()))
        .collect::<HashMap<_, _>>();

    let set_qa: HashSet<_> = qa_chains.keys().cloned().collect();
    let set_pr: HashSet<_> = prov_chains.keys().cloned().collect();
    let set_su: HashSet<_> = supply.keys().cloned().collect();
    if set_qa != set_pr || set_qa != set_su {
        anyhow::bail!(
            "chain set mismatch: qa_report {:?}, provenance {:?}, supply_audit {:?}",
            set_qa,
            set_pr,
            set_su
        );
    }

    for chain in &set_qa {
        let q = qa_chains.get(chain).unwrap();
        let p = prov_chains.get(chain).unwrap();
        let s = supply.get(chain).unwrap();

        if q.chain_id != p.chain_id || q.chain_id != s.chain_id {
            anyhow::bail!(
                "chain_id mismatch for {chain:?}: qa {} provenance {} supply {}",
                q.chain_id,
                p.chain_id,
                s.chain_id
            );
        }
        if !addr_eq(&q.contract_address, &p.contract_address)
            || !addr_eq(&q.contract_address, &s.contract_address)
        {
            anyhow::bail!(
                "contract_address mismatch for chain {chain:?}: qa {} provenance {} supply {}",
                q.contract_address,
                p.contract_address,
                s.contract_address
            );
        }
        if q.from_block != p.from_block || q.from_block != s.from_block {
            anyhow::bail!(
                "from_block mismatch for {chain:?}: qa {} provenance {} supply {}",
                q.from_block,
                p.from_block,
                s.from_block
            );
        }
        if q.resolved_to_block != p.resolved_to_block || q.resolved_to_block != s.resolved_to_block {
            anyhow::bail!(
                "resolved_to_block mismatch for {chain:?}: qa {:?} provenance {:?} supply {:?}",
                q.resolved_to_block,
                p.resolved_to_block,
                s.resolved_to_block
            );
        }
    }

    Ok(qa_chains)
}

fn write_stress_csv(path: &Path, rows: &[StressSummaryRow]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    for row in rows {
        wtr.serialize(row)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_summary_md(
    path: &Path,
    asset: &str,
    report_at: &str,
    qa: &QaReportFile,
    prov: &ProvenanceFile,
    rows: &[StressSummaryRow],
) -> Result<()> {
    let mut md = String::new();
    md.push_str(&format!("# v0.1.5 stress summary — {}\n\n", norm_asset(asset)));
    md.push_str(&format!("**Report generated:** {report_at}\n\n"));
    md.push_str("## Inputs (transfer-audit)\n\n");
    md.push_str(&format!(
        "- `qa_report.json` (generated {})\n",
        qa.generated_at
    ));
    if let Some(ref pb) = qa.provenance {
        md.push_str(&format!(
            "- QA provenance block: from_block={}, to_block_requested={:?}, generated_at={}\n",
            pb.from_block, pb.to_block_requested, pb.generated_at
        ));
    }
    md.push_str(&format!(
        "- `provenance.json` (generated {})\n",
        prov.generated_at
    ));
    md.push_str("- `supply_audit.csv`\n\n");
    md.push_str(
        "## Stress model (v0.1.5)\n\n\
Accounting stress combines the **supply invariant** QA lane with a **mint/burn event spike** \
signal (mint + burn counts vs cohort median of other chains in this run). \
Activity stress uses **deduped transfer event count** and **active address touches** \
(`active_senders + active_recipients` in the window — an upper-bound style footprint, not a unique-holder census), \
each vs the same cohort. With only one chain, cohort-relative spikes are **UNAVAILABLE**.\n\n",
    );

    md.push_str("## QA gates (all chains)\n\n");
    md.push_str("| chain | stress_qa_gates | metadata | historical_supply | no_dup | decode | supply_inv | provenance_stamp | decode_errors |\n");
    md.push_str("|-------|-----------------|----------|-------------------|--------|--------|------------|------------------|---------------|\n");
    for r in rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.chain,
            r.stress_qa_gates,
            r.metadata_call_pass,
            r.historical_supply_pass,
            r.no_duplicate_logs_pass,
            r.transfer_decode_pass,
            r.supply_invariant_pass,
            r.provenance_stamped,
            r.full_decode_error_count
        ));
    }

    md.push_str("\n## Accounting stress\n\n");
    md.push_str("| chain | supply_invariant | mint_burn_spike | stress_accounting | mint+burn events | peer median (m+b) |\n");
    md.push_str("|-------|------------------|-----------------|-------------------|------------------|-------------------|\n");
    for r in rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            r.chain,
            r.accounting_supply_invariant,
            r.accounting_mint_burn_spike,
            r.stress_accounting,
            r.mint_burn_event_intensity,
            r.cohort_median_mint_burn_events
        ));
    }

    md.push_str("\n## Activity stress\n\n");
    md.push_str("| chain | transfer_spike | active_address_spike | stress_activity | transfers | active_touches | peer med (xfer / touches) |\n");
    md.push_str("|-------|----------------|----------------------|-----------------|-----------|----------------|---------------------------|\n");
    for r in rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} / {} |\n",
            r.chain,
            r.activity_transfer_spike,
            r.activity_active_address_spike,
            r.stress_activity,
            r.transfer_event_count,
            r.active_address_touches,
            r.cohort_median_transfer_events,
            r.cohort_median_active_address_touches
        ));
    }

    md.push_str("\n## Composite\n\n");
    md.push_str("| chain | stress_overall |\n");
    md.push_str("|-------|----------------|\n");
    for r in rows {
        md.push_str(&format!("| {} | {} |\n", r.chain, r.stress_overall));
    }

    md.push_str("\n---\n\n_Derived from transfer-audit artifacts only; cohort spikes are relative to sibling chains in the same artifact bundle, not historical baselines._\n");
    std::fs::write(path, md)?;
    Ok(())
}

/// Build v0.1.5 stress summary CSV + markdown from `transfer-audit` outputs.
pub fn run(asset: &str) -> Result<()> {
    let out_dir = ensure_out_dir(asset)?;
    let qa_path = out_dir.join("qa_report.json");
    let prov_path = out_dir.join("provenance.json");
    let supply_path = out_dir.join("supply_audit.csv");

    if !qa_path.exists() {
        anyhow::bail!(
            "qa_report.json not found at {}; run `transfer-audit` first",
            qa_path.display()
        );
    }
    if !prov_path.exists() {
        anyhow::bail!(
            "provenance.json not found at {}; run `transfer-audit` first",
            prov_path.display()
        );
    }
    if !supply_path.exists() {
        anyhow::bail!(
            "supply_audit.csv not found at {}; run `transfer-audit` first",
            supply_path.display()
        );
    }

    let qa: QaReportFile = serde_json::from_str(
        &std::fs::read_to_string(&qa_path).with_context(|| qa_path.display().to_string())?,
    )
    .with_context(|| format!("parse {}", qa_path.display()))?;

    let prov: ProvenanceFile = serde_json::from_str(
        &std::fs::read_to_string(&prov_path).with_context(|| prov_path.display().to_string())?,
    )
    .with_context(|| format!("parse {}", prov_path.display()))?;

    let supply = load_supply_csv(&supply_path).context("read supply_audit.csv")?;

    let qa_by_chain = validate_cross_artifacts(asset, &qa, &prov, &supply)?;

    let mut chain_names: Vec<String> = supply.keys().cloned().collect();
    chain_names.sort();

    let mut mint_burn_by_chain: HashMap<String, usize> = HashMap::new();
    let mut transfers_by_chain: HashMap<String, usize> = HashMap::new();
    let mut active_touches_by_chain: HashMap<String, usize> = HashMap::new();
    for c in &chain_names {
        let s = supply.get(c).unwrap();
        mint_burn_by_chain.insert(c.clone(), s.mint_count.saturating_add(s.burn_count));
        transfers_by_chain.insert(c.clone(), s.transfer_event_count);
        active_touches_by_chain.insert(
            c.clone(),
            s.active_senders.saturating_add(s.active_recipients),
        );
    }

    let report_at = Utc::now().to_rfc3339();
    let mut rows: Vec<StressSummaryRow> = Vec::new();
    let cohort_peer_count = chain_names.len().saturating_sub(1);

    for chain in chain_names {
        let q = qa_by_chain.get(&chain).unwrap();
        let s = supply.get(&chain).unwrap();
        let qa_all_pass = chain_stress_pass(&q.gates);

        let mb_val = *mint_burn_by_chain.get(&chain).unwrap();
        let tr_val = *transfers_by_chain.get(&chain).unwrap();
        let ac_val = *active_touches_by_chain.get(&chain).unwrap();

        let mb_med = peer_median_excluding(&chain, &mint_burn_by_chain);
        let tr_med = peer_median_excluding(&chain, &transfers_by_chain);
        let ac_med = peer_median_excluding(&chain, &active_touches_by_chain);

        let mb_tier = spike_tier(
            mb_val,
            mb_med,
            SPIKE_MINT_BURN_MIN_ABS,
            SPIKE_HIGH_MULT,
            SPIKE_ELEVATED_MULT,
        );
        let tr_tier = spike_tier(
            tr_val,
            tr_med,
            SPIKE_TRANSFER_MIN_ABS,
            SPIKE_HIGH_MULT,
            SPIKE_ELEVATED_MULT,
        );
        let ac_tier = spike_tier(
            ac_val,
            ac_med,
            SPIKE_ACTIVE_TOUCH_MIN_ABS,
            SPIKE_HIGH_MULT,
            SPIKE_ELEVATED_MULT,
        );

        let supply_lane = supply_invariant_lane(&q.gates);
        let stress_acct = stress_accounting_lane(supply_lane, mb_tier);
        let stress_act = stress_activity_lane(tr_tier, ac_tier);
        let overall = stress_overall_lane(qa_all_pass, stress_acct, stress_act);

        rows.push(StressSummaryRow {
            chain: chain.clone(),
            chain_id: s.chain_id,
            contract_address: s.contract_address.clone(),
            from_block: s.from_block,
            resolved_to_block: s.resolved_to_block,
            stress_qa_gates: if qa_all_pass {
                "PASS".into()
            } else {
                "FAIL".into()
            },
            accounting_supply_invariant: supply_lane.into(),
            accounting_mint_burn_spike: spike_tier_label(mb_tier).into(),
            activity_transfer_spike: spike_tier_label(tr_tier).into(),
            activity_active_address_spike: spike_tier_label(ac_tier).into(),
            stress_accounting: stress_acct.into(),
            stress_activity: stress_act.into(),
            stress_overall: overall.into(),
            metadata_call_pass: q.gates.metadata_call_pass.clone(),
            historical_supply_pass: q.gates.historical_supply_pass.clone(),
            no_duplicate_logs_pass: q.gates.no_duplicate_logs_pass.clone(),
            transfer_decode_pass: q.gates.transfer_decode_pass.clone(),
            supply_invariant_pass: q.gates.supply_invariant_pass.clone(),
            provenance_stamped: q.gates.provenance_stamped.clone(),
            duplicate_count: q.duplicate_count,
            full_decode_error_count: q.full_decode_error_count,
            transfer_event_count: s.transfer_event_count,
            active_senders: s.active_senders,
            active_recipients: s.active_recipients,
            mint_burn_event_intensity: mb_val,
            active_address_touches: ac_val,
            cohort_peer_count,
            cohort_median_mint_burn_events: fmt_opt_median(mb_med),
            cohort_median_transfer_events: fmt_opt_median(tr_med),
            cohort_median_active_address_touches: fmt_opt_median(ac_med),
            mint_count: s.mint_count,
            burn_count: s.burn_count,
            plain_transfer_count: s.plain_transfer_count,
            sum_mints_raw: s.sum_mints_raw.clone(),
            sum_burns_raw: s.sum_burns_raw.clone(),
            net_mint_raw: s.net_mint_raw.clone().unwrap_or_default(),
            onchain_delta_raw: s.onchain_delta_raw.clone().unwrap_or_default(),
            discrepancy_raw: s.discrepancy_raw.clone().unwrap_or_default(),
            qa_error_count: q.errors.len(),
        });
    }

    let csv_path = out_dir.join("v0_1_5_stress_summary.csv");
    let md_path = out_dir.join("v0_1_5_summary.md");
    write_stress_csv(&csv_path, &rows)?;
    write_summary_md(&md_path, asset, &report_at, &qa, &prov, &rows)?;

    println!(
        "\n=== v0.1.5 report for {} ===",
        norm_asset(asset)
    );
    println!("Chains: {}", rows.len());
    for r in &rows {
        println!(
            "  {} | overall {} | accounting {} | activity {} | transfers {} | mint+burn {}",
            r.chain,
            r.stress_overall,
            r.stress_accounting,
            r.stress_activity,
            r.transfer_event_count,
            r.mint_burn_event_intensity
        );
    }
    println!("\nWritten:");
    println!("  {}", csv_path.display());
    println!("  {}", md_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_median_excludes_self_three_chains() {
        let mut m: HashMap<String, usize> = HashMap::new();
        m.insert("a".into(), 10);
        m.insert("b".into(), 20);
        m.insert("c".into(), 30);
        assert!((peer_median_excluding("a", &m).unwrap() - 25.0).abs() < 0.001);
        assert!((peer_median_excluding("b", &m).unwrap() - 20.0).abs() < 0.001);
    }

    #[test]
    fn peer_median_single_chain_none() {
        let mut m: HashMap<String, usize> = HashMap::new();
        m.insert("only".into(), 100);
        assert!(peer_median_excluding("only", &m).is_none());
    }

    #[test]
    fn spike_high_when_far_above_median() {
        let tier = spike_tier(3000, Some(500.0), 500, 3.0, 1.75);
        assert_eq!(tier, SpikeTier::High);
    }

    #[test]
    fn spike_unavailable_without_peers() {
        let tier = spike_tier(100, None, 50, 3.0, 1.75);
        assert_eq!(tier, SpikeTier::Unavailable);
    }
}