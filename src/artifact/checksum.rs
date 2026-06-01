//! SHA-256 checksums for artifact files referenced in manifests.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

const READ_BUF_SIZE: usize = 64 * 1024;

/// Stream `path` and return a lowercase hex SHA-256 digest of the file bytes.
pub fn sha256_file_hex(path: &Path) -> Result<String> {
    let file =
        File::open(path).with_context(|| format!("open file for checksum {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; READ_BUF_SIZE];
    loop {
        let n = reader
            .read(&mut buf)
            .with_context(|| format!("read file for checksum {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// SHA-256 digest of `bytes` as lowercase hex (no `0x` prefix).
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_empty_vector() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_file_hex_matches_in_memory() {
        let dir = std::env::temp_dir().join(format!("stablecoin_checksum_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("artifact.json");
        std::fs::write(&path, r#"{"ok":true}"#).unwrap();
        assert_eq!(
            sha256_file_hex(&path).unwrap(),
            sha256_hex(br#"{"ok":true}"#)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha256_file_hex_streams_large_file() {
        let dir =
            std::env::temp_dir().join(format!("stablecoin_checksum_large_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("large.bin");
        let chunk = vec![0xABu8; READ_BUF_SIZE + 1];
        let mut file = std::fs::File::create(&path).unwrap();
        for _ in 0..4 {
            std::io::Write::write_all(&mut file, &chunk).unwrap();
        }
        drop(file);
        let mut expected = Sha256::new();
        for _ in 0..4 {
            expected.update(&chunk);
        }
        let expected_hex: String = expected
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(sha256_file_hex(&path).unwrap(), expected_hex);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
