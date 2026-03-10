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
    AppState, SearchConfig, auth::extract_access_context, capability::capability_statement,
    error::AppError, search_params,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/metadata", get(get_metadata))
        .route("/fhir", axum::routing::post(process_bundle))
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
        .route(
            "/fhir/{resource_type}/{id}/_history",
            get(read_resource_history),
        )
}

#[derive(Debug)]
pub struct ParsedSearchParams {
    count: u32,
    after_id: Option<String>,
    filters: Vec<search_params::SearchFilter>,
    canonical_filters: Vec<(String, String)>,
}

struct SearchPage<'a> {
    count: u32,
    after_id: Option<&'a str>,
    next_after_id: Option<&'a str>,
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

    let params = parse_search_params(&resource_type, query, state.search)?;
    let results = state
        .store
        .search(
            &access.tenant_id,
            &resource_type,
            &params.filters,
            i64::from(params.count),
            params.after_id.as_deref(),
        )
        .await?;

    let response = Json(build_search_bundle(
        &state.fhir_base_url,
        &resource_type,
        SearchPage {
            count: params.count,
            after_id: params.after_id.as_deref(),
            next_after_id: results.next_after_id.as_deref(),
        },
        results.total,
        results.resources,
        &params.canonical_filters,
    ));

    Ok((StatusCode::OK, response).into_response())
}

#[utoipa::path(post, path = "/fhir/{resource_type}",
    params(("resource_type" = String, Path, description = "FHIR resource type")),
    responses((status = 201, description = "Resource created"), (status = 200, description = "Existing resource returned (If-None-Exist match)"), (status = 400, description = "Validation error"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 409, description = "Multiple matches for If-None-Exist"), (status = 413, description = "Payload too large")),
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

    // Handle If-None-Exist conditional create
    if let Some(if_none_exist) = parse_if_none_exist(&headers)? {
        let query_params = parse_if_none_exist_query(&resource_type, &if_none_exist, state.search)?;
        let results = state
            .store
            .search(
                &access.tenant_id,
                &resource_type,
                &query_params.filters,
                2, // only need to know if 0, 1, or >1 matches
                None,
            )
            .await?;

        match results.total {
            0 => { /* no match — proceed with create below */ }
            1 => {
                // Exactly one match: return existing resource (200 OK)
                let existing = &results.resources[0];
                let mut response_headers = HeaderMap::new();
                response_headers.insert(
                    "ETag",
                    HeaderValue::from_str(&format!("W/\"{}\"", existing.version_id))
                        .map_err(|e| AppError::Internal(format!("invalid ETag header: {e}")))?,
                );
                response_headers.insert(
                    "Last-Modified",
                    HeaderValue::from_str(&existing.last_updated.to_rfc3339()).map_err(|e| {
                        AppError::Internal(format!("invalid Last-Modified header: {e}"))
                    })?,
                );
                return Ok((
                    StatusCode::OK,
                    response_headers,
                    Json(existing.resource.clone()),
                )
                    .into_response());
            }
            _ => {
                // Multiple matches: return 412 Precondition Failed per FHIR spec
                return Err(AppError::PreconditionFailed(
                    "If-None-Exist matched multiple resources".to_owned(),
                ));
            }
        }
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

#[utoipa::path(get, path = "/fhir/{resource_type}/{id}/_history",
    params(("resource_type" = String, Path, description = "FHIR resource type"), ("id" = String, Path, description = "Resource ID")),
    responses((status = 200, description = "Resource history Bundle"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 404, description = "Not found")),
    security(("bearer_auth" = [])))]
pub async fn read_resource_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((resource_type, id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    if !access.can_read || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }

    let history = state
        .store
        .read_history(&access.tenant_id, &resource_type, &id)
        .await?;

    if history.is_empty() {
        return Err(AppError::NotFound);
    }

    Ok((
        StatusCode::OK,
        Json(build_history_bundle(
            &state.fhir_base_url,
            &resource_type,
            &id,
            history,
        )),
    )
        .into_response())
}

#[utoipa::path(put, path = "/fhir/{resource_type}/{id}",
    params(("resource_type" = String, Path, description = "FHIR resource type"), ("id" = String, Path, description = "Resource ID")),
    responses((status = 200, description = "Resource updated"), (status = 400, description = "Validation error"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 404, description = "Not found"), (status = 412, description = "If-Match missing or stale"), (status = 413, description = "Payload too large")),
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
    let expected_version = parse_if_match_version(&headers)?;

    let Json(mut body) = parse_json_payload(payload)?;
    validate_resource_payload(&resource_type, &mut body, Some(&id))?;
    state.validator.validate_resource(&resource_type, &body)?;

    let updated = match expected_version {
        Some(version) => {
            state
                .store
                .update_if_version_matches(&access.tenant_id, &resource_type, &id, version, body)
                .await?
        }
        None => {
            state
                .store
                .update_existing(&access.tenant_id, &resource_type, &id, body)
                .await?
        }
    };

    let stored = match updated {
        Some(stored) => stored,
        None => {
            let current = state
                .store
                .read(&access.tenant_id, &resource_type, &id)
                .await?;
            match current {
                None => return Err(AppError::NotFound),
                Some(current) => {
                    return Err(AppError::PreconditionFailed(format!(
                        "If-Match version mismatch: current version is {}",
                        current.version_id
                    )));
                }
            }
        }
    };

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
    responses((status = 200, description = "Resource patched"), (status = 400, description = "Invalid patch"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 404, description = "Not found"), (status = 412, description = "If-Match missing or stale")),
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
    let expected_version = parse_if_match_version(&headers)?;

    let Json(patch_body) = parse_json_payload(payload)?;
    let patch_ops: json_patch::Patch = serde_json::from_value(patch_body)
        .map_err(|e| AppError::BadRequest(format!("invalid JSON Patch document: {e}")))?;

    let found = state
        .store
        .read(&access.tenant_id, &resource_type, &id)
        .await?;

    let existing = found.ok_or(AppError::NotFound)?;
    if let Some(expected_version) = expected_version
        && existing.version_id != expected_version
    {
        return Err(AppError::PreconditionFailed(format!(
            "If-Match version mismatch: current version is {}",
            existing.version_id
        )));
    }
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

    let updated = match expected_version {
        Some(version) => {
            state
                .store
                .update_if_version_matches(
                    &access.tenant_id,
                    &resource_type,
                    &id,
                    version,
                    resource,
                )
                .await?
        }
        None => {
            state
                .store
                .update_existing(&access.tenant_id, &resource_type, &id, resource)
                .await?
        }
    };

    let stored = match updated {
        Some(stored) => stored,
        None => {
            let current = state
                .store
                .read(&access.tenant_id, &resource_type, &id)
                .await?;
            match current {
                None => return Err(AppError::NotFound),
                Some(current) => {
                    return Err(AppError::PreconditionFailed(format!(
                        "If-Match version mismatch: current version is {}",
                        current.version_id
                    )));
                }
            }
        }
    };

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

/// Process a FHIR Bundle of type `transaction` or `batch`.
///
/// - `transaction`: all entries are processed atomically in a single SQL transaction.
///   If any entry fails, the entire transaction is rolled back and an OperationOutcome is returned.
/// - `batch`: each entry is processed independently. Failures for individual entries are reported
///   inline in the response Bundle while other entries succeed normally.
#[utoipa::path(post, path = "/fhir",
    responses(
        (status = 200, description = "Bundle response"),
        (status = 400, description = "Invalid Bundle"),
        (status = 401, description = "Missing or invalid bearer token"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = [])))]
pub async fn process_bundle(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    if !access.can_write {
        return Err(AppError::Forbidden);
    }

    let Json(body) = parse_json_payload(payload)?;

    let resource_type = body
        .get("resourceType")
        .and_then(Value::as_str)
        .unwrap_or("");
    if resource_type != "Bundle" {
        return Err(AppError::BadRequest(
            "expected resourceType 'Bundle'".to_owned(),
        ));
    }

    let bundle_type = body.get("type").and_then(Value::as_str).unwrap_or("");
    let is_transaction = match bundle_type {
        "transaction" => true,
        "batch" => false,
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported Bundle type '{other}'; expected 'transaction' or 'batch'"
            )));
        }
    };

    let entries = body
        .get("entry")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if is_transaction {
        process_transaction(&state, &access.tenant_id, entries).await
    } else {
        process_batch(&state, &access.tenant_id, entries).await
    }
}

/// Parse a Bundle entry's `request` into method + url parts.
struct EntryRequest {
    method: String,
    resource_type: String,
    id: Option<String>,
}

fn parse_entry_request(entry: &Value) -> Result<EntryRequest, String> {
    let request = entry.get("request").ok_or("entry is missing 'request'")?;

    let method = request
        .get("method")
        .and_then(Value::as_str)
        .ok_or("entry.request.method is required")?
        .to_uppercase();

    let url = request
        .get("url")
        .and_then(Value::as_str)
        .ok_or("entry.request.url is required")?;

    // URL is relative, e.g. "Patient", "Patient/123", "Observation/abc"
    let url = url.trim_start_matches('/');
    let mut parts = url.splitn(2, '/');
    let resource_type = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or("entry.request.url must contain a resource type")?
        .to_owned();
    let id = parts.next().map(|s| s.to_owned());

    Ok(EntryRequest {
        method,
        resource_type,
        id,
    })
}

/// Build a single response entry for a successful operation.
fn success_entry(
    status: &str,
    resource: Option<&Value>,
    location: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
) -> Value {
    let mut response = json!({ "status": status });
    if let Some(loc) = location {
        response["location"] = Value::String(loc);
    }
    if let Some(etag) = etag {
        response["etag"] = Value::String(etag);
    }
    if let Some(lm) = last_modified {
        response["lastModified"] = Value::String(lm);
    }
    let mut entry = json!({ "response": response });
    if let Some(res) = resource {
        entry["resource"] = res.clone();
    }
    entry
}

/// Build a single response entry for a failed operation.
fn error_entry(status: &str, diagnostics: &str) -> Value {
    json!({
        "response": {
            "status": status,
        },
        "resource": {
            "resourceType": "OperationOutcome",
            "issue": [{
                "severity": "error",
                "code": "exception",
                "diagnostics": diagnostics,
            }]
        }
    })
}

/// Process a single Bundle entry against the store.
/// Returns the response entry Value on success.
async fn process_single_entry<E>(
    executor: &mut E,
    tenant_id: &str,
    entry: &Value,
    validator: &crate::validation::FhirSchemaValidator,
) -> Result<Value, String>
where
    E: BundleExecutor,
{
    let req = parse_entry_request(entry)?;

    match req.method.as_str() {
        "POST" => {
            // Create
            let mut resource = entry
                .get("resource")
                .cloned()
                .ok_or("POST entry must include a resource")?;

            validate_resource_payload(&req.resource_type, &mut resource, None)
                .map_err(|e| e.to_string())?;
            let id = resource
                .get("id")
                .and_then(Value::as_str)
                .ok_or("resource id is required after validation")?
                .to_owned();

            validator
                .validate_resource(&req.resource_type, &resource)
                .map_err(|e| e.to_string())?;

            let stored = executor
                .exec_upsert(tenant_id, &req.resource_type, &id, resource)
                .await
                .map_err(|e| e.to_string())?;

            Ok(success_entry(
                "201 Created",
                Some(&stored.resource),
                Some(format!("{}/{}", req.resource_type, id)),
                Some(format!("W/\"{}\"", stored.version_id)),
                Some(stored.last_updated.to_rfc3339()),
            ))
        }
        "PUT" => {
            // Update
            let id = req
                .id
                .as_deref()
                .ok_or("PUT requires a resource id in the URL")?;
            let mut resource = entry
                .get("resource")
                .cloned()
                .ok_or("PUT entry must include a resource")?;

            validate_resource_payload(&req.resource_type, &mut resource, Some(id))
                .map_err(|e| e.to_string())?;

            validator
                .validate_resource(&req.resource_type, &resource)
                .map_err(|e| e.to_string())?;

            let stored = executor
                .exec_upsert(tenant_id, &req.resource_type, id, resource)
                .await
                .map_err(|e| e.to_string())?;

            Ok(success_entry(
                "200 OK",
                Some(&stored.resource),
                Some(format!("{}/{}", req.resource_type, id)),
                Some(format!("W/\"{}\"", stored.version_id)),
                Some(stored.last_updated.to_rfc3339()),
            ))
        }
        "GET" => {
            // Read
            let id = req
                .id
                .as_deref()
                .ok_or("GET requires a resource id in the URL")?;

            let found = executor
                .exec_read(tenant_id, &req.resource_type, id)
                .await
                .map_err(|e| e.to_string())?;

            match found {
                Some(stored) => Ok(success_entry(
                    "200 OK",
                    Some(&stored.resource),
                    None,
                    Some(format!("W/\"{}\"", stored.version_id)),
                    Some(stored.last_updated.to_rfc3339()),
                )),
                None => Err("resource not found".to_owned()),
            }
        }
        "DELETE" => {
            let id = req
                .id
                .as_deref()
                .ok_or("DELETE requires a resource id in the URL")?;

            let deleted = executor
                .exec_delete(tenant_id, &req.resource_type, id)
                .await
                .map_err(|e| e.to_string())?;

            if deleted {
                Ok(success_entry("204 No Content", None, None, None, None))
            } else {
                Err("resource not found".to_owned())
            }
        }
        other => Err(format!("unsupported HTTP method '{other}'")),
    }
}

/// Trait abstracting database execution for Bundle entries so both
/// transaction (single TX) and batch (per-entry) modes share the same logic.
#[allow(async_fn_in_trait)]
trait BundleExecutor {
    async fn exec_upsert(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<crate::store::StoredResource, AppError>;

    async fn exec_read(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<Option<crate::store::StoredResource>, AppError>;

    async fn exec_delete(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<bool, AppError>;
}

/// Executor that operates inside an existing database transaction.
struct TxBundleExecutor<'a> {
    tx: crate::store::TxExecutor<'a>,
}

impl BundleExecutor for TxBundleExecutor<'_> {
    async fn exec_upsert(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<crate::store::StoredResource, AppError> {
        crate::store::PgStore::upsert_in_tx(&mut self.tx, tenant_id, resource_type, id, resource)
            .await
    }

    async fn exec_read(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<Option<crate::store::StoredResource>, AppError> {
        crate::store::PgStore::read_in_tx(&mut self.tx, tenant_id, resource_type, id).await
    }

    async fn exec_delete(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<bool, AppError> {
        crate::store::PgStore::delete_in_tx(&mut self.tx, tenant_id, resource_type, id).await
    }
}

/// Executor that uses independent pool connections (for batch mode).
struct PoolBundleExecutor<'a> {
    store: &'a crate::store::PgStore,
}

impl BundleExecutor for PoolBundleExecutor<'_> {
    async fn exec_upsert(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<crate::store::StoredResource, AppError> {
        self.store
            .upsert(tenant_id, resource_type, id, resource)
            .await
    }

    async fn exec_read(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<Option<crate::store::StoredResource>, AppError> {
        self.store.read(tenant_id, resource_type, id).await
    }

    async fn exec_delete(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<bool, AppError> {
        self.store.delete(tenant_id, resource_type, id).await
    }
}

/// Process a `transaction` Bundle: all entries succeed or all fail.
async fn process_transaction(
    state: &AppState,
    tenant_id: &str,
    entries: Vec<Value>,
) -> Result<Response, AppError> {
    let mut executor = TxBundleExecutor {
        tx: state.store.begin_tx().await?,
    };

    let mut response_entries = Vec::with_capacity(entries.len());

    for entry in &entries {
        match process_single_entry(&mut executor, tenant_id, entry, &state.validator).await {
            Ok(resp) => response_entries.push(resp),
            Err(msg) => {
                // Transaction mode: any failure aborts the whole thing.
                // The transaction is dropped (rolled back) automatically.
                return Err(AppError::BadRequest(format!("transaction failed: {msg}")));
            }
        }
    }

    // All entries succeeded — commit.
    executor
        .tx
        .commit()
        .await
        .map_err(|e| AppError::Internal(format!("transaction commit failed: {e}")))?;

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction-response",
        "entry": response_entries,
    });

    Ok((StatusCode::OK, Json(bundle)).into_response())
}

/// Process a `batch` Bundle: each entry is independent.
async fn process_batch(
    state: &AppState,
    tenant_id: &str,
    entries: Vec<Value>,
) -> Result<Response, AppError> {
    let mut executor = PoolBundleExecutor {
        store: &state.store,
    };

    let mut response_entries = Vec::with_capacity(entries.len());

    for entry in &entries {
        match process_single_entry(&mut executor, tenant_id, entry, &state.validator).await {
            Ok(resp) => response_entries.push(resp),
            Err(msg) => {
                // Batch mode: report the error inline but continue processing.
                response_entries.push(error_entry("400 Bad Request", &msg));
            }
        }
    }

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch-response",
        "entry": response_entries,
    });

    Ok((StatusCode::OK, Json(bundle)).into_response())
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

/// Parse the `If-None-Exist` header for conditional create.
///
/// Returns the raw query string (e.g. "identifier=http://example.org|12345")
/// or None if the header is absent.
fn parse_if_none_exist(headers: &HeaderMap) -> Result<Option<String>, AppError> {
    let Some(header_value) = headers.get("If-None-Exist") else {
        return Ok(None);
    };

    let raw = header_value
        .to_str()
        .map_err(|e| AppError::BadRequest(format!("invalid If-None-Exist header: {e}")))?
        .trim();

    if raw.is_empty() {
        return Err(AppError::BadRequest(
            "If-None-Exist header must not be empty".to_owned(),
        ));
    }

    Ok(Some(raw.to_owned()))
}

/// Parse the If-None-Exist query string into search filters.
fn parse_if_none_exist_query(
    resource_type: &str,
    query_string: &str,
    search: SearchConfig,
) -> Result<ParsedSearchParams, AppError> {
    let query: BTreeMap<String, String> = url::form_urlencoded::parse(query_string.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if query.is_empty() {
        return Err(AppError::BadRequest(
            "If-None-Exist must contain at least one search parameter".to_owned(),
        ));
    }

    parse_search_params(resource_type, query, search)
}

fn parse_if_match_version(headers: &HeaderMap) -> Result<Option<i64>, AppError> {
    let Some(header_value) = headers.get("If-Match") else {
        return Ok(None);
    };

    let raw = header_value
        .to_str()
        .map_err(|e| AppError::BadRequest(format!("invalid If-Match header: {e}")))?
        .trim();

    if raw == "*" {
        return Err(AppError::BadRequest(
            "If-Match wildcard '*' is not supported; use a concrete version ETag".to_owned(),
        ));
    }

    let version_text = if raw.starts_with("W/\"") && raw.ends_with('"') {
        &raw[3..raw.len() - 1]
    } else if raw.starts_with('"') && raw.ends_with('"') {
        &raw[1..raw.len() - 1]
    } else {
        raw
    };

    let version = version_text.parse::<i64>().map_err(|_| {
        AppError::BadRequest("If-Match must be an integer version ETag like W/\"3\"".to_owned())
    })?;

    if version < 1 {
        return Err(AppError::BadRequest(
            "If-Match version must be >= 1".to_owned(),
        ));
    }

    Ok(Some(version))
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
    page: SearchPage<'_>,
    total: i64,
    resources: Vec<crate::store::StoredResource>,
    filters: &[(String, String)],
) -> Value {
    let base_url = base_url.trim_end_matches('/');
    let search_url = build_search_url(base_url, resource_type, page.count, page.after_id, filters);

    let mut links = vec![json!({
        "relation": "self",
        "url": search_url,
    })];

    if let Some(next_after_id) = page.next_after_id {
        links.push(json!({
            "relation": "next",
            "url": build_search_url(base_url, resource_type, page.count, Some(next_after_id), filters),
        }));
    }

    let entry = resources
        .into_iter()
        .map(|stored| {
            json!({
                "fullUrl": format!("{base_url}/{resource_type}/{}", stored.id),
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

fn build_history_bundle(
    base_url: &str,
    resource_type: &str,
    id: &str,
    history: Vec<crate::store::HistoricalResource>,
) -> Value {
    let base_url = base_url.trim_end_matches('/');
    let self_url = format!("{base_url}/{resource_type}/{id}/_history");
    let total = history.len() as i64;

    let entry = history
        .into_iter()
        .map(|version| {
            json!({
                "fullUrl": format!("{base_url}/{resource_type}/{}/_history/{}", version.id, version.version_id),
                "resource": version.resource,
                "request": {
                    "method": if version.deleted { "DELETE" } else { "PUT" },
                    "url": format!("{resource_type}/{}", version.id),
                },
                "response": {
                    "status": if version.deleted { "410 Gone" } else { "200 OK" },
                    "etag": format!("W/\"{}\"", version.version_id),
                    "lastModified": version.last_updated.to_rfc3339(),
                }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "resourceType": "Bundle",
        "type": "history",
        "total": total,
        "link": [{
            "relation": "self",
            "url": self_url,
        }],
        "entry": entry,
    })
}

fn parse_search_params(
    resource_type: &str,
    query: BTreeMap<String, String>,
    search: SearchConfig,
) -> Result<ParsedSearchParams, AppError> {
    let mut count = search.default_count;
    let mut after_id = None;
    let mut filters = Vec::new();
    let mut canonical_filters = Vec::new();

    // Look up the search parameters supported for this resource type
    let supported_params = search_params::search_params_for(resource_type);

    for (key, value) in query {
        match key.as_str() {
            "_count" => {
                count = parse_u32_query_param("_count", &value)?;
                if count > search.max_count {
                    return Err(AppError::BadRequest(format!(
                        "_count must be less than or equal to {}",
                        search.max_count
                    )));
                }
            }
            "_after_id" => {
                if value.is_empty() {
                    return Err(AppError::BadRequest(
                        "_after_id must not be empty".to_owned(),
                    ));
                }
                after_id = Some(value);
            }
            "_offset" => {
                return Err(AppError::BadRequest(
                    "_offset is no longer supported; use _after_id cursor pagination".to_owned(),
                ));
            }
            param_code => {
                // Look up the parameter in the registry
                if let Some(param) = supported_params.iter().find(|p| p.code == param_code) {
                    // Validate token-type parameters with pipe syntax
                    if param.param_type == search_params::SearchParamType::Token
                        && param_code == "identifier"
                    {
                        // Special validation for identifier tokens
                        validate_identifier_value(&value)?;
                    }

                    filters.push(search_params::SearchFilter {
                        param,
                        value: value.clone(),
                    });
                    canonical_filters.push((key, value));
                } else {
                    return Err(AppError::BadRequest(format!(
                        "unsupported search parameter '{param_code}' for resource type '{resource_type}'"
                    )));
                }
            }
        }
    }

    Ok(ParsedSearchParams {
        count,
        after_id,
        filters,
        canonical_filters,
    })
}

fn validate_identifier_value(value: &str) -> Result<(), AppError> {
    if value.is_empty() {
        return Err(AppError::BadRequest(
            "identifier must be 'value' or 'system|value'".to_owned(),
        ));
    }
    if let Some((system, id_value)) = value.split_once('|')
        && (system.is_empty() || id_value.is_empty())
    {
        return Err(AppError::BadRequest(
            "identifier must be 'value' or 'system|value'".to_owned(),
        ));
    }
    Ok(())
}

fn parse_u32_query_param(name: &str, value: &str) -> Result<u32, AppError> {
    value
        .parse::<u32>()
        .map_err(|_| AppError::BadRequest(format!("{name} must be an unsigned integer")))
}

fn build_search_url(
    base_url: &str,
    resource_type: &str,
    count: u32,
    after_id: Option<&str>,
    filters: &[(String, String)],
) -> String {
    let mut serializer = Serializer::new(String::new());
    serializer.append_pair("_count", &count.to_string());

    if let Some(after_id) = after_id {
        serializer.append_pair("_after_id", after_id);
    }

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
        SearchPage, build_history_bundle, build_search_bundle, parse_search_params,
        validate_identifier_value, validate_resource_payload,
    };
    use crate::{
        AppState, SearchConfig,
        auth::AuthConfig,
        build_router,
        error::AppError,
        store::{HistoricalResource, PgStore, StoredResource},
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
            SearchPage {
                count: 1,
                after_id: None,
                next_after_id: Some("example"),
            },
            2,
            vec![StoredResource {
                id: "example".to_owned(),
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
        assert_eq!(
            bundle["link"][1]["url"],
            "http://localhost:8080/fhir/Patient?_count=1&_after_id=example"
        );
        assert_eq!(bundle["entry"][0]["resource"]["id"], "example");
    }

    #[test]
    fn history_bundle_contains_versions_including_deletes() {
        let now = Utc::now();
        let bundle = build_history_bundle(
            "http://localhost:8080/fhir",
            "Patient",
            "example",
            vec![
                HistoricalResource {
                    id: "example".to_owned(),
                    version_id: 2,
                    last_updated: now,
                    deleted: true,
                    resource: json!({
                        "resourceType": "Patient",
                        "id": "example"
                    }),
                },
                HistoricalResource {
                    id: "example".to_owned(),
                    version_id: 1,
                    last_updated: now,
                    deleted: false,
                    resource: json!({
                        "resourceType": "Patient",
                        "id": "example"
                    }),
                },
            ],
        );

        assert_eq!(bundle["resourceType"], "Bundle");
        assert_eq!(bundle["type"], "history");
        assert_eq!(bundle["total"], 2);
        assert_eq!(bundle["link"][0]["relation"], "self");
        assert_eq!(bundle["entry"][0]["response"]["status"], "410 Gone");
        assert_eq!(bundle["entry"][0]["response"]["etag"], "W/\"2\"");
        assert_eq!(bundle["entry"][1]["response"]["status"], "200 OK");
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
            SearchConfig {
                default_count: 20,
                max_count: 100,
            },
        )
        .unwrap();

        assert_eq!(params.count, 5);
        assert_eq!(params.after_id, None);
        assert_eq!(params.filters.len(), 2);
        assert_eq!(params.canonical_filters.len(), 2);
    }

    #[test]
    fn parse_search_params_accepts_after_id_cursor() {
        let params = parse_search_params(
            "Patient",
            BTreeMap::from([("_after_id".to_owned(), "patient-123".to_owned())]),
            SearchConfig {
                default_count: 20,
                max_count: 100,
            },
        )
        .unwrap();

        assert_eq!(params.count, 20);
        assert_eq!(params.after_id.as_deref(), Some("patient-123"));
    }

    #[test]
    fn parse_search_params_rejects_unknown_resource_search_parameter() {
        let error = parse_search_params(
            "Patient",
            BTreeMap::from([("status".to_owned(), "final".to_owned())]),
            SearchConfig {
                default_count: 20,
                max_count: 100,
            },
        )
        .unwrap_err();

        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn validate_identifier_value_accepts_value_or_system_value() {
        validate_identifier_value("12345").unwrap();
        validate_identifier_value("urn:test|12345").unwrap();
    }

    #[test]
    fn validate_identifier_value_rejects_empty() {
        validate_identifier_value("").unwrap_err();
        validate_identifier_value("|").unwrap_err();
        validate_identifier_value("|12345").unwrap_err();
        validate_identifier_value("urn:test|").unwrap_err();
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
            search: SearchConfig {
                default_count: 20,
                max_count: 100,
            },
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
            search: SearchConfig {
                default_count: 20,
                max_count: 100,
            },
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
