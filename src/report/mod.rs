use alloy::primitives::U256;
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::Path;

pub fn ensure_out_dir(asset: &str) -> Result<std::path::PathBuf> {
    if asset.is_empty()
        || !asset
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!("asset identifier {:?} contains invalid characters", asset);
    }
    let dir = Path::new("out").join(asset.to_lowercase());
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// `run_id` may appear in paths; restrict to safe single-segment directory names.
pub fn validate_run_id(run_id: &str) -> Result<()> {
    if run_id.is_empty() {
        anyhow::bail!("run_id must not be empty");
    }
    if run_id == "." || run_id == ".." {
        anyhow::bail!("run_id must not be '.' or '..'");
    }
    if !run_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "run_id must contain only letters, digits, hyphens, and underscores; got {:?}",
            run_id
        );
    }
    Ok(())
}

/// Default run id: sortable UTC stamp with milliseconds (filesystem-safe).
pub fn default_run_id() -> String {
    let now = Utc::now();
    format!(
        "{}_{:03}Z",
        now.format("%Y%m%dT%H%M%S"),
        now.timestamp_subsec_millis()
    )
}

/// Resolved run directory: `out/<asset>/runs/<run_id>/`.
pub fn ensure_run_out_dir(asset: &str, run_id: &str) -> Result<std::path::PathBuf> {
    validate_run_id(run_id)?;
    let base = ensure_out_dir(asset)?;
    let dir = base.join("runs").join(run_id);
    std::fs::create_dir_all(&dir).with_context(|| format!("create run dir {}", dir.display()))?;
    Ok(dir)
}

pub fn format_token_amount(raw: U256, decimals: u8) -> String {
    if decimals == 0 {
        return raw.to_string();
    }
    let divisor = U256::from(10u64).pow(U256::from(decimals));
    let whole = raw / divisor;
    let frac = raw % divisor;
    let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
    format!("{}.{}", whole, frac_str)
}

/// Format a token amount with comma-separated thousands in the whole part.
pub fn format_token_amount_pretty(raw: U256, decimals: u8, symbol: &str) -> String {
    let base = format_token_amount(raw, decimals);
    let (whole, frac) = if let Some(pos) = base.find('.') {
        (&base[..pos], &base[pos..])
    } else {
        (base.as_str(), "")
    };
    // Insert thousands separators
    let digits: Vec<char> = whole.chars().collect();
    let mut result = String::new();
    let len = digits.len();
    for (i, ch) in digits.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(*ch);
    }
    result.push_str(frac);
    if symbol.is_empty() {
        result
    } else {
        format!("{} {}", result, symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U256;

    #[test]
    fn format_amount_zero_decimals() {
        assert_eq!(format_token_amount(U256::from(42u64), 0), "42");
    }

    #[test]
    fn format_amount_six_decimals_zero() {
        assert_eq!(format_token_amount(U256::ZERO, 6), "0.000000");
    }

    #[test]
    fn format_amount_six_decimals_whole() {
        // 1 USDC = 1_000_000 base units
        assert_eq!(format_token_amount(U256::from(1_000_000u64), 6), "1.000000");
    }

    #[test]
    fn format_amount_six_decimals_fractional() {
        // 1.5 USDC
        assert_eq!(format_token_amount(U256::from(1_500_000u64), 6), "1.500000");
    }

    #[test]
    fn format_amount_leading_frac_zeros() {
        // 0.000001 USDC (1 base unit)
        assert_eq!(format_token_amount(U256::from(1u64), 6), "0.000001");
    }

    #[test]
    fn format_pretty_thousands_with_symbol() {
        // 1_234_567.000000 USDC
        let raw = U256::from(1_234_567_000_000u64);
        assert_eq!(format_token_amount_pretty(raw, 6, "USDC"), "1,234,567.000000 USDC");
    }

    #[test]
    fn format_pretty_no_symbol() {
        let raw = U256::from(1_000_000u64);
        assert_eq!(format_token_amount_pretty(raw, 6, ""), "1.000000");
    }

    #[test]
    fn ensure_out_dir_rejects_dotdot() {
        assert!(ensure_out_dir("../evil").is_err());
    }

    #[test]
    fn validate_run_id_accepts_stamp() {
        validate_run_id("20260513T120000_001Z").unwrap();
    }

    #[test]
    fn validate_run_id_rejects_slash() {
        assert!(validate_run_id("a/b").is_err());
    }

    #[test]
    fn ensure_out_dir_rejects_empty() {
        assert!(ensure_out_dir("").is_err());
    }

    #[test]
    fn ensure_run_out_dir_creates_nested_path() {
        let run_id = format!("test_run_{}", std::process::id());
        let dir = ensure_run_out_dir("USDC", &run_id).unwrap();
        assert!(dir.ends_with(format!("out/usdc/runs/{run_id}")));
        assert!(dir.is_dir());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn default_run_id_is_filesystem_safe() {
        let id = default_run_id();
        validate_run_id(&id).unwrap();
        assert!(id.contains('T'));
    }

    #[test]
    fn validate_run_id_rejects_dot() {
        assert!(validate_run_id(".").is_err());
    }
}