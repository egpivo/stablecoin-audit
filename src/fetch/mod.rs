use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::rpc::types::{Filter, Log};
use alloy::sol;
use alloy::sol_types::SolEvent;
use alloy::transports::Transport;
use anyhow::Result;

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

/// Fetch all Transfer logs for a contract over a block window, in chunks.
/// Returns raw alloy Log objects.
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
