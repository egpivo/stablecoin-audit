use alloy::primitives::{Address, U256};
use alloy::rpc::types::Log;
use alloy::sol;
use alloy::sol_types::SolEvent;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferEvent {
    pub chain: String,
    pub contract_address: String,
    pub block_number: u64,
    pub tx_hash: String,
    pub log_index: u64,
    pub from: String,
    pub to: String,
    pub value_raw: String,        // U256 as decimal string
    pub value_decimal: String,    // formatted with decimals
    pub kind: String,             // "mint", "burn", "transfer"
    #[serde(skip)]
    pub value_u256: U256,         // raw value for arithmetic; not written to CSV
}

pub fn decode_transfer_log(
    log: &Log,
    chain: &str,
    contract_address: &str,
    decimals: u8,
) -> Result<TransferEvent> {
    let decoded = Transfer::decode_log(&log.inner, true).with_context(|| {
        format!(
            "decode Transfer log tx={} log_index={}",
            log.transaction_hash
                .map(|h| h.to_string())
                .unwrap_or_default(),
            log.log_index.unwrap_or(0),
        )
    })?;

    let from = format!("{:#x}", decoded.from);
    let to = format!("{:#x}", decoded.to);
    let value: U256 = decoded.value;

    let kind = if decoded.from == Address::ZERO {
        "mint"
    } else if decoded.to == Address::ZERO {
        "burn"
    } else {
        "transfer"
    };

    let value_decimal = format_token_amount(value, decimals);

    Ok(TransferEvent {
        chain: chain.to_string(),
        contract_address: contract_address.to_string(),
        block_number: log.block_number.unwrap_or(0),
        tx_hash: log
            .transaction_hash
            .map(|h| format!("{h:#x}"))
            .unwrap_or_default(),
        log_index: log.log_index.unwrap_or(0),
        from,
        to,
        value_raw: value.to_string(),
        value_decimal,
        kind: kind.to_string(),
        value_u256: value,
    })
}

fn format_token_amount(raw: U256, decimals: u8) -> String {
    if decimals == 0 {
        return raw.to_string();
    }
    let divisor = U256::from(10u64).pow(U256::from(decimals));
    let whole = raw / divisor;
    let frac = raw % divisor;
    let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
    format!("{}.{}", whole, frac_str)
}

/// Deduplicate logs by (chain, contract_address, tx_hash, log_index).
/// Returns (deduped, duplicate_count).
pub fn dedup_transfer_events(events: Vec<TransferEvent>) -> (Vec<TransferEvent>, usize) {
    use std::collections::HashSet;
    let mut seen: HashSet<(String, String, String, u64)> = HashSet::new();
    let mut deduped = Vec::with_capacity(events.len());
    let mut dup_count = 0usize;

    for ev in events {
        let key = (
            ev.chain.clone(),
            ev.contract_address.clone(),
            ev.tx_hash.clone(),
            ev.log_index,
        );
        if seen.insert(key) {
            deduped.push(ev);
        } else {
            dup_count += 1;
        }
    }

    (deduped, dup_count)
}

/// QA: attempt to decode a sample of raw logs. Returns (sample_size, fail_count, errors).
pub fn sample_decode_qa(
    logs: &[Log],
    chain: &str,
    contract_address: &str,
    decimals: u8,
    sample_size: usize,
) -> (usize, usize, Vec<String>) {
    if sample_size == 0 {
        return (0, 0, Vec::new());
    }
    let sample: Vec<&Log> = if logs.len() <= sample_size {
        logs.iter().collect()
    } else {
        // Take evenly spaced samples
        let step = logs.len() / sample_size;
        logs.iter().step_by(step).take(sample_size).collect()
    };

    let n = sample.len();
    let mut fail_count = 0usize;
    let mut errors = Vec::new();

    for log in sample {
        if let Err(e) = decode_transfer_log(log, chain, contract_address, decimals) {
            fail_count += 1;
            errors.push(format!("{e:#}"));
        }
    }

    (n, fail_count, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U256;

    fn make_event(tx_hash: &str, log_index: u64) -> TransferEvent {
        make_event_with_contract("0xcontract", tx_hash, log_index)
    }

    fn make_event_with_contract(contract: &str, tx_hash: &str, log_index: u64) -> TransferEvent {
        TransferEvent {
            chain: "ethereum".into(),
            contract_address: contract.into(),
            block_number: 1,
            tx_hash: tx_hash.into(),
            log_index,
            from: "0x0000000000000000000000000000000000000000".into(),
            to: "0xabcd".into(),
            value_raw: "1000000".into(),
            value_decimal: "1.000000".into(),
            kind: "mint".into(),
            value_u256: U256::from(1_000_000u64),
        }
    }

    #[test]
    fn dedup_same_tx_same_log_index_distinct_contract_kept() {
        let events = vec![
            make_event_with_contract("0xc1", "0xaa", 0),
            make_event_with_contract("0xc2", "0xaa", 0),
        ];
        let (deduped, dups) = dedup_transfer_events(events);
        assert_eq!(deduped.len(), 2);
        assert_eq!(dups, 0);
    }

    #[test]
    fn dedup_empty_input() {
        let (deduped, dups) = dedup_transfer_events(vec![]);
        assert_eq!(deduped.len(), 0);
        assert_eq!(dups, 0);
    }

    #[test]
    fn dedup_no_duplicates() {
        let events = vec![make_event("0xaa", 0), make_event("0xbb", 0)];
        let (deduped, dups) = dedup_transfer_events(events);
        assert_eq!(deduped.len(), 2);
        assert_eq!(dups, 0);
    }

    #[test]
    fn dedup_removes_exact_duplicate() {
        let events = vec![make_event("0xaa", 0), make_event("0xaa", 0), make_event("0xbb", 1)];
        let (deduped, dups) = dedup_transfer_events(events);
        assert_eq!(deduped.len(), 2);
        assert_eq!(dups, 1);
    }

    #[test]
    fn dedup_same_tx_different_log_index_kept() {
        let events = vec![make_event("0xaa", 0), make_event("0xaa", 1)];
        let (deduped, dups) = dedup_transfer_events(events);
        assert_eq!(deduped.len(), 2);
        assert_eq!(dups, 0);
    }

    #[test]
    fn sample_qa_zero_sample_size_does_not_panic() {
        // Previously: logs.len() / 0 would panic when logs is non-empty.
        let result = sample_decode_qa(&[], "ethereum", "0x0", 6, 0);
        assert_eq!(result, (0, 0, vec![]));
    }
}
