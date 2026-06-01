use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tower_http::services::ServeDir;

use crate::artifact::{ArtifactManifest, PackageManifest, PackageVerificationReport};

use super::artifact_store::{ArtifactStore, RunArtifactsResponse, RunsResponse};
use super::error::ApiError;
use super::path_jail::{content_type_for_path, open_artifact_file};

const TOOLKIT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<ArtifactStore>,
}

#[derive(Debug, Deserialize)]
pub struct AssetQuery {
    pub asset: Option<String>,
}

fn ui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")
}

pub fn router(store: ArtifactStore) -> Router {
    let state = AppState {
        store: Arc::new(store),
    };
    let ui = ServeDir::new(ui_dir()).append_index_html_on_directories(true);
    Router::new()
        .route("/health", get(health))
        .route("/api/runs", get(list_runs))
        .route("/api/runs/:run_id/manifest", get(get_manifest))
        .route("/api/runs/:run_id/artifacts", get(get_artifacts))
        .route(
            "/api/runs/:run_id/package",
            get(get_package).post(create_package),
        )
        .route("/api/runs/:run_id/package/download", get(download_package))
        .route("/api/runs/:run_id/package/verify", post(verify_package))
        .route("/api/artifacts/*artifact_path", get(serve_artifact))
        .route("/ui", get(|| async { Redirect::permanent("/ui/") }))
        .nest_service("/ui/", ui)
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "toolkit_version": TOOLKIT_VERSION,
    }))
}

async fn list_runs(State(state): State<AppState>) -> Result<Json<RunsResponse>, ApiError> {
    let runs = state.store.list_runs()?;
    Ok(Json(RunsResponse { runs }))
}

async fn get_manifest(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<AssetQuery>,
) -> Result<Json<ArtifactManifest>, ApiError> {
    let manifest = state.store.load_manifest(&run_id, query.asset.as_deref())?;
    Ok(Json(manifest))
}

async fn get_artifacts(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<AssetQuery>,
) -> Result<Json<RunArtifactsResponse>, ApiError> {
    let (run_id, asset, artifacts) = state
        .store
        .list_run_artifacts(&run_id, query.asset.as_deref())?;
    Ok(Json(RunArtifactsResponse {
        run_id,
        asset,
        artifacts,
    }))
}

async fn create_package(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<AssetQuery>,
) -> Result<Json<PackageManifest>, ApiError> {
    let package = state
        .store
        .generate_package(&run_id, query.asset.as_deref())?;
    Ok(Json(package))
}

async fn get_package(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<AssetQuery>,
) -> Result<Json<PackageManifest>, ApiError> {
    let package = state.store.load_package(&run_id, query.asset.as_deref())?;
    Ok(Json(package))
}

async fn download_package(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<AssetQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let (manifest, bytes) = state
        .store
        .download_package(&run_id, query.asset.as_deref())?;
    let filename = crate::artifact::package_download_filename(&manifest);
    let disposition =
        header::HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(|e| ApiError::io_error(format!("invalid Content-Disposition: {e}")))?;
    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/zip"),
            ),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    ))
}

async fn verify_package(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<AssetQuery>,
) -> Result<Json<PackageVerificationReport>, ApiError> {
    let report = state
        .store
        .verify_package(&run_id, query.asset.as_deref())?;
    Ok(Json(report))
}

async fn serve_artifact(
    State(state): State<AppState>,
    Path(artifact_path): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let file_path = open_artifact_file(state.store.root(), &artifact_path)?;
    let bytes = std::fs::read(&file_path).map_err(|e| ApiError::not_found(e.to_string()))?;
    let content_type = content_type_for_path(&file_path);
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        bytes,
    ))
}
