use alloy::primitives::{Address, B256};
use alloy::providers::Provider;
use alloy::rpc::types::Filter;
use alloy::sol;
use alloy::sol_types::SolEvent;
use serde::{Deserialize, Serialize};

// ─── Event signatures ─────────────────────────────────────────────────────

sol! {
    event Blacklisted(address indexed account);
    event UnBlacklisted(address indexed account);
    event Pause();
    event Unpause();
    event MinterConfigured(address indexed minter, uint256 minterAllowedAmount);
    event MinterRemoved(address indexed oldMinter);
    event MasterMinterChanged(address indexed newMasterMaster);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event Upgraded(address indexed implementation);
}

/// All 9 known control-event topic0 hashes.
pub static KNOWN_SIGNATURES: &[B256] = &[
    Blacklisted::SIGNATURE_HASH,
    UnBlacklisted::SIGNATURE_HASH,
    Pause::SIGNATURE_HASH,
    Unpause::SIGNATURE_HASH,
    MinterConfigured::SIGNATURE_HASH,
    MinterRemoved::SIGNATURE_HASH,
    MasterMinterChanged::SIGNATURE_HASH,
    OwnershipTransferred::SIGNATURE_HASH,
    Upgraded::SIGNATURE_HASH,
];

// ─── Output record ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct ControlEventRecord {
    pub chain: String,
    pub block_number: u64,
    pub tx_hash: String,
    pub log_index: u64,
    pub event_name: String,
    pub args_json: String,
    pub decode_status: String,
}

// ─── Decode a single log ──────────────────────────────────────────────────

pub fn decode_control_log(log: &alloy::rpc::types::Log, chain: &str) -> ControlEventRecord {
    let block_number = log.block_number.unwrap_or(0);
    let tx_hash = log
        .transaction_hash
        .map(|h| format!("{h:#x}"))
        .unwrap_or_else(|| "0x".into());
    let log_index = log.log_index.unwrap_or(0);

    let topic0 = match log.inner.data.topics().first() {
        Some(t) => *t,
        None => {
            return ControlEventRecord {
                chain: chain.to_string(),
                block_number,
                tx_hash,
                log_index,
                event_name: "unknown".into(),
                args_json: "{}".into(),
                decode_status: "unknown_signature".into(),
            };
        }
    };

    // Helper: raw topics as hex object string (fallback for unknown)
    let raw_topics_json = || -> String {
        let topics: Vec<String> = log
            .inner
            .data
            .topics()
            .iter()
            .map(|t| format!("\"0x{t:x}\""))
            .collect();
        format!("{{\"topics\":[{}]}}", topics.join(","))
    };

    fn fmt_addr(a: Address) -> String {
        format!("{a:#x}")
    }

    macro_rules! try_decode {
        ($event_type:ty, $name:literal, $fmt:expr) => {{
            match <$event_type>::decode_log(&log.inner, true) {
                Ok(ev) => ControlEventRecord {
                    chain: chain.to_string(),
                    block_number,
                    tx_hash,
                    log_index,
                    event_name: $name.into(),
                    args_json: $fmt(ev.data),
                    decode_status: "decoded".into(),
                },
                Err(_) => ControlEventRecord {
                    chain: chain.to_string(),
                    block_number,
                    tx_hash,
                    log_index,
                    event_name: $name.into(),
                    args_json: "{}".into(),
                    decode_status: "decode_error".into(),
                },
            }
        }};
    }

    if topic0 == Blacklisted::SIGNATURE_HASH {
        try_decode!(Blacklisted, "Blacklisted", |ev: Blacklisted| {
            format!("{{\"account\":\"{}\"}}", fmt_addr(ev.account))
        })
    } else if topic0 == UnBlacklisted::SIGNATURE_HASH {
        try_decode!(UnBlacklisted, "UnBlacklisted", |ev: UnBlacklisted| {
            format!("{{\"account\":\"{}\"}}", fmt_addr(ev.account))
        })
    } else if topic0 == Pause::SIGNATURE_HASH {
        try_decode!(Pause, "Pause", |_ev: Pause| "{}".to_string())
    } else if topic0 == Unpause::SIGNATURE_HASH {
        try_decode!(Unpause, "Unpause", |_ev: Unpause| "{}".to_string())
    } else if topic0 == MinterConfigured::SIGNATURE_HASH {
        try_decode!(
            MinterConfigured,
            "MinterConfigured",
            |ev: MinterConfigured| {
                format!(
                    "{{\"minter\":\"{}\",\"allowed_amount\":\"{}\"}}",
                    fmt_addr(ev.minter),
                    ev.minterAllowedAmount
                )
            }
        )
    } else if topic0 == MinterRemoved::SIGNATURE_HASH {
        try_decode!(MinterRemoved, "MinterRemoved", |ev: MinterRemoved| {
            format!("{{\"old_minter\":\"{}\"}}", fmt_addr(ev.oldMinter))
        })
    } else if topic0 == MasterMinterChanged::SIGNATURE_HASH {
        try_decode!(
            MasterMinterChanged,
            "MasterMinterChanged",
            |ev: MasterMinterChanged| {
                format!(
                    "{{\"new_master_minter\":\"{}\"}}",
                    fmt_addr(ev.newMasterMaster)
                )
            }
        )
    } else if topic0 == OwnershipTransferred::SIGNATURE_HASH {
        try_decode!(
            OwnershipTransferred,
            "OwnershipTransferred",
            |ev: OwnershipTransferred| {
                format!(
                    "{{\"previous_owner\":\"{}\",\"new_owner\":\"{}\"}}",
                    fmt_addr(ev.previousOwner),
                    fmt_addr(ev.newOwner)
                )
            }
        )
    } else if topic0 == Upgraded::SIGNATURE_HASH {
        try_decode!(Upgraded, "Upgraded", |ev: Upgraded| {
            format!("{{\"implementation\":\"{}\"}}", fmt_addr(ev.implementation))
        })
    } else {
        ControlEventRecord {
            chain: chain.to_string(),
            block_number,
            tx_hash,
            log_index,
            event_name: "unknown".into(),
            args_json: raw_topics_json(),
            decode_status: "unknown_signature".into(),
        }
    }
}

// ─── Fetch control events for one chain/contract ─────────────────────────

pub async fn fetch_control_events<P, T>(
    provider: &P,
    contract_address: Address,
    from_block: u64,
    to_block: u64,
    chain: &str,
) -> (Vec<ControlEventRecord>, String)
where
    P: Provider<T>,
    T: alloy::transports::Transport + Clone,
{
    let filter = Filter::new()
        .address(contract_address)
        .event_signature(KNOWN_SIGNATURES.to_vec())
        .from_block(from_block)
        .to_block(to_block);

    let logs = match provider.get_logs(&filter).await {
        Ok(l) => l,
        Err(e) => {
            return (Vec::new(), format!("error: {e:#}"));
        }
    };

    let records: Vec<ControlEventRecord> = logs
        .iter()
        .map(|log| decode_control_log(log, chain))
        .collect();

    let status = if records.iter().any(|r| r.decode_status == "decode_error") {
        "partial"
    } else {
        "pass"
    };
    (records, status.into())
}
