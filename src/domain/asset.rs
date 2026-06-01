use anyhow::Result;

/// Validate CLI path segments: asset, chain, run-id style identifiers.
pub fn validate_identifier(value: &str, flag: &str) -> Result<()> {
    if !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        anyhow::bail!(
            "{flag} must contain only letters, digits, hyphens, and underscores; got {:?}",
            value
        )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_identifier;

    #[test]
    fn accepts_usdc() {
        validate_identifier("USDC", "--asset").unwrap();
    }

    #[test]
    fn rejects_slash() {
        assert!(validate_identifier("a/b", "--asset").is_err());
    }
}
