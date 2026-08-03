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

/// Two equivalent `If-None-Exist` conditions with reordered parameters must
/// resolve to the same lock key, so the second conditional create returns the
/// resource written by the first instead of producing a duplicate.
#[tokio::test]
async fn conditional_create_reordered_parameters_share_lock() {
    let tenant = "cond-create-reordered";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // Identifier + name parameters: first request and reordered second request
    // must collapse to the same canonical condition.
    let condition_a = "identifier=http://example.org/mrn|shared-001&name=Smith";
    let condition_b = "name=Smith&identifier=http://example.org/mrn|shared-001";

    let patient = json!({
        "resourceType": "Patient",
        "identifier": [{
            "system": "http://example.org/mrn",
            "value": "shared-001"
        }],
        "name": [{"family": "Smith"}]
    });

    let app = common::build_test_app(pool.clone());
    let (status_a, body_a) = send_request(
        app.clone(),
        post_resource_conditional("Patient", &patient, &token, condition_a),
    )
    .await;
    assert_eq!(status_a, StatusCode::CREATED, "first create: {body_a}");

    let app = common::build_test_app(pool.clone());
    let (status_b, body_b) = send_request(
        app,
        post_resource_conditional("Patient", &patient, &token, condition_b),
    )
    .await;
    assert_eq!(status_b, StatusCode::OK, "second create should hit existing: {body_b}");

    // The reordered condition must return the same resource id.
    assert_eq!(body_a["id"], body_b["id"]);

    // Exactly one logical resource was persisted for this identifier.
    let count = common::count_resources(&pool, tenant).await;
    assert_eq!(count, 1, "expected exactly one Patient for the shared condition");

    let history =
        common::count_history_entries(&pool, tenant).await;
    assert_eq!(history, 1, "expected a single history entry from the first create");

    clean_tenant(&pool, tenant).await;
}

/// Two concurrent identical conditional creates produce only one resource.
///
/// The advisory lock serializes both requests; the loser observes the row
/// written by the winner and returns 200 OK with the existing resource instead
/// of creating a duplicate.
#[tokio::test]
async fn conditional_create_concurrent_identical_produces_one_resource() {
    let tenant = "cond-create-concurrent";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let condition = "identifier=http://example.org/mrn|race-001";

    let patient = json!({
        "resourceType": "Patient",
        "identifier": [{
            "system": "http://example.org/mrn",
            "value": "race-001"
        }]
    });

    // Two independent apps sharing the same pool fire the same conditional
    // create at the same time. The advisory lock is what makes this safe.
    let app_a = common::build_test_app(pool.clone());
    let app_b = common::build_test_app(pool.clone());
    let req_a = post_resource_conditional("Patient", &patient, &token, condition);
    let req_b = post_resource_conditional("Patient", &patient, &token, condition);

    let (res_a, res_b) = tokio::join!(
        async { send_request(app_a, req_a).await },
        async { send_request(app_b, req_b).await },
    );

    let statuses = [res_a.0, res_b.0];
    let created = statuses.iter().filter(|s| **s == StatusCode::CREATED).count();
    let matched = statuses.iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(
        created, 1,
        "exactly one request should create the resource; responses: {statuses:?}"
    );
    assert_eq!(
        matched, 1,
        "exactly one request should match the existing resource; responses: {statuses:?}"
    );

    let count = common::count_resources(&pool, tenant).await;
    assert_eq!(count, 1, "exactly one logical resource must remain persisted");

    clean_tenant(&pool, tenant).await;
}

/// Unrelated conditional creates (different conditions) must not serialize
/// against each other: two different identifiers under the same tenant and
/// type both create their own resource.
#[tokio::test]
async fn conditional_create_unrelated_conditions_do_not_collide() {
    let tenant = "cond-create-no-collide";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let condition_a = "identifier=http://example.org/mrn|independent-a";
    let condition_b = "identifier=http://example.org/mrn|independent-b";

    let patient_a = json!({
        "resourceType": "Patient",
        "identifier": [{"system": "http://example.org/mrn", "value": "independent-a"}]
    });
    let patient_b = json!({
        "resourceType": "Patient",
        "identifier": [{"system": "http://example.org/mrn", "value": "independent-b"}]
    });

    let app_a = common::build_test_app(pool.clone());
    let app_b = common::build_test_app(pool.clone());
    let req_a = post_resource_conditional("Patient", &patient_a, &token, condition_a);
    let req_b = post_resource_conditional("Patient", &patient_b, &token, condition_b);

    let (res_a, res_b) = tokio::join!(
        async { send_request(app_a, req_a).await },
        async { send_request(app_b, req_b).await },
    );
    assert_eq!(res_a.0, StatusCode::CREATED, "got: {:?}", res_a);
    assert_eq!(res_b.0, StatusCode::CREATED, "got: {:?}", res_b);
    assert_ne!(res_a.1["id"], res_b.1["id"], "the two resources must be distinct");

    let count = common::count_resources(&pool, tenant).await;
    assert_eq!(count, 2, "both unrelated resources must be persisted");

    clean_tenant(&pool, tenant).await;
}
