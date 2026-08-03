//! Integration tests for FHIR CRUD operations with a real PostgreSQL database.
//!
//! Each test uses its own unique tenant via JWT tokens to avoid interference
//! when tests run in parallel against the same database.

mod common;

use axum::http::StatusCode;
use common::{
    build_test_app_auth_required, clean_tenant, count_history_entries, count_resources,
    get_resource_with_token, post_resource_with_token, put_resource_with_token,
    put_resource_with_token_if_match, send_request, setup_test_db, tenant_token, test_data,
};
use fhir_server::store::PgStore;
use sqlx::Row;
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
    assert_ne!(body["id"], "example", "POST must ignore the submitted id");
    assert_eq!(body["id"].as_str().unwrap().len(), 36);
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
    assert!(location.starts_with("/fhir/Patient/"));
    assert_ne!(location, "/fhir/Patient/minimal-patient");

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
    let id = created["id"].as_str().unwrap();
    let app = build_test_app_auth_required(pool);
    let (status, read) = send_request(app, get_resource_with_token("Patient", id, &token)).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(created, read, "Read resource must match created resource");
}

#[tokio::test]
async fn read_returns_etag_and_last_modified() {
    let (pool, token) = setup("crud-read-headers").await;
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let (_, created) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    let id = created["id"].as_str().unwrap();

    let app = build_test_app_auth_required(pool);
    let response = app
        .oneshot(get_resource_with_token("Patient", id, &token))
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
    let (_, created) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    let id = created["id"].as_str().unwrap();

    let app = build_test_app_auth_required(pool);
    let (status, _) = send_request(app, get_resource_with_token("Observation", id, &token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ─── UPDATE (PUT) ───────────────────────────────────────────────────────────

#[tokio::test]
async fn update_resource_returns_200() {
    let (pool, token) = setup("crud-update-200").await;
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let (_, created) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    let id = created["id"].as_str().unwrap().to_owned();

    let mut updated = created;
    updated["active"] = serde_json::json!(true);

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        put_resource_with_token("Patient", &id, &updated, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["active"], true);
}

#[tokio::test]
async fn update_creates_missing_resource_with_client_id() {
    let (pool, token) = setup("crud-update-create").await;
    let app = build_test_app_auth_required(pool.clone());
    let patient = serde_json::json!({
        "resourceType": "Patient",
        "id": "client-defined-id",
        "active": true
    });

    let request = axum::http::Request::builder()
        .method("PUT")
        .uri("/fhir/Patient/client-defined-id")
        .header("content-type", "application/json")
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(axum::body::Body::from(
            serde_json::to_string(&patient).unwrap(),
        ))
        .unwrap();
    let response = app
        .oneshot(request)
        .await
        .expect("update-create should complete");

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["ETag"], "W/\"1\"");
    assert_eq!(
        response.headers()["Location"],
        "/fhir/Patient/client-defined-id/_history/1"
    );
    assert_eq!(count_history_entries(&pool, "crud-update-create").await, 1);

    let app = build_test_app_auth_required(pool);
    let (status, stored) = send_request(
        app,
        get_resource_with_token("Patient", "client-defined-id", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stored, patient);
}

#[tokio::test]
async fn update_with_if_match_does_not_create_missing_resource() {
    let (pool, token) = setup("crud-update-if-match-missing").await;
    let app = build_test_app_auth_required(pool.clone());
    let patient = serde_json::json!({
        "resourceType": "Patient",
        "id": "missing-client-id"
    });

    let (status, body) = send_request(
        app,
        put_resource_with_token_if_match(
            "Patient",
            "missing-client-id",
            &patient,
            &token,
            "W/\"1\"",
        ),
    )
    .await;

    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(
        count_resources(&pool, "crud-update-if-match-missing").await,
        0
    );
}

#[tokio::test]
async fn update_increments_version() {
    let (pool, token) = setup("crud-update-version").await;
    let patient = test_data::minimal_patient();

    // Create (v1)
    let app = build_test_app_auth_required(pool.clone());
    let (status, patient) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = patient["id"].as_str().unwrap().to_owned();

    // Update (v2)
    let app = build_test_app_auth_required(pool.clone());
    let resp = app
        .oneshot(put_resource_with_token_if_match(
            "Patient", &id, &patient, &token, "W/\"1\"",
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
        .oneshot(put_resource_with_token_if_match(
            "Patient", &id, &patient, &token, "W/\"2\"",
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("ETag").unwrap().to_str().unwrap(),
        "W/\"3\""
    );
}

#[tokio::test]
async fn create_and_update_write_history_versions() {
    let (pool, token) = setup("crud-history-versions").await;
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let (status, created) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_owned();

    let mut updated = created;
    updated["active"] = serde_json::json!(true);

    let app = build_test_app_auth_required(pool.clone());
    let response = app
        .oneshot(put_resource_with_token("Patient", &id, &updated, &token))
        .await
        .expect("update should complete");
    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(
        count_history_entries(&pool, "crud-history-versions").await,
        2
    );

    let rows = sqlx::query(
        r#"
        SELECT version_id, deleted
        FROM fhir_resource_history
        WHERE tenant_id = $1 AND resource_type = 'Patient' AND id = $2
        ORDER BY version_id ASC
        "#,
    )
    .bind("crud-history-versions")
    .bind(&id)
    .fetch_all(&pool)
    .await
    .expect("history query should succeed");

    let history: Vec<(i64, bool)> = rows
        .into_iter()
        .map(|row| {
            (
                row.get::<i64, _>("version_id"),
                row.get::<bool, _>("deleted"),
            )
        })
        .collect();

    assert_eq!(history, vec![(1, false), (2, false)]);
}

#[tokio::test]
async fn update_sets_id_from_url() {
    let (pool, token) = setup("crud-update-id-url").await;
    let mut patient = test_data::minimal_patient();
    patient.as_object_mut().unwrap().remove("id");

    let app = build_test_app_auth_required(pool.clone());
    let mut create = test_data::minimal_patient();
    create["id"] = serde_json::json!("url-id-test");
    let (status, created) =
        send_request(app, post_resource_with_token("Patient", &create, &token)).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        put_resource_with_token("Patient", id, &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], id);
}

#[tokio::test]
async fn update_rejects_mismatched_id() {
    let (pool, token) = setup("crud-update-mismatch-id").await;
    let mut patient = test_data::minimal_patient();
    patient["id"] = serde_json::json!("different-id");

    let app = build_test_app_auth_required(pool.clone());
    let mut create = test_data::minimal_patient();
    create["id"] = serde_json::json!("url-id");
    let (status, created) =
        send_request(app, post_resource_with_token("Patient", &create, &token)).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        put_resource_with_token("Patient", id, &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn update_rejects_mismatched_resource_type() {
    let (pool, token) = setup("crud-update-mismatch-type").await;
    let obs = test_data::minimal_observation();

    let app = build_test_app_auth_required(pool.clone());
    let mut create = test_data::minimal_patient();
    create["id"] = serde_json::json!("minimal-obs");
    let (status, created) =
        send_request(app, post_resource_with_token("Patient", &create, &token)).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();

    let app = build_test_app_auth_required(pool);
    let (status, body) =
        send_request(app, put_resource_with_token("Patient", id, &obs, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["resourceType"], "OperationOutcome");
}

// ─── CREATE THEN READ ROUNDTRIP ACROSS ALL RESOURCE TYPES ───────────────────

#[tokio::test]
async fn roundtrip_all_valid_resources() {
    let (pool, token) = setup("crud-roundtrip").await;

    for (resource_type, resource) in test_data::all_valid_resources() {
        let submitted_id = resource["id"].as_str().expect("test resource must have id");

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
            "Failed to create {resource_type}/{submitted_id}: {created}"
        );
        let id = created["id"]
            .as_str()
            .expect("created resource must have id");
        assert_ne!(id, submitted_id);

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
    let (s, created_patient) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (s, created_observation) =
        send_request(app, post_resource_with_token("Observation", &obs, &token)).await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", created_patient["id"].as_str().unwrap(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let app = build_test_app_auth_required(pool.clone());
    let (s, _) = send_request(
        app,
        get_resource_with_token(
            "Observation",
            created_observation["id"].as_str().unwrap(),
            &token,
        ),
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
    let (status, created) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_owned();

    let mut updated = created;
    updated["active"] = serde_json::json!(false);

    let app = build_test_app_auth_required(pool.clone());
    let (status, body) = send_request(
        app,
        put_resource_with_token("Patient", &id, &updated, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["active"], false);
    assert_eq!(body["name"][0]["family"], "Chalmers");
    assert_eq!(body["gender"], "male");
    assert_eq!(body["birthDate"], "1974-12-25");
}

#[tokio::test]
async fn update_without_if_match_succeeds() {
    let (pool, token) = setup("crud-update-no-if-match").await;
    let app = build_test_app_auth_required(pool.clone());
    let patient = test_data::minimal_patient();

    let (status, patient) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let id = patient["id"].as_str().unwrap();
    let request = axum::http::Request::builder()
        .method("PUT")
        .uri(format!("/fhir/Patient/{id}"))
        .header("content-type", "application/json")
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(axum::body::Body::from(
            serde_json::to_string(&patient).unwrap(),
        ))
        .unwrap();

    let (status, body) = send_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "Patient");
}

#[tokio::test]
async fn update_stale_if_match_returns_412() {
    let (pool, token) = setup("crud-update-stale-if-match").await;
    let app = build_test_app_auth_required(pool.clone());
    let patient = test_data::minimal_patient();

    let (status, patient) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = patient["id"].as_str().unwrap().to_owned();

    let mut first_update = patient.clone();
    first_update["active"] = serde_json::json!(true);

    let (status, _) = send_request(
        app.clone(),
        put_resource_with_token_if_match("Patient", &id, &first_update, &token, "W/\"1\""),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut stale_update = patient.clone();
    stale_update["active"] = serde_json::json!(false);

    let (status, body) = send_request(
        app.clone(),
        put_resource_with_token_if_match("Patient", &id, &stale_update, &token, "W/\"1\""),
    )
    .await;

    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(body["issue"][0]["code"], "conflict");

    let (status, body) = send_request(app, get_resource_with_token("Patient", &id, &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["active"], true);
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
        .uri("/fhir/metadata")
        .body(axum::body::Body::empty())
        .unwrap();
    let (status, body) = send_request(app.clone(), req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "CapabilityStatement");
    assert_eq!(body["status"], "active");
    assert_eq!(body["kind"], "instance");
    assert_eq!(body["fhirVersion"], "6.0.0-ballot3");
    assert_eq!(body["format"][0], "json");
    assert_eq!(body["implementation"]["url"], "http://localhost:8080/fhir");
    assert_eq!(
        body["url"],
        "https://sintef.github.io/NisseFHIR/CapabilityStatement/nissefhir"
    );

    let rest = &body["rest"][0];
    assert_eq!(rest["mode"], "server");
    assert!(rest["resource"].is_array());
    assert!(
        rest["resource"]
            .as_array()
            .unwrap()
            .iter()
            .all(|resource| resource["type"] != "*")
    );
    assert!(
        rest["resource"]
            .as_array()
            .unwrap()
            .iter()
            .all(|resource| resource["updateCreate"] == true)
    );

    let patient = rest["resource"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["type"] == "Patient")
        .unwrap();
    let advertised = patient["interaction"]
        .as_array()
        .unwrap()
        .iter()
        .map(|interaction| interaction["code"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    let routed = [
        ("create", "POST", "/fhir/Patient"),
        ("read", "GET", "/fhir/Patient/route-contract"),
        (
            "history-instance",
            "GET",
            "/fhir/Patient/route-contract/_history",
        ),
        ("update", "PUT", "/fhir/Patient/route-contract"),
        ("patch", "PATCH", "/fhir/Patient/route-contract"),
        ("delete", "DELETE", "/fhir/Patient/route-contract"),
        ("search-type", "GET", "/fhir/Patient"),
    ];
    assert_eq!(
        advertised,
        routed
            .iter()
            .map(|(code, _, _)| *code)
            .collect::<std::collections::BTreeSet<_>>()
    );
    for (code, method, uri) in routed {
        let request = axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .body(axum::body::Body::empty())
            .unwrap();
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("route contract request should complete");
        assert_ne!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "advertised {code} interaction has no route"
        );
    }

    let system_interactions = rest["interaction"]
        .as_array()
        .unwrap()
        .iter()
        .map(|interaction| interaction["code"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(system_interactions, ["batch", "transaction"].into());
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/fhir")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app
        .oneshot(request)
        .await
        .expect("Bundle route contract request should complete");
    assert_ne!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn legacy_metadata_alias_returns_capability_statement() {
    let pool = setup_test_db().await;
    let app = common::build_test_app(pool);

    let req = axum::http::Request::builder()
        .uri("/metadata")
        .body(axum::body::Body::empty())
        .unwrap();
    let (status, body) = send_request(app, req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "CapabilityStatement");
}

// ─── SERVER-ASSIGNED CREATE IDS ─────────────────────────────────────────────

#[tokio::test]
async fn repeated_create_ignores_submitted_id_and_creates_distinct_resources() {
    let (pool, token) = setup("crud-double-create").await;
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let (s1, first) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(s1, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (s2, second) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(s2, StatusCode::CREATED);

    assert_ne!(first["id"], "minimal-patient");
    assert_ne!(second["id"], "minimal-patient");
    assert_ne!(first["id"], second["id"]);
    assert_eq!(count_resources(&pool, "crud-double-create").await, 2);
}

#[tokio::test]
async fn concurrent_store_creates_for_same_identity_have_one_winner() {
    let tenant = "crud-concurrent-create";
    let (pool, _) = setup(tenant).await;
    let store = PgStore::new(pool.clone());
    let resource = serde_json::json!({
        "resourceType": "Patient",
        "id": "generated-collision",
        "active": true
    });

    let first_store = store.clone();
    let first_resource = resource.clone();
    let first = async move {
        first_store
            .create(tenant, "Patient", "generated-collision", first_resource)
            .await
    };
    let second = async move {
        store
            .create(tenant, "Patient", "generated-collision", resource)
            .await
    };

    let (first, second) = tokio::join!(first, second);
    let winners = [first.unwrap(), second.unwrap()]
        .into_iter()
        .filter(Option::is_some)
        .count();

    assert_eq!(winners, 1);
    assert_eq!(count_resources(&pool, tenant).await, 1);
    assert_eq!(count_history_entries(&pool, tenant).await, 1);
}

// ─── PAYLOAD TOO LARGE ─────────────────────────────────────────────────────

#[tokio::test]
async fn create_rejects_payload_too_large() {
    let (pool, token) = setup("crud-too-large").await;
    let app = build_test_app_auth_required(pool);

    // 10 MB is the configured max body size; send > 10 MB
    let large_string = "x".repeat(11 * 1024 * 1024);
    let body = serde_json::json!({
        "resourceType": "Patient",
        "id": "too-large",
        "text": { "status": "generated", "div": large_string }
    });

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/fhir/Patient")
        .header("content-type", "application/json")
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(axum::body::Body::from(
            serde_json::to_string(&body).unwrap(),
        ))
        .expect("request should build");

    let (status, _body) = send_request(app, req).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}
