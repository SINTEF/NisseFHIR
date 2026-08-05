use axum::{
    Json, Router,
    extract::{Path, RawQuery, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use json_patch::patch as apply_json_patch;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppState, auth::extract_access_context, capability::capability_statement, error::AppError,
    media_type::BodyKind, search_params,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/fhir/metadata", get(get_metadata))
        // Temporary backwards-compatible alias. The standard endpoint is
        // `[base]/metadata`, which is `/fhir/metadata` for the built-in base.
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

#[utoipa::path(get, path = "/healthz", responses((status = 200, description = "Server is healthy")))]
pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

#[utoipa::path(get, path = "/readyz", responses((status = 200, description = "Server is ready"), (status = 503, description = "Server is not ready (database unavailable)")))]
pub async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    if state.store.is_ready().await {
        (StatusCode::OK, Json(json!({"status": "ready"})))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status": "unavailable"})),
        )
    }
}

#[utoipa::path(get, path = "/fhir/metadata", responses((status = 200, description = "FHIR CapabilityStatement")))]
pub async fn get_metadata(State(state): State<AppState>) -> impl IntoResponse {
    Json(capability_statement(
        &state.fhir_base_url,
        !state.cors_allowed_origins.is_empty(),
    ))
}

#[utoipa::path(get, path = "/fhir/{resource_type}",
    params(("resource_type" = String, Path, description = "FHIR resource type")),
    responses((status = 200, description = "Search results Bundle"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden")),
    security(("bearer_auth" = [])))]
pub async fn search_resources(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(resource_type): Path<String>,
    RawQuery(query): RawQuery,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    validate_path_resource_type(&resource_type)?;
    if !access.can_read || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }

    let query = crate::search::parse_query_pairs(query.as_deref().unwrap_or(""));
    let params = crate::search::parse_search_params(&resource_type, query, state.search)?;
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

    let response = Json(crate::search::build_search_bundle(
        &state.fhir_base_url,
        &resource_type,
        crate::search::SearchPage {
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
    responses((status = 201, description = "Resource created"), (status = 200, description = "Existing resource returned (If-None-Exist match)"), (status = 400, description = "Validation error"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 412, description = "If-None-Exist matched multiple resources"), (status = 413, description = "Payload too large")),
    security(("bearer_auth" = [])))]
pub async fn create_resource(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(resource_type): Path<String>,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    validate_path_resource_type(&resource_type)?;
    if !access.can_write || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }

    crate::media_type::validate_request_content_type(&headers, BodyKind::FhirResource)?;

    // Parse the `If-None-Exist` header up front so that an empty or malformed
    // header fails fast with 400 before we touch the request body.
    let if_none_exist_params = match parse_if_none_exist(&headers)? {
        Some(raw) => Some(crate::search::parse_if_none_exist_query(
            &resource_type,
            &raw,
            state.search,
        )?),
        None => None,
    };

    // Validate the payload and assign a server-side id before entering the
    // critical section. Doing this work outside the advisory lock keeps the
    // serialized window short.
    let Json(mut body) = parse_json_payload(payload)?;
    validate_resource_payload(&resource_type, &body, None)?;
    assign_resource_id(&mut body, None)?;
    state.validator.validate_resource(&resource_type, &body)?;
    let id = body
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            AppError::Internal("resource is missing its id after assignment".to_owned())
        })?;

    // Atomic conditional create path: search + create run inside one
    // PostgreSQL transaction guarded by a tenant+type+condition advisory lock.
    if let Some(params) = if_none_exist_params {
        let lock_key = crate::store::conditional_create_lock_key(
            &access.tenant_id,
            &resource_type,
            &params.filters,
        );
        let outcome = state
            .store
            .conditional_create_atomic(
                &access.tenant_id,
                &resource_type,
                &params.filters,
                lock_key,
                &id,
                body,
            )
            .await?;

        match outcome {
            crate::store::ConditionalCreateOutcome::Existing(existing) => {
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
                return Ok(
                    (StatusCode::OK, response_headers, Json(existing.resource)).into_response()
                );
            }
            crate::store::ConditionalCreateOutcome::MultipleMatches => {
                return Err(AppError::PreconditionFailed(
                    "If-None-Exist matched multiple resources".to_owned(),
                ));
            }
            crate::store::ConditionalCreateOutcome::Created(stored) => {
                let stored_id = stored.id.clone();
                let mut response_headers = HeaderMap::new();
                response_headers.insert(
                    "ETag",
                    HeaderValue::from_str(&format!("W/\"{}\"", stored.version_id))
                        .map_err(|e| AppError::Internal(format!("invalid ETag header: {e}")))?,
                );
                response_headers.insert(
                    "Last-Modified",
                    HeaderValue::from_str(&stored.last_updated.to_rfc3339()).map_err(|e| {
                        AppError::Internal(format!("invalid Last-Modified header: {e}"))
                    })?,
                );
                response_headers.insert(
                    "Location",
                    HeaderValue::from_str(&format!(
                        "{}/{resource_type}/{stored_id}",
                        state.fhir_base_url.trim_end_matches('/')
                    ))
                    .map_err(|e| AppError::Internal(format!("invalid Location header: {e}")))?,
                );
                return Ok(
                    (StatusCode::CREATED, response_headers, Json(stored.resource)).into_response(),
                );
            }
        }
    }

    // Non-conditional create path.
    let stored = state
        .store
        .create(&access.tenant_id, &resource_type, &id, body)
        .await?
        .ok_or_else(|| {
            AppError::Conflict("a resource with the generated id already exists".to_owned())
        })?;
    let id = &stored.id;

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
        HeaderValue::from_str(&format!(
            "{}/{resource_type}/{id}",
            state.fhir_base_url.trim_end_matches('/')
        ))
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
    validate_path_resource_type(&resource_type)?;
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
    params(("resource_type" = String, Path, description = "FHIR resource type"), ("id" = String, Path, description = "Resource ID"), ("_count" = Option<u32>, Query, description = "Page size (bounded by SEARCH_MAX_COUNT)"), ("_after_id" = Option<String>, Query, description = "Version-id cursor from a previous page's next link")),
    responses((status = 200, description = "Resource history Bundle"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 404, description = "Not found")),
    security(("bearer_auth" = [])))]
pub async fn read_resource_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((resource_type, id)): Path<(String, String)>,
    RawQuery(query): RawQuery,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    validate_path_resource_type(&resource_type)?;
    if !access.can_read || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }

    let query = crate::search::parse_query_pairs(query.as_deref().unwrap_or(""));
    let params = crate::search::parse_history_params(query, state.search)?;

    let results = state
        .store
        .read_history(
            &access.tenant_id,
            &resource_type,
            &id,
            i64::from(params.count),
            params.after_version_id,
        )
        .await?;

    if !results.exists {
        return Err(AppError::NotFound);
    }

    let after_id_str = params.after_version_id.map(|v| v.to_string());

    Ok((
        StatusCode::OK,
        Json(crate::search::build_history_bundle(
            &state.fhir_base_url,
            &resource_type,
            &id,
            crate::search::HistoryPage {
                count: params.count,
                after_id: after_id_str.as_deref(),
                next_after_id: results.next_after_version_id,
            },
            results.versions,
        )),
    )
        .into_response())
}

#[utoipa::path(put, path = "/fhir/{resource_type}/{id}",
    params(("resource_type" = String, Path, description = "FHIR resource type"), ("id" = String, Path, description = "Resource ID")),
    responses((status = 200, description = "Resource updated"), (status = 201, description = "Resource created"), (status = 400, description = "Validation error"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 412, description = "If-Match missing or stale"), (status = 413, description = "Payload too large")),
    security(("bearer_auth" = [])))]
pub async fn update_resource(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((resource_type, id)): Path<(String, String)>,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    validate_path_resource_type(&resource_type)?;
    if !access.can_write || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }
    let expected_version = parse_if_match_version(&headers)?;

    crate::media_type::validate_request_content_type(&headers, BodyKind::FhirResource)?;

    let Json(mut body) = parse_json_payload(payload)?;
    validate_resource_payload(&resource_type, &body, Some(&id))?;
    assign_resource_id(&mut body, Some(&id))?;
    state.validator.validate_resource(&resource_type, &body)?;

    let (stored, created) = match expected_version {
        Some(version) => {
            let updated = state
                .store
                .update_if_version_matches(&access.tenant_id, &resource_type, &id, version, body)
                .await?;

            let stored = match updated {
                Some(stored) => stored,
                None => {
                    let current = state
                        .store
                        .read(&access.tenant_id, &resource_type, &id)
                        .await?;
                    let message = match current {
                        Some(current) => format!(
                            "If-Match version mismatch: current version is {}",
                            current.version_id
                        ),
                        None => "If-Match cannot update a missing resource".to_owned(),
                    };
                    return Err(AppError::PreconditionFailed(message));
                }
            };
            (stored, false)
        }
        None => {
            let result = state
                .store
                .upsert(&access.tenant_id, &resource_type, &id, body)
                .await?;
            (result.stored, result.created)
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
    response_headers.insert(
        "Location",
        HeaderValue::from_str(&format!(
            "{}/{}/{}/_history/{}",
            state.fhir_base_url.trim_end_matches('/'),
            resource_type,
            id,
            stored.version_id
        ))
        .map_err(|e| AppError::Internal(format!("invalid Location header: {e}")))?,
    );

    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, response_headers, Json(stored.resource)).into_response())
}

/// Delete a resource.
///
/// When an `If-Match` header carrying a concrete version ETag (e.g. `W/"3"`)
/// is supplied, the resource is deleted only if its current version matches;
/// otherwise `412 Precondition Failed` is returned and nothing is deleted.
/// Without `If-Match` the delete is unconditional: the current version is
/// removed regardless of how many times it has been rewritten since the
/// client last read it.
#[utoipa::path(delete, path = "/fhir/{resource_type}/{id}",
    params(("resource_type" = String, Path, description = "FHIR resource type"), ("id" = String, Path, description = "Resource ID")),
    responses((status = 204, description = "Resource deleted"), (status = 400, description = "Malformed If-Match header"), (status = 401, description = "Missing or invalid bearer token"), (status = 403, description = "Forbidden"), (status = 404, description = "Not found"), (status = 412, description = "If-Match version does not match the current resource version")),
    security(("bearer_auth" = [])))]
pub async fn delete_resource(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((resource_type, id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let access = extract_access_context(&headers, &state.auth)?;
    validate_path_resource_type(&resource_type)?;
    if !access.can_write || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }
    let expected_version = parse_if_match_version(&headers)?;

    let outcome = match expected_version {
        Some(version) => {
            state
                .store
                .delete_if_version_matches(&access.tenant_id, &resource_type, &id, version)
                .await?
        }
        None => {
            // No If-Match header: unconditional delete (removes the current version).
            let deleted = state
                .store
                .delete(&access.tenant_id, &resource_type, &id)
                .await?;
            if !deleted {
                return Err(AppError::NotFound);
            }
            return Ok(StatusCode::NO_CONTENT.into_response());
        }
    };

    match outcome {
        crate::store::DeleteIfMatchOutcome::Deleted { .. } => {
            Ok(StatusCode::NO_CONTENT.into_response())
        }
        crate::store::DeleteIfMatchOutcome::VersionMismatch => Err(AppError::PreconditionFailed(
            "If-Match version mismatch: resource was updated by another writer".to_owned(),
        )),
        crate::store::DeleteIfMatchOutcome::NotFound => Err(AppError::NotFound),
    }
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
    validate_path_resource_type(&resource_type)?;
    if !access.can_write || !access.can_access_resource_type(&resource_type) {
        return Err(AppError::Forbidden);
    }
    let expected_version = parse_if_match_version(&headers)?;

    crate::media_type::validate_request_content_type(&headers, BodyKind::JsonPatch)?;

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

    crate::media_type::validate_request_content_type(&headers, BodyKind::FhirResource)?;

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
        crate::bundle::process_transaction(&state, &access, entries).await
    } else {
        crate::bundle::process_batch(&state, &access, entries).await
    }
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

fn parse_if_match_version(headers: &HeaderMap) -> Result<Option<i64>, AppError> {
    let Some(header_value) = headers.get("If-Match") else {
        return Ok(None);
    };

    let raw = header_value
        .to_str()
        .map_err(|e| AppError::BadRequest(format!("invalid If-Match header: {e}")))?
        .trim();

    parse_if_match_value(raw)
        .map(Some)
        .map_err(AppError::BadRequest)
}

pub(crate) fn parse_if_match_value(raw: &str) -> Result<i64, String> {
    let raw = raw.trim();
    if raw == "*" {
        return Err(
            "If-Match wildcard '*' is not supported; use a concrete version ETag".to_owned(),
        );
    }

    let version_text = if raw.starts_with("W/\"") && raw.ends_with('"') {
        &raw[3..raw.len() - 1]
    } else if raw.starts_with('"') && raw.ends_with('"') {
        &raw[1..raw.len() - 1]
    } else {
        raw
    };

    let version = version_text
        .parse::<i64>()
        .map_err(|_| "If-Match must be an integer version ETag like W/\"3\"".to_owned())?;

    if version < 1 {
        return Err("If-Match version must be >= 1".to_owned());
    }

    Ok(version)
}

/// Validate that `body` is a JSON object whose `resourceType` matches
/// `path_resource_type`, and that, when `expected_id` is provided, any `id`
/// present in the payload matches it. This function is pure: it does not
/// mutate `body`.
pub(crate) fn validate_resource_payload(
    path_resource_type: &str,
    body: &Value,
    expected_id: Option<&str>,
) -> Result<(), AppError> {
    let object = body
        .as_object()
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

    if let Some(id) = expected_id
        && let Some(payload_id) = object.get("id").and_then(Value::as_str)
        && payload_id != id
    {
        return Err(AppError::BadRequest(
            "resource id in payload does not match URL id".to_owned(),
        ));
    }

    Ok(())
}

/// Assign the resource `id`. When `expected_id` is `Some` (PUT/update), the
/// payload `id` is rewritten with the URL id. Otherwise (POST/create), a
/// fresh server-assigned UUIDv4 is generated; a logical id supplied in a POST
/// representation is source-system metadata and SHALL be ignored.
pub(crate) fn assign_resource_id(
    body: &mut Value,
    expected_id: Option<&str>,
) -> Result<(), AppError> {
    let object = body
        .as_object_mut()
        .ok_or_else(|| AppError::BadRequest("resource payload must be a JSON object".to_owned()))?;
    let id = expected_id
        .map(|id| id.to_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    object.insert("id".to_owned(), Value::String(id));
    Ok(())
}

fn validate_path_resource_type(resource_type: &str) -> Result<(), AppError> {
    if search_params::is_valid_resource_type(resource_type) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "unsupported FHIR resource type '{resource_type}'"
        )))
    }
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

    use super::{assign_resource_id, validate_path_resource_type, validate_resource_payload};
    use crate::{
        AppState, SearchConfig, auth::AuthConfig, build_router, search_params::sql::GeoSearchMode,
        store::PgStore, validation::FhirSchemaValidator,
    };

    const TEST_SECRET: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn generates_id_for_create() {
        let mut body = json!({"resourceType": "Patient"});
        validate_resource_payload("Patient", &body, None).expect("valid payload");
        assign_resource_id(&mut body, None).expect("id assigned");
        assert!(body.get("id").and_then(|v| v.as_str()).is_some());
    }

    #[test]
    fn create_ignores_submitted_id() {
        let mut body = json!({"resourceType": "Patient", "id": "source-id"});
        validate_resource_payload("Patient", &body, None).expect("valid payload");
        assign_resource_id(&mut body, None).expect("id assigned");
        assert_ne!(body["id"], "source-id");
        assert_eq!(body["id"].as_str().unwrap().len(), 36);
    }

    #[test]
    fn rejects_mismatched_type() {
        let body = json!({"resourceType": "Observation"});
        let err = validate_resource_payload("Patient", &body, None).expect_err("must fail");
        assert!(err.to_string().contains("does not match path"));
    }

    #[test]
    fn rejects_unsupported_path_resource_type() {
        let err = validate_path_resource_type("ObviouslyNotAValidType").expect_err("must fail");
        assert!(
            err.to_string()
                .contains("unsupported FHIR resource type 'ObviouslyNotAValidType'")
        );
    }

    #[tokio::test]
    async fn schema_validation_returns_operation_outcome() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost/postgres")
            .expect("lazy pool should build");

        let app = build_router(AppState {
            store: PgStore::new(pool, GeoSearchMode::EarthDistance),
            auth: AuthConfig::from_hmac_secret(jsonwebtoken::Algorithm::HS256, TEST_SECRET),
            fhir_base_url: "http://localhost:8080/fhir".to_owned(),
            search: SearchConfig {
                default_count: 50,
                max_count: 500,
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
            store: PgStore::new(pool, GeoSearchMode::EarthDistance),
            auth: AuthConfig::from_hmac_secret(jsonwebtoken::Algorithm::HS256, TEST_SECRET),
            fhir_base_url: "http://localhost:8080/fhir".to_owned(),
            search: SearchConfig {
                default_count: 50,
                max_count: 500,
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

    fn test_app(database_url: &str) -> AppState {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy(database_url)
            .expect("lazy pool should build");
        AppState {
            store: PgStore::new(pool, GeoSearchMode::EarthDistance),
            auth: AuthConfig::from_hmac_secret(jsonwebtoken::Algorithm::HS256, TEST_SECRET),
            fhir_base_url: "http://localhost:8080/fhir".to_owned(),
            search: SearchConfig {
                default_count: 50,
                max_count: 500,
            },
            validator: Arc::new(FhirSchemaValidator::new().expect("validator should load")),
            cors_allowed_origins: Vec::new(),
            serve_docs: false,
        }
    }

    #[tokio::test]
    async fn readyz_returns_200_when_database_reachable() {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            return;
        };
        let app = build_router(test_app(&url));
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/readyz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readyz_returns_503_when_database_unreachable() {
        let app = build_router(test_app(
            "postgres://postgres:postgres@127.0.0.1:1/postgres",
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/readyz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
