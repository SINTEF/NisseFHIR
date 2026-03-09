//! Integration tests for authentication and authorization.

mod common;

use axum::http::StatusCode;
use common::{
    build_test_app, build_test_app_auth_required, clean_tenant, expired_token, get_resource,
    get_resource_with_token, post_resource, post_resource_with_token, put_resource_with_token,
    read_only_token, restricted_token, send_request, setup_test_db, tenant_token,
    write_only_token,
    test_data,
};

// ─── UNAUTHENTICATED MODE ──────────────────────────────────────────────────

#[tokio::test]
async fn unauthenticated_mode_allows_all_operations() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool.clone()); // allow_unauthenticated = true
    let patient = test_data::minimal_patient();

    let (status, _) = send_request(app, post_resource("Patient", &patient)).await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app(pool.clone());
    let (status, _) = send_request(app, get_resource("Patient", "minimal-patient")).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn unauthenticated_mode_uses_public_tenant() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "public").await;
    let app = build_test_app(pool.clone());
    let patient = test_data::minimal_patient();

    let _ = send_request(app, post_resource("Patient", &patient)).await;

    // Verify it's stored under the "public" tenant
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM fhir_resources WHERE tenant_id = 'public'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

// ─── AUTH REQUIRED MODE ────────────────────────────────────────────────────

#[tokio::test]
async fn auth_required_rejects_unauthenticated_create() {
    let pool = setup_test_db().await;
    let app = build_test_app_auth_required(pool.clone());
    let patient = test_data::minimal_patient();

    let (status, body) = send_request(app, post_resource("Patient", &patient)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(body["issue"][0]["code"], "login");
}

#[tokio::test]
async fn auth_required_rejects_unauthenticated_read() {
    let pool = setup_test_db().await;
    let app = build_test_app_auth_required(pool.clone());

    let (status, body) = send_request(app, get_resource("Patient", "whatever")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn auth_required_accepts_valid_token() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "auth-valid-token").await;
    let token = tenant_token("auth-valid-token");
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, get_resource_with_token("Patient", "minimal-patient", &token)).await;
    assert_eq!(status, StatusCode::OK);
}

// ─── EXPIRED TOKENS ────────────────────────────────────────────────────────

#[tokio::test]
async fn expired_token_is_rejected() {
    let pool = setup_test_db().await;
    let token = expired_token("auth-expired");

    let app = build_test_app_auth_required(pool.clone());
    let patient = test_data::minimal_patient();
    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["issue"][0]["code"], "login");
}

// ─── WRONG SECRET ──────────────────────────────────────────────────────────

#[tokio::test]
async fn token_with_wrong_secret_is_rejected() {
    let pool = setup_test_db().await;

    // Create a token signed with a different secret
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &common::TestClaims {
            sub: Some("auth-wrong-secret".to_owned()),
            tenant: None,
            scope: Some("read write".to_owned()),
            resource_types: None,
            exp: 4_102_444_800,
        },
        &jsonwebtoken::EncodingKey::from_secret("wrong-secret".as_bytes()),
    )
    .unwrap();

    let app = build_test_app_auth_required(pool.clone());
    let patient = test_data::minimal_patient();
    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ─── SCOPE ENFORCEMENT ─────────────────────────────────────────────────────

#[tokio::test]
async fn read_only_token_cannot_create() {
    let pool = setup_test_db().await;
    let token = read_only_token("auth-ro-deny");

    let app = build_test_app_auth_required(pool.clone());
    let patient = test_data::minimal_patient();
    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["issue"][0]["code"], "forbidden");
}

#[tokio::test]
async fn read_only_token_can_read() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "auth-ro-read").await;
    // First create with a full-access token
    let rw_token = tenant_token("auth-ro-read");
    let patient = test_data::minimal_patient();
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &rw_token)).await;
    assert_eq!(status, StatusCode::CREATED);

    // Then read with read-only
    let ro_token = read_only_token("auth-ro-read");
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, get_resource_with_token("Patient", "minimal-patient", &ro_token)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn write_only_token_cannot_read() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "auth-wo-deny").await;
    // First create with a full-access token
    let rw_token = tenant_token("auth-wo-deny");
    let patient = test_data::minimal_patient();
    let app = build_test_app_auth_required(pool.clone());
    let _ = send_request(app, post_resource_with_token("Patient", &patient, &rw_token)).await;

    // Try read with write-only
    let wo_token = write_only_token("auth-wo-deny");
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, get_resource_with_token("Patient", "minimal-patient", &wo_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn write_only_token_can_create() {
    let pool = setup_test_db().await;
    let token = write_only_token("auth-wo-create");
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);
}

// ─── RESOURCE TYPE RESTRICTION ─────────────────────────────────────────────

#[tokio::test]
async fn restricted_token_allows_permitted_type() {
    let pool = setup_test_db().await;
    let token = restricted_token("auth-restrict-ok", vec!["Patient".to_string()]);
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn restricted_token_denies_unpermitted_type() {
    let pool = setup_test_db().await;
    let token = restricted_token("auth-restrict-deny", vec!["Patient".to_string()]);
    let obs = test_data::minimal_observation();

    let app = build_test_app_auth_required(pool.clone());
    let (status, body) =
        send_request(app, post_resource_with_token("Observation", &obs, &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["issue"][0]["code"], "forbidden");
}

#[tokio::test]
async fn restricted_token_denies_read_of_unpermitted_type() {
    let pool = setup_test_db().await;
    let token = restricted_token("auth-restrict-rd", vec!["Patient".to_string()]);

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, get_resource_with_token("Observation", "whatever", &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ─── TENANT ISOLATION ──────────────────────────────────────────────────────

#[tokio::test]
async fn tenants_are_isolated() {
    let pool = setup_test_db().await;
    let patient = test_data::minimal_patient();

    // Use unique tenant names for this test
    clean_tenant(&pool, "iso-a").await;
    clean_tenant(&pool, "iso-b").await;

    // Create under iso-a
    let token_a = tenant_token("iso-a");
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token_a)).await;
    assert_eq!(status, StatusCode::CREATED);

    // iso-a can read it
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", "minimal-patient", &token_a),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // iso-b cannot read it (gets 404, not 403)
    let token_b = tenant_token("iso-b");
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", "minimal-patient", &token_b),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn different_tenants_can_have_same_id() {
    let pool = setup_test_db().await;
    let patient = test_data::minimal_patient();

    clean_tenant(&pool, "dup-a").await;
    clean_tenant(&pool, "dup-b").await;
    let token_a = tenant_token("dup-a");
    let token_b = tenant_token("dup-b");

    // Both tenants create a Patient with the same id
    let app = build_test_app_auth_required(pool.clone());
    let (s, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token_a)).await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (s, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token_b)).await;
    assert_eq!(s, StatusCode::CREATED);

    // Each can read their own copy
    let app = build_test_app_auth_required(pool.clone());
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", "minimal-patient", &token_a),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let app = build_test_app_auth_required(pool.clone());
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", "minimal-patient", &token_b),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}


// ─── TOKEN CLAIM VARIANTS ──────────────────────────────────────────────────

#[tokio::test]
async fn tenant_claim_takes_precedence_over_sub() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "tenant-value").await;
    clean_tenant(&pool, "sub-value").await;
    let token = common::create_test_token(&common::TestClaims {
        sub: Some("sub-value".to_owned()),
        tenant: Some("tenant-value".to_owned()),
        scope: Some("read write".to_owned()),
        resource_types: None,
        exp: 4_102_444_800,
    });

    let patient = test_data::minimal_patient();
    let app = build_test_app_auth_required(pool.clone());
    let _ = send_request(app, post_resource_with_token("Patient", &patient, &token)).await;

    // Verify stored under "tenant-value", not "sub-value"
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fhir_resources WHERE tenant_id = 'tenant-value'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);

    let count_sub: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fhir_resources WHERE tenant_id = 'sub-value'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count_sub, 0);
}

#[tokio::test]
async fn token_with_no_tenant_and_no_sub_is_rejected() {
    let pool = setup_test_db().await;
    let token = common::create_test_token(&common::TestClaims {
        sub: None,
        tenant: None,
        scope: Some("read write".to_owned()),
        resource_types: None,
        exp: 4_102_444_800,
    });

    let patient = test_data::minimal_patient();
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ─── PUT AUTH ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn put_requires_write_scope() {
    let pool = setup_test_db().await;
    let ro_token = read_only_token("auth-put-scope");
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        put_resource_with_token("Patient", "minimal-patient", &patient, &ro_token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn put_with_restricted_token_wrong_type_is_forbidden() {
    let pool = setup_test_db().await;
    let token = restricted_token("auth-put-restrict", vec!["Observation".to_string()]);
    let patient = test_data::minimal_patient();

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        put_resource_with_token("Patient", "minimal-patient", &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
