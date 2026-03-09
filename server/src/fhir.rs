use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppState, auth::extract_access_context, capability::capability_statement, error::AppError,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/metadata", get(get_metadata))
        .route("/fhir/{resource_type}", post(create_resource))
        .route(
            "/fhir/{resource_type}/{id}",
            get(read_resource).put(update_resource),
        )
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

async fn get_metadata(State(state): State<AppState>) -> impl IntoResponse {
    Json(capability_statement(&state.fhir_base_url))
}

async fn create_resource(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(resource_type): Path<String>,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    if !access.can_write || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }

    let Json(mut body) = parse_json_payload(payload)?;
    validate_resource_payload(&resource_type, &mut body, None)?;
    let id = body
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::BadRequest("resource id is required".to_owned()))?;

    state.validator.validate_resource(&resource_type, &body)?;

    let stored = state
        .store
        .upsert(&access.tenant_id, &resource_type, id, body.clone())
        .await?;

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        "ETag",
        HeaderValue::from_str(&format!("W/\"{}\"", stored.version_id))
            .map_err(|e| AppError::Internal(format!("invalid ETag header: {e}")))?,
    );
    response_headers.insert(
        "Last-Modified",
        HeaderValue::from_str(&stored.last_updated.to_rfc3339())
            .map_err(|e| AppError::Internal(format!("invalid Last-Modified header: {e}")))?,
    );
    response_headers.insert(
        "Location",
        HeaderValue::from_str(&format!("/fhir/{resource_type}/{id}"))
            .map_err(|e| AppError::Internal(format!("invalid Location header: {e}")))?,
    );

    Ok((StatusCode::CREATED, response_headers, Json(stored.resource)).into_response())
}

async fn read_resource(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((resource_type, id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    if !access.can_read || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }

    let found = state
        .store
        .read(&access.tenant_id, &resource_type, &id)
        .await?;

    let resource = found.ok_or(AppError::NotFound)?;

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        "ETag",
        HeaderValue::from_str(&format!("W/\"{}\"", resource.version_id))
            .map_err(|e| AppError::Internal(format!("invalid ETag header: {e}")))?,
    );
    response_headers.insert(
        "Last-Modified",
        HeaderValue::from_str(&resource.last_updated.to_rfc3339())
            .map_err(|e| AppError::Internal(format!("invalid Last-Modified header: {e}")))?,
    );

    Ok((StatusCode::OK, response_headers, Json(resource.resource)).into_response())
}

async fn update_resource(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((resource_type, id)): Path<(String, String)>,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    if !access.can_write || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }

    let Json(mut body) = parse_json_payload(payload)?;
    validate_resource_payload(&resource_type, &mut body, Some(&id))?;
    state.validator.validate_resource(&resource_type, &body)?;

    let stored = state
        .store
        .upsert(&access.tenant_id, &resource_type, &id, body)
        .await?;

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        "ETag",
        HeaderValue::from_str(&format!("W/\"{}\"", stored.version_id))
            .map_err(|e| AppError::Internal(format!("invalid ETag header: {e}")))?,
    );
    response_headers.insert(
        "Last-Modified",
        HeaderValue::from_str(&stored.last_updated.to_rfc3339())
            .map_err(|e| AppError::Internal(format!("invalid Last-Modified header: {e}")))?,
    );

    Ok((StatusCode::OK, response_headers, Json(stored.resource)).into_response())
}

fn parse_json_payload(payload: Result<Json<Value>, JsonRejection>) -> Result<Json<Value>, AppError> {
    payload.map_err(|rejection| {
        AppError::BadRequest(format!("invalid JSON payload: {}", rejection.body_text()))
    })
}

fn validate_resource_payload(
    path_resource_type: &str,
    body: &mut Value,
    expected_id: Option<&str>,
) -> Result<(), AppError> {
    let object = body
        .as_object_mut()
        .ok_or_else(|| AppError::BadRequest("resource payload must be a JSON object".to_owned()))?;

    let resource_type = object
        .get("resourceType")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::BadRequest("resourceType is required".to_owned()))?;

    if !resource_type.eq_ignore_ascii_case(path_resource_type) {
        return Err(AppError::BadRequest(format!(
            "resourceType '{resource_type}' does not match path '{path_resource_type}'"
        )));
    }

    if let Some(id) = expected_id {
        if let Some(payload_id) = object.get("id").and_then(Value::as_str)
            && payload_id != id
        {
            return Err(AppError::BadRequest(
                "resource id in payload does not match URL id".to_owned(),
            ));
        }
        object.insert("id".to_owned(), Value::String(id.to_owned()));
    } else if object.get("id").and_then(Value::as_str).is_none() {
        object.insert("id".to_owned(), Value::String(Uuid::new_v4().to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    use super::validate_resource_payload;
    use crate::{
        AppState,
        auth::AuthConfig,
        build_router,
        store::PgStore,
        validation::FhirSchemaValidator,
    };

    #[test]
    fn generates_id_for_create() {
        let mut body = json!({"resourceType": "Patient"});
        validate_resource_payload("Patient", &mut body, None).expect("valid payload");
        assert!(body.get("id").and_then(|v| v.as_str()).is_some());
    }

    #[test]
    fn rejects_mismatched_type() {
        let mut body = json!({"resourceType": "Observation"});
        let err = validate_resource_payload("Patient", &mut body, None).expect_err("must fail");
        assert!(err.to_string().contains("does not match path"));
    }

    #[tokio::test]
    async fn schema_validation_returns_operation_outcome() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost/postgres")
            .expect("lazy pool should build");

        let app = build_router(AppState {
            store: PgStore::new(pool),
            auth: AuthConfig {
                jwt_secret: "secret".to_owned(),
                allow_unauthenticated: true,
            },
            fhir_base_url: "http://localhost:8080/fhir".to_owned(),
            validator: Arc::new(FhirSchemaValidator::new().expect("validator should load")),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/fhir/Patient")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"resourceType":"Patient","bogus":true}"#,
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let value: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert_eq!(value["resourceType"], "OperationOutcome");
        assert_eq!(value["issue"][0]["code"], "invalid");
    }

    #[tokio::test]
    async fn malformed_json_returns_operation_outcome() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost/postgres")
            .expect("lazy pool should build");

        let app = build_router(AppState {
            store: PgStore::new(pool),
            auth: AuthConfig {
                jwt_secret: "secret".to_owned(),
                allow_unauthenticated: true,
            },
            fhir_base_url: "http://localhost:8080/fhir".to_owned(),
            validator: Arc::new(FhirSchemaValidator::new().expect("validator should load")),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/fhir/Patient")
                    .header("content-type", "application/json")
                    .body(Body::from("{"))
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let value: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert_eq!(value["resourceType"], "OperationOutcome");
        assert!(value["issue"][0]["diagnostics"]
            .as_str()
            .expect("diagnostics should be present")
            .contains("invalid JSON payload"));
    }
}
