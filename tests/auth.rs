//! Integration tests for authentication and authorization.

mod common;

use axum::http::StatusCode;
use common::{
    build_test_app_auth_required, clean_tenant, expired_token, get_resource,
    get_resource_with_token, post_resource, post_resource_with_token, put_resource_with_token,
    read_only_token, restricted_token, send_request, setup_test_db, tenant_token, test_data,
    write_only_token,
};

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
    let (status, created) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", created["id"].as_str().unwrap(), &token),
    )
    .await;
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
        &jsonwebtoken::EncodingKey::from_secret("wrong-secret-wrong-secret-wrong!!".as_bytes()),
    )
    .unwrap();

    let app = build_test_app_auth_required(pool.clone());
    let patient = test_data::minimal_patient();
    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_missing_exp_is_rejected() {
    let pool = setup_test_db().await;
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &fhir_server::auth::Claims {
            sub: Some("auth-no-exp".to_owned()),
            tenant: None,
            scope: Some("read write".to_owned()),
            resource_types: None,
            exp: None,
        },
        &jsonwebtoken::EncodingKey::from_secret(common::TEST_JWT_SECRET.as_bytes()),
    )
    .unwrap();

    let patient = test_data::minimal_patient();
    let app = build_test_app_auth_required(pool.clone());
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
    let (status, created) = send_request(
        app,
        post_resource_with_token("Patient", &patient, &rw_token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Then read with read-only
    let ro_token = read_only_token("auth-ro-read");
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", created["id"].as_str().unwrap(), &ro_token),
    )
    .await;
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
    let _ = send_request(
        app,
        post_resource_with_token("Patient", &patient, &rw_token),
    )
    .await;

    // Try read with write-only
    let wo_token = write_only_token("auth-wo-deny");
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", "minimal-patient", &wo_token),
    )
    .await;
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
    let (status, _) = send_request(
        app,
        get_resource_with_token("Observation", "whatever", &token),
    )
    .await;
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
    let (status, created) =
        send_request(app, post_resource_with_token("Patient", &patient, &token_a)).await;
    assert_eq!(status, StatusCode::CREATED);

    // iso-a can read it
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", created["id"].as_str().unwrap(), &token_a),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // iso-b cannot read it (gets 404, not 403)
    let token_b = tenant_token("iso-b");
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", created["id"].as_str().unwrap(), &token_b),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn different_tenants_receive_independent_server_ids() {
    let pool = setup_test_db().await;
    let patient = test_data::minimal_patient();

    clean_tenant(&pool, "dup-a").await;
    clean_tenant(&pool, "dup-b").await;
    let token_a = tenant_token("dup-a");
    let token_b = tenant_token("dup-b");

    // Both tenants submit the same source id; each receives a server id.
    let app = build_test_app_auth_required(pool.clone());
    let (s, patient_a) =
        send_request(app, post_resource_with_token("Patient", &patient, &token_a)).await;
    assert_eq!(s, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (s, patient_b) =
        send_request(app, post_resource_with_token("Patient", &patient, &token_b)).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_ne!(patient_a["id"], patient_b["id"]);

    // Each can read their own copy
    let app = build_test_app_auth_required(pool.clone());
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", patient_a["id"].as_str().unwrap(), &token_a),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let app = build_test_app_auth_required(pool.clone());
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", patient_b["id"].as_str().unwrap(), &token_b),
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
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM fhir_resources WHERE tenant_id = 'tenant-value'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);

    let count_sub: i64 =
        sqlx::query_scalar("SELECT count(*) FROM fhir_resources WHERE tenant_id = 'sub-value'")
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
