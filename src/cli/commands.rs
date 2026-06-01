use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::domain::asset::validate_identifier;
use crate::rpc::transfer_audit::parse_window_arg;

pub async fn run_async<I, S>(args: I) -> Result<()>
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
                anyhow::bail!(
                    "--from-block 0 is not supported; use the contract deployment_block or later"
                );
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
        Commands::StablecoinMapPackage {
            output_dir,
            dependency_summary,
            liquidity_pairs,
            artemis_start,
            artemis_end,
            skip_network,
        } => {
            crate::stablecoin_map::run(crate::stablecoin_map::PackageOptions {
                output_dir,
                dependency_summary,
                liquidity_pairs,
                artemis_start,
                artemis_end,
                skip_network,
            })
            .await?;
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
                    anyhow::anyhow!(
                        "--to-block must be a block number or latest, got {:?}",
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
            crate::rpc::control_audit::run(&asset, &chains, from_block, tb).await?;
        }
        #[cfg(feature = "experimental")]
        Commands::ControlReport { asset } => {
            validate_identifier(&asset, "--asset")?;
            crate::rpc::control_report_cmd::run(&asset)?;
        }
        #[cfg(feature = "api")]
        Commands::Serve {
            artifact_root,
            host,
            port,
        } => {
            crate::api::serve(artifact_root, &host, port).await?;
        }
    }
    Ok(())
}

#[derive(Parser)]
#[command(
    name = "stablecoin-audit",
    version,
    about = "Reproducible windowed supply-invariant audits; optional experimental control/fetch surfaces"
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
    StablecoinMapPackage {
        #[arg(long, default_value = "data/benchmarks")]
        output_dir: std::path::PathBuf,
        #[arg(
            long,
            default_value = "data/benchmarks/stablecoin_pair_dependence_summary.csv"
        )]
        dependency_summary: std::path::PathBuf,
        #[arg(long, default_value = "data/benchmarks/stablecoin_liquidity_pairs.csv")]
        liquidity_pairs: std::path::PathBuf,
        #[arg(long, default_value = "2026-04-28")]
        artemis_start: String,
        #[arg(long, default_value = "2026-05-27")]
        artemis_end: String,
        #[arg(long)]
        skip_network: bool,
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
    #[cfg(feature = "api")]
    Serve {
        #[arg(long, default_value = "out")]
        artifact_root: std::path::PathBuf,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value = "8080")]
        port: u16,
    },
}
