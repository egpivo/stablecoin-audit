//! Named audit workflows (CLI commands map here; API v0.4 will enqueue the same names).

pub const TRANSFER_AUDIT: &str = "transfer-audit";
pub const CROSS_CHAIN_SUMMARY: &str = "cross-chain-summary";
pub const RESOLVE_WINDOW: &str = "resolve-window";
pub const METADATA: &str = "metadata";
pub const STABLECOIN_MAP_PACKAGE: &str = "stablecoin-map-package";

#[cfg(feature = "experimental")]
pub const FETCH: &str = "fetch";
#[cfg(feature = "experimental")]
pub const REPORT: &str = "report";
#[cfg(feature = "experimental")]
pub const CONTROL_AUDIT: &str = "control-audit";
#[cfg(feature = "experimental")]
pub const CONTROL_REPORT: &str = "control-report";
