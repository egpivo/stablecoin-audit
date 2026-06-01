//! Milestone 4 — cross-chain window summary from `transfer-audit` artifacts.

use alloy::primitives::I256;
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::artifact::{upsert_cross_chain_summary_manifest, CrossChainSummaryManifestParams};
use crate::report::{ensure_run_out_dir, validate_run_id};

#[derive(Debug, Deserialize)]
struct QaReportFile {
    asset: String,
    generated_at: String,
    #[serde(default)]
    run_id: Option<String>,
    provenance: QaProvenanceBlock,
    chains: Vec<QaChainFile>,
}

#[derive(Debug, Deserialize)]
struct QaProvenanceBlock {
    from_block: u64,
    #[serde(default)]
    to_block_requested: Option<String>,
    generated_at: String,
    /// When true, each chain may use a different native block span; `from_block` here is informational (minimum).
    #[serde(default)]
    per_chain_spans: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct QaGatesSnapshot {
    metadata_call_pass: String,
    historical_supply_pass: String,
    no_duplicate_logs_pass: String,
    transfer_decode_pass: String,
    supply_invariant_pass: String,
    provenance_stamped: String,
}

#[derive(Debug, Deserialize)]
struct QaChainFile {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    resolved_to_block: Option<u64>,
    gates: QaGatesSnapshot,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct SupplyAuditRow {
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
pub(crate) struct CrossChainSummary {
    schema_version: u32,
    asset: String,
    /// `transfer-audit` run directory name under `out/<asset>/runs/`.
    source_run_id: String,
    generated_at: String,
    transfer_audit_qa_generated_at: String,
    transfer_audit_provenance_generated_at: String,
    window_from_block: u64,
    window_to_block_requested: Option<String>,
    chain_count: usize,
    /// Sum of per-chain `onchain_delta` strings parsed as **signed** `I256` (same encoding as transfer-audit); `None` if any chain lacks delta or on overflow.
    sum_onchain_delta_raw: Option<String>,
    chains: Vec<CrossChainChainSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct CrossChainChainSummary {
    chain: String,
    chain_id: u64,
    contract_address: String,
    from_block: u64,
    resolved_to_block: Option<u64>,
    /// QA gate strings from `qa_report.json` for this chain.
    gates: QaGatesSnapshot,
    transfer_event_count: usize,
    active_senders: usize,
    active_recipients: usize,
    mint_count: usize,
    burn_count: usize,
    plain_transfer_count: usize,
    /// `totalSupply` at end of window, **decimal-formatted** (same as `supply_audit.csv` / transfer-audit); not raw base units.
    total_supply_at_end_decimal: Option<String>,
    /// Signed on-chain supply delta over the window (`I256::to_string()` from transfer-audit).
    onchain_delta_raw: Option<String>,
}

fn norm_asset(s: &str) -> String {
    s.trim().to_uppercase()
}

fn addr_eq(a: &str, b: &str) -> bool {
    let strip = |x: &str| x.trim_start_matches("0x").to_lowercase();
    strip(a) == strip(b)
}

/// `qa_report.json` top-level provenance must align with `supply_audit.csv` unless `per_chain_spans` is set.
fn validate_provenance_window(
    qa: &QaReportFile,
    supply: &HashMap<String, SupplyAuditRow>,
) -> Result<()> {
    if qa.provenance.per_chain_spans {
        return Ok(());
    }

    let prov_from = qa.provenance.from_block;

    for q in &qa.chains {
        if q.from_block != prov_from {
            anyhow::bail!(
                "QA chain {:?} from_block {} does not match qa.provenance.from_block {}",
                q.chain,
                q.from_block,
                prov_from
            );
        }
    }

    let prov_to = qa.provenance.to_block_requested.as_deref().ok_or_else(|| {
        anyhow::anyhow!("qa_report.json provenance.to_block_requested is missing")
    })?;

    let mut supply_reqs: HashSet<String> = HashSet::new();
    for s in supply.values() {
        if s.from_block != prov_from {
            anyhow::bail!(
                "supply_audit.csv chain {:?} from_block {} does not match qa.provenance.from_block {}",
                s.chain,
                s.from_block,
                prov_from
            );
        }
        supply_reqs.insert(s.to_block_requested.trim().to_string());
    }

    if supply_reqs.len() > 1 {
        anyhow::bail!(
            "supply_audit.csv to_block_requested differs across chains: {:?}; bundle is inconsistent",
            supply_reqs
        );
    }

    let supply_req = supply_reqs
        .into_iter()
        .next()
        .expect("supply non-empty when chains validated");

    if !to_block_requested_consistent(prov_to, &supply_req) {
        anyhow::bail!(
            "window mismatch: qa.provenance.to_block_requested {:?} vs supply_audit to_block_requested {:?}",
            prov_to,
            supply_req
        );
    }

    Ok(())
}

/// True when provenance end request and per-run supply CSV string describe the same window bound.
fn to_block_requested_consistent(provenance: &str, supply: &str) -> bool {
    let p = provenance.trim();
    let s = supply.trim();
    if p.eq_ignore_ascii_case("latest") && s.eq_ignore_ascii_case("latest") {
        return true;
    }
    match (p.parse::<u64>(), s.parse::<u64>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => p == s,
    }
}

pub(crate) fn load_supply_csv(path: &Path) -> Result<HashMap<String, SupplyAuditRow>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut m = HashMap::new();
    for res in rdr.deserialize::<SupplyAuditRow>() {
        let row = res.with_context(|| format!("parse row in {}", path.display()))?;
        let c = row.chain.clone();
        if m.insert(c.clone(), row).is_some() {
            anyhow::bail!("duplicate chain {c:?} in {}", path.display());
        }
    }
    Ok(m)
}

fn validate_and_build(
    asset_cli: &str,
    run_id_cli: &str,
    qa: &QaReportFile,
    supply: &HashMap<String, SupplyAuditRow>,
) -> Result<(Vec<CrossChainChainSummary>, Option<String>, Vec<String>)> {
    let want = norm_asset(asset_cli);
    if norm_asset(&qa.asset) != want {
        anyhow::bail!(
            "qa_report.json asset {:?} does not match --asset {:?}",
            qa.asset,
            asset_cli
        );
    }
    if let Some(ref qid) = qa.run_id {
        if qid != run_id_cli {
            anyhow::bail!(
                "qa_report.json run_id {:?} does not match --run-id {:?}; refusing to summarize the wrong bundle",
                qid,
                run_id_cli
            );
        }
    }

    let mut qa_seen: HashSet<&str> = HashSet::new();
    for c in &qa.chains {
        if !qa_seen.insert(c.chain.as_str()) {
            anyhow::bail!("duplicate chain {:?} in qa_report.json", c.chain);
        }
    }

    let set_qa: HashSet<_> = qa.chains.iter().map(|c| c.chain.as_str()).collect();
    let set_su: HashSet<_> = supply.keys().map(|s| s.as_str()).collect();
    if set_qa != set_su {
        anyhow::bail!(
            "chain set mismatch between qa_report.json and supply_audit.csv: qa {:?} vs supply {:?}",
            set_qa,
            set_su
        );
    }

    if qa.chains.len() < 2 {
        anyhow::bail!(
            "cross-chain summary needs at least 2 chains; got {}. Re-run transfer-audit with e.g. --chains ethereum,base,arbitrum",
            qa.chains.len()
        );
    }

    validate_provenance_window(qa, supply)?;

    let mut warnings = vec![
        "Per-chain totalSupply(end) sums are not circulating supply across chains (bridged inventory double-counts). \
Use this table for same-window, per-deployment accounting only."
            .to_string(),
    ];
    if qa.provenance.per_chain_spans {
        warnings.push(
            "This run used per-chain native block spans (`--window`). Rows are comparable under one schema; \
block numbers and window lengths are not assumed equal across chains. The signed delta sum is an arithmetic aggregate of per-chain windows."
                .to_string(),
        );
    }

    let mut chain_names: Vec<String> = qa.chains.iter().map(|c| c.chain.clone()).collect();
    chain_names.sort();

    let mut sum_delta: Option<I256> = Some(I256::ZERO);
    let mut summaries = Vec::new();

    for chain in chain_names {
        let q = qa
            .chains
            .iter()
            .find(|c| c.chain == chain)
            .expect("chain in sorted list");
        let s = supply.get(&chain).expect("aligned set");

        if q.chain_id != s.chain_id {
            anyhow::bail!(
                "chain_id mismatch for {chain:?}: qa {} supply {}",
                q.chain_id,
                s.chain_id
            );
        }
        if !addr_eq(&q.contract_address, &s.contract_address) {
            anyhow::bail!(
                "contract_address mismatch for {chain:?}: qa {} supply {}",
                q.contract_address,
                s.contract_address
            );
        }
        if q.from_block != s.from_block {
            anyhow::bail!(
                "from_block mismatch for {chain:?}: qa {} supply {}",
                q.from_block,
                s.from_block
            );
        }
        if q.resolved_to_block != s.resolved_to_block {
            anyhow::bail!(
                "resolved_to_block mismatch for {chain:?}: qa {:?} supply {:?}",
                q.resolved_to_block,
                s.resolved_to_block
            );
        }

        if let Some(ref raw) = s.onchain_delta_raw {
            let v: I256 = raw.parse().with_context(|| {
                format!("parse onchain_delta as I256 for chain {chain}: {raw:?}")
            })?;
            sum_delta =
                match sum_delta {
                    Some(acc) => Some(acc.checked_add(v).ok_or_else(|| {
                        anyhow::anyhow!("integer overflow summing onchain deltas")
                    })?),
                    None => None,
                };
        } else {
            sum_delta = None;
        }

        summaries.push(CrossChainChainSummary {
            chain: chain.clone(),
            chain_id: s.chain_id,
            contract_address: s.contract_address.clone(),
            from_block: s.from_block,
            resolved_to_block: s.resolved_to_block,
            gates: q.gates.clone(),
            transfer_event_count: s.transfer_event_count,
            active_senders: s.active_senders,
            active_recipients: s.active_recipients,
            mint_count: s.mint_count,
            burn_count: s.burn_count,
            plain_transfer_count: s.plain_transfer_count,
            total_supply_at_end_decimal: s.total_supply_at_end.clone(),
            onchain_delta_raw: s.onchain_delta_raw.clone(),
        });
    }

    let sum_str = sum_delta.map(|i| i.to_string());
    Ok((summaries, sum_str, warnings))
}

pub(crate) fn write_markdown(path: &Path, summary: &CrossChainSummary) -> Result<()> {
    let mut md = String::new();
    md.push_str(&format!(
        "# Cross-chain window summary — {}\n\n",
        summary.asset
    ));
    md.push_str(&format!("**Generated:** {}\n\n", summary.generated_at));
    md.push_str(&format!(
        "**Source transfer-audit run:** `{}` (read `qa_report.json` + `supply_audit.csv` from this run only)\n\n",
        summary.source_run_id
    ));
    md.push_str(&format!(
        "**Transfer-audit QA:** {} (provenance block generated_at: {})\n\n",
        summary.transfer_audit_qa_generated_at, summary.transfer_audit_provenance_generated_at
    ));
    if summary
        .window_to_block_requested
        .as_deref()
        .map(|s| s == "per_chain")
        .unwrap_or(false)
    {
        md.push_str(&format!(
            "**Window:** per-chain native block spans (min from_block in bundle: `{}`). \
See each row for `from_block` → resolved end; heights are not comparable across chains.\n\n",
            summary.window_from_block
        ));
    } else {
        md.push_str(&format!(
            "**Window:** from_block={}, to_block_requested={:?}\n\n",
            summary.window_from_block, summary.window_to_block_requested
        ));
    }

    for w in &summary.warnings {
        md.push_str(&format!("> {w}\n\n"));
    }

    if let Some(ref s) = summary.sum_onchain_delta_raw {
        md.push_str(&format!(
            "**Sum of per-chain on-chain supply deltas (signed I256, same string form as transfer-audit):** `{s}`\n\n"
        ));
    } else {
        md.push_str("**Sum of per-chain on-chain supply deltas:** unavailable (one or more chains missing `onchain_delta_raw`, or overflow when summing)\n\n");
    }

    md.push_str("| Chain | Resolved end | Transfers | Senders | Recipients | Mints | Burns | totalSupply@end (decimal) | On-chain Δ (signed) | metadata | hist_supply | supply_inv | decode | no_dup | prov_stamp |\n");
    md.push_str("|-------|---------------:|----------:|--------:|-----------:|------:|------:|--------------------------:|--------------------:|----------|------------|------------|--------|--------|------------|\n");
    for r in &summary.chains {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.chain,
            r.resolved_to_block
                .map(|b| b.to_string())
                .unwrap_or_else(|| "—".into()),
            r.transfer_event_count,
            r.active_senders,
            r.active_recipients,
            r.mint_count,
            r.burn_count,
            r.total_supply_at_end_decimal.as_deref().unwrap_or("—"),
            r.onchain_delta_raw.as_deref().unwrap_or("—"),
            r.gates.metadata_call_pass,
            r.gates.historical_supply_pass,
            r.gates.supply_invariant_pass,
            r.gates.transfer_decode_pass,
            r.gates.no_duplicate_logs_pass,
            r.gates.provenance_stamped,
        ));
    }

    md.push_str("\n---\n\n_v0.1: chain-level on-chain accounting comparison in the declared window(s). Not bridge netting, reserve attestation, peg or purchasing-power analysis, or holder census._\n");
    std::fs::write(path, md)?;
    Ok(())
}

/// Build `cross_chain_summary.json` and `cross_chain_summary.md` from one `transfer-audit` run directory
/// `out/<asset>/runs/<run_id>/`.
pub fn run(asset: &str, run_id: &str) -> Result<()> {
    validate_run_id(run_id)?;
    let out_dir = ensure_run_out_dir(asset, run_id)?;
    let qa_path = out_dir.join("qa_report.json");
    let supply_path = out_dir.join("supply_audit.csv");

    if !qa_path.exists() {
        anyhow::bail!(
            "qa_report.json not found at {}; run `transfer-audit --run-id {}` (or omit for a timestamp id) first",
            qa_path.display(),
            run_id
        );
    }
    if !supply_path.exists() {
        anyhow::bail!(
            "supply_audit.csv not found at {}; run `transfer-audit` for this run_id first",
            supply_path.display()
        );
    }

    let qa: QaReportFile = serde_json::from_str(
        &std::fs::read_to_string(&qa_path).with_context(|| qa_path.display().to_string())?,
    )
    .with_context(|| format!("parse {}", qa_path.display()))?;

    let supply = load_supply_csv(&supply_path).context("read supply_audit.csv")?;

    let (chains, sum_onchain_delta_raw, warnings) =
        validate_and_build(asset, run_id, &qa, &supply)?;

    let generated_at = Utc::now().to_rfc3339();
    let summary = CrossChainSummary {
        schema_version: 2,
        asset: norm_asset(asset),
        source_run_id: run_id.to_string(),
        generated_at: generated_at.clone(),
        transfer_audit_qa_generated_at: qa.generated_at.clone(),
        transfer_audit_provenance_generated_at: qa.provenance.generated_at.clone(),
        window_from_block: qa.provenance.from_block,
        window_to_block_requested: qa.provenance.to_block_requested.clone(),
        chain_count: chains.len(),
        sum_onchain_delta_raw,
        chains,
        warnings,
    };

    let json_path = out_dir.join("cross_chain_summary.json");
    let md_path = out_dir.join("cross_chain_summary.md");
    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&summary).context("serialize cross_chain_summary.json")?,
    )?;
    write_markdown(&md_path, &summary)?;

    let summary_warnings = summary.warnings.clone();
    upsert_cross_chain_summary_manifest(
        &out_dir,
        &CrossChainSummaryManifestParams {
            completed_at: generated_at.clone(),
            warnings: summary_warnings,
        },
    )?;

    println!(
        "\n=== Cross-chain summary ({}) — run `{}` — {} chains ===",
        norm_asset(asset),
        run_id,
        summary.chain_count
    );
    if let Some(ref s) = summary.sum_onchain_delta_raw {
        println!("Sum on-chain supply deltas (signed I256 string): {s}");
    }
    println!("\nWritten:");
    println!("  {}", json_path.display());
    println!("  {}", md_path.display());
    println!("  {}", out_dir.join("artifact_manifest.json").display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn to_block_req_latest_case_insensitive() {
        assert!(to_block_requested_consistent("latest", "LATEST"));
    }

    #[test]
    fn to_block_req_numeric_equal() {
        assert!(to_block_requested_consistent(" 12345 ", "12345"));
    }

    #[test]
    fn to_block_req_mismatch() {
        assert!(!to_block_requested_consistent("100", "101"));
    }

    #[test]
    fn addr_eq_ignores_case_and_prefix() {
        assert!(addr_eq("0xAbC", "abc"));
        assert!(!addr_eq("0xabc", "0xabd"));
    }

    #[test]
    fn norm_asset_uppercases() {
        assert_eq!(norm_asset(" usdc "), "USDC");
    }

    fn gate_pass() -> QaGatesSnapshot {
        QaGatesSnapshot {
            metadata_call_pass: "PASS".into(),
            historical_supply_pass: "PASS".into(),
            no_duplicate_logs_pass: "PASS".into(),
            transfer_decode_pass: "PASS".into(),
            supply_invariant_pass: "PASS".into(),
            provenance_stamped: "PASS".into(),
        }
    }

    fn sample_supply_row(chain: &str, from: u64, to: u64, delta: &str) -> SupplyAuditRow {
        SupplyAuditRow {
            chain: chain.into(),
            chain_id: if chain == "ethereum" { 1 } else { 8453 },
            contract_address: format!("0x{chain}"),
            from_block: from,
            resolved_to_block: Some(to),
            to_block_requested: to.to_string(),
            chunk_size: 500,
            transfer_event_count: 10,
            active_senders: 1,
            active_recipients: 1,
            mint_count: 1,
            burn_count: 0,
            plain_transfer_count: 9,
            sum_mints_raw: "0".into(),
            sum_burns_raw: "0".into(),
            net_mint_raw: Some(delta.into()),
            total_supply_at_start_minus_1: None,
            total_supply_at_start_minus_1_provenance: "on-chain".into(),
            total_supply_at_end: None,
            onchain_delta_raw: Some(delta.into()),
            discrepancy_raw: Some("0".into()),
            metadata_call_pass: true,
            historical_supply_pass: true,
            no_duplicate_logs_pass: Some(true),
            transfer_decode_pass: Some(true),
            supply_invariant_pass: Some(true),
            duplicate_count: 0,
            full_decode_error_count: 0,
        }
    }

    fn sample_qa_chain(chain: &str, from: u64, to: u64) -> QaChainFile {
        QaChainFile {
            chain: chain.into(),
            chain_id: if chain == "ethereum" { 1 } else { 8453 },
            contract_address: format!("0x{chain}"),
            from_block: from,
            resolved_to_block: Some(to),
            gates: gate_pass(),
        }
    }

    #[test]
    fn validate_and_build_sums_deltas() {
        let qa = QaReportFile {
            asset: "USDC".into(),
            generated_at: "t".into(),
            run_id: Some("run1".into()),
            provenance: QaProvenanceBlock {
                from_block: 100,
                to_block_requested: Some("200".into()),
                generated_at: "t".into(),
                per_chain_spans: false,
            },
            chains: vec![
                sample_qa_chain("ethereum", 100, 200),
                sample_qa_chain("base", 100, 200),
            ],
        };
        let mut supply = HashMap::new();
        supply.insert(
            "ethereum".into(),
            sample_supply_row("ethereum", 100, 200, "1000"),
        );
        supply.insert("base".into(), sample_supply_row("base", 100, 200, "-500"));

        let (_, sum, warnings) = validate_and_build("usdc", "run1", &qa, &supply).unwrap();
        assert_eq!(sum.as_deref(), Some("500"));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn validate_and_build_rejects_single_chain() {
        let qa = QaReportFile {
            asset: "USDC".into(),
            generated_at: "t".into(),
            run_id: None,
            provenance: QaProvenanceBlock {
                from_block: 1,
                to_block_requested: Some("2".into()),
                generated_at: "t".into(),
                per_chain_spans: false,
            },
            chains: vec![sample_qa_chain("ethereum", 1, 2)],
        };
        let mut supply = HashMap::new();
        supply.insert("ethereum".into(), sample_supply_row("ethereum", 1, 2, "0"));
        assert!(validate_and_build("USDC", "x", &qa, &supply).is_err());
    }

    #[test]
    fn load_supply_csv_from_benchmark_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/benchmarks/usdc_7d_20260501_20260508/supply_audit.csv");
        let m = load_supply_csv(&path).unwrap();
        assert_eq!(m.len(), 3);
        assert!(m.contains_key("ethereum"));
    }

    #[test]
    fn validate_provenance_window_rejects_mismatch() {
        let qa = QaReportFile {
            asset: "USDC".into(),
            generated_at: "t".into(),
            run_id: None,
            provenance: QaProvenanceBlock {
                from_block: 100,
                to_block_requested: Some("200".into()),
                generated_at: "t".into(),
                per_chain_spans: false,
            },
            chains: vec![
                sample_qa_chain("ethereum", 100, 200),
                sample_qa_chain("base", 100, 200),
            ],
        };
        let mut supply = HashMap::new();
        supply.insert(
            "ethereum".into(),
            sample_supply_row("ethereum", 99, 200, "0"),
        );
        supply.insert("base".into(), sample_supply_row("base", 100, 200, "0"));
        assert!(validate_provenance_window(&qa, &supply).is_err());
    }

    #[test]
    fn validate_provenance_skips_when_per_chain_spans() {
        let qa = QaReportFile {
            asset: "USDC".into(),
            generated_at: "t".into(),
            run_id: None,
            provenance: QaProvenanceBlock {
                from_block: 1,
                to_block_requested: Some("per_chain".into()),
                generated_at: "t".into(),
                per_chain_spans: true,
            },
            chains: vec![
                sample_qa_chain("ethereum", 10, 20),
                sample_qa_chain("base", 30, 40),
            ],
        };
        let mut supply = HashMap::new();
        supply.insert(
            "ethereum".into(),
            sample_supply_row("ethereum", 10, 20, "1"),
        );
        supply.insert("base".into(), sample_supply_row("base", 30, 40, "2"));
        validate_provenance_window(&qa, &supply).unwrap();
    }

    fn remove_run_path_if_present(path: &std::path::Path) {
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(path);
        } else if path.is_file() {
            let _ = std::fs::remove_file(path);
        }
    }

    fn write_minimal_transfer_audit_manifest(out: &std::path::Path, run_id: &str) {
        use crate::artifact::{
            build_transfer_audit_manifest, write_manifest, TransferAuditManifestParams,
        };
        std::fs::write(out.join("decoded_transfers.csv"), "chain\n").unwrap();
        std::fs::write(
            out.join("provenance.json"),
            r#"{"schema":"transfer-audit-provenance-v1"}"#,
        )
        .unwrap();
        std::fs::write(out.join("summary.md"), "# summary").unwrap();
        std::fs::write(out.join("supply_audit.md"), "# supply").unwrap();
        let params = TransferAuditManifestParams {
            asset: "USDC".into(),
            run_id: run_id.to_string(),
            generated_at: "2026-05-15T08:00:00+00:00".into(),
            per_chain_spans: false,
            provenance_from_block: 24996368,
            provenance_to_block_requested: Some("25046605".into()),
            chains: vec![],
            warnings: vec![],
        };
        let manifest = build_transfer_audit_manifest(out, &params).unwrap();
        write_manifest(out, &manifest).unwrap();
    }

    fn seed_transfer_audit_run(out: &std::path::Path, run_id: &str) {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/benchmarks/usdc_7d_20260501_20260508");
        std::fs::copy(
            fixture.join("supply_audit.csv"),
            out.join("supply_audit.csv"),
        )
        .unwrap();
        let mut qa: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(fixture.join("qa_report.json")).unwrap())
                .unwrap();
        qa["run_id"] = serde_json::Value::String(run_id.to_string());
        std::fs::write(
            out.join("qa_report.json"),
            serde_json::to_string_pretty(&qa).unwrap(),
        )
        .unwrap();
        write_minimal_transfer_audit_manifest(out, run_id);
    }

    #[test]
    fn run_upserts_artifact_manifest() {
        let run_id = format!(
            "cc_run_manifest_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let run_path = std::path::Path::new("out")
            .join("usdc")
            .join("runs")
            .join(&run_id);
        remove_run_path_if_present(&run_path);
        let out = crate::report::ensure_run_out_dir("USDC", &run_id).unwrap();
        seed_transfer_audit_run(&out, &run_id);
        run("USDC", &run_id).unwrap();
        let m = crate::artifact::load_artifact_manifest(&out).unwrap();
        assert_eq!(m.command, "transfer-audit");
        assert_eq!(
            m.artifacts
                .iter()
                .filter(|a| a.path.starts_with("cross_chain_summary"))
                .count(),
            2
        );
        let cc_steps: Vec<_> = m
            .workflow_steps
            .iter()
            .filter(|s| s.command == "cross-chain-summary")
            .collect();
        assert_eq!(cc_steps.len(), 1);
        run("USDC", &run_id).unwrap();
        let m2 = crate::artifact::load_artifact_manifest(&out).unwrap();
        assert_eq!(
            m2.artifacts
                .iter()
                .filter(|a| a.path.starts_with("cross_chain_summary"))
                .count(),
            2
        );
        let _ = std::fs::remove_dir_all(&out);
    }

    #[cfg(feature = "api")]
    #[tokio::test]
    async fn api_lists_cross_chain_artifacts_after_run() {
        use std::collections::HashSet;

        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        use crate::api::{router, ArtifactStore};
        use crate::report::ensure_run_out_dir;

        let run_id = format!(
            "cc_api_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let run_path = std::path::Path::new("out")
            .join("usdc")
            .join("runs")
            .join(&run_id);
        remove_run_path_if_present(&run_path);
        let out = ensure_run_out_dir("USDC", &run_id).unwrap();
        seed_transfer_audit_run(&out, &run_id);
        run("USDC", &run_id).unwrap();

        let store = ArtifactStore::open("out").unwrap();
        let listed: HashSet<String> = store
            .list_runs()
            .unwrap()
            .into_iter()
            .map(|r| r.run_id)
            .collect();
        assert!(listed.contains(&run_id));

        let app = router(store);
        let uri = format!("/api/runs/{run_id}/artifacts?asset=USDC");
        let response = app
            .oneshot(Request::get(&uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let paths: Vec<String> = v["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["path"].as_str().unwrap().to_string())
            .collect();
        assert!(paths
            .iter()
            .any(|p| p.ends_with("cross_chain_summary.json")));
        assert!(paths.iter().any(|p| p.ends_with("cross_chain_summary.md")));
        let _ = std::fs::remove_dir_all(&out);
    }
}
