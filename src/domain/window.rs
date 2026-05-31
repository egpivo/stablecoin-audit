//! Block window specifications (UTC and per-chain bounds).
//!
//! Parsing for `CHAIN:FROM:TO` remains in `rpc::transfer_audit` until v0.2+ migrates
//! window construction behind `application::workflow` without semantic changes.

/// Per-chain inclusive block window `[from_block, to_block]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerChainBlockWindow {
    pub chain: String,
    pub from_block: u64,
    pub to_block: u64,
}
