use std::path::{Path, PathBuf};

use crate::artifact::{resolve_artifact_under_root, validate_relative_artifact_path};

use super::error::ApiError;

/// Open a file under `artifact_root` for read-only serving.
pub fn open_artifact_file(artifact_root: &Path, artifact_path: &str) -> Result<PathBuf, ApiError> {
    validate_relative_artifact_path(artifact_path)
        .map_err(|e| ApiError::invalid_path(e.to_string()))?;
    resolve_artifact_under_root(artifact_root, artifact_path, true)
        .map_err(|e| map_resolve_error(e, artifact_path))
}

fn map_resolve_error(err: anyhow::Error, path: &str) -> ApiError {
    let msg = err.to_string();
    if msg.contains("does not exist") {
        ApiError::not_found(format!("artifact not found: {path}"))
    } else if msg.contains("must refer to a file")
        || msg.contains("must not")
        || msg.contains("escapes")
        || msg.contains("invalid")
    {
        ApiError::invalid_path(msg)
    } else {
        ApiError::io_error(msg)
    }
}

pub fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => "application/json",
        Some("csv") => "text/csv",
        Some("md") => "text/markdown",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "stablecoin_api_jail_{}_{}_{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("usdc/runs/run_a")).unwrap();
        let mut f = std::fs::File::create(dir.join("usdc/runs/run_a/qa_report.json")).unwrap();
        writeln!(f, "{{}}").unwrap();
        dir
    }

    #[test]
    fn opens_valid_relative_path() {
        let root = fixture_root("opens_valid");
        let p = open_artifact_file(&root, "usdc/runs/run_a/qa_report.json").unwrap();
        assert!(p.is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_parent_segment() {
        let root = fixture_root("rejects_parent");
        assert!(open_artifact_file(&root, "../secret").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_absolute_path() {
        let root = fixture_root("rejects_absolute");
        let err = open_artifact_file(&root, "/etc/passwd").unwrap_err();
        assert_eq!(err.code, super::super::error::ErrorCode::InvalidPath);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_absolute_path_even_when_jailed_copy_exists() {
        let root = fixture_root("rejects_absolute_jailed");
        std::fs::create_dir_all(root.join("etc")).unwrap();
        std::fs::write(root.join("etc/passwd"), "not the real passwd").unwrap();
        let err = open_artifact_file(&root, "/etc/passwd").unwrap_err();
        assert_eq!(err.code, super::super::error::ErrorCode::InvalidPath);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_missing_file() {
        let root = fixture_root("rejects_missing");
        let err = open_artifact_file(&root, "usdc/runs/run_a/missing.json").unwrap_err();
        assert_eq!(err.code, super::super::error::ErrorCode::NotFound);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = fixture_root("rejects_symlink");
        let outside = std::env::temp_dir().join(format!(
            "stablecoin_api_outside_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&outside, "secret").unwrap();
        let link = root.join("usdc/runs/run_a/evil.json");
        symlink(&outside, &link).unwrap();
        let err = open_artifact_file(&root, "usdc/runs/run_a/evil.json").unwrap_err();
        assert_eq!(err.code, super::super::error::ErrorCode::InvalidPath);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_file(&outside);
    }
}
