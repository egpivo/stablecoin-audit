use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::rpc::types::{Filter, Log};
use alloy::sol;
use alloy::sol_types::SolEvent;
use alloy::transports::Transport;
use anyhow::Result;
use std::time::Duration;

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

/// Fetch all Transfer logs for a contract over a block window, in chunks (no checkpointing).
pub async fn fetch_transfer_logs<T: Transport + Clone>(
    provider: &impl Provider<T>,
    params: &FetchParams,
) -> Result<Vec<Log>> {
    let mut all_logs: Vec<Log> = Vec::new();
    fetch_transfer_logs_incremental(
        provider,
        params,
        params.from_block,
        |_, _, _, _, logs| {
            all_logs.extend(logs.iter().cloned());
            Ok(())
        },
    )
    .await?;
    Ok(all_logs)
}

/// Fetch logs chunk-by-chunk from `resume_from` through `params.to_block`.
/// Invokes `on_chunk` after each successful RPC (for checkpoint + progress).
pub async fn fetch_transfer_logs_incremental<T: Transport + Clone>(
    provider: &impl Provider<T>,
    params: &FetchParams,
    resume_from: u64,
    mut on_chunk: impl FnMut(u64, u64, u64, u64, &[Log]) -> Result<()>,
) -> Result<()> {
    if params.chunk_size == 0 {
        anyhow::bail!("chunk_size must be at least 1");
    }
    let mut start = resume_from.max(params.from_block);
    if start > params.to_block {
        return Ok(());
    }

    let total_chunks = count_chunks(params.from_block, params.to_block, params.chunk_size);
    let mut chunks_done = if start > params.from_block {
        count_chunks(params.from_block, start.saturating_sub(1), params.chunk_size)
    } else {
        0
    };

    while start <= params.to_block {
        let end = start.saturating_add(params.chunk_size - 1).min(params.to_block);

        let filter = Filter::new()
            .address(params.contract_address)
            .event_signature(Transfer::SIGNATURE_HASH)
            .from_block(start)
            .to_block(end);

        let logs = fetch_logs_with_retry(provider, &filter, start, end).await?;
        chunks_done += 1;
        on_chunk(start, end, chunks_done, total_chunks, &logs)?;

        if end == params.to_block {
            break;
        }
        start = end + 1;
    }

    Ok(())
}

const LOG_FETCH_MAX_ATTEMPTS: u32 = 5;
const LOG_FETCH_RETRY_BASE_MS: u64 = 2000;

async fn fetch_logs_with_retry<T: Transport + Clone>(
    provider: &impl Provider<T>,
    filter: &Filter,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<Log>> {
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 1..=LOG_FETCH_MAX_ATTEMPTS {
        match provider.get_logs(filter).await {
            Ok(logs) => return Ok(logs),
            Err(e) => {
                let err = anyhow::Error::new(e).context(format!(
                    "eth_getLogs blocks {from_block}..{to_block} (attempt {attempt}/{LOG_FETCH_MAX_ATTEMPTS})"
                ));
                last_err = Some(err);
                if attempt < LOG_FETCH_MAX_ATTEMPTS {
                    let delay_ms = LOG_FETCH_RETRY_BASE_MS * 2u64.pow(attempt - 1);
                    eprintln!(
                        "[fetch] RPC error on blocks {from_block}..{to_block}, retry in {delay_ms}ms"
                    );
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("eth_getLogs failed with no error detail")))
}

fn count_chunks(from_block: u64, to_block: u64, chunk_size: u64) -> u64 {
    if to_block < from_block || chunk_size == 0 {
        return 0;
    }
    (to_block - from_block + 1).div_ceil(chunk_size)
}
