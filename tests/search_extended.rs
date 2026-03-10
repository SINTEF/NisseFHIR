mod common;

use axum::http::StatusCode;
use common::{
    build_test_app, clean_tenant, post_resource_with_token, search_resource_with_token,
    send_request, setup_test_db, tenant_token,
    test_data::{condition_example, encounter_example},
};

// Each test uses its own tenant to avoid race conditions when tests
// run concurrently.

// ---------------------------------------------------------------------------
// Condition search tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_condition_by_clinical_status() {
    let pool = setup_test_db().await;
    let tenant = "sx-cond-clin-status";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let app = build_test_app(pool.clone());
    let (s, _) = send_request(
        app,
        post_resource_with_token("Condition", &condition_example(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app(pool.clone());
    let (s, body) = send_request(
        app,
        search_resource_with_token("Condition", Some("clinical-status=active"), &token),
    )
    .await;

    assert_eq!(s, StatusCode::OK, "search failed: {body}");
    assert_eq!(body["resourceType"], "Bundle");
    let total = body["total"].as_i64().unwrap_or(0);
    assert!(total >= 1, "expected at least 1 match, got {total}");

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn search_condition_by_code() {
    let pool = setup_test_db().await;
    let tenant = "sx-cond-code";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let app = build_test_app(pool.clone());
    let (s, _) = send_request(
        app,
        post_resource_with_token("Condition", &condition_example(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    // Search by SNOMED code
    let app = build_test_app(pool.clone());
    let (s, body) = send_request(
        app,
        search_resource_with_token(
            "Condition",
            Some("code=http://snomed.info/sct|386661006"),
            &token,
        ),
    )
    .await;

    assert_eq!(s, StatusCode::OK, "search failed: {body}");
    let total = body["total"].as_i64().unwrap_or(0);
    assert!(
        total >= 1,
        "expected at least 1 match for code search, got {total}"
    );

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn search_condition_by_subject() {
    let pool = setup_test_db().await;
    let tenant = "sx-cond-subject";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let app = build_test_app(pool.clone());
    let (s, _) = send_request(
        app,
        post_resource_with_token("Condition", &condition_example(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    // Search by patient reference (maps to "patient" param → subject.reference)
    let app = build_test_app(pool.clone());
    let (s, body) = send_request(
        app,
        search_resource_with_token("Condition", Some("patient=Patient/example"), &token),
    )
    .await;

    assert_eq!(s, StatusCode::OK, "search failed: {body}");
    let total = body["total"].as_i64().unwrap_or(0);
    assert!(
        total >= 1,
        "expected at least 1 match for subject search, got {total}"
    );

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn search_condition_by_category() {
    let pool = setup_test_db().await;
    let tenant = "sx-cond-category";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let app = build_test_app(pool.clone());
    let (s, _) = send_request(
        app,
        post_resource_with_token("Condition", &condition_example(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app(pool.clone());
    let (s, body) = send_request(
        app,
        search_resource_with_token("Condition", Some("category=encounter-diagnosis"), &token),
    )
    .await;

    assert_eq!(s, StatusCode::OK, "search failed: {body}");
    let total = body["total"].as_i64().unwrap_or(0);
    assert!(
        total >= 1,
        "expected at least 1 match for category search, got {total}"
    );

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn search_condition_by_onset_date() {
    let pool = setup_test_db().await;
    let tenant = "sx-cond-onset";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let app = build_test_app(pool.clone());
    let (s, _) = send_request(
        app,
        post_resource_with_token("Condition", &condition_example(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app(pool.clone());
    let (s, body) = send_request(
        app,
        search_resource_with_token("Condition", Some("onset-date=2012-05-24"), &token),
    )
    .await;

    assert_eq!(s, StatusCode::OK, "search failed: {body}");
    let total = body["total"].as_i64().unwrap_or(0);
    assert!(
        total >= 1,
        "expected at least 1 match for onset-date search, got {total}"
    );

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn search_condition_no_match_returns_empty_bundle() {
    let pool = setup_test_db().await;
    let tenant = "sx-cond-nomatch";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let app = build_test_app(pool.clone());
    let (s, _) = send_request(
        app,
        post_resource_with_token("Condition", &condition_example(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    // Search for a code that doesn't exist
    let app = build_test_app(pool.clone());
    let (s, body) = send_request(
        app,
        search_resource_with_token("Condition", Some("code=nonexistent-code"), &token),
    )
    .await;

    assert_eq!(s, StatusCode::OK, "search should succeed: {body}");
    let total = body["total"].as_i64().unwrap_or(-1);
    assert_eq!(total, 0, "expected 0 matches for nonexistent code");

    clean_tenant(&pool, tenant).await;
}

// ---------------------------------------------------------------------------
// Encounter search tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_encounter_by_status() {
    let pool = setup_test_db().await;
    let tenant = "sx-enc-status";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let app = build_test_app(pool.clone());
    let (s, _) = send_request(
        app,
        post_resource_with_token("Encounter", &encounter_example(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app(pool.clone());
    let (s, body) = send_request(
        app,
        search_resource_with_token("Encounter", Some("status=completed"), &token),
    )
    .await;

    assert_eq!(s, StatusCode::OK, "search failed: {body}");
    let total = body["total"].as_i64().unwrap_or(0);
    assert!(
        total >= 1,
        "expected at least 1 match for status search, got {total}"
    );

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn search_encounter_by_subject() {
    let pool = setup_test_db().await;
    let tenant = "sx-enc-subject";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let app = build_test_app(pool.clone());
    let (s, _) = send_request(
        app,
        post_resource_with_token("Encounter", &encounter_example(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app(pool.clone());
    let (s, body) = send_request(
        app,
        search_resource_with_token("Encounter", Some("subject=Patient/example"), &token),
    )
    .await;

    assert_eq!(s, StatusCode::OK, "search failed: {body}");
    let total = body["total"].as_i64().unwrap_or(0);
    assert!(
        total >= 1,
        "expected at least 1 match for subject search, got {total}"
    );

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn search_encounter_by_type() {
    let pool = setup_test_db().await;
    let tenant = "sx-enc-type";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let app = build_test_app(pool.clone());
    let (s, _) = send_request(
        app,
        post_resource_with_token("Encounter", &encounter_example(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app(pool.clone());
    let (s, body) = send_request(
        app,
        search_resource_with_token(
            "Encounter",
            Some("type=http://snomed.info/sct|11429006"),
            &token,
        ),
    )
    .await;

    assert_eq!(s, StatusCode::OK, "search failed: {body}");
    let total = body["total"].as_i64().unwrap_or(0);
    assert!(
        total >= 1,
        "expected at least 1 match for type search, got {total}"
    );

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn search_encounter_no_match_status() {
    let pool = setup_test_db().await;
    let tenant = "sx-enc-nomatch";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let app = build_test_app(pool.clone());
    let (s, _) = send_request(
        app,
        post_resource_with_token("Encounter", &encounter_example(), &token),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    // Search for a status that doesn't match
    let app = build_test_app(pool.clone());
    let (s, body) = send_request(
        app,
        search_resource_with_token("Encounter", Some("status=planned"), &token),
    )
    .await;

    assert_eq!(s, StatusCode::OK, "search should succeed: {body}");
    let total = body["total"].as_i64().unwrap_or(-1);
    assert_eq!(total, 0, "expected 0 matches for wrong status");

    clean_tenant(&pool, tenant).await;
}
