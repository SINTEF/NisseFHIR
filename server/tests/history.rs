mod common;

use axum::http::StatusCode;

use common::{
    build_test_app_auth_required, clean_tenant, get_resource_history_with_token,
    post_resource_with_token, put_resource_with_token, read_only_token, restricted_token,
    send_request, setup_test_db, tenant_token, test_data,
};

#[tokio::test]
async fn history_returns_bundle_with_versions_descending() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-desc").await;
    let token = tenant_token("history-desc");
    let app = build_test_app_auth_required(pool.clone());

    let patient = test_data::minimal_patient();
    let (status, _) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let mut updated = patient.clone();
    updated["active"] = serde_json::json!(true);
    let (status, _) = send_request(
        app.clone(),
        put_resource_with_token("Patient", "minimal-patient", &updated, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send_request(
        app,
        get_resource_history_with_token("Patient", "minimal-patient", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "Bundle");
    assert_eq!(body["type"], "history");
    assert_eq!(body["total"], 2);
    assert_eq!(
        body["link"][0]["url"],
        "http://localhost:8080/fhir/Patient/minimal-patient/_history"
    );
    assert_eq!(body["entry"][0]["request"]["url"], "Patient/minimal-patient");
    assert_eq!(body["entry"][0]["response"]["etag"], "W/\"2\"");
    assert_eq!(body["entry"][1]["response"]["etag"], "W/\"1\"");
    assert_eq!(body["entry"][0]["resource"]["active"], true);
    assert_eq!(body["entry"][1]["resource"]["id"], "minimal-patient");
}

#[tokio::test]
async fn history_unauthenticated_rejected_when_required() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-unauth").await;
    let token = tenant_token("history-unauth");
    let app = build_test_app_auth_required(pool.clone());

    let (status, _) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/fhir/Patient/minimal-patient/_history")
        .body(axum::body::Body::empty())
        .expect("request should build");

    let (status, body) = send_request(app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["issue"][0]["code"], "login");
}

#[tokio::test]
async fn history_includes_delete_tombstone_entry() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-delete").await;
    let token = tenant_token("history-delete");
    let app = build_test_app_auth_required(pool.clone());

    let (status, _) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let delete_request = axum::http::Request::builder()
        .method("DELETE")
        .uri("/fhir/Patient/minimal-patient")
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(axum::body::Body::empty())
        .expect("delete request should build");
    let (status, _) = send_request(app.clone(), delete_request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = send_request(
        app,
        get_resource_history_with_token("Patient", "minimal-patient", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 2);
    assert_eq!(body["entry"][0]["response"]["status"], "410 Gone");
    assert_eq!(body["entry"][0]["request"]["method"], "DELETE");
    assert_eq!(body["entry"][0]["response"]["etag"], "W/\"2\"");
}

#[tokio::test]
async fn history_requires_read_scope() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-read-scope").await;
    let rw_token = tenant_token("history-read-scope");
    let write_only = common::write_only_token("history-read-scope");
    let app = build_test_app_auth_required(pool.clone());

    let (status, _) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &rw_token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = send_request(
        app,
        get_resource_history_with_token("Patient", "minimal-patient", &write_only),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["issue"][0]["code"], "forbidden");
}

#[tokio::test]
async fn history_respects_resource_type_restrictions() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-restricted").await;
    let token = tenant_token("history-restricted");
    let restricted = restricted_token("history-restricted", vec!["Observation".to_owned()]);
    let app = build_test_app_auth_required(pool.clone());

    let (status, _) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = send_request(
        app,
        get_resource_history_with_token("Patient", "minimal-patient", &restricted),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["issue"][0]["code"], "forbidden");
}

#[tokio::test]
async fn history_respects_tenant_isolation() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-tenant-a").await;
    clean_tenant(&pool, "history-tenant-b").await;

    let token_a = tenant_token("history-tenant-a");
    let token_b = tenant_token("history-tenant-b");
    let app = build_test_app_auth_required(pool.clone());

    let (status, _) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token_a),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = send_request(
        app,
        get_resource_history_with_token("Patient", "minimal-patient", &token_b),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn history_returns_404_when_no_versions_exist() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-missing").await;
    let token = read_only_token("history-missing");
    let app = build_test_app_auth_required(pool);

    let (status, body) = send_request(
        app,
        get_resource_history_with_token("Patient", "does-not-exist", &token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["issue"][0]["code"], "not-found");
}
