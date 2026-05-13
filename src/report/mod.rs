use alloy::primitives::U256;
use anyhow::Result;
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
    fn ensure_out_dir_rejects_empty() {
        assert!(ensure_out_dir("").is_err());
    }
}
