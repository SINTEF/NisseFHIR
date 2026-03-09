use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct OperationIssue {
    severity: &'static str,
    code: &'static str,
    diagnostics: String,
}

impl OperationIssue {
    pub fn error(code: &'static str, diagnostics: impl Into<String>) -> Self {
        Self {
            severity: "error",
            code,
            diagnostics: diagnostics.into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("validation failed")]
    Validation(Vec<OperationIssue>),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct OperationOutcomeBody {
    #[serde(rename = "resourceType")]
    resource_type: &'static str,
    issue: Vec<OperationIssue>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, issues) = match self {
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                vec![OperationIssue::error("login", "missing or invalid bearer token")],
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                vec![OperationIssue::error(
                    "forbidden",
                    "token does not grant access to this resource or interaction",
                )],
            ),
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                vec![OperationIssue::error("not-found", "requested resource was not found")],
            ),
            AppError::BadRequest(message) => (
                StatusCode::BAD_REQUEST,
                vec![OperationIssue::error("invalid", message)],
            ),
            AppError::Validation(issues) => (StatusCode::BAD_REQUEST, issues),
            AppError::Database(_) | AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                vec![OperationIssue::error(
                    "exception",
                    "internal server error",
                )],
            ),
        };

        let body = OperationOutcomeBody {
            resource_type: "OperationOutcome",
            issue: issues,
        };

        (status, Json(body)).into_response()
    }
}
