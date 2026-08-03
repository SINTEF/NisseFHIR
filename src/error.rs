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
    #[error("precondition failed: {0}")]
    PreconditionFailed(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("unsupported media type: {0}")]
    UnsupportedMediaType(String),
    #[error("not acceptable: {0}")]
    NotAcceptable(String),
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
                vec![OperationIssue::error(
                    "login",
                    "missing or invalid bearer token",
                )],
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
                vec![OperationIssue::error(
                    "not-found",
                    "requested resource was not found",
                )],
            ),
            AppError::BadRequest(message) => (
                StatusCode::BAD_REQUEST,
                vec![OperationIssue::error("invalid", message)],
            ),
            AppError::PreconditionFailed(message) => (
                StatusCode::PRECONDITION_FAILED,
                vec![OperationIssue::error("conflict", message)],
            ),
            AppError::Conflict(message) => (
                StatusCode::CONFLICT,
                vec![OperationIssue::error("conflict", message)],
            ),
            AppError::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                vec![OperationIssue::error(
                    "too-costly",
                    "request payload exceeds the maximum allowed size",
                )],
            ),
            AppError::UnsupportedMediaType(message) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                vec![OperationIssue::error("not-supported", message)],
            ),
            AppError::NotAcceptable(message) => (
                StatusCode::NOT_ACCEPTABLE,
                vec![OperationIssue::error("not-supported", message)],
            ),
            AppError::Validation(issues) => (StatusCode::BAD_REQUEST, issues),
            AppError::Database(_) | AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                vec![OperationIssue::error("exception", "internal server error")],
            ),
        };

        let body = OperationOutcomeBody {
            resource_type: "OperationOutcome",
            issue: issues,
        };

        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    use super::{AppError, OperationIssue};

    /// Extract status and body from an AppError response.
    fn error_response(err: AppError) -> (StatusCode, serde_json::Value) {
        let response = err.into_response();
        let status = response.status();
        // We can't easily read the body synchronously, so just check status.
        // For body checks we test via integration tests.
        (status, serde_json::Value::Null)
    }

    #[test]
    fn unauthorized_maps_to_401() {
        let (status, _) = error_response(AppError::Unauthorized);
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn forbidden_maps_to_403() {
        let (status, _) = error_response(AppError::Forbidden);
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn not_found_maps_to_404() {
        let (status, _) = error_response(AppError::NotFound);
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn bad_request_maps_to_400() {
        let (status, _) = error_response(AppError::BadRequest("test".to_owned()));
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn payload_too_large_maps_to_413() {
        let (status, _) = error_response(AppError::PayloadTooLarge);
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[test]
    fn unsupported_media_type_maps_to_415() {
        let (status, _) = error_response(AppError::UnsupportedMediaType("x".to_owned()));
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[test]
    fn not_acceptable_maps_to_406() {
        let (status, _) = error_response(AppError::NotAcceptable("x".to_owned()));
        assert_eq!(status, StatusCode::NOT_ACCEPTABLE);
    }

    #[test]
    fn precondition_failed_maps_to_412() {
        let (status, _) = error_response(AppError::PreconditionFailed("stale".to_owned()));
        assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    }

    #[test]
    fn conflict_maps_to_409() {
        let (status, _) = error_response(AppError::Conflict("duplicate".to_owned()));
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[test]
    fn validation_maps_to_400() {
        let issues = vec![OperationIssue::error("invalid", "bad field")];
        let (status, _) = error_response(AppError::Validation(issues));
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn internal_maps_to_500() {
        let (status, _) = error_response(AppError::Internal("boom".to_owned()));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn operation_issue_error_sets_severity() {
        let issue = OperationIssue::error("invalid", "test diagnostics");
        let json = serde_json::to_value(&issue).unwrap();
        assert_eq!(json["severity"], "error");
        assert_eq!(json["code"], "invalid");
        assert_eq!(json["diagnostics"], "test diagnostics");
    }
}
