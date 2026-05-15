use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

fn validate_identifier(value: &str, flag: &str) -> Result<()> {
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

mod config;
mod report;
mod rpc;

mod decode;
mod fetch;

#[cfg(feature = "experimental")]
mod control_events;

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
    /// Transfer-log audit: decode, dedup, supply invariant (v0.1)
    TransferAudit {
        /// Asset symbol (e.g. USDC)
        #[arg(long, default_value = "USDC")]
        asset: String,
        /// Comma-separated chains (e.g. ethereum,base,arbitrum). Ignored when using `--window`.
        #[arg(long, value_delimiter = ',')]
        chains: Vec<String>,
        /// Start of block window (shared across `--chains`). Omit when using `--window`.
        #[arg(long)]
        from_block: Option<u64>,
        /// End block height, or `latest`. Omit when using `--window`.
        #[arg(long)]
        to_block: Option<String>,
        /// Per-chain block window `chain:from:to` (inclusive `to`). Repeatable smoke-test style.
        /// Cannot be combined with `--chains`, `--from-block`, or `--to-block`.
        #[arg(long = "window", value_name = "CHAIN:FROM:TO", action = clap::ArgAction::Append)]
        windows: Vec<String>,
        /// Blocks per eth_getLogs request (default: 500)
        #[arg(long)]
        chunk_size: Option<u64>,
        /// Run directory `out/<asset>/runs/<run_id>/` (alphanumeric, `-`, `_`). Default: UTC timestamp id.
        #[arg(long)]
        run_id: Option<String>,
        /// Discard `checkpoint/` and re-fetch every chain (default: resume when checkpoint exists).
        #[arg(long)]
        fresh: bool,
    },
    Metadata {
        /// Asset symbol (e.g. USDC)
        #[arg(long, default_value = "USDC")]
        asset: String,
        /// Comma-separated chains (e.g. ethereum,base,arbitrum)
        #[arg(long, value_delimiter = ',')]
        chains: Vec<String>,
        /// Start of block window
        #[arg(long)]
        from_block: u64,
        /// End of block window (omit for latest)
        #[arg(long)]
        to_block: Option<u64>,
    },
    /// [experimental] Fetch Transfer logs, decode, dedup, QA, and output CSV
    #[cfg(feature = "experimental")]
    Fetch {
        /// Asset symbol (e.g. USDC)
        #[arg(long, default_value = "USDC")]
        asset: String,
        /// Comma-separated chains
        #[arg(long, value_delimiter = ',')]
        chains: Vec<String>,
        /// Start of block window
        #[arg(long)]
        from_block: u64,
        /// End of block window
        #[arg(long)]
        to_block: u64,
        /// Blocks per eth_getLogs request (default: 500)
        #[arg(long)]
        chunk_size: Option<u64>,
    },
    /// [experimental] Generate cross-chain comparison report from existing fetch output
    #[cfg(feature = "experimental")]
    Report {
        /// Asset symbol (e.g. USDC)
        #[arg(long, default_value = "USDC")]
        asset: String,
    },
    /// Resolve a UTC wall-clock window to per-chain `--window` args (RPC block headers only; v0.1)
    ResolveWindow {
        #[arg(long, default_value = "USDC")]
        asset: String,
        #[arg(long, value_delimiter = ',')]
        chains: Vec<String>,
        /// UTC start (RFC3339), e.g. `2026-05-01T00:00:00Z` — first block has header time ≥ this
        #[arg(long = "from")]
        window_from: String,
        /// UTC end (RFC3339), e.g. `2026-05-08T00:00:00Z` — last block has header time ≤ this
        #[arg(long = "to")]
        window_to: String,
    },
    /// Write `cross_chain_summary.{json,md}` from one `transfer-audit` run directory (v0.1)
    CrossChainSummary {
        /// Asset symbol (e.g. USDC)
        #[arg(long, default_value = "USDC")]
        asset: String,
        /// Same `--run-id` as the `transfer-audit` run under `out/<asset>/runs/<run_id>/`
        #[arg(long)]
        run_id: String,
    },
    /// [experimental] Milestone 5: issuer control events — query, decode, `control_events_<chain>.csv`, QA/provenance, `risk_flags.md`
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
    /// [experimental] v0.2 benchmark from `control-audit` artifacts (requires full QA↔provenance bundle)
    #[cfg(feature = "experimental")]
    ControlReport {
        #[arg(long, default_value = "USDC")]
        asset: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
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
                    .map(|w| rpc::transfer_audit::parse_window_arg(w))
                    .collect::<Result<Vec<_>>>()?;
                rpc::transfer_audit::run_per_chain_windows(&asset, parsed, chunk_size, run_id, fresh)
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
                rpc::transfer_audit::run(&asset, &chains, from_block, tb, chunk_size, run_id, fresh)
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
            rpc::resolve_window::run(&asset, &chains, &window_from, &window_to).await?;
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
            rpc::metadata::run(&asset, &chains, from_block, to_block).await?;
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
            rpc::fetch_logs::run(&asset, &chains, from_block, to_block, chunk_size).await?;
        }
        #[cfg(feature = "experimental")]
        Commands::Report { asset } => {
            validate_identifier(&asset, "--asset")?;
            rpc::report_cmd::run(&asset)?;
        }
        Commands::CrossChainSummary { asset, run_id } => {
            validate_identifier(&asset, "--asset")?;
            validate_identifier(&run_id, "--run-id")?;
            rpc::cross_chain_summary::run(&asset, &run_id)?;
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
            rpc::control_audit::run(&asset, &chains, from_block, tb).await?;
        }
        #[cfg(feature = "experimental")]
        Commands::ControlReport { asset } => {
            validate_identifier(&asset, "--asset")?;
            rpc::control_report_cmd::run(&asset)?;
        }
    }
    Ok(())
}
