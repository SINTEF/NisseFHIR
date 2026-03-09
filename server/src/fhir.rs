use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use json_patch::patch as apply_json_patch;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use url::form_urlencoded::Serializer;
use uuid::Uuid;

use crate::{
    AppState, auth::extract_access_context, capability::capability_statement, error::AppError,
    store::SearchFilter,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/metadata", get(get_metadata))
        .route(
            "/fhir/{resource_type}",
            get(search_resources).post(create_resource),
        )
        .route(
            "/fhir/{resource_type}/{id}",
            get(read_resource)
                .put(update_resource)
                .patch(patch_resource)
                .delete(delete_resource),
        )
}

const DEFAULT_SEARCH_COUNT: u32 = 20;
const MAX_SEARCH_COUNT: u32 = 100;

#[derive(Debug)]
pub struct ParsedSearchParams {
    count: u32,
    offset: u32,
    filters: Vec<SearchFilter>,
    canonical_filters: Vec<(String, String)>,
}

#[utoipa::path(get, path = "/healthz", responses((status = 200, description = "Server is healthy")))]
pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

#[utoipa::path(get, path = "/metadata", responses((status = 200, description = "FHIR CapabilityStatement")))]
pub async fn get_metadata(State(state): State<AppState>) -> impl IntoResponse {
    Json(capability_statement(&state.fhir_base_url))
}

#[utoipa::path(get, path = "/fhir/{resource_type}",
    params(("resource_type" = String, Path, description = "FHIR resource type")),
    responses((status = 200, description = "Search results Bundle"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden")),
    security(("bearer_auth" = [])))]
pub async fn search_resources(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(resource_type): Path<String>,
    Query(query): Query<BTreeMap<String, String>>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    if !access.can_read || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }

    let params = parse_search_params(&resource_type, query)?;
    let results = state
        .store
        .search(
            &access.tenant_id,
            &resource_type,
            &params.filters,
            i64::from(params.count),
            i64::from(params.offset),
        )
        .await?;

    let response = Json(build_search_bundle(
        &state.fhir_base_url,
        &resource_type,
        params.count,
        params.offset,
        results.total,
        results.resources,
        &params.canonical_filters,
    ));

    Ok((StatusCode::OK, response).into_response())
}

#[utoipa::path(post, path = "/fhir/{resource_type}",
    params(("resource_type" = String, Path, description = "FHIR resource type")),
    responses((status = 201, description = "Resource created"), (status = 400, description = "Validation error"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 413, description = "Payload too large")),
    security(("bearer_auth" = [])))]
pub async fn create_resource(
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

#[utoipa::path(get, path = "/fhir/{resource_type}/{id}",
    params(("resource_type" = String, Path, description = "FHIR resource type"), ("id" = String, Path, description = "Resource ID")),
    responses((status = 200, description = "Resource found"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 404, description = "Not found")),
    security(("bearer_auth" = [])))]
pub async fn read_resource(
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

#[utoipa::path(put, path = "/fhir/{resource_type}/{id}",
    params(("resource_type" = String, Path, description = "FHIR resource type"), ("id" = String, Path, description = "Resource ID")),
    responses((status = 200, description = "Resource updated"), (status = 400, description = "Validation error"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 413, description = "Payload too large")),
    security(("bearer_auth" = [])))]
pub async fn update_resource(
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

#[utoipa::path(delete, path = "/fhir/{resource_type}/{id}",
    params(("resource_type" = String, Path, description = "FHIR resource type"), ("id" = String, Path, description = "Resource ID")),
    responses((status = 204, description = "Resource deleted"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 404, description = "Not found")),
    security(("bearer_auth" = [])))]
pub async fn delete_resource(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((resource_type, id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    if !access.can_write || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }

    let deleted = state
        .store
        .delete(&access.tenant_id, &resource_type, &id)
        .await?;

    if !deleted {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT.into_response())
}

#[utoipa::path(patch, path = "/fhir/{resource_type}/{id}",
    params(("resource_type" = String, Path, description = "FHIR resource type"), ("id" = String, Path, description = "Resource ID")),
    responses((status = 200, description = "Resource patched"), (status = 400, description = "Invalid patch"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 404, description = "Not found")),
    security(("bearer_auth" = [])))]
pub async fn patch_resource(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((resource_type, id)): Path<(String, String)>,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    if !access.can_write || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }

    let Json(patch_body) = parse_json_payload(payload)?;
    let patch_ops: json_patch::Patch = serde_json::from_value(patch_body)
        .map_err(|e| AppError::BadRequest(format!("invalid JSON Patch document: {e}")))?;

    let found = state
        .store
        .read(&access.tenant_id, &resource_type, &id)
        .await?;

    let existing = found.ok_or(AppError::NotFound)?;
    let mut resource = existing.resource;

    apply_json_patch(&mut resource, &patch_ops)
        .map_err(|e| AppError::BadRequest(format!("JSON Patch failed: {e}")))?;

    // Ensure resourceType and id are not altered by the patch
    if resource.get("resourceType").and_then(Value::as_str) != Some(&resource_type) {
        return Err(AppError::BadRequest(
            "patch must not change resourceType".to_owned(),
        ));
    }
    if resource.get("id").and_then(Value::as_str) != Some(id.as_str()) {
        return Err(AppError::BadRequest(
            "patch must not change resource id".to_owned(),
        ));
    }

    state
        .validator
        .validate_resource(&resource_type, &resource)?;

    let stored = state
        .store
        .upsert(&access.tenant_id, &resource_type, &id, resource)
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

fn parse_json_payload(
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Json<Value>, AppError> {
    payload.map_err(|rejection| {
        let message = rejection.body_text();
        if message.contains("length limit") || message.contains("payload too large") {
            AppError::PayloadTooLarge
        } else {
            AppError::BadRequest(format!("invalid JSON payload: {message}"))
        }
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

fn build_search_bundle(
    base_url: &str,
    resource_type: &str,
    count: u32,
    offset: u32,
    total: i64,
    resources: Vec<crate::store::StoredResource>,
    filters: &[(String, String)],
) -> Value {
    let base_url = base_url.trim_end_matches('/');
    let search_url = build_search_url(base_url, resource_type, count, offset, filters);
    let next_offset = offset.saturating_add(count);
    let has_next = i64::from(next_offset) < total;

    let mut links = vec![json!({
        "relation": "self",
        "url": search_url,
    })];

    if has_next {
        links.push(json!({
            "relation": "next",
            "url": build_search_url(base_url, resource_type, count, next_offset, filters),
        }));
    }

    let entry = resources
        .into_iter()
        .map(|stored| {
            let resource_id = stored
                .resource
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();

            json!({
                "fullUrl": format!("{base_url}/{resource_type}/{resource_id}"),
                "resource": stored.resource,
                "search": {
                    "mode": "match"
                }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "resourceType": "Bundle",
        "type": "searchset",
        "total": total,
        "link": links,
        "entry": entry,
    })
}

fn parse_search_params(
    resource_type: &str,
    query: BTreeMap<String, String>,
) -> Result<ParsedSearchParams, AppError> {
    let mut count = DEFAULT_SEARCH_COUNT;
    let mut offset = 0;
    let mut filters = Vec::new();
    let mut canonical_filters = Vec::new();

    for (key, value) in query {
        match key.as_str() {
            "_count" => {
                count = parse_u32_query_param("_count", &value)?;
                if count > MAX_SEARCH_COUNT {
                    return Err(AppError::BadRequest(format!(
                        "_count must be less than or equal to {MAX_SEARCH_COUNT}"
                    )));
                }
            }
            "_offset" => {
                offset = parse_u32_query_param("_offset", &value)?;
            }
            "name" if resource_type.eq_ignore_ascii_case("Patient") => {
                filters.push(SearchFilter::PatientName(value.clone()));
                canonical_filters.push((key, value));
            }
            "birthdate" if resource_type.eq_ignore_ascii_case("Patient") => {
                filters.push(SearchFilter::PatientBirthDate(value.clone()));
                canonical_filters.push((key, value));
            }
            "identifier" if resource_type.eq_ignore_ascii_case("Patient") => {
                let (system, identifier_value) = parse_identifier_filter(&value)?;
                filters.push(SearchFilter::PatientIdentifier {
                    system,
                    value: identifier_value,
                });
                canonical_filters.push((key, value));
            }
            "code" if resource_type.eq_ignore_ascii_case("Observation") => {
                filters.push(SearchFilter::ObservationCode(value.clone()));
                canonical_filters.push((key, value));
            }
            "status" if resource_type.eq_ignore_ascii_case("Observation") => {
                filters.push(SearchFilter::ObservationStatus(value.clone()));
                canonical_filters.push((key, value));
            }
            "subject" if resource_type.eq_ignore_ascii_case("Observation") => {
                filters.push(SearchFilter::ObservationSubject(value.clone()));
                canonical_filters.push((key, value));
            }
            _ => {
                return Err(AppError::BadRequest(format!(
                    "unsupported search parameter '{key}' for resource type '{resource_type}'"
                )));
            }
        }
    }

    Ok(ParsedSearchParams {
        count,
        offset,
        filters,
        canonical_filters,
    })
}

fn parse_u32_query_param(name: &str, value: &str) -> Result<u32, AppError> {
    value
        .parse::<u32>()
        .map_err(|_| AppError::BadRequest(format!("{name} must be an unsigned integer")))
}

fn parse_identifier_filter(value: &str) -> Result<(Option<String>, String), AppError> {
    if let Some((system, identifier_value)) = value.split_once('|') {
        if system.is_empty() || identifier_value.is_empty() {
            return Err(AppError::BadRequest(
                "identifier must be 'value' or 'system|value'".to_owned(),
            ));
        }

        return Ok((Some(system.to_owned()), identifier_value.to_owned()));
    }

    if value.is_empty() {
        return Err(AppError::BadRequest(
            "identifier must be 'value' or 'system|value'".to_owned(),
        ));
    }

    Ok((None, value.to_owned()))
}

fn build_search_url(
    base_url: &str,
    resource_type: &str,
    count: u32,
    offset: u32,
    filters: &[(String, String)],
) -> String {
    let mut serializer = Serializer::new(String::new());
    serializer.append_pair("_count", &count.to_string());
    serializer.append_pair("_offset", &offset.to_string());

    for (key, value) in filters {
        serializer.append_pair(key, value);
    }

    format!("{base_url}/{resource_type}?{}", serializer.finish())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    use super::{
        build_search_bundle, parse_identifier_filter, parse_search_params,
        validate_resource_payload,
    };
    use crate::{
        AppState,
        auth::AuthConfig,
        build_router,
        error::AppError,
        store::{PgStore, StoredResource},
        validation::FhirSchemaValidator,
    };
    use chrono::Utc;

    const TEST_SECRET: &str = "0123456789abcdef0123456789abcdef";

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

    #[test]
    fn search_bundle_contains_self_and_next_links() {
        let bundle = build_search_bundle(
            "http://localhost:8080/fhir",
            "Patient",
            1,
            0,
            2,
            vec![StoredResource {
                version_id: 1,
                last_updated: Utc::now(),
                resource: json!({
                    "resourceType": "Patient",
                    "id": "example"
                }),
            }],
            &[],
        );

        assert_eq!(bundle["resourceType"], "Bundle");
        assert_eq!(bundle["type"], "searchset");
        assert_eq!(bundle["total"], 2);
        assert_eq!(bundle["link"][0]["relation"], "self");
        assert_eq!(bundle["link"][1]["relation"], "next");
        assert_eq!(bundle["entry"][0]["resource"]["id"], "example");
    }

    #[test]
    fn parse_search_params_builds_patient_filters() {
        let params = parse_search_params(
            "Patient",
            BTreeMap::from([
                ("_count".to_owned(), "5".to_owned()),
                ("name".to_owned(), "peter".to_owned()),
                (
                    "identifier".to_owned(),
                    "urn:oid:1.2.36.146.595.217.0.1|12345".to_owned(),
                ),
            ]),
        )
        .unwrap();

        assert_eq!(params.count, 5);
        assert_eq!(params.offset, 0);
        assert_eq!(params.filters.len(), 2);
        assert_eq!(params.canonical_filters.len(), 2);
    }

    #[test]
    fn parse_search_params_rejects_unknown_resource_search_parameter() {
        let error = parse_search_params(
            "Patient",
            BTreeMap::from([("status".to_owned(), "final".to_owned())]),
        )
        .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn parse_identifier_filter_accepts_value_or_system_value() {
        assert_eq!(
            parse_identifier_filter("12345").unwrap(),
            (None, "12345".to_owned())
        );
        assert_eq!(
            parse_identifier_filter("urn:test|12345").unwrap(),
            (Some("urn:test".to_owned()), "12345".to_owned())
        );
    }

    #[tokio::test]
    async fn schema_validation_returns_operation_outcome() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost/postgres")
            .expect("lazy pool should build");

        let app = build_router(AppState {
            store: PgStore::new(pool),
            auth: AuthConfig::from_hmac_secret(jsonwebtoken::Algorithm::HS256, TEST_SECRET),
            fhir_base_url: "http://localhost:8080/fhir".to_owned(),
            validator: Arc::new(FhirSchemaValidator::new().expect("validator should load")),
            cors_allowed_origins: Vec::new(),
            serve_docs: false,
        });

        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &crate::auth::Claims {
                sub: Some("test-tenant".to_owned()),
                tenant: None,
                scope: Some("read write".to_owned()),
                resource_types: None,
                exp: Some(4_102_444_800),
            },
            &jsonwebtoken::EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .expect("token should encode");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/fhir/Patient")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::from(r#"{"resourceType":"Patient","bogus":true}"#))
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
            auth: AuthConfig::from_hmac_secret(jsonwebtoken::Algorithm::HS256, TEST_SECRET),
            fhir_base_url: "http://localhost:8080/fhir".to_owned(),
            validator: Arc::new(FhirSchemaValidator::new().expect("validator should load")),
            cors_allowed_origins: Vec::new(),
            serve_docs: false,
        });

        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &crate::auth::Claims {
                sub: Some("test-tenant".to_owned()),
                tenant: None,
                scope: Some("read write".to_owned()),
                resource_types: None,
                exp: Some(4_102_444_800),
            },
            &jsonwebtoken::EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .expect("token should encode");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/fhir/Patient")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {token}"))
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
        assert!(
            value["issue"][0]["diagnostics"]
                .as_str()
                .expect("diagnostics should be present")
                .contains("invalid JSON payload")
        );
    }
}
