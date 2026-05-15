pub mod metadata;
pub mod resolve_window;
pub mod transfer_audit;
pub mod transfer_checkpoint;
#[cfg(feature = "experimental")]
pub mod fetch_logs;
#[cfg(feature = "experimental")]
pub mod report_cmd;
pub mod cross_chain_summary;
#[cfg(feature = "experimental")]
pub mod control_audit;
#[cfg(feature = "experimental")]
pub mod control_report_cmd;

use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use anyhow::Result;

pub type HttpProvider = RootProvider<Http<Client>>;

pub fn build_provider(rpc_url: &str) -> Result<HttpProvider> {
    let url: alloy::transports::http::reqwest::Url = rpc_url.parse()?;
    Ok(ProviderBuilder::new().on_http(url))
}
