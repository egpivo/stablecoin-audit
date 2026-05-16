pub mod config;
pub mod decode;
pub mod fetch;
pub mod report;
pub mod rpc;

#[cfg(feature = "experimental")]
pub mod control_events;

pub use report::{default_run_id, ensure_run_out_dir, validate_run_id};

/// CLI entry (used by the binary and integration tests).
pub fn run_cli<I, S>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    cli::run(args)
}

mod cli {
    use anyhow::{Context, Result};
    use clap::{Parser, Subcommand};

    use crate::rpc::transfer_audit::parse_window_arg;

    pub(super) fn run<I, S>(args: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: Into<std::ffi::OsString> + Clone,
    {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("tokio runtime")?;
        runtime.block_on(run_async(args))
    }

    async fn run_async<I, S>(args: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: Into<std::ffi::OsString> + Clone,
    {
        dotenvy::dotenv().ok();
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::TransferAudit {
                asset,
                chains,
                from_block,
                to_block,
                chunk_size,
                windows,
                run_id,
                fresh,
            } => {
                validate_identifier(&asset, "--asset")?;
                if let Some(cs) = chunk_size {
                    if cs == 0 {
                        anyhow::bail!("--chunk-size must be at least 1");
                    }
                }

                if !windows.is_empty() {
                    if from_block.is_some() || to_block.is_some() {
                        anyhow::bail!(
                            "--window cannot be combined with --from-block or --to-block (each window carries its own bounds)"
                        );
                    }
                    if !chains.is_empty() {
                        anyhow::bail!("--window cannot be combined with --chains (chain names come from each --window)");
                    }
                    let parsed: Vec<(String, u64, u64)> = windows
                        .iter()
                        .map(|w| parse_window_arg(w))
                        .collect::<Result<Vec<_>>>()?;
                    crate::rpc::transfer_audit::run_per_chain_windows(
                        &asset, parsed, chunk_size, run_id, fresh,
                    )
                    .await?;
                } else {
                    for chain in &chains {
                        validate_identifier(chain, "--chains")?;
                    }
                    let from_block = from_block.context(
                        "--from-block is required unless you pass one or more --window chain:from:to",
                    )?;
                    let to_block = to_block.context(
                        "--to-block is required unless you pass one or more --window chain:from:to",
                    )?;
                    if from_block == 0 {
                        anyhow::bail!("--from-block 0 is not supported; use the contract deployment_block or later");
                    }
                    let tb = to_block.trim();
                    if !tb.eq_ignore_ascii_case("latest") {
                        let b: u64 = tb.parse().map_err(|_| {
                            anyhow::anyhow!(
                                "--to-block must be a block number or 'latest'; got {:?}",
                                to_block
                            )
                        })?;
                        if b < from_block {
                            anyhow::bail!("--to-block ({b}) must be >= --from-block ({from_block})");
                        }
                    }
                    let chains = if chains.is_empty() {
                        vec!["ethereum".into(), "base".into(), "arbitrum".into()]
                    } else {
                        chains
                    };
                    crate::rpc::transfer_audit::run(
                        &asset, &chains, from_block, tb, chunk_size, run_id, fresh,
                    )
                    .await?;
                }
            }
            Commands::ResolveWindow {
                asset,
                chains,
                window_from,
                window_to,
            } => {
                validate_identifier(&asset, "--asset")?;
                for c in &chains {
                    validate_identifier(c, "--chains")?;
                }
                let chains = if chains.is_empty() {
                    vec!["ethereum".into(), "base".into(), "arbitrum".into()]
                } else {
                    chains
                };
                crate::rpc::resolve_window::run(&asset, &chains, &window_from, &window_to).await?;
            }
            Commands::Metadata {
                asset,
                chains,
                from_block,
                to_block,
            } => {
                validate_identifier(&asset, "--asset")?;
                for chain in &chains {
                    validate_identifier(chain, "--chains")?;
                }
                if from_block == 0 {
                    anyhow::bail!("--from-block 0 is not supported; use the contract deployment_block or later");
                }
                if let Some(to) = to_block {
                    if to < from_block {
                        anyhow::bail!("--to-block ({to}) must be >= --from-block ({from_block})");
                    }
                }
                let chains = if chains.is_empty() {
                    vec!["ethereum".into(), "base".into(), "arbitrum".into()]
                } else {
                    chains
                };
                crate::rpc::metadata::run(&asset, &chains, from_block, to_block).await?;
            }
            #[cfg(feature = "experimental")]
            Commands::Fetch {
                asset,
                chains,
                from_block,
                to_block,
                chunk_size,
            } => {
                validate_identifier(&asset, "--asset")?;
                for chain in &chains {
                    validate_identifier(chain, "--chains")?;
                }
                if let Some(cs) = chunk_size {
                    if cs == 0 {
                        anyhow::bail!("--chunk-size must be at least 1");
                    }
                }
                if from_block == 0 {
                    anyhow::bail!("--from-block 0 is not supported");
                }
                if to_block < from_block {
                    anyhow::bail!("--to-block ({to_block}) must be >= --from-block ({from_block})");
                }
                let chains = if chains.is_empty() {
                    vec!["ethereum".into(), "base".into(), "arbitrum".into()]
                } else {
                    chains
                };
                crate::rpc::fetch_logs::run(&asset, &chains, from_block, to_block, chunk_size).await?;
            }
            #[cfg(feature = "experimental")]
            Commands::Report { asset } => {
                validate_identifier(&asset, "--asset")?;
                crate::rpc::report_cmd::run(&asset)?;
            }
            Commands::CrossChainSummary { asset, run_id } => {
                validate_identifier(&asset, "--asset")?;
                validate_identifier(&run_id, "--run-id")?;
                crate::rpc::cross_chain_summary::run(&asset, &run_id)?;
            }
            #[cfg(feature = "experimental")]
            Commands::ControlAudit {
                asset,
                chains,
                from_block,
                to_block,
            } => {
                validate_identifier(&asset, "--asset")?;
                for chain in &chains {
                    validate_identifier(chain, "--chains")?;
                }
                if from_block == 0 {
                    anyhow::bail!("--from-block 0 is not supported");
                }
                let tb = to_block.trim();
                if !tb.eq_ignore_ascii_case("latest") {
                    let b: u64 = tb.parse().map_err(|_| {
                        anyhow::anyhow!("--to-block must be a block number or latest, got {:?}", to_block)
                    })?;
                    if b < from_block {
                        anyhow::bail!("--to-block ({b}) must be >= --from-block ({from_block})");
                    }
                }
                let chains = if chains.is_empty() {
                    vec!["ethereum".into(), "base".into(), "arbitrum".into()]
                } else {
                    chains
                };
                crate::rpc::control_audit::run(&asset, &chains, from_block, tb).await?;
            }
            #[cfg(feature = "experimental")]
            Commands::ControlReport { asset } => {
                validate_identifier(&asset, "--asset")?;
                crate::rpc::control_report_cmd::run(&asset)?;
            }
        }
        Ok(())
    }

    pub fn validate_identifier(value: &str, flag: &str) -> Result<()> {
        if !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            Ok(())
        } else {
            anyhow::bail!(
                "{flag} must contain only letters, digits, hyphens, and underscores; got {:?}",
                value
            )
        }
    }

    #[derive(Parser)]
    #[command(
        name = "stablecoin-audit",
        about = "Reproducible windowed supply-invariant audits (v0.1); optional experimental control/fetch surfaces"
    )]
    struct Cli {
        #[command(subcommand)]
        command: Commands,
    }

    #[derive(Subcommand)]
    enum Commands {
        TransferAudit {
            #[arg(long, default_value = "USDC")]
            asset: String,
            #[arg(long, value_delimiter = ',')]
            chains: Vec<String>,
            #[arg(long)]
            from_block: Option<u64>,
            #[arg(long)]
            to_block: Option<String>,
            #[arg(long = "window", value_name = "CHAIN:FROM:TO", action = clap::ArgAction::Append)]
            windows: Vec<String>,
            #[arg(long)]
            chunk_size: Option<u64>,
            #[arg(long)]
            run_id: Option<String>,
            #[arg(long)]
            fresh: bool,
        },
        Metadata {
            #[arg(long, default_value = "USDC")]
            asset: String,
            #[arg(long, value_delimiter = ',')]
            chains: Vec<String>,
            #[arg(long)]
            from_block: u64,
            #[arg(long)]
            to_block: Option<u64>,
        },
        #[cfg(feature = "experimental")]
        Fetch {
            #[arg(long, default_value = "USDC")]
            asset: String,
            #[arg(long, value_delimiter = ',')]
            chains: Vec<String>,
            #[arg(long)]
            from_block: u64,
            #[arg(long)]
            to_block: u64,
            #[arg(long)]
            chunk_size: Option<u64>,
        },
        #[cfg(feature = "experimental")]
        Report {
            #[arg(long, default_value = "USDC")]
            asset: String,
        },
        ResolveWindow {
            #[arg(long, default_value = "USDC")]
            asset: String,
            #[arg(long, value_delimiter = ',')]
            chains: Vec<String>,
            #[arg(long = "from")]
            window_from: String,
            #[arg(long = "to")]
            window_to: String,
        },
        CrossChainSummary {
            #[arg(long, default_value = "USDC")]
            asset: String,
            #[arg(long)]
            run_id: String,
        },
        #[cfg(feature = "experimental")]
        ControlAudit {
            #[arg(long, default_value = "USDC")]
            asset: String,
            #[arg(long, value_delimiter = ',')]
            chains: Vec<String>,
            #[arg(long)]
            from_block: u64,
            #[arg(long)]
            to_block: String,
        },
        #[cfg(feature = "experimental")]
        ControlReport {
            #[arg(long, default_value = "USDC")]
            asset: String,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::cli::validate_identifier;
    use super::run_cli;
    use std::path::Path;

    #[test]
    fn validate_identifier_accepts_usdc() {
        validate_identifier("USDC", "--asset").unwrap();
    }

    #[test]
    fn validate_identifier_rejects_slash() {
        assert!(validate_identifier("a/b", "--asset").is_err());
    }

    #[test]
    fn cli_rejects_chunk_size_zero() {
        let err = run_cli([
            "stablecoin-audit",
            "transfer-audit",
            "--from-block",
            "100",
            "--to-block",
            "200",
            "--chunk-size",
            "0",
        ]);
        assert!(err.is_err());
    }

    #[test]
    fn cli_rejects_window_with_from_block() {
        assert!(run_cli([
            "stablecoin-audit",
            "transfer-audit",
            "--from-block",
            "1",
            "--window",
            "ethereum:100:200",
        ])
        .is_err());
    }

    #[test]
    fn cli_metadata_rejects_zero_from_block() {
        assert!(run_cli([
            "stablecoin-audit",
            "metadata",
            "--from-block",
            "0",
            "--chains",
            "ethereum",
        ])
        .is_err());
    }

    #[test]
    fn cli_resolve_window_rejects_inverted_range() {
        assert!(run_cli([
            "stablecoin-audit",
            "resolve-window",
            "--from",
            "2026-05-08T00:00:00Z",
            "--to",
            "2026-05-01T00:00:00Z",
            "--chains",
            "ethereum",
        ])
        .is_err());
    }

    #[test]
    fn cli_transfer_audit_rejects_from_block_zero() {
        assert!(run_cli([
            "stablecoin-audit",
            "transfer-audit",
            "--from-block",
            "0",
            "--to-block",
            "100",
        ])
        .is_err());
    }

    #[test]
    fn cross_chain_summary_from_benchmark_fixture() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/benchmarks/usdc_7d_20260501_20260508");
        let run_id = format!("itest_{}", std::process::id());
        let out_dir = crate::ensure_run_out_dir("USDC", &run_id).unwrap();
        for name in [
            "supply_audit.csv",
            "provenance.json",
            "summary.md",
            "supply_audit.md",
        ] {
            std::fs::copy(fixture.join(name), out_dir.join(name)).unwrap();
        }
        let mut qa: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(fixture.join("qa_report.json")).unwrap())
                .unwrap();
        qa["run_id"] = serde_json::Value::String(run_id.clone());
        std::fs::write(
            out_dir.join("qa_report.json"),
            serde_json::to_string_pretty(&qa).unwrap(),
        )
        .unwrap();
        run_cli([
            "stablecoin-audit",
            "cross-chain-summary",
            "--asset",
            "USDC",
            "--run-id",
            &run_id,
        ])
        .unwrap();
        assert!(out_dir.join("cross_chain_summary.json").is_file());
        assert!(out_dir.join("cross_chain_summary.md").is_file());
        let _ = std::fs::remove_dir_all(&out_dir);
    }
}
