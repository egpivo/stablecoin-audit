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
}
