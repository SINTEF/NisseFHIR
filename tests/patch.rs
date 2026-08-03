//! Integration tests for FHIR PATCH (JSON Patch RFC 6902) operations.
//!
//! Each test uses its own unique tenant via JWT tokens to avoid interference
//! when tests run in parallel against the same database.

mod common;

use axum::http::StatusCode;
use common::{
    assert_operation_outcome, build_test_app_auth_required, clean_tenant, get_resource_with_token,
    post_resource_with_token, read_only_token, restricted_token, send_request, setup_test_db,
    tenant_token,
};
use serde_json::json;
use tower::ServiceExt;

/// Helper: set up a test with its own isolated tenant.
async fn setup(tenant: &str) -> (sqlx::PgPool, String) {
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    (pool, token)
}

async fn create_patient(app: &axum::Router, patient: &serde_json::Value, token: &str) -> String {
    let (status, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", patient, token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    created["id"].as_str().unwrap().to_owned()
}

/// Build an authenticated PATCH request.
fn patch_resource_with_token(
    resource_type: &str,
    id: &str,
    body: &serde_json::Value,
    token: &str,
) -> axum::http::Request<axum::body::Body> {
    patch_resource_with_token_if_match(resource_type, id, body, token, "W/\"1\"")
}

fn patch_resource_with_token_if_match(
    resource_type: &str,
    id: &str,
    body: &serde_json::Value,
    token: &str,
    if_match: &str,
) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .method("PATCH")
        .uri(format!("/fhir/{resource_type}/{id}"))
        .header("content-type", "application/json-patch+json")
        .header(axum::http::header::IF_MATCH, if_match)
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(axum::body::Body::from(serde_json::to_string(body).unwrap()))
        .expect("request should build")
}

// ─── PATCH: add field ───────────────────────────────────────────────────────

#[tokio::test]
async fn patch_add_field() {
    let (pool, token) = setup("patch-add").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-add-1",
        "active": true
    });

    let id = create_patient(&app, &patient, &token).await;

    let patch = json!([
        {"op": "add", "path": "/birthDate", "value": "1990-01-01"}
    ]);

    let (status, body) = send_request(
        app,
        patch_resource_with_token("Patient", &id, &patch, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "Patient");
    assert_eq!(body["birthDate"], "1990-01-01");
    assert_eq!(body["active"], true);
}

// ─── PATCH: replace field ───────────────────────────────────────────────────

#[tokio::test]
async fn patch_replace_field() {
    let (pool, token) = setup("patch-replace").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-replace-1",
        "active": true
    });

    let id = create_patient(&app, &patient, &token).await;

    let patch = json!([
        {"op": "replace", "path": "/active", "value": false}
    ]);

    let (status, body) = send_request(
        app,
        patch_resource_with_token("Patient", &id, &patch, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["active"], false);
}

// ─── PATCH: remove field ────────────────────────────────────────────────────

#[tokio::test]
async fn patch_remove_field() {
    let (pool, token) = setup("patch-remove").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-remove-1",
        "active": true,
        "birthDate": "1990-01-01"
    });

    let id = create_patient(&app, &patient, &token).await;

    let patch = json!([
        {"op": "remove", "path": "/birthDate"}
    ]);

    let (status, body) = send_request(
        app,
        patch_resource_with_token("Patient", &id, &patch, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("birthDate").is_none());
    assert_eq!(body["active"], true);
}

// ─── PATCH: nonexistent resource ────────────────────────────────────────────

#[tokio::test]
async fn patch_nonexistent_resource_returns_404() {
    let (pool, token) = setup("patch-404").await;
    let app = build_test_app_auth_required(pool);

    let patch = json!([
        {"op": "add", "path": "/active", "value": true}
    ]);

    let (status, body) = send_request(
        app,
        patch_resource_with_token("Patient", "does-not-exist", &patch, &token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_operation_outcome(&body, "not-found");
}

// ─── PATCH: invalid operation ───────────────────────────────────────────────

#[tokio::test]
async fn patch_invalid_op_returns_400() {
    let (pool, token) = setup("patch-invalid").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-invalid-op",
        "active": true
    });

    let id = create_patient(&app, &patient, &token).await;

    let patch = json!([
        {"op": "replace", "path": "/nonexistent/deep/path", "value": "x"}
    ]);

    let (status, body) = send_request(
        app,
        patch_resource_with_token("Patient", &id, &patch, &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

// ─── PATCH: version increment ───────────────────────────────────────────────

#[tokio::test]
async fn patch_increments_version() {
    let (pool, token) = setup("patch-version").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-version-1",
        "active": true
    });

    let id = create_patient(&app, &patient, &token).await;

    let patch = json!([
        {"op": "replace", "path": "/active", "value": false}
    ]);

    let response: axum::response::Response = app
        .clone()
        .oneshot(patch_resource_with_token("Patient", &id, &patch, &token))
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    let etag = response.headers().get("etag").expect("should have ETag");
    assert_eq!(etag.to_str().unwrap(), "W/\"2\"");
}

// ─── PATCH: rejects resourceType change ─────────────────────────────────────

#[tokio::test]
async fn patch_rejects_resource_type_change() {
    let (pool, token) = setup("patch-type-change").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-type-change",
        "active": true
    });

    let id = create_patient(&app, &patient, &token).await;

    let patch = json!([
        {"op": "replace", "path": "/resourceType", "value": "Observation"}
    ]);

    let (status, body) = send_request(
        app,
        patch_resource_with_token("Patient", &id, &patch, &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

// ─── PATCH: requires write scope ────────────────────────────────────────────

#[tokio::test]
async fn patch_requires_write_scope() {
    let (pool, token) = setup("patch-write-scope").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-write-scope-1",
        "active": true
    });
    let id = create_patient(&app, &patient, &token).await;

    let ro_token = read_only_token("patch-write-scope");
    let patch = json!([
        {"op": "replace", "path": "/active", "value": false}
    ]);
    let (status, _) = send_request(
        app,
        patch_resource_with_token("Patient", &id, &patch, &ro_token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ─── PATCH: resource type restriction ───────────────────────────────────────

#[tokio::test]
async fn patch_respects_resource_type_restriction() {
    let (pool, token) = setup("patch-restrict").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-restrict-1",
        "active": true
    });
    let id = create_patient(&app, &patient, &token).await;

    let obs_only_token = restricted_token("patch-restrict", vec!["Observation".to_owned()]);
    let patch = json!([
        {"op": "replace", "path": "/active", "value": false}
    ]);
    let (status, _) = send_request(
        app,
        patch_resource_with_token("Patient", &id, &patch, &obs_only_token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ─── PATCH: result is readable via GET ──────────────────────────────────────

#[tokio::test]
async fn patch_result_is_readable() {
    let (pool, token) = setup("patch-readable").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-read-1",
        "active": true
    });
    let id = create_patient(&app, &patient, &token).await;

    let patch = json!([
        {"op": "add", "path": "/birthDate", "value": "2000-06-15"}
    ]);
    let (status, _) = send_request(
        app.clone(),
        patch_resource_with_token("Patient", &id, &patch, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send_request(app, get_resource_with_token("Patient", &id, &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["birthDate"], "2000-06-15");
    assert_eq!(body["active"], true);
}

// ─── PATCH: schema validation ───────────────────────────────────────────────

#[tokio::test]
async fn patch_validates_result_against_schema() {
    let (pool, token) = setup("patch-schema").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-schema-1",
        "active": true
    });
    let id = create_patient(&app, &patient, &token).await;

    // Add an invalid property — schema validation should reject it
    let patch = json!([
        {"op": "add", "path": "/bogusField", "value": "should fail"}
    ]);
    let (status, body) = send_request(
        app,
        patch_resource_with_token("Patient", &id, &patch, &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

// ─── PATCH: removing id is rejected ─────────────────────────────────────────

#[tokio::test]
async fn patch_rejects_id_removal() {
    let (pool, token) = setup("patch-rm-id").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-rm-id-1",
        "active": true
    });
    let id = create_patient(&app, &patient, &token).await;

    let patch = json!([
        {"op": "remove", "path": "/id"}
    ]);
    let (status, body) = send_request(
        app,
        patch_resource_with_token("Patient", &id, &patch, &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

// ─── PATCH: replacing id is rejected ────────────────────────────────────────

#[tokio::test]
async fn patch_rejects_id_change() {
    let (pool, token) = setup("patch-chg-id").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-chg-id-1",
        "active": true
    });
    let id = create_patient(&app, &patient, &token).await;

    let patch = json!([
        {"op": "replace", "path": "/id", "value": "different-id"}
    ]);
    let (status, body) = send_request(
        app,
        patch_resource_with_token("Patient", &id, &patch, &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn patch_without_if_match_succeeds() {
    let (pool, token) = setup("patch-no-if-match").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-missing-if-match-1",
        "active": true
    });
    let id = create_patient(&app, &patient, &token).await;

    let patch = json!([
        {"op": "replace", "path": "/active", "value": false}
    ]);

    let request = axum::http::Request::builder()
        .method("PATCH")
        .uri(format!("/fhir/Patient/{id}"))
        .header("content-type", "application/json-patch+json")
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(axum::body::Body::from(
            serde_json::to_string(&patch).unwrap(),
        ))
        .unwrap();

    let (status, body) = send_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["active"], false);
}

#[tokio::test]
async fn patch_stale_if_match_returns_412() {
    let (pool, token) = setup("patch-stale-if-match").await;
    let app = build_test_app_auth_required(pool);

    let patient = json!({
        "resourceType": "Patient",
        "id": "patch-stale-if-match-1",
        "active": true
    });
    let id = create_patient(&app, &patient, &token).await;

    let patch_true = json!([
        {"op": "replace", "path": "/active", "value": true}
    ]);
    let (status, _) = send_request(
        app.clone(),
        patch_resource_with_token_if_match("Patient", &id, &patch_true, &token, "W/\"1\""),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let patch_false = json!([
        {"op": "replace", "path": "/active", "value": false}
    ]);
    let (status, body) = send_request(
        app.clone(),
        patch_resource_with_token_if_match("Patient", &id, &patch_false, &token, "W/\"1\""),
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_operation_outcome(&body, "conflict");
}
