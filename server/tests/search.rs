mod common;

use axum::http::StatusCode;
use common::{
    build_test_app, build_test_app_auth_required, clean_tenant, post_resource,
    post_resource_with_token, restricted_token, search_resource, search_resource_with_token,
    send_request, setup_test_db, tenant_token, test_data,
};

#[tokio::test]
async fn search_returns_searchset_bundle() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-bundle").await;
    let token = tenant_token("search-bundle");

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(app, search_resource_with_token("Patient", None, &token)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "Bundle");
    assert_eq!(body["type"], "searchset");
    assert_eq!(body["total"], 1);
    assert_eq!(body["entry"][0]["resource"]["id"], "minimal-patient");
    assert_eq!(body["entry"][0]["search"]["mode"], "match");
}

#[tokio::test]
async fn search_is_limited_and_paged() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-paging").await;
    let token = tenant_token("search-paging");

    for id in ["patient-a", "patient-b", "patient-c"] {
        let mut patient = test_data::minimal_patient();
        patient["id"] = serde_json::json!(id);

        let app = build_test_app_auth_required(pool.clone());
        let (status, _) = send_request(
            app,
            post_resource_with_token("Patient", &patient, &token),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let app = build_test_app_auth_required(pool.clone());
    let (status, first_page) = send_request(
        app,
        search_resource_with_token("Patient", Some("_count=2&_offset=0"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_page["total"], 3);
    assert_eq!(first_page["entry"].as_array().unwrap().len(), 2);
    assert_eq!(first_page["link"][1]["relation"], "next");

    let app = build_test_app_auth_required(pool);
    let (status, second_page) = send_request(
        app,
        search_resource_with_token("Patient", Some("_count=2&_offset=2"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_page["entry"].as_array().unwrap().len(), 1);
    assert_eq!(second_page["link"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn search_respects_tenant_isolation() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-tenant-a").await;
    clean_tenant(&pool, "search-tenant-b").await;

    let token_a = tenant_token("search-tenant-a");
    let token_b = tenant_token("search-tenant-b");

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token_a),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (status, body) = send_request(app, search_resource_with_token("Patient", None, &token_b)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert_eq!(body["entry"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn search_rejects_forbidden_resource_type() {
    let pool = setup_test_db().await;
    let token = restricted_token("search-restricted", vec!["Observation".to_owned()]);

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(app, search_resource_with_token("Patient", None, &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["issue"][0]["code"], "forbidden");
}

#[tokio::test]
async fn search_in_unauthenticated_mode_uses_public_tenant() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "public").await;

    let app = build_test_app(pool.clone());
    let (status, _) = send_request(app, post_resource("Patient", &test_data::minimal_patient())).await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app(pool);
    let (status, body) = send_request(app, search_resource("Patient")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
}

#[tokio::test]
async fn search_rejects_count_above_limit() {
    let pool = setup_test_db().await;
    let token = tenant_token("search-limit");

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("_count=101"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");
}