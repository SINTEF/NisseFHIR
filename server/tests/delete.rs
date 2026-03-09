mod common;

use axum::http::StatusCode;
use common::{
    build_test_app_auth_required, clean_tenant, count_history_entries, count_resources,
    delete_resource, delete_resource_with_token, get_resource_with_token, post_resource_with_token,
    read_only_token, restricted_token, send_request, setup_test_db, tenant_token, test_data,
};
use sqlx::Row;
use tower::ServiceExt;

async fn setup(tenant: &str) -> (sqlx::PgPool, String) {
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    (pool, tenant.to_owned())
}

#[tokio::test]
async fn delete_existing_resource_returns_204() {
    let (pool, _) = setup("del-204").await;
    let app = build_test_app_auth_required(pool.clone());
    let token = tenant_token("del-204");
    let patient = test_data::minimal_patient();

    let (status, _) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = send_request(
        app,
        delete_resource_with_token("Patient", "minimal-patient", &token),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn delete_nonexistent_resource_returns_404() {
    let (pool, _) = setup("del-404").await;
    let app = build_test_app_auth_required(pool);
    let token = tenant_token("del-404");

    let (status, _) = send_request(
        app,
        delete_resource_with_token("Patient", "does-not-exist", &token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleted_resource_no_longer_readable() {
    let (pool, _) = setup("del-gone").await;
    let app = build_test_app_auth_required(pool.clone());
    let token = tenant_token("del-gone");
    let patient = test_data::minimal_patient();

    send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, &token),
    )
    .await;
    send_request(
        app.clone(),
        delete_resource_with_token("Patient", "minimal-patient", &token),
    )
    .await;

    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", "minimal-patient", &token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_reduces_resource_count() {
    let (pool, _) = setup("del-count").await;
    let app = build_test_app_auth_required(pool.clone());
    let token = tenant_token("del-count");

    send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(count_resources(&pool, "del-count").await, 1);

    send_request(
        app,
        delete_resource_with_token("Patient", "minimal-patient", &token),
    )
    .await;
    assert_eq!(count_resources(&pool, "del-count").await, 0);
}

#[tokio::test]
async fn delete_requires_write_scope() {
    let (pool, _) = setup("del-scope").await;
    let app = build_test_app_auth_required(pool.clone());
    let token = tenant_token("del-scope");
    let ro_token = read_only_token("del-scope");

    send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;

    let (status, _) = send_request(
        app,
        delete_resource_with_token("Patient", "minimal-patient", &ro_token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn delete_respects_resource_type_restriction() {
    let (pool, _) = setup("del-restrict").await;
    let app = build_test_app_auth_required(pool.clone());
    let full_token = tenant_token("del-restrict");
    let obs_only = restricted_token("del-restrict", vec!["Observation".to_owned()]);

    send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &full_token),
    )
    .await;

    let (status, _) = send_request(
        app,
        delete_resource_with_token("Patient", "minimal-patient", &obs_only),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn delete_respects_tenant_isolation() {
    let (pool, _) = setup("del-iso-a").await;
    clean_tenant(&pool, "del-iso-b").await;
    let app = build_test_app_auth_required(pool.clone());
    let token_a = tenant_token("del-iso-a");
    let token_b = tenant_token("del-iso-b");

    // Create in tenant A
    send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token_a),
    )
    .await;

    // Tenant B cannot delete tenant A's resource
    let (status, _) = send_request(
        app.clone(),
        delete_resource_with_token("Patient", "minimal-patient", &token_b),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Tenant A can still read it
    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", "minimal-patient", &token_a),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn delete_unauthenticated_rejected_when_required() {
    let (pool, _) = setup("del-unauth").await;
    let app = build_test_app_auth_required(pool);

    let (status, _) = send_request(app, delete_resource("Patient", "any-id")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn delete_writes_history_tombstone() {
    let (pool, _) = setup("del-history").await;
    let app = build_test_app_auth_required(pool.clone());
    let token = tenant_token("del-history");

    let (status, _) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = send_request(
        app,
        delete_resource_with_token("Patient", "minimal-patient", &token),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_eq!(count_history_entries(&pool, "del-history").await, 2);

    let row = sqlx::query(
        r#"
        SELECT version_id, deleted
        FROM fhir_resource_history
        WHERE tenant_id = $1 AND resource_type = 'Patient' AND id = 'minimal-patient'
        ORDER BY version_id DESC
        LIMIT 1
        "#,
    )
    .bind("del-history")
    .fetch_one(&pool)
    .await
    .expect("history query should succeed");

    assert_eq!(row.get::<i64, _>("version_id"), 2);
    assert!(row.get::<bool, _>("deleted"));
}

#[tokio::test]
async fn recreate_after_delete_continues_version_sequence() {
    let (pool, _) = setup("del-recreate-version").await;
    let app = build_test_app_auth_required(pool.clone());
    let token = tenant_token("del-recreate-version");

    let response = app
        .clone()
        .oneshot(post_resource_with_token(
            "Patient",
            &test_data::minimal_patient(),
            &token,
        ))
        .await
        .expect("create should complete");
    assert_eq!(response.headers().get("ETag").unwrap(), "W/\"1\"");

    let response = app
        .clone()
        .oneshot(delete_resource_with_token(
            "Patient",
            "minimal-patient",
            &token,
        ))
        .await
        .expect("delete should complete");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(post_resource_with_token(
            "Patient",
            &test_data::minimal_patient(),
            &token,
        ))
        .await
        .expect("recreate should complete");
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers().get("ETag").unwrap(), "W/\"3\"");
}
