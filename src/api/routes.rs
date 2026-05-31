use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::artifact::ArtifactManifest;

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

pub fn router(store: ArtifactStore) -> Router {
    let state = AppState {
        store: Arc::new(store),
    };
    Router::new()
        .route("/health", get(health))
        .route("/api/runs", get(list_runs))
        .route("/api/runs/:run_id/manifest", get(get_manifest))
        .route("/api/runs/:run_id/artifacts", get(get_artifacts))
        .route("/api/artifacts/*artifact_path", get(serve_artifact))
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
