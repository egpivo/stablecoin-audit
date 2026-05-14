//! Map a UTC wall-clock interval to per-chain block bounds for `transfer-audit --window`.
//! No logs, no supply math — only `eth_blockNumber` + `eth_getBlockByNumber`.

use alloy::eips::BlockNumberOrTag;
use alloy::providers::Provider;
use alloy::rpc::types::BlockTransactionsKind;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use crate::config::load_single_token_config;
use crate::rpc::{build_provider, HttpProvider};

async fn block_timestamp_u64(provider: &HttpProvider, block_number: u64) -> Result<u64> {
    let tag = BlockNumberOrTag::from(block_number);
    let blk = provider
        .get_block_by_number(tag, BlockTransactionsKind::Hashes)
        .await
        .with_context(|| format!("getBlockByNumber({block_number})"))?;
    let Some(b) = blk else {
        anyhow::bail!("no block returned for height {block_number}");
    };
    Ok(b.header.timestamp)
}

/// Smallest block height `h` in `[1, latest]` with `timestamp(h) >= target` (seconds).
async fn first_block_with_timestamp_ge(
    provider: &HttpProvider,
    target: u64,
    latest: u64,
) -> Result<u64> {
    let ts_latest = block_timestamp_u64(provider, latest).await?;
    if ts_latest < target {
        anyhow::bail!(
            "chain head block {latest} has timestamp {ts_latest} < target {target}; \
             window start is in the future for this RPC"
        );
    }
    let ts1 = block_timestamp_u64(provider, 1).await?;
    if ts1 >= target {
        return Ok(1);
    }
    let mut lo = 1u64;
    let mut hi = latest;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let t = block_timestamp_u64(provider, mid).await?;
        if t >= target {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    Ok(lo)
}

/// Largest block height `h` in `[1, latest]` with `timestamp(h) <= target` (seconds).
async fn last_block_with_timestamp_le(
    provider: &HttpProvider,
    target: u64,
    latest: u64,
) -> Result<u64> {
    let ts1 = block_timestamp_u64(provider, 1).await?;
    if ts1 > target {
        anyhow::bail!(
            "block 1 timestamp {ts1} is already after window end {target} for this chain"
        );
    }
    let ts_latest = block_timestamp_u64(provider, latest).await?;
    if ts_latest <= target {
        return Ok(latest);
    }
    let mut lo = 1u64;
    let mut hi = latest;
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let t = block_timestamp_u64(provider, mid).await?;
        if t <= target {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Ok(lo)
}

fn parse_rfc3339_utc(s: &str) -> Result<DateTime<Utc>> {
    let t = s.trim();
    DateTime::parse_from_rfc3339(t)
        .map(|d| d.with_timezone(&Utc))
        .or_else(|_| {
            t.parse::<DateTime<Utc>>()
                .map_err(|e| anyhow::anyhow!("invalid RFC3339 datetime {:?}: {e}", s))
        })
}

fn fmt_ts(ts: u64) -> String {
    DateTime::<Utc>::from_timestamp(ts as i64, 0)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_else(|| format!("unix_{ts}"))
}

pub async fn run(asset: &str, chains: &[String], from_s: &str, to_s: &str) -> Result<()> {
    let from_dt = parse_rfc3339_utc(from_s).context("parse --from")?;
    let to_dt = parse_rfc3339_utc(to_s).context("parse --to")?;
    if from_dt >= to_dt {
        anyhow::bail!("--from must be strictly before --to (got from={from_dt}, to={to_dt})");
    }
    let from_sec = from_dt.timestamp();
    let to_sec = to_dt.timestamp();
    if from_sec < 0 || to_sec < 0 {
        anyhow::bail!("only non-negative unix times are supported for block headers");
    }
    let from_u = from_sec as u64;
    let to_u = to_sec as u64;

    println!(
        "# Resolved wall-clock window (UTC): {} → {}",
        from_dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        to_dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    );
    println!("# Smallest block with header time ≥ --from; largest block with header time ≤ --to.");
    println!();

    let mut rows: Vec<(String, u64, u64, u64, u64)> = Vec::new();

    for chain in chains {
        let config = load_single_token_config(asset, chain)
            .with_context(|| format!("load config for chain {chain:?}"))?;
        let rpc_url = config.rpc_url().with_context(|| format!("RPC env for {chain}"))?;
        let provider = build_provider(&rpc_url)?;

        let latest = provider
            .get_block_number()
            .await
            .with_context(|| format!("eth_blockNumber for {chain}"))?;

        let start_b = first_block_with_timestamp_ge(&provider, from_u, latest)
            .await
            .with_context(|| format!("resolve start block for {chain}"))?;
        let end_b = last_block_with_timestamp_le(&provider, to_u, latest)
            .await
            .with_context(|| format!("resolve end block for {chain}"))?;

        if start_b > end_b {
            anyhow::bail!(
                "chain {chain}: resolved start block {start_b} > end block {end_b}; \
                 widen the UTC window or check RPC clock vs wall clock"
            );
        }

        let ts_start = block_timestamp_u64(&provider, start_b).await?;
        let ts_end = block_timestamp_u64(&provider, end_b).await?;

        rows.push((chain.clone(), start_b, end_b, ts_start, ts_end));
    }

    rows.sort_by(|a, b| a.0.cmp(&b.0));

    println!("# Per-chain bounds (inclusive end block, same as transfer-audit --window):");
    for (chain, sb, eb, tst, tet) in &rows {
        println!(
            "# {:10} blocks {} → {}  (header times {} → {})",
            chain, sb, eb, fmt_ts(*tst), fmt_ts(*tet),
        );
    }
    println!();
    print!(
        "cargo run -- transfer-audit --asset {} --run-id <your_run_id>",
        asset.to_uppercase()
    );
    for (chain, sb, eb, _, _) in &rows {
        print!(" \\\n  --window {chain}:{sb}:{eb}");
    }
    println!();
    println!();
    println!("# Paste the command above, substitute <your_run_id>, then run cross-chain-summary with the same id.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_rfc3339_utc;

    #[test]
    fn parse_from_z() {
        let d = parse_rfc3339_utc("2026-05-01T00:00:00Z").unwrap();
        assert_eq!(d.date_naive().to_string(), "2026-05-01");
    }
}
