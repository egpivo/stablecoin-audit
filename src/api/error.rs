use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidPath,
    NotFound,
    ManifestNotFound,
    AmbiguousRunId,
    PackageNotFound,
    PackageCorrupt,
    ValidationError,
    IoError,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: ErrorCode,
    pub message: String,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: ErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn invalid_path(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: ErrorCode::InvalidPath,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: ErrorCode::NotFound,
            message: message.into(),
        }
    }

    pub fn manifest_not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: ErrorCode::ManifestNotFound,
            message: message.into(),
        }
    }

    pub fn package_not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: ErrorCode::PackageNotFound,
            message: message.into(),
        }
    }

    pub fn package_corrupt(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: ErrorCode::PackageCorrupt,
            message: message.into(),
        }
    }

    pub fn ambiguous_run_id(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: ErrorCode::AmbiguousRunId,
            message: message.into(),
        }
    }

    pub fn validation_error(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: ErrorCode::ValidationError,
            message: message.into(),
        }
    }

    pub fn io_error(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: ErrorCode::IoError,
            message: message.into(),
        }
    }

    pub fn from_anyhow(err: anyhow::Error) -> Self {
        Self::io_error(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.code,
            message: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        Self::io_error(err.to_string())
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
