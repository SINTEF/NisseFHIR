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

    let (status, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = send_request(
        app,
        delete_resource_with_token("Patient", created["id"].as_str().unwrap(), &token),
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

    let (_, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, &token),
    )
    .await;
    send_request(
        app.clone(),
        delete_resource_with_token("Patient", created["id"].as_str().unwrap(), &token),
    )
    .await;

    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", created["id"].as_str().unwrap(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_reduces_resource_count() {
    let (pool, _) = setup("del-count").await;
    let app = build_test_app_auth_required(pool.clone());
    let token = tenant_token("del-count");

    let (_, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(count_resources(&pool, "del-count").await, 1);

    send_request(
        app,
        delete_resource_with_token("Patient", created["id"].as_str().unwrap(), &token),
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

    let (_, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;

    let (status, _) = send_request(
        app,
        delete_resource_with_token("Patient", created["id"].as_str().unwrap(), &ro_token),
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

    let (_, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &full_token),
    )
    .await;

    let (status, _) = send_request(
        app,
        delete_resource_with_token("Patient", created["id"].as_str().unwrap(), &obs_only),
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
    let (_, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token_a),
    )
    .await;

    // Tenant B cannot delete tenant A's resource
    let (status, _) = send_request(
        app.clone(),
        delete_resource_with_token("Patient", created["id"].as_str().unwrap(), &token_b),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Tenant A can still read it
    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", created["id"].as_str().unwrap(), &token_a),
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

    let (status, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();

    let (status, _) = send_request(app, delete_resource_with_token("Patient", id, &token)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_eq!(count_history_entries(&pool, "del-history").await, 2);

    let row = sqlx::query(
        r#"
        SELECT version_id, deleted
        FROM fhir_resource_history
        WHERE tenant_id = $1 AND resource_type = 'Patient' AND id = $2
        ORDER BY version_id DESC
        LIMIT 1
        "#,
    )
    .bind("del-history")
    .bind(id)
    .fetch_one(&pool)
    .await
    .expect("history query should succeed");

    assert_eq!(row.get::<i64, _>("version_id"), 2);
    assert!(row.get::<bool, _>("deleted"));
}

#[tokio::test]
async fn create_after_delete_gets_a_new_identity_and_initial_version() {
    let (pool, _) = setup("del-recreate-version").await;
    let app = build_test_app_auth_required(pool.clone());
    let token = tenant_token("del-recreate-version");

    let (status, first) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let first_id = first["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(delete_resource_with_token("Patient", first_id, &token))
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
    assert_eq!(response.headers().get("ETag").unwrap(), "W/\"1\"");
}
