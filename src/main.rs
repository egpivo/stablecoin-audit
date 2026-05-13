use anyhow::Result;
use clap::{Parser, Subcommand};
#[cfg(feature = "experimental")]
use std::collections::HashSet;

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
    /// [experimental] Transfer-log audit: per-chain windows and supply invariant
    #[cfg(feature = "experimental")]
    TransferAudit {
        /// Asset symbol (e.g. USDC)
        #[arg(long, default_value = "USDC")]
        asset: String,
        /// Repeatable per-chain window: chain:start:end_or_latest
        #[arg(long = "window")]
        windows: Vec<String>,
        /// Fallback start block when --window is omitted
        #[arg(long)]
        from_block: Option<u64>,
        /// Fallback end block when --window is omitted
        #[arg(long)]
        to_block: Option<String>,
        /// Fallback chains when --window is omitted
        #[arg(long, value_delimiter = ',')]
        chains: Vec<String>,
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
            windows,
            from_block,
            to_block,
            chains,
            chunk_size,
        } => {
            validate_identifier(&asset, "--asset")?;
            if let Some(cs) = chunk_size {
                if cs == 0 {
                    anyhow::bail!("--chunk-size must be at least 1");
                }
            }
            let parsed_windows = if !windows.is_empty() {
                parse_windows(&windows)?
            } else {
                let from = from_block.ok_or_else(|| {
                    anyhow::anyhow!("provide --window or fallback --from-block/--to-block")
                })?;
                let to_raw = to_block
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("provide --window or fallback --to-block"))?;
                if from == 0 {
                    anyhow::bail!(
                        "--from-block 0 is not supported; use the contract deployment_block or later"
                    );
                }
                let chains = if chains.is_empty() {
                    vec!["ethereum".to_string(), "base".to_string(), "arbitrum".to_string()]
                } else {
                    chains
                };
                let mut fallback = Vec::with_capacity(chains.len());
                for chain in chains {
                    validate_identifier(&chain, "--chains")?;
                    let end_block = parse_end_block(to_raw)?;
                    if let rpc::transfer_audit::EndBlock::Number(end) = end_block {
                        if end < from {
                            anyhow::bail!(
                                "--to-block ({end}) must be >= --from-block ({from}) for chain {}",
                                chain
                            );
                        }
                    }
                    fallback.push(rpc::transfer_audit::WindowSpec {
                        chain,
                        start_block: from,
                        end_block,
                    });
                }
                fallback
            };
            rpc::transfer_audit::run(&asset, &parsed_windows, chunk_size).await?;
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

#[cfg(feature = "experimental")]
fn parse_end_block(raw: &str) -> Result<rpc::transfer_audit::EndBlock> {
    if raw.eq_ignore_ascii_case("latest") {
        Ok(rpc::transfer_audit::EndBlock::Latest)
    } else {
        let n = raw
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("end block must be integer or latest; got {:?}", raw))?;
        Ok(rpc::transfer_audit::EndBlock::Number(n))
    }
}

#[cfg(feature = "experimental")]
fn parse_windows(raw_windows: &[String]) -> Result<Vec<rpc::transfer_audit::WindowSpec>> {
    let mut out = Vec::with_capacity(raw_windows.len());
    let mut seen = HashSet::new();

    for raw in raw_windows {
        let mut parts = raw.split(':');
        let chain = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("invalid --window {:?}: missing chain", raw))?
            .trim()
            .to_string();
        let start_raw = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("invalid --window {:?}: missing start", raw))?;
        let end_raw = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("invalid --window {:?}: missing end", raw))?;
        if parts.next().is_some() {
            anyhow::bail!("invalid --window {:?}: expected chain:start:end", raw);
        }

        validate_identifier(&chain, "--window chain")?;
        if !seen.insert(chain.clone()) {
            anyhow::bail!("duplicate --window chain {:?} is not allowed", chain);
        }

        let start_block = start_raw.parse::<u64>().map_err(|_| {
            anyhow::anyhow!("invalid --window {:?}: start must be a positive integer", raw)
        })?;
        if start_block == 0 {
            anyhow::bail!("invalid --window {:?}: start must be >= 1", raw);
        }

        let end_block = parse_end_block(end_raw.trim())?;
        if let rpc::transfer_audit::EndBlock::Number(end) = end_block {
            if end < start_block {
                anyhow::bail!(
                    "invalid --window {:?}: end {} must be >= start {}",
                    raw,
                    end,
                    start_block
                );
            }
        }

        out.push(rpc::transfer_audit::WindowSpec {
            chain,
            start_block,
            end_block,
        });
    }

    Ok(out)
}
