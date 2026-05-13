use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::rpc::types::{Filter, Log};
use alloy::sol;
use alloy::sol_types::SolEvent;
use alloy::transports::Transport;
use anyhow::Result;
use serde::Serialize;
use std::time::Duration;
use tokio::time::sleep;

sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
}

#[allow(dead_code)]
pub const TRANSFER_SIGNATURE_HASH: alloy::primitives::B256 = Transfer::SIGNATURE_HASH;

pub struct FetchParams {
    pub contract_address: Address,
    pub from_block: u64,
    pub to_block: u64,
    pub chunk_size: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct ChunkingStats {
    pub initial_chunk: u64,
    pub final_chunk: u64,
    pub chunks_total: u64,
    pub retries_total: u64,
    pub backoffs_total: u64,
}

/// Fetch all Transfer logs for a contract over a block window, in chunks.
/// Returns raw alloy Log objects.
#[cfg_attr(not(feature = "experimental"), allow(dead_code))]
pub async fn fetch_transfer_logs<T: Transport + Clone>(
    provider: &impl Provider<T>,
    params: &FetchParams,
) -> Result<Vec<Log>> {
    if params.chunk_size == 0 {
        anyhow::bail!("chunk_size must be at least 1");
    }
    let mut all_logs: Vec<Log> = Vec::new();
    let mut start = params.from_block;

    while start <= params.to_block {
        let end = start.saturating_add(params.chunk_size - 1).min(params.to_block);

        let filter = Filter::new()
            .address(params.contract_address)
            .event_signature(Transfer::SIGNATURE_HASH)
            .from_block(start)
            .to_block(end);

        let logs = provider.get_logs(&filter).await?;
        all_logs.extend(logs);
        if end == params.to_block {
            break;
        }
        start = end + 1;
    }

    Ok(all_logs)
}

/// Fetch Transfer logs with adaptive chunk size and retry/backoff.
///
/// Strategy:
/// - Start with `params.chunk_size`
/// - On repeated get_logs failure, halve chunk size down to `min_chunk_size`
/// - Retry each chunk up to `max_retries_per_chunk` with exponential backoff
pub async fn fetch_transfer_logs_adaptive<T: Transport + Clone>(
    provider: &impl Provider<T>,
    params: &FetchParams,
    min_chunk_size: u64,
    max_retries_per_chunk: u32,
) -> Result<(Vec<Log>, ChunkingStats)> {
    if params.chunk_size == 0 {
        anyhow::bail!("chunk_size must be at least 1");
    }
    if min_chunk_size == 0 {
        anyhow::bail!("min_chunk_size must be at least 1");
    }

    let mut stats = ChunkingStats {
        initial_chunk: params.chunk_size,
        final_chunk: params.chunk_size,
        chunks_total: 0,
        retries_total: 0,
        backoffs_total: 0,
    };

    let mut chunk_size = params.chunk_size.max(min_chunk_size);
    let mut all_logs: Vec<Log> = Vec::new();
    let mut start = params.from_block;

    while start <= params.to_block {
        let end = start.saturating_add(chunk_size - 1).min(params.to_block);

        let filter = Filter::new()
            .address(params.contract_address)
            .event_signature(Transfer::SIGNATURE_HASH)
            .from_block(start)
            .to_block(end);

        let mut attempt: u32 = 0;
        let logs = loop {
            match provider.get_logs(&filter).await {
                Ok(logs) => break Ok(logs),
                Err(err) => {
                    if attempt < max_retries_per_chunk {
                        let wait_ms = 200u64.saturating_mul(2u64.saturating_pow(attempt));
                        stats.retries_total += 1;
                        stats.backoffs_total += 1;
                        attempt += 1;
                        sleep(Duration::from_millis(wait_ms)).await;
                        continue;
                    }
                    break Err(err);
                }
            }
        };

        match logs {
            Ok(logs) => {
                stats.chunks_total += 1;
                all_logs.extend(logs);
                if end == params.to_block {
                    break;
                }
                start = end + 1;
            }
            Err(e) => {
                if chunk_size > min_chunk_size {
                    chunk_size = (chunk_size / 2).max(min_chunk_size);
                    stats.final_chunk = chunk_size;
                    continue;
                }
                anyhow::bail!(
                    "eth_getLogs failed at min chunk size {} for range {}..{}: {:#}",
                    min_chunk_size,
                    start,
                    end,
                    e
                );
            }
        }
    }

    stats.final_chunk = chunk_size;
    Ok((all_logs, stats))
}
