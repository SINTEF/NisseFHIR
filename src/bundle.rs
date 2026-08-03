use std::collections::{HashMap, HashSet};

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppState,
    auth::AccessContext,
    error::{AppError, OperationIssue},
    fhir::{assign_resource_id, parse_if_match_value, validate_resource_payload},
    validation::FhirSchemaValidator,
};

/// Parse a Bundle entry's `request` into method + url parts.
struct EntryRequest {
    method: String,
    resource_type: String,
    id: Option<String>,
    if_match: Option<i64>,
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
    let if_match = request
        .get("ifMatch")
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "entry.request.ifMatch must be a string".to_owned())
                .and_then(parse_if_match_value)
        })
        .transpose()?;

    Ok(EntryRequest {
        method,
        resource_type,
        id,
        if_match,
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
///
/// Per the FHIR Bundle spec, the `OperationOutcome` is placed in
/// `entry.response.outcome`, never in `entry.resource`.
fn error_entry(err: &EntryError) -> Value {
    json!({
        "response": {
            "status": err.status_line(),
            "outcome": err.outcome(),
        }
    })
}

/// Error while processing a single Bundle entry.
///
/// Carries a structured HTTP classification so batch responses can report
/// accurate statuses (`400`, `403`, `404`, `409`, `412`, `5xx`) instead of
/// collapsing every failure to `400 Bad Request`, and so a failed
/// transaction can roll back with the appropriate top-level status.
#[derive(Debug)]
enum EntryError {
    /// 400 Bad Request — malformed entry or payload.
    BadRequest(String),
    /// 403 Forbidden — token lacks access to this resource type/interaction.
    Forbidden(String),
    /// 404 Not Found — targeted resource does not exist.
    NotFound(String),
    /// 409 Conflict — id collision or duplicate.
    Conflict(String),
    /// 412 Precondition Failed — If-Match did not match.
    PreconditionFailed(String),
    /// 400 Bad Request with structured OperationOutcome issues.
    Validation(Vec<OperationIssue>),
    /// 500 Internal Server Error — internal/DB failure. No message is exposed.
    Internal,
}

impl EntryError {
    /// Map an [`AppError`] returned by the store or validator to an
    /// [`EntryError`], preserving the HTTP classification and never exposing
    /// internal database messages or SQL details.
    fn from_app(err: AppError) -> Self {
        match err {
            AppError::Conflict(msg) => EntryError::Conflict(msg),
            AppError::PreconditionFailed(msg) => EntryError::PreconditionFailed(msg),
            AppError::Validation(issues) => EntryError::Validation(issues),
            AppError::BadRequest(msg) => EntryError::BadRequest(msg),
            AppError::PayloadTooLarge => EntryError::BadRequest(
                "request payload exceeds the maximum allowed size".to_owned(),
            ),
            AppError::UnsupportedMediaType(msg) | AppError::NotAcceptable(msg) => {
                EntryError::BadRequest(msg)
            }
            AppError::Forbidden => EntryError::Forbidden(
                "token does not grant access to this resource or interaction".to_owned(),
            ),
            AppError::Unauthorized => {
                EntryError::Forbidden("missing or invalid bearer token".to_owned())
            }
            AppError::NotFound => {
                EntryError::NotFound("requested resource was not found".to_owned())
            }
            // Database errors and internal errors never expose their message.
            AppError::Database(_) | AppError::Internal(_) => EntryError::Internal,
        }
    }

    /// HTTP status line ("400 Bad Request") used in batch response entries.
    fn status_line(&self) -> &'static str {
        match self {
            EntryError::BadRequest(_) | EntryError::Validation(_) => "400 Bad Request",
            EntryError::Forbidden(_) => "403 Forbidden",
            EntryError::NotFound(_) => "404 Not Found",
            EntryError::Conflict(_) => "409 Conflict",
            EntryError::PreconditionFailed(_) => "412 Precondition Failed",
            EntryError::Internal => "500 Internal Server Error",
        }
    }

    /// Build the `OperationOutcome` body for this entry failure.
    fn outcome(&self) -> Value {
        match self {
            EntryError::Validation(issues) => {
                let issue_values: Vec<Value> = issues
                    .iter()
                    .map(|issue| {
                        serde_json::to_value(issue).unwrap_or_else(|_| {
                            json!({
                                "severity": "error",
                                "code": "invalid",
                                "diagnostics": "validation failed",
                            })
                        })
                    })
                    .collect();
                json!({
                    "resourceType": "OperationOutcome",
                    "issue": issue_values,
                })
            }
            EntryError::Internal => json!({
                "resourceType": "OperationOutcome",
                "issue": [{
                    "severity": "error",
                    "code": "exception",
                    "diagnostics": "internal server error",
                }],
            }),
            EntryError::BadRequest(msg) => issue_outcome("invalid", msg),
            EntryError::Forbidden(msg) => issue_outcome("forbidden", msg),
            EntryError::NotFound(msg) => issue_outcome("not-found", msg),
            EntryError::Conflict(msg) => issue_outcome("conflict", msg),
            EntryError::PreconditionFailed(msg) => issue_outcome("conflict", msg),
        }
    }

    /// Convert back to a top-level [`AppError`] for transaction rollback.
    fn into_app_error(self) -> AppError {
        match self {
            EntryError::BadRequest(msg) => AppError::BadRequest(msg),
            EntryError::Forbidden(_) => AppError::Forbidden,
            EntryError::NotFound(_) => AppError::NotFound,
            EntryError::Conflict(msg) => AppError::Conflict(msg),
            EntryError::PreconditionFailed(msg) => AppError::PreconditionFailed(msg),
            EntryError::Validation(issues) => AppError::Validation(issues),
            EntryError::Internal => AppError::Internal("bundle entry processing failed".to_owned()),
        }
    }
}

fn issue_outcome(code: &str, diagnostics: &str) -> Value {
    json!({
        "resourceType": "OperationOutcome",
        "issue": [{
            "severity": "error",
            "code": code,
            "diagnostics": diagnostics,
        }],
    })
}

impl From<String> for EntryError {
    fn from(message: String) -> Self {
        Self::BadRequest(message)
    }
}

impl From<&str> for EntryError {
    fn from(message: &str) -> Self {
        Self::BadRequest(message.to_owned())
    }
}

/// Apply the same resource-type and interaction authorization used by the
/// standalone CRUD handlers to a single Bundle entry.
fn authorize_entry(access: &AccessContext, req: &EntryRequest) -> Result<(), EntryError> {
    let interaction_allowed = match req.method.as_str() {
        "GET" => access.can_read,
        // Unknown methods are rejected later as a bad request, not as an
        // authorization failure.
        _ => access.can_write,
    };

    if !interaction_allowed || !access.can_access_resource_type(&req.resource_type) {
        return Err(EntryError::Forbidden(format!(
            "token does not grant {} access to resource type '{}'",
            req.method, req.resource_type
        )));
    }

    Ok(())
}

/// Identities worked out for a transaction Bundle before any entry executes.
///
/// A transaction may reference resources it is creating in the same Bundle,
/// including forwards and in cycles, so every identity has to be known before
/// the first write. Planning up front also makes the outcome independent of
/// entry order.
struct TransactionPlan {
    /// `ResourceType/id` per entry, for entries that establish an identity.
    targets: Vec<Option<String>>,
    /// Server-assigned id per entry, for `POST` entries only.
    assigned_ids: Vec<Option<String>>,
    /// `fullUrl` → `ResourceType/id`, used to rewrite internal links.
    identities: HashMap<String, String>,
}

/// Assign ids, reject ambiguous identities, and collect the `fullUrl` mapping
/// for a transaction Bundle.
fn plan_transaction(entries: &[Value]) -> Result<TransactionPlan, EntryError> {
    let mut targets = Vec::with_capacity(entries.len());
    let mut assigned_ids = Vec::with_capacity(entries.len());
    let mut identities = HashMap::new();
    let mut claimed_targets = HashSet::new();
    let mut seen_full_urls = HashSet::new();

    for entry in entries {
        let req = parse_entry_request(entry)?;

        // Only creates and updates establish an identity other entries can
        // point at; reads and deletes target resources that already exist.
        let (assigned_id, target) = match req.method.as_str() {
            "POST" => {
                let id = Uuid::new_v4().to_string();
                let target = format!("{}/{}", req.resource_type, id);
                (Some(id), Some(target))
            }
            "PUT" => {
                let id = req
                    .id
                    .as_deref()
                    .ok_or("PUT requires a resource id in the URL")?;
                (None, Some(format!("{}/{}", req.resource_type, id)))
            }
            _ => (None, None),
        };

        if let Some(target) = &target
            && !claimed_targets.insert(target.clone())
        {
            return Err(EntryError::Conflict(format!(
                "transaction contains more than one entry targeting '{target}'"
            )));
        }

        if let Some(full_url) = entry.get("fullUrl").and_then(Value::as_str)
            && !full_url.is_empty()
        {
            if !seen_full_urls.insert(full_url.to_owned()) {
                return Err(EntryError::Conflict(format!(
                    "transaction contains duplicate fullUrl '{full_url}'"
                )));
            }
            if let Some(target) = &target {
                identities.insert(full_url.to_owned(), target.clone());
            }
        }

        targets.push(target);
        assigned_ids.push(assigned_id);
    }

    Ok(TransactionPlan {
        targets,
        assigned_ids,
        identities,
    })
}

/// Rewrite links in each entry's resource so they use the identities this
/// server assigned.
fn resolve_entry_links(entries: &mut [Value], identities: &HashMap<String, String>) {
    if identities.is_empty() {
        return;
    }
    for entry in entries {
        if let Some(resource) = entry.get_mut("resource") {
            resolve_links(resource, identities);
        }
    }
}

/// Replace `fullUrl` links with their resolved identity throughout a resource.
///
/// Only `Reference.reference` values and `href`/`src` attributes in narrative
/// XHTML are rewritten. Other URI-valued elements — canonicals in particular,
/// which the specification excludes — are left alone, and the traversal is
/// structural so no string-wide replacement can corrupt unrelated data.
fn resolve_links(value: &mut Value, identities: &HashMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                match (key.as_str(), child) {
                    ("reference", Value::String(link)) => {
                        if let Some(resolved) = resolve_link(link, identities) {
                            *link = resolved;
                        }
                    }
                    ("div", Value::String(div)) => *div = resolve_narrative(div, identities),
                    (_, child) => resolve_links(child, identities),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                resolve_links(item, identities);
            }
        }
        _ => {}
    }
}

/// Resolve a single link, matching either the whole value or the URL portion
/// preceding a fragment. Links without a matching entry are left unchanged, as
/// the specification requires.
fn resolve_link(link: &str, identities: &HashMap<String, String>) -> Option<String> {
    if let Some(target) = identities.get(link) {
        return Some(target.clone());
    }
    let (url, fragment) = link.split_once('#')?;
    let target = identities.get(url)?;
    Some(format!("{target}#{fragment}"))
}

/// Resolve links held in narrative XHTML attributes.
fn resolve_narrative(div: &str, identities: &HashMap<String, String>) -> String {
    let mut resolved = div.to_owned();
    for attribute in ["href=\"", "src=\""] {
        resolved = resolve_attribute(&resolved, attribute, identities);
    }
    resolved
}

fn resolve_attribute(div: &str, attribute: &str, identities: &HashMap<String, String>) -> String {
    let mut resolved = String::with_capacity(div.len());
    let mut rest = div;

    while let Some(start) = rest.find(attribute) {
        let (head, tail) = rest.split_at(start + attribute.len());
        resolved.push_str(head);
        let Some(end) = tail.find('"') else {
            // Unterminated attribute: leave the remainder untouched.
            rest = tail;
            break;
        };
        let (link, remainder) = tail.split_at(end);
        match resolve_link(link, identities) {
            Some(target) => resolved.push_str(&target),
            None => resolved.push_str(link),
        }
        rest = remainder;
    }

    resolved.push_str(rest);
    resolved
}

/// Process a single Bundle entry against the store.
/// Returns the response entry Value on success.
async fn process_single_entry<E>(
    executor: &mut E,
    tenant_id: &str,
    entry: &Value,
    validator: &FhirSchemaValidator,
    access: &AccessContext,
    // Id reserved for this entry by transaction planning, if any.
    assigned_id: Option<&str>,
) -> Result<Value, EntryError>
where
    E: BundleExecutor,
{
    let req = parse_entry_request(entry)?;
    authorize_entry(access, &req)?;

    match req.method.as_str() {
        "POST" => {
            // Create
            let mut resource = entry
                .get("resource")
                .cloned()
                .ok_or("POST entry must include a resource")?;

            validate_resource_payload(&req.resource_type, &resource, None)
                .map_err(EntryError::from_app)?;
            assign_resource_id(&mut resource, assigned_id).map_err(EntryError::from_app)?;
            validator
                .validate_resource(&req.resource_type, &resource)
                .map_err(EntryError::from_app)?;

            let id = resource
                .get("id")
                .and_then(Value::as_str)
                .ok_or("resource is missing its id after assignment")?
                .to_owned();
            let stored = executor
                .exec_create(tenant_id, &req.resource_type, &id, resource)
                .await
                .map_err(EntryError::from_app)?
                .ok_or_else(|| {
                    EntryError::Conflict(
                        "a resource with the generated id already exists".to_owned(),
                    )
                })?;
            let id = &stored.id;

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

            validate_resource_payload(&req.resource_type, &resource, Some(id))
                .map_err(EntryError::from_app)?;
            assign_resource_id(&mut resource, Some(id)).map_err(EntryError::from_app)?;

            validator
                .validate_resource(&req.resource_type, &resource)
                .map_err(EntryError::from_app)?;

            let (stored, created) = if let Some(expected_version) = req.if_match {
                let stored = executor
                    .exec_update_if_version_matches(
                        tenant_id,
                        &req.resource_type,
                        id,
                        expected_version,
                        resource,
                    )
                    .await
                    .map_err(EntryError::from_app)?
                    .ok_or_else(|| {
                        EntryError::PreconditionFailed(
                            "If-Match version does not match an existing resource".to_owned(),
                        )
                    })?;
                (stored, false)
            } else {
                let result = executor
                    .exec_upsert(tenant_id, &req.resource_type, id, resource)
                    .await
                    .map_err(EntryError::from_app)?;
                (result.stored, result.created)
            };
            let status = if created { "201 Created" } else { "200 OK" };

            Ok(success_entry(
                status,
                Some(&stored.resource),
                Some(format!(
                    "{}/{}/_history/{}",
                    req.resource_type, id, stored.version_id
                )),
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
                .map_err(EntryError::from_app)?;

            match found {
                Some(stored) => Ok(success_entry(
                    "200 OK",
                    Some(&stored.resource),
                    None,
                    Some(format!("W/\"{}\"", stored.version_id)),
                    Some(stored.last_updated.to_rfc3339()),
                )),
                None => Err(EntryError::NotFound("resource not found".to_owned())),
            }
        }
        "DELETE" => {
            let id = req
                .id
                .as_deref()
                .ok_or("DELETE requires a resource id in the URL")?;

            if let Some(expected_version) = req.if_match {
                match executor
                    .exec_delete_if_version_matches(
                        tenant_id,
                        &req.resource_type,
                        id,
                        expected_version,
                    )
                    .await
                    .map_err(EntryError::from_app)?
                {
                    crate::store::DeleteIfMatchOutcome::Deleted { .. } => {
                        Ok(success_entry("204 No Content", None, None, None, None))
                    }
                    crate::store::DeleteIfMatchOutcome::VersionMismatch => {
                        Err(EntryError::PreconditionFailed(
                            "If-Match version does not match an existing resource".to_owned(),
                        ))
                    }
                    crate::store::DeleteIfMatchOutcome::NotFound => {
                        Err(EntryError::NotFound("resource not found".to_owned()))
                    }
                }
            } else {
                let deleted = executor
                    .exec_delete(tenant_id, &req.resource_type, id)
                    .await
                    .map_err(EntryError::from_app)?;

                if deleted {
                    Ok(success_entry("204 No Content", None, None, None, None))
                } else {
                    Err(EntryError::NotFound("resource not found".to_owned()))
                }
            }
        }
        other => Err(EntryError::BadRequest(format!(
            "unsupported HTTP method '{other}'"
        ))),
    }
}

/// Trait abstracting database execution for Bundle entries so both
/// transaction (single TX) and batch (per-entry) modes share the same logic.
#[allow(async_fn_in_trait)]
trait BundleExecutor {
    async fn exec_create(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<Option<crate::store::StoredResource>, AppError>;

    async fn exec_upsert(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<crate::store::UpsertResult, AppError>;

    async fn exec_update_if_version_matches(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
        resource: Value,
    ) -> Result<Option<crate::store::StoredResource>, AppError>;

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

    async fn exec_delete_if_version_matches(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
    ) -> Result<crate::store::DeleteIfMatchOutcome, AppError>;
}

/// Executor that operates inside an existing database transaction.
struct TxBundleExecutor<'a> {
    tx: crate::store::TxExecutor<'a>,
}

impl BundleExecutor for TxBundleExecutor<'_> {
    async fn exec_create(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<Option<crate::store::StoredResource>, AppError> {
        crate::store::PgStore::create_in_tx(&mut self.tx, tenant_id, resource_type, id, resource)
            .await
    }

    async fn exec_upsert(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<crate::store::UpsertResult, AppError> {
        crate::store::PgStore::upsert_in_tx(&mut self.tx, tenant_id, resource_type, id, resource)
            .await
    }

    async fn exec_update_if_version_matches(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
        resource: Value,
    ) -> Result<Option<crate::store::StoredResource>, AppError> {
        crate::store::PgStore::update_if_version_matches_in_tx(
            &mut self.tx,
            tenant_id,
            resource_type,
            id,
            expected_version,
            resource,
        )
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

    async fn exec_delete_if_version_matches(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
    ) -> Result<crate::store::DeleteIfMatchOutcome, AppError> {
        crate::store::PgStore::delete_if_version_matches_in_tx(
            &mut self.tx,
            tenant_id,
            resource_type,
            id,
            expected_version,
        )
        .await
    }
}

/// Executor that uses independent pool connections (for batch mode).
struct PoolBundleExecutor<'a> {
    store: &'a crate::store::PgStore,
}

impl BundleExecutor for PoolBundleExecutor<'_> {
    async fn exec_create(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<Option<crate::store::StoredResource>, AppError> {
        self.store
            .create(tenant_id, resource_type, id, resource)
            .await
    }

    async fn exec_upsert(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<crate::store::UpsertResult, AppError> {
        self.store
            .upsert(tenant_id, resource_type, id, resource)
            .await
    }

    async fn exec_update_if_version_matches(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
        resource: Value,
    ) -> Result<Option<crate::store::StoredResource>, AppError> {
        self.store
            .update_if_version_matches(tenant_id, resource_type, id, expected_version, resource)
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

    async fn exec_delete_if_version_matches(
        &mut self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
    ) -> Result<crate::store::DeleteIfMatchOutcome, AppError> {
        self.store
            .delete_if_version_matches(tenant_id, resource_type, id, expected_version)
            .await
    }
}

/// Process a `transaction` Bundle: all entries succeed or all fail.
pub(crate) async fn process_transaction(
    state: &AppState,
    access: &AccessContext,
    mut entries: Vec<Value>,
) -> Result<Response, AppError> {
    // Plan before touching the database, so every entry knows the identity of
    // every other entry regardless of the order they appear in.
    let plan = plan_transaction(&entries).map_err(EntryError::into_app_error)?;
    resolve_entry_links(&mut entries, &plan.identities);

    let base_url = state.fhir_base_url.trim_end_matches('/');
    let mut executor = TxBundleExecutor {
        tx: state.store.begin_tx().await?,
    };

    let mut response_entries = Vec::with_capacity(entries.len());

    for ((entry, assigned_id), target) in entries.iter().zip(&plan.assigned_ids).zip(&plan.targets)
    {
        match process_single_entry(
            &mut executor,
            &access.tenant_id,
            entry,
            &state.validator,
            access,
            assigned_id.as_deref(),
        )
        .await
        {
            Ok(mut resp) => {
                if let Some(target) = target {
                    resp["fullUrl"] = Value::String(format!("{base_url}/{target}"));
                }
                response_entries.push(resp);
            }
            Err(entry_err) => {
                // Transaction mode: any failure aborts the whole thing.
                // The transaction is dropped (rolled back) automatically, and
                // we return a top-level OperationOutcome with the entry's
                // appropriate HTTP status.
                return Err(entry_err.into_app_error());
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
pub(crate) async fn process_batch(
    state: &AppState,
    access: &AccessContext,
    entries: Vec<Value>,
) -> Result<Response, AppError> {
    let mut executor = PoolBundleExecutor {
        store: &state.store,
    };

    let mut response_entries = Vec::with_capacity(entries.len());

    for entry in &entries {
        match process_single_entry(
            &mut executor,
            &access.tenant_id,
            entry,
            &state.validator,
            access,
            // Batch entries are independent, so there is no cross-entry
            // identity to reserve.
            None,
        )
        .await
        {
            Ok(resp) => response_entries.push(resp),
            // Batch mode: report the structured error inline and continue. The
            // OperationOutcome is placed in entry.response.outcome.
            Err(entry_err) => response_entries.push(error_entry(&entry_err)),
        }
    }

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch-response",
        "entry": response_entries,
    });

    Ok((StatusCode::OK, Json(bundle)).into_response())
}

#[cfg(test)]
mod tests {
    // ───────────────────────────────────────────────────────────────
    // EntryError classification / outcome mapping
    // ───────────────────────────────────────────────────────────────

    use super::{EntryError, error_entry, plan_transaction, resolve_entry_links};
    use crate::error::OperationIssue;
    use serde_json::{Value, json};

    fn post_entry(full_url: &str, resource: Value) -> Value {
        json!({
            "fullUrl": full_url,
            "resource": resource,
            "request": {"method": "POST", "url": resource_type_of(&resource)},
        })
    }

    fn resource_type_of(resource: &Value) -> String {
        resource["resourceType"]
            .as_str()
            .expect("resourceType present")
            .to_owned()
    }

    // ───────────────────────────────────────────────────────────────
    // Transaction planning and link resolution
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn plan_assigns_ids_and_maps_full_urls() {
        let entries = vec![post_entry(
            "urn:uuid:aaa",
            json!({"resourceType": "Patient"}),
        )];
        let plan = plan_transaction(&entries).expect("plan should succeed");

        let id = plan.assigned_ids[0].as_deref().expect("POST gets an id");
        assert_eq!(
            plan.targets[0].as_deref(),
            Some(format!("Patient/{id}")).as_deref()
        );
        assert_eq!(plan.identities["urn:uuid:aaa"], format!("Patient/{id}"));
    }

    #[test]
    fn plan_rejects_duplicate_full_urls_and_targets() {
        let duplicate_full_urls = vec![
            post_entry("urn:uuid:aaa", json!({"resourceType": "Patient"})),
            post_entry("urn:uuid:aaa", json!({"resourceType": "Observation"})),
        ];
        assert!(matches!(
            plan_transaction(&duplicate_full_urls),
            Err(EntryError::Conflict(_))
        ));

        let overlapping_targets = vec![
            json!({"request": {"method": "PUT", "url": "Patient/1"}}),
            json!({"request": {"method": "PUT", "url": "Patient/1"}}),
        ];
        assert!(matches!(
            plan_transaction(&overlapping_targets),
            Err(EntryError::Conflict(_))
        ));
    }

    #[test]
    fn resolution_is_independent_of_entry_order() {
        // The Observation references the Patient that appears after it.
        let mut entries = vec![
            post_entry(
                "urn:uuid:obs",
                json!({
                    "resourceType": "Observation",
                    "subject": {"reference": "urn:uuid:pat"},
                }),
            ),
            post_entry(
                "urn:uuid:pat",
                json!({
                    "resourceType": "Patient",
                    "link": [{"other": {"reference": "urn:uuid:obs"}}],
                }),
            ),
        ];
        let plan = plan_transaction(&entries).expect("plan should succeed");
        resolve_entry_links(&mut entries, &plan.identities);

        let patient = plan.targets[1].as_deref().expect("patient target");
        let observation = plan.targets[0].as_deref().expect("observation target");
        assert_eq!(entries[0]["resource"]["subject"]["reference"], patient);
        assert_eq!(
            entries[1]["resource"]["link"][0]["other"]["reference"],
            observation
        );
    }

    #[test]
    fn resolution_handles_fragments_narratives_and_leaves_the_rest_alone() {
        let mut entries = vec![
            post_entry(
                "urn:uuid:pat",
                json!({
                    "resourceType": "Patient",
                    // A canonical that happens to carry the same value must not
                    // be rewritten.
                    "meta": {"profile": ["urn:uuid:pat"]},
                }),
            ),
            post_entry(
                "urn:uuid:obs",
                json!({
                    "resourceType": "Observation",
                    "subject": {"reference": "urn:uuid:pat#section"},
                    "performer": [{"reference": "urn:uuid:absent"}],
                    "encounter": {"reference": "http://example.org/fhir/Encounter/7"},
                    "text": {
                        "div": "<div><a href=\"urn:uuid:pat\">subject</a> and <a href=\"http://example.org\">other</a></div>",
                    },
                }),
            ),
        ];
        let plan = plan_transaction(&entries).expect("plan should succeed");
        resolve_entry_links(&mut entries, &plan.identities);

        let patient = plan.targets[0].as_deref().expect("patient target");
        let observation = &entries[1]["resource"];

        assert_eq!(
            observation["subject"]["reference"],
            format!("{patient}#section")
        );
        // Unmatched and external links are stored as sent.
        assert_eq!(observation["performer"][0]["reference"], "urn:uuid:absent");
        assert_eq!(
            observation["encounter"]["reference"],
            "http://example.org/fhir/Encounter/7"
        );
        assert_eq!(
            observation["text"]["div"],
            format!(
                "<div><a href=\"{patient}\">subject</a> and <a href=\"http://example.org\">other</a></div>"
            )
        );
        // Canonicals are excluded from replacement.
        assert_eq!(entries[0]["resource"]["meta"]["profile"][0], "urn:uuid:pat");
    }

    #[test]
    fn entry_error_from_app_preserves_classification() {
        use crate::error::AppError;

        assert!(matches!(
            EntryError::from_app(AppError::BadRequest("bad".to_owned())),
            EntryError::BadRequest(_)
        ));
        assert!(matches!(
            EntryError::from_app(AppError::Forbidden),
            EntryError::Forbidden(_)
        ));
        assert!(matches!(
            EntryError::from_app(AppError::NotFound),
            EntryError::NotFound(_)
        ));
        assert!(matches!(
            EntryError::from_app(AppError::Conflict("dup".to_owned())),
            EntryError::Conflict(_)
        ));
        assert!(matches!(
            EntryError::from_app(AppError::PreconditionFailed("stale".to_owned())),
            EntryError::PreconditionFailed(_)
        ));
        assert!(matches!(
            EntryError::from_app(AppError::Validation(vec![OperationIssue::error(
                "invalid", "x"
            )])),
            EntryError::Validation(_)
        ));
    }

    #[test]
    fn entry_error_internal_never_exposes_db_message() {
        use crate::error::AppError;

        let db_err = AppError::Database(sqlx::Error::Configuration("sql secrets".into()));
        let entry_err = EntryError::from_app(db_err);
        let outcome = entry_err.outcome();
        let diagnostics = outcome["issue"][0]["diagnostics"]
            .as_str()
            .expect("diagnostics present");
        assert!(!diagnostics.contains("sql secrets"));
        assert_eq!(entry_err.status_line(), "500 Internal Server Error");
        assert_eq!(outcome["issue"][0]["code"], "exception");

        let internal = EntryError::from_app(AppError::Internal("boom detail".to_owned()));
        let outcome = internal.outcome();
        assert!(!outcome.to_string().contains("boom detail"));
    }

    #[test]
    fn entry_error_status_lines_match_http_classes() {
        assert_eq!(
            EntryError::BadRequest("x".to_owned()).status_line(),
            "400 Bad Request"
        );
        assert_eq!(
            EntryError::Forbidden("x".to_owned()).status_line(),
            "403 Forbidden"
        );
        assert_eq!(
            EntryError::NotFound("x".to_owned()).status_line(),
            "404 Not Found"
        );
        assert_eq!(
            EntryError::Conflict("x".to_owned()).status_line(),
            "409 Conflict"
        );
        assert_eq!(
            EntryError::PreconditionFailed("x".to_owned()).status_line(),
            "412 Precondition Failed"
        );
        assert_eq!(
            EntryError::Internal.status_line(),
            "500 Internal Server Error"
        );
        assert_eq!(
            EntryError::Validation(vec![OperationIssue::error("invalid", "x")]).status_line(),
            "400 Bad Request"
        );
    }

    #[test]
    fn error_entry_places_outcome_in_response_not_resource() {
        let entry = error_entry(&EntryError::NotFound("missing".to_owned()));
        assert_eq!(entry["response"]["status"], "404 Not Found");
        assert_eq!(
            entry["response"]["outcome"]["resourceType"],
            "OperationOutcome"
        );
        assert_eq!(
            entry["response"]["outcome"]["issue"][0]["code"],
            "not-found"
        );
        // The outcome must never be placed in entry.resource.
        assert!(entry.get("resource").is_none());
    }

    #[test]
    fn entry_error_into_app_preserves_status_for_transaction_rollback() {
        use crate::error::AppError;

        assert!(matches!(
            EntryError::Forbidden("x".to_owned()).into_app_error(),
            AppError::Forbidden
        ));
        assert!(matches!(
            EntryError::NotFound("x".to_owned()).into_app_error(),
            AppError::NotFound
        ));
        assert!(matches!(
            EntryError::Conflict("dup".to_owned()).into_app_error(),
            AppError::Conflict(_)
        ));
        assert!(matches!(
            EntryError::PreconditionFailed("stale".to_owned()).into_app_error(),
            AppError::PreconditionFailed(_)
        ));
        assert!(matches!(
            EntryError::Validation(vec![OperationIssue::error("invalid", "x")]).into_app_error(),
            AppError::Validation(_)
        ));
        assert!(matches!(
            EntryError::Internal.into_app_error(),
            AppError::Internal(_)
        ));
    }
}
