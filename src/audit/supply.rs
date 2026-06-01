//! Supply snapshot and mint/burn reconciliation logic (separable from transfer-audit RPC).

use std::collections::HashSet;

use alloy::primitives::{I256, U256};
use serde::{Deserialize, Serialize};

use crate::decode::{dedup_transfer_events, TransferEvent};

const ZERO_ADDR: &str = "0x0000000000000000000000000000000000000000";

/// Per-chain supply audit row (workflow CSV schema; canonical snapshots derive from this).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyAuditRow {
    pub chain: String,
    pub chain_id: u64,
    pub contract_address: String,
    pub from_block: u64,
    pub resolved_to_block: Option<u64>,
    pub to_block_requested: String,
    pub chunk_size: u64,
    pub transfer_event_count: usize,
    pub active_senders: usize,
    pub active_recipients: usize,
    pub mint_count: usize,
    pub burn_count: usize,
    pub plain_transfer_count: usize,
    pub sum_mints_raw: String,
    pub sum_burns_raw: String,
    pub net_mint_raw: Option<String>,
    pub total_supply_at_start_minus_1: Option<String>,
    pub total_supply_at_start_minus_1_provenance: String,
    pub total_supply_at_end: Option<String>,
    pub onchain_delta_raw: Option<String>,
    pub discrepancy_raw: Option<String>,
    pub metadata_call_pass: bool,
    pub historical_supply_pass: bool,
    pub no_duplicate_logs_pass: Option<bool>,
    pub transfer_decode_pass: Option<bool>,
    pub supply_invariant_pass: Option<bool>,
    pub duplicate_count: usize,
    pub full_decode_error_count: usize,
    #[serde(skip)]
    pub total_supply_start_raw: Option<String>,
    #[serde(skip)]
    pub total_supply_end_raw: Option<String>,
    #[serde(skip)]
    pub total_supply_start_block_timestamp_rfc3339: Option<String>,
    #[serde(skip)]
    pub window_start_block_timestamp_rfc3339: Option<String>,
    #[serde(skip)]
    pub window_end_block_timestamp_rfc3339: Option<String>,
}

/// Deduped transfer aggregates and supply-invariant fields (no RPC).
pub struct SupplyMetrics {
    pub deduped: Vec<TransferEvent>,
    pub duplicate_count: usize,
    pub mint_count: usize,
    pub burn_count: usize,
    pub plain_transfer_count: usize,
    pub active_senders: usize,
    pub active_recipients: usize,
    pub sum_mints: U256,
    pub sum_burns: U256,
    pub net_mint: Option<I256>,
    pub onchain_delta: Option<I256>,
    pub discrepancy: Option<I256>,
    pub supply_invariant_pass: Option<bool>,
    pub no_duplicate_logs_pass: bool,
    pub transfer_decode_pass: bool,
}

/// Mint/burn aggregates vs pinned `totalSupply` boundaries (signed I256).
pub fn compute_supply_invariant(
    sum_mints: U256,
    sum_burns: U256,
    supply_start: U256,
    supply_end: U256,
) -> (I256, I256, I256, bool) {
    let net_mint = I256::from_raw(sum_mints) - I256::from_raw(sum_burns);
    let onchain_delta = I256::from_raw(supply_end) - I256::from_raw(supply_start);
    let discrepancy = net_mint - onchain_delta;
    (
        net_mint,
        onchain_delta,
        discrepancy,
        discrepancy == I256::ZERO,
    )
}

pub fn build_supply_metrics_from_events(
    events: Vec<TransferEvent>,
    decode_errors: usize,
    supply_start: Option<U256>,
    supply_end: Option<U256>,
) -> SupplyMetrics {
    let (deduped, dup_count) = dedup_transfer_events(events);

    let mint_count = deduped.iter().filter(|e| e.kind == "mint").count();
    let burn_count = deduped.iter().filter(|e| e.kind == "burn").count();
    let plain_transfer_count = deduped.iter().filter(|e| e.kind == "transfer").count();

    let mut senders: HashSet<String> = HashSet::new();
    let mut recipients: HashSet<String> = HashSet::new();
    for e in &deduped {
        if e.from != ZERO_ADDR {
            senders.insert(e.from.clone());
        }
        if e.to != ZERO_ADDR {
            recipients.insert(e.to.clone());
        }
    }

    let sum_mints: U256 = deduped
        .iter()
        .filter(|e| e.kind == "mint")
        .fold(U256::ZERO, |acc, e| acc + e.value_u256);
    let sum_burns: U256 = deduped
        .iter()
        .filter(|e| e.kind == "burn")
        .fold(U256::ZERO, |acc, e| acc + e.value_u256);

    let (net_mint_opt, onchain_delta_opt, discrepancy_opt, invariant_pass) = if decode_errors > 0 {
        (None, None, None, None)
    } else {
        match (supply_start, supply_end) {
            (Some(start), Some(end)) => {
                let (net_mint, onchain_delta, discrepancy, pass) =
                    compute_supply_invariant(sum_mints, sum_burns, start, end);
                (
                    Some(net_mint),
                    Some(onchain_delta),
                    Some(discrepancy),
                    Some(pass),
                )
            }
            _ => (None, None, None, None),
        }
    };

    SupplyMetrics {
        deduped,
        duplicate_count: dup_count,
        mint_count,
        burn_count,
        plain_transfer_count,
        active_senders: senders.len(),
        active_recipients: recipients.len(),
        sum_mints,
        sum_burns,
        net_mint: net_mint_opt,
        onchain_delta: onchain_delta_opt,
        discrepancy: discrepancy_opt,
        supply_invariant_pass: invariant_pass,
        no_duplicate_logs_pass: dup_count == 0,
        transfer_decode_pass: decode_errors == 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_supply_invariant_balanced() {
        let sum_mints = U256::from(100u64);
        let sum_burns = U256::from(20u64);
        let start = U256::from(1000u64);
        let end = U256::from(1080u64);
        let (_, _, disc, pass) = compute_supply_invariant(sum_mints, sum_burns, start, end);
        assert!(pass);
        assert_eq!(disc, I256::ZERO);
    }
}
