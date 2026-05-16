use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenConfig {
    pub asset: String,
    pub chain: String,
    pub chain_id: u64,
    pub contract_address: String,
    pub decimals: u8,
    pub issuer: String,
    pub form: String,
    pub rpc_url_env: String,
    pub deployment_block: Option<u64>,
    pub expected_interfaces: Vec<String>,
}

impl TokenConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn rpc_url(&self) -> Result<String> {
        std::env::var(&self.rpc_url_env)
            .with_context(|| format!("env var {} not set — add it to .env", self.rpc_url_env))
    }
}

pub fn load_single_token_config(asset: &str, chain: &str) -> Result<TokenConfig> {
    for (val, label) in [(&asset, "asset"), (&chain, "chain")] {
        if val.is_empty()
            || !val
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            anyhow::bail!("{label} identifier {:?} contains invalid characters", val);
        }
    }
    let path = Path::new("configs/tokens").join(format!(
        "{}.{}.yml",
        asset.to_lowercase(),
        chain.to_lowercase()
    ));
    TokenConfig::load(&path).with_context(|| format!("loading config {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_usdc_ethereum_config() {
        let cfg = load_single_token_config("USDC", "ethereum").unwrap();
        assert_eq!(cfg.chain, "ethereum");
        assert_eq!(cfg.chain_id, 1);
        assert_eq!(cfg.decimals, 6);
        assert!(cfg.contract_address.starts_with("0x"));
    }

    #[test]
    fn rejects_invalid_asset_chars() {
        assert!(load_single_token_config("../evil", "ethereum").is_err());
    }

    #[test]
    fn rejects_empty_chain() {
        assert!(load_single_token_config("USDC", "").is_err());
    }

    #[test]
    fn rpc_url_errors_when_env_missing() {
        let cfg = load_single_token_config("USDC", "ethereum").unwrap();
        let key = cfg.rpc_url_env.clone();
        let saved = std::env::var(&key).ok();
        std::env::remove_var(&key);
        assert!(cfg.rpc_url().is_err());
        if let Some(v) = saved {
            std::env::set_var(&key, v);
        }
    }
}
