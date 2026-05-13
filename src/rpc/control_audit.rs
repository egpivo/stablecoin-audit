use alloy::primitives::Address;
use alloy::providers::Provider;
use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::path::Path;
use std::str::FromStr;

use crate::config::load_single_token_config;
use crate::control_events::{fetch_control_events, ControlEventRecord, KNOWN_SIGNATURES};
use crate::report::ensure_out_dir;
use crate::rpc::build_provider;

#[derive(Serialize)]
struct ChainControlQa {
    chain: String,
    chain_id: u64,
    contract_address: String,
    rpc_provider_alias: String,
    from_block: u64,
    to_block: Option<u64>,
    control_event_count: usize,
    control_event_query_status: String,
    control_decode_error_count: usize,
    gates: ControlQaGates,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct ControlQaGates {
    control_event_query_pass: String,
    control_decode_pass: String,
    provenance_stamped_pass: String,
    no_simulated_data_pass: String,
}

#[derive(Serialize)]
struct ControlQaReport {
    asset: String,
    generated_at: String,
    chains: Vec<ChainControlQa>,
}

#[derive(Serialize)]
struct ControlProvenanceChain {
    asset: String,
    chain: String,
    chain_id: u64,
    contract_address: String,
    rpc_provider_alias: String,
    from_block: u64,
    to_block: Option<u64>,
    fetched_at: String,
    generated_at: String,
    topics: Vec<String>,
    data_source: String,
    simulated_data: bool,
}

#[derive(Serialize)]
struct ControlProvenanceReport {
    asset: String,
    generated_at: String,
    data_source: String,
    simulated_data: bool,
    chains: Vec<ControlProvenanceChain>,
}

#[derive(Clone, Copy)]
struct RowBase<'a> {
    chain: &'a str,
    chain_id: u64,
    contract_address: &'a str,
    rpc_provider_alias: &'a str,
    from_block: u64,
    to_block: Option<u64>,
}

struct RowMetrics {
    control_event_count: usize,
    control_event_query_status: String,
    control_decode_error_count: usize,
}

pub async fn run(asset: &str, chains: &[String], from_block: u64, to_block_raw: &str) -> Result<()> {
    let generated_at = Utc::now().to_rfc3339();
    let out_dir = ensure_out_dir(asset)?;
    let mut qa_rows = Vec::new();
    let mut provenance_rows = Vec::new();
    let mut any_hard_error = false;

    let requested_to = if to_block_raw.eq_ignore_ascii_case("latest") {
        None
    } else {
        Some(
            to_block_raw.parse::<u64>().map_err(|_| {
                anyhow::anyhow!("--to-block must be a block number or latest, got {:?}", to_block_raw)
            })?,
        )
    };

    for chain in chains {
        let mut errors = Vec::<String>::new();
        let mut control_event_count = 0usize;
        let mut decode_error_count = 0usize;
        let mut query_status = "unavailable".to_string();

        let config = match load_single_token_config(asset, chain) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("config: {e:#}"));
                qa_rows.push(failed_row(chain, from_block, requested_to, errors.clone()));
                any_hard_error = true;
                continue;
            }
        };
        let chain_id = config.chain_id;
        let contract_address = config.contract_address.clone();
        let rpc_provider_alias = config.rpc_url_env.clone();
        let mut base = RowBase {
            chain,
            chain_id,
            contract_address: &contract_address,
            rpc_provider_alias: &rpc_provider_alias,
            from_block,
            to_block: None,
        };

        let rpc_url = match config.rpc_url() {
            Ok(u) => u,
            Err(e) => {
                errors.push(format!("{e:#}"));
                qa_rows.push(build_row(
                    base,
                    RowMetrics {
                        control_event_count,
                        control_event_query_status: query_status.clone(),
                        control_decode_error_count: decode_error_count,
                    },
                    ControlQaGates {
                        control_event_query_pass: "FAIL".into(),
                        control_decode_pass: "FAIL".into(),
                        provenance_stamped_pass: gate(!generated_at.is_empty() && !rpc_provider_alias.is_empty()),
                        no_simulated_data_pass: "PASS".into(),
                    },
                    errors.clone(),
                ));
                any_hard_error = true;
                continue;
            }
        };

        let provider = match build_provider(&rpc_url) {
            Ok(p) => p,
            Err(e) => {
                errors.push(format!("provider: {e:#}"));
                qa_rows.push(build_row(
                    base,
                    RowMetrics {
                        control_event_count,
                        control_event_query_status: query_status.clone(),
                        control_decode_error_count: decode_error_count,
                    },
                    ControlQaGates {
                        control_event_query_pass: "FAIL".into(),
                        control_decode_pass: "FAIL".into(),
                        provenance_stamped_pass: gate(!generated_at.is_empty() && !rpc_provider_alias.is_empty()),
                        no_simulated_data_pass: "PASS".into(),
                    },
                    errors.clone(),
                ));
                any_hard_error = true;
                continue;
            }
        };

        match provider.get_chain_id().await {
            Ok(id) if id != config.chain_id => {
                errors.push(format!(
                    "chain_id mismatch: rpc={id}, config={}",
                    config.chain_id
                ));
                qa_rows.push(build_row(
                    base,
                    RowMetrics {
                        control_event_count,
                        control_event_query_status: query_status.clone(),
                        control_decode_error_count: decode_error_count,
                    },
                    ControlQaGates {
                        control_event_query_pass: "FAIL".into(),
                        control_decode_pass: "FAIL".into(),
                        provenance_stamped_pass: gate(!generated_at.is_empty() && !rpc_provider_alias.is_empty()),
                        no_simulated_data_pass: "PASS".into(),
                    },
                    errors.clone(),
                ));
                any_hard_error = true;
                continue;
            }
            Err(e) => {
                errors.push(format!("eth_chainId failed: {e:#}"));
                qa_rows.push(build_row(
                    base,
                    RowMetrics {
                        control_event_count,
                        control_event_query_status: query_status.clone(),
                        control_decode_error_count: decode_error_count,
                    },
                    ControlQaGates {
                        control_event_query_pass: "FAIL".into(),
                        control_decode_pass: "FAIL".into(),
                        provenance_stamped_pass: gate(!generated_at.is_empty() && !rpc_provider_alias.is_empty()),
                        no_simulated_data_pass: "PASS".into(),
                    },
                    errors.clone(),
                ));
                any_hard_error = true;
                continue;
            }
            Ok(_) => {}
        }

        let resolved_to = match requested_to {
            Some(v) => Some(v),
            None => match provider.get_block_number().await {
                Ok(v) => Some(v),
                Err(e) => {
                    errors.push(format!("get_block_number failed: {e:#}"));
                    None
                }
            },
        };

        let Some(to_block) = resolved_to else {
            qa_rows.push(build_row(
                base,
                RowMetrics {
                    control_event_count,
                    control_event_query_status: query_status.clone(),
                    control_decode_error_count: decode_error_count,
                },
                ControlQaGates {
                    control_event_query_pass: "FAIL".into(),
                    control_decode_pass: "FAIL".into(),
                    provenance_stamped_pass: gate(!generated_at.is_empty() && !rpc_provider_alias.is_empty()),
                    no_simulated_data_pass: "PASS".into(),
                },
                errors.clone(),
            ));
            any_hard_error = true;
            continue;
        };
        base.to_block = Some(to_block);

        let addr = match Address::from_str(&contract_address) {
            Ok(a) => a,
            Err(e) => {
                errors.push(format!("contract address parse failed: {e:#}"));
                qa_rows.push(build_row(
                    base,
                    RowMetrics {
                        control_event_count,
                        control_event_query_status: query_status.clone(),
                        control_decode_error_count: decode_error_count,
                    },
                    ControlQaGates {
                        control_event_query_pass: "FAIL".into(),
                        control_decode_pass: "FAIL".into(),
                        provenance_stamped_pass: gate(!generated_at.is_empty() && !rpc_provider_alias.is_empty()),
                        no_simulated_data_pass: "PASS".into(),
                    },
                    errors.clone(),
                ));
                any_hard_error = true;
                continue;
            }
        };

        let (events, status) = fetch_control_events(&provider, addr, from_block, to_block, chain).await;
        let fetched_at = Utc::now().to_rfc3339();
        query_status = status;
        control_event_count = events.len();
        decode_error_count = events
            .iter()
            .filter(|e| e.decode_status == "decode_error")
            .count();

        let csv_path = out_dir.join(format!("control_events_{chain}.csv"));
        write_control_events_csv(&csv_path, &events)?;

        let query_pass = query_status == "pass" || query_status == "partial";
        let decode_pass = decode_error_count == 0;
        let provenance_pass = !rpc_provider_alias.is_empty()
            && !generated_at.is_empty()
            && !fetched_at.is_empty()
            && to_block >= from_block;

        qa_rows.push(build_row(
            base,
            RowMetrics {
                control_event_count,
                control_event_query_status: query_status.clone(),
                control_decode_error_count: decode_error_count,
            },
            ControlQaGates {
                control_event_query_pass: gate(query_pass),
                control_decode_pass: gate(decode_pass),
                provenance_stamped_pass: gate(provenance_pass),
                no_simulated_data_pass: "PASS".into(),
            },
            errors.clone(),
        ));

        provenance_rows.push(ControlProvenanceChain {
            asset: asset.to_uppercase(),
            chain: chain.to_string(),
            chain_id,
            contract_address: contract_address.clone(),
            rpc_provider_alias: rpc_provider_alias.clone(),
            from_block,
            to_block: Some(to_block),
            fetched_at,
            generated_at: generated_at.clone(),
            topics: KNOWN_SIGNATURES
                .iter()
                .map(|t| format!("{t:#x}"))
                .collect(),
            data_source: "onchain_rpc".into(),
            simulated_data: false,
        });
    }

    let qa_report = ControlQaReport {
        asset: asset.to_uppercase(),
        generated_at: generated_at.clone(),
        chains: qa_rows,
    };
    std::fs::write(
        out_dir.join("control_qa_report.json"),
        serde_json::to_string_pretty(&qa_report)?,
    )?;

    let prov = ControlProvenanceReport {
        asset: asset.to_uppercase(),
        generated_at: generated_at.clone(),
        data_source: "onchain_rpc".into(),
        simulated_data: false,
        chains: provenance_rows,
    };
    std::fs::write(
        out_dir.join("control_provenance.json"),
        serde_json::to_string_pretty(&prov)?,
    )?;

    write_control_risk_flags_md(&out_dir, asset, &generated_at, &qa_report)?;

    write_control_summary_md(&out_dir, asset, &generated_at, &qa_report)?;
    println!("\nControl-audit outputs under {}:", out_dir.display());
    println!(
        "  control_events_<chain>.csv, control_qa_report.json, control_provenance.json,\n  control_surface_summary.md, risk_flags.md"
    );

    if any_hard_error {
        anyhow::bail!("one or more chains had hard errors; partial control-audit outputs were written");
    }

    Ok(())
}

fn failed_row(chain: &str, from_block: u64, to_block: Option<u64>, errors: Vec<String>) -> ChainControlQa {
    let base = RowBase {
        chain,
        chain_id: 0,
        contract_address: "unknown",
        rpc_provider_alias: "unknown",
        from_block,
        to_block,
    };
    let metrics = RowMetrics {
        control_event_count: 0,
        control_event_query_status: "unavailable".into(),
        control_decode_error_count: 0,
    };
    let gates = ControlQaGates {
        control_event_query_pass: "FAIL".into(),
        control_decode_pass: "FAIL".into(),
        provenance_stamped_pass: "FAIL".into(),
        no_simulated_data_pass: "PASS".into(),
    };
    build_row(base, metrics, gates, errors)
}

fn build_row(base: RowBase<'_>, metrics: RowMetrics, gates: ControlQaGates, errors: Vec<String>) -> ChainControlQa {
    ChainControlQa {
        chain: base.chain.to_string(),
        chain_id: base.chain_id,
        contract_address: base.contract_address.to_string(),
        rpc_provider_alias: base.rpc_provider_alias.to_string(),
        from_block: base.from_block,
        to_block: base.to_block,
        control_event_count: metrics.control_event_count,
        control_event_query_status: metrics.control_event_query_status,
        control_decode_error_count: metrics.control_decode_error_count,
        gates,
        errors,
    }
}

fn gate(pass: bool) -> String {
    if pass { "PASS".into() } else { "FAIL".into() }
}

fn write_control_events_csv(path: &std::path::Path, events: &[ControlEventRecord]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    for ev in events {
        wtr.serialize(ev)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_control_risk_flags_md(
    out_dir: &Path,
    asset: &str,
    report_generated_at: &str,
    report: &ControlQaReport,
) -> Result<()> {
    let mut md = String::new();
    md.push_str("# Risk Flags — Control surface\n\n");
    md.push_str(&format!(
        "## {} — Generated {}\n\n",
        asset.to_uppercase(),
        report_generated_at
    ));

    for row in &report.chains {
        md.push_str(&format!(
            "### {} (blocks {} → {})\n",
            row.chain,
            row.from_block,
            row.to_block
                .map(|b| b.to_string())
                .unwrap_or_else(|| "unavailable".into())
        ));

        md.push_str(&format!(
            "- QA gates: control_query={} decode={} provenance_stamped={} no_simulated_data={}\n",
            row.gates.control_event_query_pass,
            row.gates.control_decode_pass,
            row.gates.provenance_stamped_pass,
            row.gates.no_simulated_data_pass,
        ));
        md.push_str(&format!(
            "- Control events in window: {} (query status: {})\n",
            row.control_event_count, row.control_event_query_status
        ));

        let qs = row.control_event_query_status.as_str();
        if qs.starts_with("error") {
            md.push_str(&format!("- [WARN] Control event query failed: {qs}\n"));
        } else if qs == "unavailable" {
            md.push_str("- [INFO] Control event query unavailable (chain did not complete RPC setup)\n");
        } else if row.control_event_count == 0 {
            md.push_str("- [INFO] No issuer control events observed in this window\n");
        } else {
            if qs == "partial" {
                md.push_str("- [WARN] Partial decode: one or more control logs failed to decode\n");
            }
            let csv_path = out_dir.join(format!("control_events_{}.csv", row.chain));
            if csv_path.exists() {
                match load_control_events_for_risk(&csv_path) {
                    Ok(events) => {
                        for ev in events {
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
                    Err(e) => {
                        md.push_str(&format!("- [WARN] Could not read control_events CSV: {e:#}\n"));
                    }
                }
            }
        }

        for e in &row.errors {
            md.push_str(&format!("- [WARN] {e}\n"));
        }
        md.push('\n');
    }

    md.push_str("---\n\n_Issuer control-surface signals only; not wallet attribution or AML._\n");
    std::fs::write(out_dir.join("risk_flags.md"), md)?;
    Ok(())
}

fn load_control_events_for_risk(path: &Path) -> Result<Vec<ControlEventRecord>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut v = Vec::new();
    for res in rdr.deserialize::<ControlEventRecord>() {
        v.push(res?);
    }
    Ok(v)
}

fn write_control_summary_md(
    out_dir: &std::path::Path,
    asset: &str,
    generated_at: &str,
    report: &ControlQaReport,
) -> Result<()> {
    let mut md = String::new();
    md.push_str(&format!("# {} v0.2 Control-Surface Summary\n\n", asset.to_uppercase()));
    md.push_str(&format!("Generated at: {}\n\n", generated_at));
    md.push_str("Scope: issuer-side control actions only (pause/blacklist/minter/admin/upgrade).\n");
    md.push_str("Non-scope: wallet attribution, AML scoring, intent inference.\n\n");
    md.push_str("| Chain | Window | Control Events | Query | Decode | Provenance |\n");
    md.push_str("|---|---|---:|---|---|---|\n");
    for row in &report.chains {
        md.push_str(&format!(
            "| {} | {} -> {} | {} | {} | {} | {} |\n",
            row.chain,
            row.from_block,
            row.to_block.map(|v| v.to_string()).unwrap_or_else(|| "unavailable".into()),
            row.control_event_count,
            row.gates.control_event_query_pass,
            row.gates.control_decode_pass,
            row.gates.provenance_stamped_pass,
        ));
    }
    md.push_str("\n---\n");
    md.push_str("This output is audit evidence of control-surface observability, not behavioral attribution.\n");
    std::fs::write(out_dir.join("control_surface_summary.md"), md)?;
    Ok(())
}
