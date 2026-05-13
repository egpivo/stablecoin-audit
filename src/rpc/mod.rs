pub mod metadata;
#[cfg(feature = "experimental")]
pub mod fetch_logs;
#[cfg(feature = "experimental")]
pub mod report_cmd;

use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use anyhow::Result;

pub type HttpProvider = RootProvider<Http<Client>>;

pub fn build_provider(rpc_url: &str) -> Result<HttpProvider> {
    let url: alloy::transports::http::reqwest::Url = rpc_url.parse()?;
    Ok(ProviderBuilder::new().on_http(url))
}
