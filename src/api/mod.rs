pub mod artifact_store;
pub mod error;
pub mod path_jail;
pub mod routes;

use std::net::SocketAddr;
use std::path::Path;

use anyhow::Context;

pub use artifact_store::ArtifactStore;
pub use error::{ApiError, ErrorCode};
pub use routes::router;

/// Bind and serve the read-only evidence API.
pub async fn serve(artifact_root: impl AsRef<Path>, host: &str, port: u16) -> anyhow::Result<()> {
    let store = ArtifactStore::open(artifact_root.as_ref()).map_err(|e| anyhow::anyhow!("{e}"))?;
    let app = router(store);
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .with_context(|| format!("invalid listen address {host}:{port}"))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    eprintln!("stablecoin-audit API listening on http://{addr}");
    axum::serve(listener, app).await.context("HTTP server")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{
        transfer_audit_manifest::{ManifestChainInput, TransferAuditManifestParams},
        write_transfer_audit_manifest,
    };
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn seed_transfer_audit_run(root: &std::path::Path, run_id: &str) {
        let run_dir = root.join(format!("usdc/runs/{run_id}"));
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("qa_report.json"), r#"{"asset":"USDC"}"#).unwrap();
        std::fs::write(
            run_dir.join("provenance.json"),
            r#"{"schema":"transfer-audit-provenance-v1"}"#,
        )
        .unwrap();
        std::fs::write(run_dir.join("supply_audit.md"), "# supply").unwrap();
        std::fs::write(run_dir.join("summary.md"), "# summary").unwrap();
        std::fs::write(run_dir.join("supply_audit.csv"), "chain\nethereum\n").unwrap();
        std::fs::write(run_dir.join("decoded_transfers.csv"), "chain\n").unwrap();
        write_transfer_audit_manifest(
            &run_dir,
            &TransferAuditManifestParams {
                asset: "USDC".into(),
                run_id: run_id.to_string(),
                generated_at: "2026-05-15T08:03:31.695921+00:00".into(),
                per_chain_spans: true,
                provenance_from_block: 100,
                provenance_to_block_requested: None,
                chains: vec![ManifestChainInput {
                    chain: "ethereum".into(),
                    from_block: 100,
                    to_block_requested: "200".into(),
                    window_start_rfc3339: Some("2026-05-01T00:00:00Z".into()),
                    window_end_rfc3339: Some("2026-05-08T00:00:00Z".into()),
                    errors: vec![],
                }],
                warnings: vec![],
            },
        )
        .unwrap();
    }

    fn test_store() -> ArtifactStore {
        let root = std::env::temp_dir().join(format!(
            "stablecoin_api_store_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        seed_transfer_audit_run(&root, "test_run");
        ArtifactStore::open(&root).unwrap()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = router(test_store());
        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn lists_runs_with_manifest() {
        let app = router(test_store());
        let response = app
            .oneshot(Request::get("/api/runs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["runs"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn returns_manifest_for_run() {
        let store = test_store();
        let expected = crate::artifact::sha256_file_hex(
            &store.root().join("usdc/runs/test_run/qa_report.json"),
        )
        .unwrap();
        let app = router(store);
        let uri = "/api/runs/test_run/manifest?asset=USDC";
        let response = app
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let m: crate::artifact::ArtifactManifest = serde_json::from_slice(&body).unwrap();
        assert_eq!(m.command, "transfer-audit");
        assert_eq!(m.run_id.as_deref(), Some("test_run"));
        let qa = m
            .artifacts
            .iter()
            .find(|a| a.path == "qa_report.json")
            .expect("qa_report.json in manifest");
        assert_eq!(qa.checksum_sha256.as_deref(), Some(expected.as_str()));
    }

    #[tokio::test]
    async fn run_artifacts_listing_includes_checksum() {
        let store = test_store();
        let expected = crate::artifact::sha256_file_hex(
            &store.root().join("usdc/runs/test_run/qa_report.json"),
        )
        .unwrap();
        let app = router(store);
        let response = app
            .oneshot(
                Request::get("/api/runs/test_run/artifacts?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let qa = v["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["path"].as_str().unwrap().ends_with("qa_report.json"))
            .expect("qa_report in artifact list");
        assert_eq!(qa["checksum_sha256"].as_str(), Some(expected.as_str()));
    }

    #[tokio::test]
    async fn serves_artifact_bytes() {
        let store = test_store();
        let app = router(store);
        let response = app
            .oneshot(
                Request::get("/api/artifacts/usdc/runs/test_run/qa_report.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    fn seed_cross_chain_run(root: &std::path::Path, run_id: &str) {
        let run_dir = root.join(format!("usdc/runs/{run_id}"));
        std::fs::create_dir_all(&run_dir).unwrap();
        seed_transfer_audit_run(root, run_id);
        std::fs::write(run_dir.join("cross_chain_summary.json"), r#"{"chains":[]}"#).unwrap();
        std::fs::write(run_dir.join("cross_chain_summary.md"), "# cc").unwrap();
        crate::artifact::upsert_cross_chain_summary_manifest(
            &run_dir,
            &crate::artifact::CrossChainSummaryManifestParams {
                completed_at: "2026-05-16T10:00:00+00:00".into(),
                warnings: vec![],
            },
        )
        .unwrap();
    }

    #[tokio::test]
    async fn post_package_generates_metadata() {
        let root = std::env::temp_dir().join(format!(
            "stablecoin_api_pkg_post_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_run");
        let store = ArtifactStore::open(&root).unwrap();
        let app = router(store);

        let response = app
            .oneshot(
                Request::post("/api/runs/pkg_run/package?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let pkg: crate::artifact::PackageManifest = serde_json::from_slice(&body).unwrap();
        assert_eq!(pkg.package_kind, crate::artifact::PACKAGE_KIND);
        assert_eq!(pkg.run_id, "pkg_run");
        assert_eq!(pkg.asset, "USDC");
        assert!(root
            .join("usdc/runs/pkg_run")
            .join(crate::artifact::PACKAGE_ZIP_FILENAME)
            .is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn get_package_returns_existing_metadata() {
        let root = std::env::temp_dir().join(format!(
            "stablecoin_api_pkg_get_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_get");
        let store = ArtifactStore::open(&root).unwrap();
        store.generate_package("pkg_get", Some("USDC")).unwrap();
        let app = router(store);

        let response = app
            .oneshot(
                Request::get("/api/runs/pkg_get/package?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let pkg: crate::artifact::PackageManifest = serde_json::from_slice(&body).unwrap();
        assert_eq!(pkg.run_id, "pkg_get");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn get_package_not_found_without_generation() {
        let store = test_store();
        let app = router(store);
        let response = app
            .oneshot(
                Request::get("/api/runs/test_run/package?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "package_not_found");
    }

    fn temp_api_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "stablecoin_api_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[tokio::test]
    async fn does_not_list_run_without_artifact_manifest() {
        let root = temp_api_root("no_manifest");
        let _ = std::fs::remove_dir_all(&root);
        let run_dir = root.join("usdc/runs/incomplete_run");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("qa_report.json"), r#"{"asset":"USDC"}"#).unwrap();
        std::fs::write(run_dir.join("supply_audit.csv"), "chain\n").unwrap();

        let store = ArtifactStore::open(&root).unwrap();
        assert!(!store
            .list_runs()
            .unwrap()
            .iter()
            .any(|r| r.run_id == "incomplete_run"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn does_not_list_run_with_invalid_manifest() {
        let root = temp_api_root("bad_manifest");
        let _ = std::fs::remove_dir_all(&root);
        let run_dir = root.join("usdc/runs/bad_manifest");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(
            run_dir.join("artifact_manifest.json"),
            r#"{"schema":"wrong","command":"transfer-audit"}"#,
        )
        .unwrap();

        let store = ArtifactStore::open(&root).unwrap();
        assert!(!store
            .list_runs()
            .unwrap()
            .iter()
            .any(|r| r.run_id == "bad_manifest"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn manifest_endpoint_requires_valid_artifact_manifest() {
        let root = temp_api_root("manifest_only");
        let _ = std::fs::remove_dir_all(&root);
        seed_transfer_audit_run(&root, "manifest_run");
        let app = router(ArtifactStore::open(&root).unwrap());

        let response = app
            .oneshot(
                Request::get("/api/runs/manifest_run/manifest?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let m: crate::artifact::ArtifactManifest = serde_json::from_slice(&body).unwrap();
        assert_eq!(m.schema, crate::artifact::SCHEMA);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn artifacts_endpoint_lists_manifest_entries_only() {
        let root = temp_api_root("artifacts_only");
        let _ = std::fs::remove_dir_all(&root);
        seed_transfer_audit_run(&root, "listed_only");
        std::fs::write(
            root.join("usdc/runs/listed_only/orphan.csv"),
            "not,in,manifest\n",
        )
        .unwrap();

        let app = router(ArtifactStore::open(&root).unwrap());
        let response = app
            .oneshot(
                Request::get("/api/runs/listed_only/artifacts?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let paths: Vec<&str> = v["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["path"].as_str().unwrap())
            .collect();
        assert!(!paths.iter().any(|p| p.ends_with("orphan.csv")));
        assert!(paths.iter().any(|p| p.ends_with("qa_report.json")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn post_package_fails_without_artifact_manifest() {
        let root = temp_api_root("pkg_no_manifest");
        let _ = std::fs::remove_dir_all(&root);
        let run_dir = root.join("usdc/runs/no_manifest_pkg");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("qa_report.json"), "{}").unwrap();

        let app = router(ArtifactStore::open(&root).unwrap());
        let response = app
            .oneshot(
                Request::post("/api/runs/no_manifest_pkg/package?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn post_package_fails_when_manifest_artifact_missing_on_disk() {
        let root = temp_api_root("pkg_missing_file");
        let _ = std::fs::remove_dir_all(&root);
        seed_transfer_audit_run(&root, "pkg_missing");
        std::fs::remove_file(root.join("usdc/runs/pkg_missing/qa_report.json")).unwrap();

        let app = router(ArtifactStore::open(&root).unwrap());
        let response = app
            .oneshot(
                Request::post("/api/runs/pkg_missing/package?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "not_found");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn package_preserves_artifact_manifest_refs_and_checksums() {
        let root = temp_api_root("pkg_preserve");
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_preserve");
        let store = ArtifactStore::open(&root).unwrap();
        let artifact_manifest = store.load_manifest("pkg_preserve", Some("USDC")).unwrap();
        let pkg = store
            .generate_package("pkg_preserve", Some("USDC"))
            .unwrap();
        let zip_path = root
            .join("usdc/runs/pkg_preserve")
            .join(crate::artifact::PACKAGE_ZIP_FILENAME);

        for included in &pkg.artifacts {
            let source = artifact_manifest
                .artifacts
                .iter()
                .find(|a| a.path == included.path)
                .expect("artifact listed in artifact_manifest.json");
            assert_eq!(included.kind, source.kind);
            assert_eq!(included.format, source.format);
            assert_eq!(included.checksum_sha256, source.checksum_sha256);
        }
        assert_eq!(
            pkg.package_checksum_sha256,
            crate::artifact::package_content_checksum(&zip_path).unwrap()
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
