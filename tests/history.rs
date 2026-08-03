mod common;

use axum::http::StatusCode;

use common::{
    build_test_app_auth_required, clean_tenant, delete_resource_with_token,
    get_resource_history_with_query, get_resource_history_with_token, post_resource_with_token,
    put_resource_with_token, put_resource_with_token_if_match, read_only_token, restricted_token,
    send_request, setup_test_db, tenant_token, test_data,
};

#[tokio::test]
async fn history_returns_bundle_with_versions_descending() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-desc").await;
    let token = tenant_token("history-desc");
    let app = build_test_app_auth_required(pool.clone());

    let patient = test_data::minimal_patient();
    let (status, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_owned();

    let mut updated = created;
    updated["active"] = serde_json::json!(true);
    let (status, _) = send_request(
        app.clone(),
        put_resource_with_token("Patient", &id, &updated, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) =
        send_request(app, get_resource_history_with_token("Patient", &id, &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "Bundle");
    assert_eq!(body["type"], "history");
    assert!(body.get("total").is_none());
    assert_eq!(
        body["link"][0]["url"],
        format!("http://localhost:8080/fhir/Patient/{id}/_history?_count=50")
    );
    assert_eq!(body["entry"][0]["request"]["url"], format!("Patient/{id}"));
    assert_eq!(body["entry"][0]["response"]["etag"], "W/\"2\"");
    assert_eq!(body["entry"][1]["response"]["etag"], "W/\"1\"");
    assert_eq!(body["entry"][0]["resource"]["active"], true);
    assert_eq!(body["entry"][1]["resource"]["id"], id);
}

#[tokio::test]
async fn history_unauthenticated_rejected_when_required() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-unauth").await;
    let token = tenant_token("history-unauth");
    let app = build_test_app_auth_required(pool.clone());

    let (status, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();

    let request = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/fhir/Patient/{id}/_history"))
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

    let (status, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();

    let delete_request = axum::http::Request::builder()
        .method("DELETE")
        .uri(format!("/fhir/Patient/{id}"))
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(axum::body::Body::empty())
        .expect("delete request should build");
    let (status, _) = send_request(app.clone(), delete_request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) =
        send_request(app, get_resource_history_with_token("Patient", id, &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("total").is_none());
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

/// Create the given number of versions for one Patient: one create plus
/// `updates` additional updates, then a delete tombstone.
async fn create_versioned_patient(app: axum::Router, token: &str, updates: usize) -> String {
    let patient = test_data::minimal_patient();
    let (status, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_owned();

    // After create the latest version is 1; each successful update increments
    // it, so the If-Match ETag must track the current version.
    for (n, current_version) in (0..updates).zip(1_i64..) {
        let mut updated = created.clone();
        updated["active"] = serde_json::json!(n % 2 == 0);
        let (status, _) = send_request(
            app.clone(),
            put_resource_with_token_if_match(
                "Patient",
                &id,
                &updated,
                token,
                &format!("W/\"{current_version}\""),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    // Final deletion creates a tombstone version.
    let (status, _) = send_request(
        app.clone(),
        delete_resource_with_token("Patient", &id, token),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    id
}

#[tokio::test]
async fn history_paginates_across_multiple_pages_with_tombstone() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-page").await;
    let token = tenant_token("history-page");
    let app = build_test_app_auth_required(pool);

    // create + 3 updates + delete = 5 versions (ids 1..=5, newest is the
    // tombstone at version 5).
    let id = create_versioned_patient(app.clone(), &token, 3).await;

    // Page 1: _count=2 => versions 5 (DELETE) and 4.
    let (status, body) = send_request(
        app.clone(),
        get_resource_history_with_query("Patient", &id, "_count=2", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("total").is_none());
    assert_eq!(body["type"], "history");
    assert_eq!(body["entry"].as_array().unwrap().len(), 2);
    assert_eq!(body["entry"][0]["response"]["etag"], "W/\"5\"");
    assert_eq!(body["entry"][0]["response"]["status"], "410 Gone");
    assert_eq!(body["entry"][0]["request"]["method"], "DELETE");
    assert_eq!(body["entry"][1]["response"]["etag"], "W/\"4\"");
    assert_eq!(
        body["link"][0]["url"],
        format!("http://localhost:8080/fhir/Patient/{id}/_history?_count=2")
    );
    assert_eq!(body["link"][1]["relation"], "next");
    assert_eq!(
        body["link"][1]["url"],
        format!("http://localhost:8080/fhir/Patient/{id}/_history?_count=2&_after_id=4")
    );

    let next_query = body["link"][1]["url"]
        .as_str()
        .unwrap()
        .split_once('?')
        .unwrap()
        .1
        .to_owned();

    // Page 2: follow next => versions 3 and 2.
    let (status, body) = send_request(
        app.clone(),
        get_resource_history_with_query("Patient", &id, &next_query, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("total").is_none());
    assert_eq!(body["entry"].as_array().unwrap().len(), 2);
    assert_eq!(body["entry"][0]["response"]["etag"], "W/\"3\"");
    assert_eq!(body["entry"][1]["response"]["etag"], "W/\"2\"");
    assert_eq!(
        body["link"][0]["url"],
        format!("http://localhost:8080/fhir/Patient/{id}/_history?_count=2&_after_id=4")
    );
    assert_eq!(body["link"][1]["relation"], "next");
    assert_eq!(
        body["link"][1]["url"],
        format!("http://localhost:8080/fhir/Patient/{id}/_history?_count=2&_after_id=2")
    );

    // Page 3: remaining single version (1), no next link.
    let (status, body) = send_request(
        app,
        get_resource_history_with_query("Patient", &id, "_count=2&_after_id=2", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("total").is_none());
    assert_eq!(body["entry"].as_array().unwrap().len(), 1);
    assert_eq!(body["entry"][0]["response"]["etag"], "W/\"1\"");
    let relations: Vec<&str> = body["link"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["relation"].as_str().unwrap())
        .collect();
    assert!(!relations.contains(&"next"));
}

#[tokio::test]
async fn history_count_above_configured_max_is_rejected() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-clamp").await;
    let token = tenant_token("history-clamp");
    let app = build_test_app_auth_required(pool);

    let id = create_versioned_patient(app.clone(), &token, 3).await;

    // _count above the configured max (500) must be rejected.
    let (status, body) = send_request(
        app,
        get_resource_history_with_query("Patient", &id, "_count=1000", &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn history_after_cursor_past_end_returns_empty_page_without_total() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "history-empty-page").await;
    let token = tenant_token("history-empty-page");
    let app = build_test_app_auth_required(pool);

    let id = create_versioned_patient(app.clone(), &token, 1).await;
    // 3 versions exist (1..=3). Ask for versions older than 1 => empty page.
    let (status, body) = send_request(
        app,
        get_resource_history_with_query("Patient", &id, "_count=10&_after_id=1", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("total").is_none());
    assert_eq!(body["entry"].as_array().unwrap().len(), 0);
    let relations: Vec<&str> = body["link"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["relation"].as_str().unwrap())
        .collect();
    assert!(!relations.contains(&"next"));
}
