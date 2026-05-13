use anyhow::Result;
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

// M2-M5 modules: compiled only with --features experimental
#[cfg(feature = "experimental")]
mod control_events;
#[cfg(feature = "experimental")]
mod decode;
#[cfg(feature = "experimental")]
mod fetch;

#[derive(Parser)]
#[command(
    name = "stablecoin-audit",
    about = "Reproducible windowed audits of stablecoin supply and control events"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// [experimental] Transfer-log audit: decode, dedup, supply reconciliation
    #[cfg(feature = "experimental")]
    TransferAudit {
        /// Asset symbol (e.g. USDC)
        #[arg(long, default_value = "USDC")]
        asset: String,
        /// Comma-separated chains (e.g. ethereum,base,arbitrum)
        #[arg(long, value_delimiter = ',')]
        chains: Vec<String>,
        /// Start of block window
        #[arg(long)]
        from_block: u64,
        /// End block height, or `latest`
        #[arg(long)]
        to_block: String,
        /// Blocks per eth_getLogs request (default: 500)
        #[arg(long)]
        chunk_size: Option<u64>,
    },
    /// Fetch and report token metadata and totalSupply for a block window
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
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    match cli.command {
        #[cfg(feature = "experimental")]
        Commands::TransferAudit {
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
            if from_block == 0 {
                anyhow::bail!("--from-block 0 is not supported; use the contract deployment_block or later");
            }
            if let Some(cs) = chunk_size {
                if cs == 0 {
                    anyhow::bail!("--chunk-size must be at least 1");
                }
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
            rpc::transfer_audit::run(&asset, &chains, from_block, tb, chunk_size).await?;
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
    }
    Ok(())
}
