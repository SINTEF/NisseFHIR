//! Integration tests for FHIR CRUD operations with a real PostgreSQL database.
//!
//! Each test uses its own unique tenant via JWT tokens to avoid interference
//! when tests run in parallel against the same database.

mod common;

use axum::http::StatusCode;
use common::{
    build_test_app_auth_required, clean_tenant, count_resources, get_resource_with_token,
    post_resource_with_token, put_resource_with_token, send_request, setup_test_db, tenant_token,
    test_data,
};
use tower::ServiceExt;

/// Helper: set up a test with its own isolated tenant.
async fn setup(tenant: &str) -> (sqlx::PgPool, String) {
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    (pool, token)
}

// ─── CREATE (POST) ──────────────────────────────────────────────────────────

#[tokio::test]
async fn create_patient_returns_201_with_resource() {
    let (pool, token) = setup("crud-create-201").await;
    let app = build_test_app_auth_required(pool);
    let patient = test_data::patient_peter_chalmers();

    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["resourceType"], "Patient");
    assert_eq!(body["id"], "example");
    assert_eq!(body["name"][0]["family"], "Chalmers");
}

#[tokio::test]
async fn create_patient_returns_correct_headers() {
    let (pool, token) = setup("crud-create-headers").await;
    let app = build_test_app_auth_required(pool);
    let patient = test_data::minimal_patient();

    let response = app
        .oneshot(post_resource_with_token("Patient", &patient, &token))
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::CREATED);

    let etag = response
        .headers()
        .get("ETag")
        .expect("ETag header must be present")
        .to_str()
        .unwrap();
    assert!(
        etag.starts_with("W/\""),
        "ETag should be a weak etag, got: {etag}"
    );

    let location = response
        .headers()
        .get("Location")
        .expect("Location header must be present")
        .to_str()
        .unwrap();
    assert_eq!(location, "/fhir/Patient/minimal-patient");

    assert!(
        response.headers().get("Last-Modified").is_some(),
        "Last-Modified header must be present"
    );
}

#[tokio::test]
async fn create_generates_id_when_missing() {
    let (pool, token) = setup("crud-create-gen-id").await;
    let app = build_test_app_auth_required(pool);
    let mut patient = test_data::minimal_patient();
    patient.as_object_mut().unwrap().remove("id");

    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;

    assert_eq!(status, StatusCode::CREATED);
    let id = body["id"].as_str().expect("id must be present");
    assert_eq!(id.len(), 36, "Generated id should be a UUID, got: {id}");
}

#[tokio::test]
async fn create_stores_resource_in_database() {
    let (pool, token) = setup("crud-create-stores").await;
    let app = build_test_app_auth_required(pool.clone());
    let patient = test_data::minimal_patient();

    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);

    assert_eq!(count_resources(&pool, "crud-create-stores").await, 1);
}

#[tokio::test]
async fn create_initial_version_is_1() {
    let (pool, token) = setup("crud-create-v1").await;
    let app = build_test_app_auth_required(pool);
    let patient = test_data::minimal_patient();

    let response = app
        .oneshot(post_resource_with_token("Patient", &patient, &token))
        .await
        .expect("request should complete");

    let etag = response.headers().get("ETag").unwrap().to_str().unwrap();
    assert_eq!(etag, "W/\"1\"", "Initial version must be 1");
}

// ─── READ (GET) ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn read_after_create_returns_same_resource() {
    let (pool, token) = setup("crud-read-after-create").await;
    let patient = test_data::patient_peter_chalmers();

    // Create
    let app = build_test_app_auth_required(pool.clone());
    let (status, created) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);

    // Read
    let app = build_test_app_auth_required(pool);
    let (status, read) =
        send_request(app, get_resource_with_token("Patient", "example", &token)).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(created, read, "Read resource must match created resource");
}

#[tokio::test]
async fn read_returns_etag_and_last_modified() {
    let (pool, token) = setup("crud-read-headers").await;
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let _ = send_request(app, post_resource_with_token("Patient", &patient, &token)).await;

    let app = build_test_app_auth_required(pool);
    let response = app
        .oneshot(get_resource_with_token(
            "Patient",
            "minimal-patient",
            &token,
        ))
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("ETag").is_some());
    assert!(response.headers().get("Last-Modified").is_some());
}

#[tokio::test]
async fn read_nonexistent_returns_404() {
    let (pool, token) = setup("crud-read-404").await;
    let app = build_test_app_auth_required(pool);

    let (status, body) = send_request(
        app,
        get_resource_with_token("Patient", "does-not-exist", &token),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(body["issue"][0]["code"], "not-found");
}

#[tokio::test]
async fn read_wrong_resource_type_returns_404() {
    let (pool, token) = setup("crud-read-wrong-type").await;
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let _ = send_request(app, post_resource_with_token("Patient", &patient, &token)).await;

    let app = build_test_app_auth_required(pool);
    let (status, _) = send_request(
        app,
        get_resource_with_token("Observation", "minimal-patient", &token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ─── UPDATE (PUT) ───────────────────────────────────────────────────────────

#[tokio::test]
async fn update_resource_returns_200() {
    let (pool, token) = setup("crud-update-200").await;
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let _ = send_request(app, post_resource_with_token("Patient", &patient, &token)).await;

    let mut updated = patient.clone();
    updated["active"] = serde_json::json!(true);

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        put_resource_with_token("Patient", "minimal-patient", &updated, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["active"], true);
}

#[tokio::test]
async fn update_increments_version() {
    let (pool, token) = setup("crud-update-version").await;
    let patient = test_data::minimal_patient();

    // Create (v1)
    let app = build_test_app_auth_required(pool.clone());
    let resp = app
        .oneshot(post_resource_with_token("Patient", &patient, &token))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("ETag").unwrap().to_str().unwrap(),
        "W/\"1\""
    );

    // Update (v2)
    let app = build_test_app_auth_required(pool.clone());
    let resp = app
        .oneshot(put_resource_with_token(
            "Patient",
            "minimal-patient",
            &patient,
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("ETag").unwrap().to_str().unwrap(),
        "W/\"2\""
    );

    // Update again (v3)
    let app = build_test_app_auth_required(pool);
    let resp = app
        .oneshot(put_resource_with_token(
            "Patient",
            "minimal-patient",
            &patient,
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("ETag").unwrap().to_str().unwrap(),
        "W/\"3\""
    );
}

#[tokio::test]
async fn update_sets_id_from_url() {
    let (pool, token) = setup("crud-update-id-url").await;
    let mut patient = test_data::minimal_patient();
    patient.as_object_mut().unwrap().remove("id");

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        put_resource_with_token("Patient", "url-id-test", &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "url-id-test");
}

#[tokio::test]
async fn update_rejects_mismatched_id() {
    let (pool, token) = setup("crud-update-mismatch-id").await;
    let mut patient = test_data::minimal_patient();
    patient["id"] = serde_json::json!("different-id");

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        put_resource_with_token("Patient", "url-id", &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn update_rejects_mismatched_resource_type() {
    let (pool, token) = setup("crud-update-mismatch-type").await;
    let obs = test_data::minimal_observation();

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        put_resource_with_token("Patient", "minimal-obs", &obs, &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["resourceType"], "OperationOutcome");
}

// ─── CREATE THEN READ ROUNDTRIP ACROSS ALL RESOURCE TYPES ───────────────────

#[tokio::test]
async fn roundtrip_all_valid_resources() {
    let (pool, token) = setup("crud-roundtrip").await;

    for (resource_type, resource) in test_data::all_valid_resources() {
        let id = resource["id"].as_str().expect("test resource must have id");

        // Create
        let app = build_test_app_auth_required(pool.clone());
        let (status, created) = send_request(
            app,
            post_resource_with_token(resource_type, &resource, &token),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "Failed to create {resource_type}/{id}: {created}"
        );

        // Read back
        let app = build_test_app_auth_required(pool.clone());
        let (status, read) =
            send_request(app, get_resource_with_token(resource_type, id, &token)).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "Failed to read {resource_type}/{id}"
        );

        assert_eq!(created, read, "Roundtrip mismatch for {resource_type}/{id}");
    }
}

// ─── MULTIPLE RESOURCES & ISOLATION ─────────────────────────────────────────

#[tokio::test]
async fn multiple_resources_coexist() {
    let (pool, token) = setup("crud-multi-coexist").await;
    let patient = test_data::minimal_patient();
    let obs = test_data::minimal_observation();

    let app = build_test_app_auth_required(pool.clone());
    let (s, _) = send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (s, _) = send_request(app, post_resource_with_token("Observation", &obs, &token)).await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", "minimal-patient", &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let app = build_test_app_auth_required(pool.clone());
    let (s, _) = send_request(
        app,
        get_resource_with_token("Observation", "minimal-obs", &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    assert_eq!(count_resources(&pool, "crud-multi-coexist").await, 2);
}

#[tokio::test]
async fn update_preserves_all_fields() {
    let (pool, token) = setup("crud-update-fields").await;
    let patient = test_data::patient_peter_chalmers();

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);

    let mut updated = patient.clone();
    updated["active"] = serde_json::json!(false);

    let app = build_test_app_auth_required(pool.clone());
    let (status, body) = send_request(
        app,
        put_resource_with_token("Patient", "example", &updated, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["active"], false);
    assert_eq!(body["name"][0]["family"], "Chalmers");
    assert_eq!(body["gender"], "male");
    assert_eq!(body["birthDate"], "1974-12-25");
}

// ─── HEALTHZ & METADATA ────────────────────────────────────────────────────

#[tokio::test]
async fn healthz_returns_ok() {
    let pool = setup_test_db().await;
    let app = common::build_test_app(pool);

    let req = axum::http::Request::builder()
        .uri("/healthz")
        .body(axum::body::Body::empty())
        .unwrap();
    let (status, body) = send_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn metadata_returns_capability_statement() {
    let pool = setup_test_db().await;
    let app = common::build_test_app(pool);

    let req = axum::http::Request::builder()
        .uri("/metadata")
        .body(axum::body::Body::empty())
        .unwrap();
    let (status, body) = send_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "CapabilityStatement");
    assert_eq!(body["status"], "active");
    assert_eq!(body["kind"], "instance");
    assert_eq!(body["fhirVersion"], "6.0.0-ballot3");
    assert_eq!(body["format"][0], "json");
    assert_eq!(body["implementation"]["url"], "http://localhost:8080/fhir");

    let rest = &body["rest"][0];
    assert_eq!(rest["mode"], "server");
    assert!(rest["resource"].is_array());
}

// ─── IDEMPOTENT UPSERT ─────────────────────────────────────────────────────

#[tokio::test]
async fn double_create_upserts_same_resource() {
    let (pool, token) = setup("crud-double-create").await;
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let (s1, _) = send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(s1, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (s2, _) = send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(s2, StatusCode::CREATED);

    assert_eq!(count_resources(&pool, "crud-double-create").await, 1);
}
