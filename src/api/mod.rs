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
        write_artifact_manifest, ArtifactFormat, ArtifactKind, ArtifactManifest, ArtifactRef,
    };
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn sample_manifest(run_id: &str) -> ArtifactManifest {
        ArtifactManifest {
            run_id: Some(run_id.to_string()),
            asset: Some("USDC".into()),
            artifacts: vec![ArtifactRef {
                kind: ArtifactKind::QaReport,
                path: "qa_report.json".into(),
                format: ArtifactFormat::Json,
                row_count: None,
                checksum_sha256: None,
                description: "QA".into(),
            }],
            ..ArtifactManifest::new("transfer-audit", "0.1.0")
        }
    }

    fn test_store() -> ArtifactStore {
        let root =
            std::env::temp_dir().join(format!("stablecoin_api_store_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let run_dir = root.join("usdc/runs/test_run");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("qa_report.json"), "{}").unwrap();
        write_artifact_manifest(&run_dir, &sample_manifest("test_run")).unwrap();
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
        let app = router(test_store());
        let uri = "/api/runs/test_run/manifest?asset=USDC";
        let response = app
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
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
