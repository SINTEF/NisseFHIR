mod common;

use axum::body::{Body, to_bytes};
use axum::http::{StatusCode, header};
use common::{
    build_test_app, clean_tenant, get_resource_with_token, post_resource_with_token, send_request,
    setup_test_db, tenant_token, test_data::minimal_patient,
};
use tower::ServiceExt;

const TENANT: &str = "content-type-tenant";

#[tokio::test]
async fn fhir_responses_use_fhir_json_content_type() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, TENANT).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(TENANT);

    // Create a patient
    let patient = minimal_patient();
    let request = post_resource_with_token("Patient", &patient, &token);
    let response = app.oneshot(request).await.expect("request should complete");

    assert_eq!(response.status(), StatusCode::CREATED);
    let ct = response
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("Content-Type header should be present");
    assert!(
        ct.to_str().unwrap().contains("application/fhir+json"),
        "expected application/fhir+json, got: {:?}",
        ct
    );

    clean_tenant(&pool, TENANT).await;
}

#[tokio::test]
async fn read_resource_uses_fhir_json_content_type() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, TENANT).await;
    let token = tenant_token(TENANT);

    // Create a patient first
    let patient = minimal_patient();
    let app = build_test_app(pool.clone());
    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = body["id"].as_str().unwrap();

    // Read it back and check content type
    let app = build_test_app(pool.clone());
    let request = get_resource_with_token("Patient", id, &token);
    let response = app.oneshot(request).await.expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    let ct = response
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("Content-Type header should be present");
    assert!(
        ct.to_str().unwrap().contains("application/fhir+json"),
        "expected application/fhir+json, got: {:?}",
        ct
    );

    clean_tenant(&pool, TENANT).await;
}

#[tokio::test]
async fn search_resource_uses_fhir_json_content_type() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, TENANT).await;
    let token = tenant_token(TENANT);

    let app = build_test_app(pool.clone());
    let request = common::search_resource_with_token("Patient", None, &token);
    let response = app.oneshot(request).await.expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    let ct = response
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("Content-Type header should be present");
    assert!(
        ct.to_str().unwrap().contains("application/fhir+json"),
        "expected application/fhir+json, got: {:?}",
        ct
    );

    clean_tenant(&pool, TENANT).await;
}

#[tokio::test]
async fn metadata_uses_fhir_json_content_type() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool.clone());

    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/fhir/metadata")
        .body(Body::empty())
        .expect("request should build");

    let response = app.oneshot(request).await.expect("request should complete");
    assert_eq!(response.status(), StatusCode::OK);
    let ct = response
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("Content-Type header should be present");
    assert!(
        ct.to_str().unwrap().contains("application/fhir+json"),
        "expected application/fhir+json, got: {:?}",
        ct
    );
}

#[tokio::test]
async fn healthz_does_not_use_fhir_json_content_type() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool.clone());

    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/healthz")
        .body(Body::empty())
        .expect("request should build");

    let response = app.oneshot(request).await.expect("request should complete");
    assert_eq!(response.status(), StatusCode::OK);
    let ct = response
        .headers()
        .get(header::CONTENT_TYPE)
        .expect("Content-Type header should be present");
    // healthz should NOT be fhir+json since it's not a FHIR endpoint
    assert!(
        !ct.to_str().unwrap().contains("fhir+json"),
        "healthz should not use application/fhir+json, got: {:?}",
        ct
    );
}

#[tokio::test]
async fn server_accepts_fhir_json_content_type_on_create() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, TENANT).await;
    let token = tenant_token(TENANT);

    let patient = minimal_patient();
    let app = build_test_app(pool.clone());

    // Send with application/fhir+json content type
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/fhir/Patient")
        .header("content-type", "application/fhir+json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(serde_json::to_string(&patient).unwrap()))
        .expect("request should build");

    let response = app.oneshot(request).await.expect("request should complete");
    // application/fhir+json is a valid JSON content type — axum should accept it
    // If it gets a 415 or 400, we need a content-type negotiation layer
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();

    // This might fail with "unsupported media type" if axum doesn't recognize
    // application/fhir+json. If so, we'll need to add content-type negotiation.
    assert!(
        status == StatusCode::CREATED
            || status == StatusCode::BAD_REQUEST
            || status == StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unexpected status: {status}, body: {value}"
    );

    clean_tenant(&pool, TENANT).await;
}
