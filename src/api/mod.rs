pub mod artifact_store;
pub mod error;
pub mod path_jail;
pub mod routes;
pub mod run_jobs;

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
    eprintln!("Evidence browser: http://{addr}/ui/");
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
                    contract_address: "0xabc".into(),
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
    async fn serves_evidence_browser_ui() {
        let app = router(test_store());
        let response = app
            .oneshot(Request::get("/ui/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Evidence Browser"));
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

    #[tokio::test]
    async fn download_package_returns_zip_with_headers() {
        let root = temp_api_root("pkg_download");
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_dl");
        let store = ArtifactStore::open(&root).unwrap();
        store.generate_package("pkg_dl", Some("USDC")).unwrap();
        let app = router(store);

        let response = app
            .oneshot(
                Request::get("/api/runs/pkg_dl/package/download?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/zip"
        );
        let disposition = response
            .headers()
            .get("content-disposition")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(disposition.contains("attachment"));
        assert!(disposition.contains("USDC_pkg_dl_stablecoin-map-package.zip"));
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(!body.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn download_package_fails_when_zip_missing() {
        let root = temp_api_root("pkg_dl_nozip");
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_nozip");
        let run_dir = root.join("usdc/runs/pkg_nozip");
        ArtifactStore::open(&root)
            .unwrap()
            .generate_package("pkg_nozip", Some("USDC"))
            .unwrap();
        std::fs::remove_file(run_dir.join(crate::artifact::PACKAGE_ZIP_FILENAME)).unwrap();

        let app = router(ArtifactStore::open(&root).unwrap());
        let response = app
            .oneshot(
                Request::get("/api/runs/pkg_nozip/package/download?asset=USDC")
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
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn download_package_fails_when_manifest_missing() {
        let root = temp_api_root("pkg_dl_noman");
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_noman");
        let run_dir = root.join("usdc/runs/pkg_noman");
        ArtifactStore::open(&root)
            .unwrap()
            .generate_package("pkg_noman", Some("USDC"))
            .unwrap();
        std::fs::remove_file(run_dir.join(crate::artifact::PACKAGE_MANIFEST_FILENAME)).unwrap();

        let app = router(ArtifactStore::open(&root).unwrap());
        let response = app
            .oneshot(
                Request::get("/api/runs/pkg_noman/package/download?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "package_corrupt");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn download_package_fails_when_manifest_invalid() {
        let root = temp_api_root("pkg_dl_badman");
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_badman");
        let run_dir = root.join("usdc/runs/pkg_badman");
        ArtifactStore::open(&root)
            .unwrap()
            .generate_package("pkg_badman", Some("USDC"))
            .unwrap();
        std::fs::write(
            run_dir.join(crate::artifact::PACKAGE_MANIFEST_FILENAME),
            r#"{"package_kind":"wrong"}"#,
        )
        .unwrap();

        let app = router(ArtifactStore::open(&root).unwrap());
        let response = app
            .oneshot(
                Request::get("/api/runs/pkg_badman/package/download?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "package_corrupt");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn verify_package_returns_valid_for_generated_package() {
        let root = temp_api_root("pkg_verify_ok");
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_vok");
        let store = ArtifactStore::open(&root).unwrap();
        store.generate_package("pkg_vok", Some("USDC")).unwrap();
        let app = router(store);

        let response = app
            .oneshot(
                Request::post("/api/runs/pkg_vok/package/verify?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let report: crate::artifact::PackageVerificationReport =
            serde_json::from_slice(&body).unwrap();
        assert!(report.package_valid);
        assert_eq!(report.run_id, "pkg_vok");
        assert_eq!(report.asset, "USDC");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn verify_package_invalid_when_checksum_mismatch() {
        let root = temp_api_root("pkg_verify_badcs");
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_vbad");
        let run_dir = root.join("usdc/runs/pkg_vbad");
        ArtifactStore::open(&root)
            .unwrap()
            .generate_package("pkg_vbad", Some("USDC"))
            .unwrap();
        let mut manifest = crate::artifact::load_package_manifest(&run_dir).unwrap();
        manifest.package_checksum_sha256 = "0".repeat(64);
        std::fs::write(
            run_dir.join(crate::artifact::PACKAGE_MANIFEST_FILENAME),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let app = router(ArtifactStore::open(&root).unwrap());
        let response = app
            .oneshot(
                Request::post("/api/runs/pkg_vbad/package/verify?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let report: crate::artifact::PackageVerificationReport =
            serde_json::from_slice(&body).unwrap();
        assert!(!report.package_valid);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn verify_package_invalid_when_artifact_checksum_mismatch() {
        let root = temp_api_root("pkg_verify_badart");
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_vart");
        let run_dir = root.join("usdc/runs/pkg_vart");
        ArtifactStore::open(&root)
            .unwrap()
            .generate_package("pkg_vart", Some("USDC"))
            .unwrap();
        let mut manifest = crate::artifact::load_package_manifest(&run_dir).unwrap();
        manifest.artifacts[0].checksum_sha256 = Some("0".repeat(64));
        std::fs::write(
            run_dir.join(crate::artifact::PACKAGE_MANIFEST_FILENAME),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let app = router(ArtifactStore::open(&root).unwrap());
        let response = app
            .oneshot(
                Request::post("/api/runs/pkg_vart/package/verify?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let report: crate::artifact::PackageVerificationReport =
            serde_json::from_slice(&body).unwrap();
        assert!(!report.package_valid);
        assert!(report.artifacts.iter().any(|a| !a.valid));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn verify_package_ignores_orphan_zip_entries() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        use zip::{ZipArchive, ZipWriter};

        let root = temp_api_root("pkg_verify_orphan");
        let _ = std::fs::remove_dir_all(&root);
        seed_cross_chain_run(&root, "pkg_orph");
        let run_dir = root.join("usdc/runs/pkg_orph");
        let manifest = ArtifactStore::open(&root)
            .unwrap()
            .generate_package("pkg_orph", Some("USDC"))
            .unwrap();
        let zip_path = run_dir.join(crate::artifact::PACKAGE_ZIP_FILENAME);

        let original = std::fs::read(&zip_path).unwrap();
        let mut rewritten = Vec::new();
        {
            let reader = std::io::Cursor::new(&original);
            let mut archive = ZipArchive::new(reader).unwrap();
            let mut writer = ZipWriter::new(std::io::Cursor::new(&mut rewritten));
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i).unwrap();
                let name = entry.name().to_string();
                writer
                    .start_file(name, SimpleFileOptions::default())
                    .unwrap();
                std::io::copy(&mut entry, &mut writer).unwrap();
            }
            writer
                .start_file("orphan.csv", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"orphan\n").unwrap();
            writer.finish().unwrap();
        }
        std::fs::write(&zip_path, rewritten).unwrap();

        let app = router(ArtifactStore::open(&root).unwrap());
        let response = app
            .oneshot(
                Request::post("/api/runs/pkg_orph/package/verify?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let report: crate::artifact::PackageVerificationReport =
            serde_json::from_slice(&body).unwrap();
        assert_eq!(report.artifacts.len(), manifest.artifacts.len());
        assert!(!report.artifacts.iter().any(|a| a.path == "orphan.csv"));
        assert!(report.artifacts.iter().all(|a| a.valid));
        assert!(!report.package_valid);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_create_run_rejects_from_block_zero() {
        use super::run_jobs::{validate_create_run, CreateRunRequest, WindowSpec};
        let err = validate_create_run(&CreateRunRequest {
            asset: "USDC".into(),
            run_id: "api_val_001".into(),
            window: WindowSpec {
                chain: "ethereum".into(),
                from_block: 0,
                to_block: 100,
            },
            fresh: true,
        })
        .unwrap_err();
        assert_eq!(err.code, super::ErrorCode::ValidationError);
        assert!(err.message.contains("from_block 0"));
    }

    #[test]
    fn validate_create_run_rejects_unknown_asset() {
        use super::run_jobs::{validate_create_run, CreateRunRequest, WindowSpec};
        let err = validate_create_run(&CreateRunRequest {
            asset: "FAKE".into(),
            run_id: "api_val_002".into(),
            window: WindowSpec {
                chain: "ethereum".into(),
                from_block: 100,
                to_block: 200,
            },
            fresh: false,
        })
        .unwrap_err();
        assert_eq!(err.code, super::ErrorCode::ValidationError);
    }

    #[tokio::test]
    async fn post_create_run_returns_accepted() {
        let root = temp_api_root("post_run");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let app = router(ArtifactStore::open(&root).unwrap());
        let body = serde_json::json!({
            "asset": "USDC",
            "run_id": format!("api_post_{}", std::process::id()),
            "window": { "chain": "ethereum", "from_block": 24000000, "to_block": 24001000 },
            "fresh": true
        });
        let response = app
            .oneshot(
                Request::post("/api/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn get_run_status_not_found_without_run() {
        let app = router(test_store());
        let response = app
            .oneshot(
                Request::get("/api/runs/no_such_run_xyz/status?asset=USDC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_run_logs_not_found_without_creating_run_dir() {
        let root = temp_api_root("logs_no_mkdir");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let app = router(ArtifactStore::open(&root).unwrap());
        let run_id = "no_such_run_logs_xyz";
        let response = app
            .oneshot(
                Request::get(format!("/api/runs/{run_id}/logs?asset=USDC"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let run_path = root.join("usdc/runs").join(run_id);
        assert!(
            !run_path.exists(),
            "logs for unknown run must not create {}",
            run_path.display()
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
