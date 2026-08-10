pub mod compartment;
pub mod cursor;
pub mod references;

use std::collections::{BTreeMap, BTreeSet};

use axum::{
    Json,
    extract::{Path, RawQuery, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    AppState,
    auth::{AccessContext, extract_access_context},
    error::AppError,
    search_params::{DateBounds, RESOURCE_TYPES, parse_fhir_date},
    store::EverythingCandidate,
};

pub const SUPPORT_RESOURCE_TYPES: &[&str] = &[
    "Binary",
    "Device",
    "Location",
    "Medication",
    "Observation",
    "Organization",
    "Practitioner",
    "PractitionerRole",
    "RelatedPerson",
    "Specimen",
];

const MAX_DISCOVERED_KEYS: usize = 10_000;
const MAX_GROUP_MEMBERS: usize = 100;
const MAX_BUNDLE_BYTES: usize = 16 * 1024 * 1024;
const CURSOR_LIFETIME_SECONDS: i64 = 900;

type CareInterval = (Option<DateTime<Utc>>, Option<DateTime<Utc>>);

#[derive(Clone, Debug, Default)]
struct EverythingRequest {
    start: Option<(String, DateBounds)>,
    end: Option<(String, DateBounds)>,
    since: Option<(String, DateTime<Utc>)>,
    resource_types: Option<BTreeSet<String>>,
    count: u32,
    cursor: Option<String>,
}

#[derive(Clone, Copy)]
enum OperationKind {
    Patient,
    Group,
}

impl OperationKind {
    fn cursor_name(self) -> &'static str {
        match self {
            Self::Patient => "patient-everything",
            Self::Group => "group-everything",
        }
    }

    fn resource_type(self) -> &'static str {
        match self {
            Self::Patient => "Patient",
            Self::Group => "Group",
        }
    }
}

pub async fn patient_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
) -> Result<Response, AppError> {
    let request = parse_request(query.as_deref().unwrap_or(""), None, &state)?;
    execute(state, headers, OperationKind::Patient, id, request).await
}

pub async fn patient_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Response, AppError> {
    crate::media_type::validate_request_content_type(
        &headers,
        crate::media_type::BodyKind::FhirResource,
    )?;
    let Json(body) =
        payload.map_err(|error| AppError::BadRequest(format!("invalid JSON payload: {error}")))?;
    let request = parse_request(query.as_deref().unwrap_or(""), Some(&body), &state)?;
    execute(state, headers, OperationKind::Patient, id, request).await
}

pub async fn group_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
) -> Result<Response, AppError> {
    let request = parse_request(query.as_deref().unwrap_or(""), None, &state)?;
    execute(state, headers, OperationKind::Group, id, request).await
}

pub async fn group_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Response, AppError> {
    crate::media_type::validate_request_content_type(
        &headers,
        crate::media_type::BodyKind::FhirResource,
    )?;
    let Json(body) =
        payload.map_err(|error| AppError::BadRequest(format!("invalid JSON payload: {error}")))?;
    let request = parse_request(query.as_deref().unwrap_or(""), Some(&body), &state)?;
    execute(state, headers, OperationKind::Group, id, request).await
}

/// Type-level Patient `$everything` is deliberately present but fails closed
/// until authorization can provide an explicit patient set.
pub async fn patient_type_get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    reject_patient_type(&state, &headers)
}

pub async fn patient_type_post(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    reject_patient_type(&state, &headers)
}

fn reject_patient_type(state: &AppState, headers: &HeaderMap) -> Result<Response, AppError> {
    let access = extract_access_context(headers, &state.auth)?;
    if !access.can_read || !access.can_access_resource_type("Patient") {
        return Err(AppError::Forbidden);
    }
    Err(AppError::UnprocessableEntity(
        "Patient/$everything requires an authorization context containing an explicit patient set"
            .to_owned(),
    ))
}

fn parse_request(
    query: &str,
    body: Option<&Value>,
    state: &AppState,
) -> Result<EverythingRequest, AppError> {
    if query.len() > crate::MAX_SEARCH_QUERY_BYTES {
        return Err(AppError::BadRequest(
            "$everything query is too large".to_owned(),
        ));
    }
    let query_values: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let body_values = body.map(parse_parameters).transpose()?.unwrap_or_default();
    if query_values.len() + body_values.len() > crate::MAX_SEARCH_PARAMETER_OCCURRENCES {
        return Err(AppError::BadRequest(
            "$everything has too many parameter occurrences".to_owned(),
        ));
    }
    let query_names: BTreeSet<_> = query_values.iter().map(|(name, _)| name.as_str()).collect();
    let body_names: BTreeSet<_> = body_values.iter().map(|(name, _)| name.as_str()).collect();
    if let Some(name) = query_names.intersection(&body_names).next() {
        return Err(AppError::BadRequest(format!(
            "parameter '{name}' must not be supplied in both the URL and Parameters body"
        )));
    }

    let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, value) in query_values.into_iter().chain(body_values) {
        if !matches!(
            name.as_str(),
            "start" | "end" | "_since" | "_type" | "_count" | "_cursor"
        ) {
            return Err(AppError::BadRequest(format!(
                "unknown $everything parameter '{name}'"
            )));
        }
        values.entry(name).or_default().push(value);
    }
    for singleton in ["start", "end", "_since", "_count", "_cursor"] {
        if values.get(singleton).is_some_and(|items| items.len() > 1) {
            return Err(AppError::BadRequest(format!(
                "duplicate parameter '{singleton}'"
            )));
        }
    }

    let start = values
        .remove("start")
        .map(|mut values| values.remove(0))
        .map(|raw| {
            parse_fhir_date(&raw)
                .map(|bounds| (raw, bounds))
                .map_err(|error| AppError::BadRequest(format!("invalid start: {error}")))
        })
        .transpose()?;
    let end = values
        .remove("end")
        .map(|mut values| values.remove(0))
        .map(|raw| {
            parse_fhir_date(&raw)
                .map(|bounds| (raw, bounds))
                .map_err(|error| AppError::BadRequest(format!("invalid end: {error}")))
        })
        .transpose()?;
    if start
        .as_ref()
        .zip(end.as_ref())
        .is_some_and(|(start, end)| start.1.start >= end.1.end)
    {
        return Err(AppError::BadRequest(
            "start/end range is empty or contradictory".to_owned(),
        ));
    }
    let since = values
        .remove("_since")
        .map(|mut values| values.remove(0))
        .map(|raw| {
            DateTime::parse_from_rfc3339(&raw)
                .map(|value| (raw, value.with_timezone(&Utc)))
                .map_err(|_| AppError::BadRequest("_since must be a valid FHIR instant".to_owned()))
        })
        .transpose()?;
    let count = values
        .remove("_count")
        .map(|mut values| values.remove(0))
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| AppError::BadRequest("_count must be a positive integer".to_owned()))
        })
        .transpose()?
        .unwrap_or(state.search.default_count);
    if count == 0 || count > state.search.max_count {
        return Err(AppError::BadRequest(
            "_count is outside the configured bounds".to_owned(),
        ));
    }
    let mut types = BTreeSet::new();
    for occurrence in values.remove("_type").unwrap_or_default() {
        for resource_type in occurrence.split(',') {
            if resource_type.is_empty()
                || !crate::search_params::is_valid_resource_type(resource_type)
            {
                return Err(AppError::BadRequest(format!(
                    "unknown resource type '{resource_type}' in _type"
                )));
            }
            if !is_everything_type(resource_type) {
                return Err(AppError::UnprocessableEntity(format!(
                    "resource type '{resource_type}' is not supported by $everything"
                )));
            }
            types.insert(resource_type.to_owned());
        }
    }
    if (start.is_some() || end.is_some())
        && (types.is_empty()
            || types.iter().any(|resource_type| {
                compartment::is_direct_patient_compartment_type(resource_type)
                    && resource_type != "Patient"
                    && !has_care_date_rule(resource_type)
                    && !SUPPORT_RESOURCE_TYPES.contains(&resource_type.as_str())
            }))
    {
        return Err(AppError::UnprocessableEntity(
            "start/end requires _type narrowed to resource types with a documented care-date rule"
                .to_owned(),
        ));
    }
    let cursor = values.remove("_cursor").map(|mut values| values.remove(0));
    Ok(EverythingRequest {
        start,
        end,
        since,
        resource_types: (!types.is_empty()).then_some(types),
        count,
        cursor,
    })
}

fn parse_parameters(body: &Value) -> Result<Vec<(String, String)>, AppError> {
    if body.get("resourceType").and_then(Value::as_str) != Some("Parameters") {
        return Err(AppError::BadRequest(
            "POST $everything body must be a Parameters resource".to_owned(),
        ));
    }
    let parameters = body
        .get("parameter")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut result = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let name = parameter
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AppError::BadRequest("Parameters.parameter.name is required".to_owned())
            })?;
        let value = match name {
            "start" | "end" => parameter
                .get("valueDate")
                .and_then(Value::as_str)
                .map(str::to_owned),
            "_since" => parameter
                .get("valueInstant")
                .and_then(Value::as_str)
                .map(str::to_owned),
            "_type" => parameter
                .get("valueCode")
                .and_then(Value::as_str)
                .map(str::to_owned),
            "_count" => parameter
                .get("valueInteger")
                .and_then(Value::as_i64)
                .map(|value| value.to_string()),
            "_cursor" => parameter
                .get("valueString")
                .and_then(Value::as_str)
                .map(str::to_owned),
            _ => {
                return Err(AppError::BadRequest(format!(
                    "unknown $everything parameter '{name}'"
                )));
            }
        }
        .ok_or_else(|| {
            AppError::BadRequest(format!("parameter '{name}' has the wrong value type"))
        })?;
        result.push((name.to_owned(), value));
    }
    Ok(result)
}

fn is_everything_type(resource_type: &str) -> bool {
    resource_type == "Patient"
        || compartment::is_direct_patient_compartment_type(resource_type)
        || SUPPORT_RESOURCE_TYPES.contains(&resource_type)
}

async fn execute(
    state: AppState,
    headers: HeaderMap,
    operation: OperationKind,
    context_id: String,
    request: EverythingRequest,
) -> Result<Response, AppError> {
    let started = std::time::Instant::now();
    let access = extract_access_context(&headers, &state.auth)?;
    authorize_context(&access, operation, &request)?;

    let patient_ids = match operation {
        OperationKind::Patient => {
            if state
                .store
                .read(&access.tenant_id, "Patient", &context_id)
                .await?
                .is_none()
            {
                return Err(AppError::NotFound);
            }
            vec![context_id.clone()]
        }
        OperationKind::Group => group_patient_ids(&state, &access, &context_id).await?,
    };

    let allowed_types = allowed_types(&access, request.resource_types.as_ref());
    let auth_fingerprint = fingerprint(&json!({
        "tenant": access.tenant_id,
        "subject": access.subject_id,
        "types": access.resource_allow_list,
    }))?;
    let filters_fingerprint = fingerprint(&json!({
        "start": request.start.as_ref().map(|value| &value.0),
        "end": request.end.as_ref().map(|value| &value.0),
        "since": request.since.as_ref().map(|value| &value.0),
        "types": request.resource_types,
        "count": request.count,
    }))?;
    let cursor_payload = if let Some(cursor) = &request.cursor {
        let payload = cursor::decode(cursor, &state.everything_cursor_secret)?;
        if payload.operation != operation.cursor_name()
            || payload.context_id != context_id
            || payload.auth_fingerprint != auth_fingerprint
            || payload.filters_fingerprint != filters_fingerprint
        {
            return Err(AppError::BadRequest(
                "paging cursor does not match this operation, authorization, or filter set"
                    .to_owned(),
            ));
        }
        Some(payload)
    } else {
        None
    };
    let fast_path = request.start.is_none() && request.end.is_none();
    let (candidates, database_total) = if fast_path {
        let after = cursor_payload.as_ref().map(|payload| {
            (
                payload.after.resource_type.as_str(),
                payload.after.id.as_str(),
                payload.after.version_id,
                payload.after.version_id != 0,
            )
        });
        let page = state
            .store
            .everything_page(
                &access.tenant_id,
                &patient_ids,
                &allowed_types,
                request.since.as_ref().map(|(_, value)| *value),
                after,
                i64::from(request.count),
            )
            .await?;
        (page.candidates, Some(page.total))
    } else {
        let mut candidates = state
            .store
            .everything_candidates(
                &access.tenant_id,
                &patient_ids,
                &allowed_types,
                request.since.as_ref().map(|(_, value)| *value),
                MAX_DISCOVERED_KEYS as i64,
            )
            .await?;
        if candidates.len() > MAX_DISCOVERED_KEYS {
            return Err(AppError::UnprocessableEntity(
                "$everything date-filtered requests inspect at most 10,000 compartment resources before applying start/end; narrow _type or _since, or use asynchronous export"
                    .to_owned(),
            ));
        }
        candidates.retain(|candidate| matches_filters(candidate, &request));
        retain_date_relevant_support(&mut candidates, &state.fhir_base_url)?;
        (candidates, None)
    };
    let discovered_count = candidates.len();
    let offset = if fast_path {
        0
    } else if let Some(payload) = cursor_payload {
        candidates
            .iter()
            .position(|candidate| candidate_cursor_key(candidate) > payload.after)
            .unwrap_or(candidates.len())
    } else {
        0
    };
    if offset > candidates.len() {
        return Err(AppError::BadRequest(
            "paging cursor is outside the result set".to_owned(),
        ));
    }
    let total = database_total.unwrap_or(candidates.len() as i64);
    let end = (offset + request.count as usize).min(candidates.len());
    let page = &candidates[offset..end];
    let self_url = operation_url(
        &state,
        operation,
        &context_id,
        &request,
        request.cursor.as_deref(),
    )?;
    let mut links = vec![json!({"relation":"self", "url":self_url})];
    if (fast_path && candidates.len() > request.count as usize)
        || (!fast_path && end < candidates.len())
    {
        let payload = cursor::CursorPayload {
            version: 1,
            operation: operation.cursor_name().to_owned(),
            context_id: context_id.clone(),
            auth_fingerprint,
            filters_fingerprint,
            after: candidate_cursor_key(
                page.last()
                    .expect("a next page requires a non-empty current page"),
            ),
            expires_at: Utc::now().timestamp() + CURSOR_LIFETIME_SECONDS,
        };
        let cursor = cursor::encode(&payload, &state.everything_cursor_secret)?;
        links.push(json!({
            "relation":"next",
            "url": operation_url(&state, operation, &context_id, &request, Some(&cursor))?
        }));
    }
    let base = state.fhir_base_url.trim_end_matches('/');
    let entries: Vec<_> = page
        .iter()
        .map(|candidate| {
            let full_url = if candidate.is_historical {
                format!(
                    "{base}/{}/{}/_history/{}",
                    candidate.resource_type, candidate.id, candidate.version_id
                )
            } else {
                format!("{base}/{}/{}", candidate.resource_type, candidate.id)
            };
            json!({
                "fullUrl": full_url,
                "resource": candidate.resource,
                "search": {"mode": if candidate.is_primary { "match" } else { "include" }}
            })
        })
        .collect();
    let bundle = json!({
        "resourceType":"Bundle",
        "type":"searchset",
        "total":total,
        "link":links,
        "entry":entries,
    });
    let bytes = serde_json::to_vec(&bundle)
        .map_err(|error| AppError::Internal(format!("failed to serialize Bundle: {error}")))?;
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(AppError::UnprocessableEntity(
            "$everything page exceeds the response byte limit; reduce _count or exclude Binary"
                .to_owned(),
        ));
    }
    ::metrics::histogram!("nissefhir_everything_duration_seconds", "operation" => operation.cursor_name())
        .record(started.elapsed().as_secs_f64());
    ::metrics::histogram!("nissefhir_everything_discovered_keys", "operation" => operation.cursor_name())
        .record(discovered_count as f64);
    ::metrics::histogram!("nissefhir_everything_response_bytes", "operation" => operation.cursor_name())
        .record(bytes.len() as f64);

    let mut response = (StatusCode::OK, Json(bundle)).into_response();
    response.headers_mut().insert(
        header::DATE,
        HeaderValue::from_str(&Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string())
            .map_err(|error| AppError::Internal(format!("invalid Date header: {error}")))?,
    );
    response
        .extensions_mut()
        .insert(crate::audit::AuditResponseMetadata {
            result_count: total,
        });
    Ok(response)
}

fn candidate_cursor_key(candidate: &EverythingCandidate) -> cursor::CursorKey {
    cursor::CursorKey {
        patient_rank: u8::from(candidate.resource_type != "Patient"),
        resource_type: candidate.resource_type.clone(),
        id: candidate.id.clone(),
        // Current rows page by stable logical identity; a concurrent update
        // cannot move a row after the cursor and make it appear twice.
        version_id: if candidate.is_historical {
            candidate.version_id
        } else {
            0
        },
    }
}

fn retain_date_relevant_support(
    candidates: &mut Vec<EverythingCandidate>,
    fhir_base_url: &str,
) -> Result<(), AppError> {
    let base_url = url::Url::parse(fhir_base_url)
        .map_err(|error| AppError::Internal(format!("invalid FHIR base URL: {error}")))?;
    let mut included = BTreeSet::new();
    for candidate in candidates.iter().filter(|candidate| candidate.is_primary) {
        for reference in references::extract_references(
            &candidate.resource_type,
            &candidate.resource,
            Some(&base_url),
        ) {
            if SUPPORT_RESOURCE_TYPES.contains(&reference.target_type.as_str()) {
                included.insert((
                    reference.target_type,
                    reference.target_id,
                    reference.target_version_id,
                ));
            }
        }
    }
    // Complete the one curated depth-two support relationship.
    let depth_two_sources: Vec<_> = candidates
        .iter()
        .filter(|candidate| {
            !candidate.is_primary
                && candidate.resource_type == "PractitionerRole"
                && included.contains(&candidate_reference_key(candidate))
        })
        .map(|candidate| (candidate.resource_type.clone(), candidate.resource.clone()))
        .collect();
    for (resource_type, resource) in depth_two_sources {
        for reference in references::extract_references(&resource_type, &resource, Some(&base_url))
        {
            if SUPPORT_RESOURCE_TYPES.contains(&reference.target_type.as_str()) {
                included.insert((
                    reference.target_type,
                    reference.target_id,
                    reference.target_version_id,
                ));
            }
        }
    }
    candidates.retain(|candidate| {
        candidate.is_primary || included.contains(&candidate_reference_key(candidate))
    });
    Ok(())
}

fn candidate_reference_key(candidate: &EverythingCandidate) -> (String, String, Option<i64>) {
    (
        candidate.resource_type.clone(),
        candidate.id.clone(),
        candidate.is_historical.then_some(candidate.version_id),
    )
}

fn authorize_context(
    access: &AccessContext,
    operation: OperationKind,
    request: &EverythingRequest,
) -> Result<(), AppError> {
    if !access.can_read
        || !access.can_access_resource_type(operation.resource_type())
        || !access.can_access_resource_type("Patient")
    {
        return Err(AppError::Forbidden);
    }
    if request.resource_types.as_ref().is_some_and(|types| {
        types
            .iter()
            .any(|resource_type| !access.can_access_resource_type(resource_type))
    }) {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

fn allowed_types(access: &AccessContext, requested: Option<&BTreeSet<String>>) -> Vec<String> {
    let mut result: BTreeSet<String> = match requested {
        Some(types) => types.clone(),
        None => RESOURCE_TYPES
            .iter()
            .filter(|resource_type| {
                is_everything_type(resource_type) && access.can_access_resource_type(resource_type)
            })
            .map(|value| (*value).to_owned())
            .collect(),
    };
    // The nominated Patient is always returned for an instance operation.
    result.insert("Patient".to_owned());
    result.into_iter().collect()
}

async fn group_patient_ids(
    state: &AppState,
    access: &AccessContext,
    id: &str,
) -> Result<Vec<String>, AppError> {
    let group = state
        .store
        .read(&access.tenant_id, "Group", id)
        .await?
        .ok_or(AppError::NotFound)?;
    let resource = group.resource;
    let enumerated = resource.get("membership").and_then(Value::as_str) == Some("enumerated")
        || resource.get("actual").and_then(Value::as_bool) == Some(true);
    if resource.get("type").and_then(Value::as_str) != Some("person") || !enumerated {
        return Err(AppError::UnprocessableEntity(
            "Group/$everything supports only enumerated (actual) person Groups".to_owned(),
        ));
    }
    let now = Utc::now();
    let mut ids = BTreeSet::new();
    for member in resource
        .get("member")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if member.get("inactive").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if !period_active(member.get("period"), now) {
            continue;
        }
        let reference = member
            .pointer("/entity/reference")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AppError::UnprocessableEntity(
                    "every Group member must have a Patient reference".to_owned(),
                )
            })?;
        let local_base_url = url::Url::parse(&state.fhir_base_url)
            .map_err(|error| AppError::Internal(format!("invalid FHIR base URL: {error}")))?;
        let (resource_type, member_id, version) =
            references::parse_local_reference(reference, Some(&local_base_url)).ok_or_else(
                || {
                    AppError::UnprocessableEntity(
                        "Group members must be local Patient references".to_owned(),
                    )
                },
            )?;
        if resource_type != "Patient" || version.is_some() {
            return Err(AppError::UnprocessableEntity(
                "Group members must be current local Patient references".to_owned(),
            ));
        }
        ids.insert(member_id);
    }
    if ids.len() > MAX_GROUP_MEMBERS {
        return Err(AppError::UnprocessableEntity(format!(
            "Group has more than the synchronous limit of {MAX_GROUP_MEMBERS} Patients"
        )));
    }
    for patient_id in &ids {
        if state
            .store
            .read(&access.tenant_id, "Patient", patient_id)
            .await?
            .is_none()
        {
            return Err(AppError::UnprocessableEntity(
                "Group contains a missing Patient member".to_owned(),
            ));
        }
    }
    Ok(ids.into_iter().collect())
}

fn period_active(period: Option<&Value>, now: DateTime<Utc>) -> bool {
    let Some(period) = period else {
        return true;
    };
    let start_ok = period
        .get("start")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_none_or(|start| start.with_timezone(&Utc) <= now);
    let end_ok = period
        .get("end")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_none_or(|end| end.with_timezone(&Utc) >= now);
    start_ok && end_ok
}

fn matches_filters(candidate: &EverythingCandidate, request: &EverythingRequest) -> bool {
    if candidate.resource_type != "Patient"
        && request
            .since
            .as_ref()
            .is_some_and(|(_, since)| candidate.last_updated < *since)
    {
        return false;
    }
    if candidate.resource_type == "Patient" || !candidate.is_primary {
        return true;
    }
    if request.start.is_none() && request.end.is_none() {
        return true;
    }
    let Some(intervals) = care_intervals(&candidate.resource_type, &candidate.resource) else {
        // Context and currently unclassified compartment types are retained;
        // update-time filtering is intentionally never substituted here.
        return true;
    };
    let requested_start = request.start.as_ref().map(|(_, bounds)| bounds.start);
    let requested_end = request.end.as_ref().map(|(_, bounds)| bounds.end);
    intervals.into_iter().any(|(start, end)| {
        requested_start.is_none_or(|bound| end.is_none_or(|end| end >= bound))
            && requested_end.is_none_or(|bound| start.is_none_or(|start| start <= bound))
    })
}

fn care_intervals(resource_type: &str, resource: &Value) -> Option<Vec<CareInterval>> {
    if resource_type == "Condition" {
        let onset = date_bound(resource, &["onsetDateTime", "onsetPeriod"], true);
        let abatement = date_bound(resource, &["abatementDateTime", "abatementPeriod"], false);
        return Some(
            (onset.is_some() || abatement.is_some())
                .then_some((onset, abatement))
                .into_iter()
                .collect(),
        );
    }
    if resource_type == "MedicationRequest" {
        let authored = resource
            .get("authoredOn")
            .and_then(Value::as_str)
            .and_then(|raw| parse_fhir_date(raw).ok())
            .map(|bounds| (Some(bounds.start), Some(bounds.end)));
        let validity = resource
            .pointer("/dispenseRequest/validityPeriod")
            .and_then(Value::as_object)
            .map(|period| {
                let start = period
                    .get("start")
                    .and_then(Value::as_str)
                    .and_then(|raw| parse_fhir_date(raw).ok())
                    .map(|bounds| bounds.start);
                let end = period
                    .get("end")
                    .and_then(Value::as_str)
                    .and_then(|raw| parse_fhir_date(raw).ok())
                    .map(|bounds| bounds.end);
                (start, end)
            });
        return Some(authored.into_iter().chain(validity).collect());
    }
    let fields: &[&str] = match resource_type {
        "Encounter" => &["actualPeriod", "plannedStartDate", "plannedEndDate"],
        "Observation" => &["effectiveDateTime", "effectivePeriod"],
        "Procedure" => &["occurrenceDateTime", "occurrencePeriod"],
        "MedicationRequest" => &["authoredOn"],
        "DocumentReference" => &["date", "period"],
        "Immunization" => &["occurrenceDateTime"],
        _ => return None,
    };
    let mut intervals = Vec::new();
    for field in fields {
        let Some(value) = resource.get(*field) else {
            continue;
        };
        if let Some(raw) = value.as_str() {
            if let Ok(bounds) = parse_fhir_date(raw) {
                intervals.push((Some(bounds.start), Some(bounds.end)));
            }
        } else if let Some(period) = value.as_object() {
            let start = period
                .get("start")
                .and_then(Value::as_str)
                .and_then(|raw| parse_fhir_date(raw).ok())
                .map(|bounds| bounds.start);
            let end = period
                .get("end")
                .and_then(Value::as_str)
                .and_then(|raw| parse_fhir_date(raw).ok())
                .map(|bounds| bounds.end);
            if start.is_some() || end.is_some() {
                intervals.push((start, end));
            }
        }
    }
    Some(intervals)
}

fn date_bound(resource: &Value, fields: &[&str], start: bool) -> Option<DateTime<Utc>> {
    for field in fields {
        let Some(value) = resource.get(*field) else {
            continue;
        };
        if let Some(raw) = value.as_str()
            && let Ok(bounds) = parse_fhir_date(raw)
        {
            return Some(if start { bounds.start } else { bounds.end });
        }
        if let Some(period) = value.as_object() {
            let key = if start { "start" } else { "end" };
            if let Some(raw) = period.get(key).and_then(Value::as_str)
                && let Ok(bounds) = parse_fhir_date(raw)
            {
                return Some(if start { bounds.start } else { bounds.end });
            }
        }
    }
    None
}

fn has_care_date_rule(resource_type: &str) -> bool {
    matches!(
        resource_type,
        "Encounter"
            | "Observation"
            | "Procedure"
            | "Condition"
            | "MedicationRequest"
            | "DocumentReference"
            | "Immunization"
    )
}

fn fingerprint(value: &Value) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        AppError::Internal(format!("failed to fingerprint cursor context: {error}"))
    })?;
    Ok(URL_SAFE_NO_PAD.encode(Sha256::digest(bytes)))
}

fn operation_url(
    state: &AppState,
    operation: OperationKind,
    context_id: &str,
    request: &EverythingRequest,
    cursor: Option<&str>,
) -> Result<String, AppError> {
    let mut url = url::Url::parse(&format!(
        "{}/{}/{}/$everything",
        state.fhir_base_url.trim_end_matches('/'),
        operation.resource_type(),
        context_id
    ))
    .map_err(|error| AppError::Internal(format!("invalid FHIR base URL: {error}")))?;
    {
        let mut query = url.query_pairs_mut();
        if let Some((raw, _)) = &request.start {
            query.append_pair("start", raw);
        }
        if let Some((raw, _)) = &request.end {
            query.append_pair("end", raw);
        }
        if let Some((raw, _)) = &request.since {
            query.append_pair("_since", raw);
        }
        if let Some(types) = &request.resource_types {
            query.append_pair(
                "_type",
                &types.iter().cloned().collect::<Vec<_>>().join(","),
            );
        }
        query.append_pair("_count", &request.count.to_string());
        if let Some(cursor) = cursor {
            query.append_pair("_cursor", cursor);
        }
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::care_intervals;

    #[test]
    fn observation_effective_period_is_extracted() {
        let intervals = care_intervals(
            "Observation",
            &json!({
                "effectivePeriod":{"start":"2025-01-01", "end":"2025-02-01"}
            }),
        )
        .unwrap();
        assert_eq!(intervals.len(), 1);
        assert!(intervals[0].0.is_some() && intervals[0].1.is_some());
    }
}
