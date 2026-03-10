mod common;

use axum::http::StatusCode;
use common::{
    clean_tenant, post_resource_conditional, post_resource_with_token, send_request, setup_test_db,
    tenant_token,
    test_data::{minimal_patient, patient_peter_chalmers},
};
use serde_json::json;

#[tokio::test]
async fn conditional_create_no_match_creates_resource() {
    let tenant = "cond-create-no-match";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    let app = common::build_test_app(pool.clone());
    let token = tenant_token(tenant);

    // If-None-Exist with no existing match → should create (201)
    let patient = json!({
        "resourceType": "Patient",
        "identifier": [{
            "system": "http://example.org/mrn",
            "value": "cond-create-001"
        }]
    });

    let (status, body) = send_request(
        app,
        post_resource_conditional(
            "Patient",
            &patient,
            &token,
            "identifier=http://example.org/mrn|cond-create-001",
        ),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "expected 201, got: {body}");
    assert_eq!(body["resourceType"], "Patient");

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn conditional_create_single_match_returns_existing() {
    let tenant = "cond-create-single-match";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // First, create a patient with a known identifier
    let patient = json!({
        "resourceType": "Patient",
        "identifier": [{
            "system": "http://example.org/mrn",
            "value": "cond-create-002"
        }]
    });

    let app = common::build_test_app(pool.clone());
    let (status, created) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);
    let existing_id = created["id"].as_str().unwrap().to_owned();

    // Now try conditional create with the same identifier — should return 200 with existing
    let new_patient = json!({
        "resourceType": "Patient",
        "identifier": [{
            "system": "http://example.org/mrn",
            "value": "cond-create-002"
        }],
        "name": [{"family": "Different"}]
    });

    let app = common::build_test_app(pool.clone());
    let (status, body) = send_request(
        app,
        post_resource_conditional(
            "Patient",
            &new_patient,
            &token,
            "identifier=http://example.org/mrn|cond-create-002",
        ),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "expected 200 for existing match, got: {body}"
    );
    assert_eq!(body["id"], existing_id, "should return existing resource");
    // Should NOT have the "Different" family name — it returned the original
    assert!(
        body.get("name").is_none()
            || body["name"]
                .as_array()
                .is_none_or(|a| a.iter().all(|n| n["family"] != "Different")),
        "should return the original resource, not the new payload"
    );

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn conditional_create_multiple_matches_returns_412() {
    let tenant = "cond-create-multiple-matches";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // Create two patients with names matching "dupli"
    let p1 = json!({
        "resourceType": "Patient",
        "name": [{"family": "Duplicate-A"}]
    });
    let p2 = json!({
        "resourceType": "Patient",
        "name": [{"family": "Duplicate-B"}]
    });

    let app = common::build_test_app(pool.clone());
    let (s, _) = send_request(app, post_resource_with_token("Patient", &p1, &token)).await;
    assert_eq!(s, StatusCode::CREATED);

    let app = common::build_test_app(pool.clone());
    let (s, _) = send_request(app, post_resource_with_token("Patient", &p2, &token)).await;
    assert_eq!(s, StatusCode::CREATED);

    // Conditional create matching name=Duplicate — should match both
    let new_patient = minimal_patient();
    let app = common::build_test_app(pool.clone());
    let (status, body) = send_request(
        app,
        post_resource_conditional("Patient", &new_patient, &token, "name=Duplicate"),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::PRECONDITION_FAILED,
        "expected 412 for multiple matches, got: {body}"
    );
    assert_eq!(body["resourceType"], "OperationOutcome");

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn conditional_create_empty_header_returns_400() {
    let tenant = "cond-create-empty-header";
    let pool = setup_test_db().await;
    let app = common::build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let patient = minimal_patient();
    let (status, body) = send_request(
        app,
        post_resource_conditional("Patient", &patient, &token, ""),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "got: {body}");
}

#[tokio::test]
async fn conditional_create_without_header_creates_normally() {
    let tenant = "cond-create-without-header";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    let app = common::build_test_app(pool.clone());
    let token = tenant_token(tenant);

    // Normal POST without If-None-Exist → always creates
    let patient = patient_peter_chalmers();
    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED, "got: {body}");

    clean_tenant(&pool, tenant).await;
}
