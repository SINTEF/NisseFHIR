mod common;

use axum::{
    body::Body,
    http::{HeaderValue, Method, Request, StatusCode, header},
};
use axum::body::to_bytes;
use common::{build_test_app, build_test_app_with_options, build_dev_test_app};
use tower::ServiceExt;

fn lazy_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://postgres:postgres@localhost/postgres")
        .expect("lazy pool should build")
}

#[tokio::test]
async fn docs_route_is_disabled_by_default() {
    let app = build_test_app(lazy_pool());

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/docs")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn docs_route_can_be_enabled_explicitly() {
    let app = build_test_app_with_options(lazy_pool(), true, Vec::new());

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/docs")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn cors_allows_only_configured_origin() {
    let app = build_test_app_with_options(
        lazy_pool(),
        false,
        vec![HeaderValue::from_static("https://app.example")],
    );

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/fhir/Patient")
                .header(header::ORIGIN, "https://app.example")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization,content-type")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("configured origin should be allowed"),
        "https://app.example"
    );
}

#[tokio::test]
async fn cors_does_not_reflect_unconfigured_origin() {
    let app = build_test_app_with_options(
        lazy_pool(),
        false,
        vec![HeaderValue::from_static("https://app.example")],
    );

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/fhir/Patient")
                .header(header::ORIGIN, "https://evil.example")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization,content-type")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none(),
        "unconfigured origin must not receive CORS allow header"
    );
}

// ---------------------------------------------------------------------------
// Dev token endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dev_token_endpoint_mints_valid_token() {
    let (app, _) = build_dev_test_app(lazy_pool());

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/dev/token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"tenant":"my-tenant","scope":"read write"}"#))
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["token"].is_string());
    assert!(json["expires_in"].is_number());
}

#[tokio::test]
async fn dev_token_defaults_if_body_is_empty_object() {
    let (app, _) = build_dev_test_app(lazy_pool());

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/dev/token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn dev_token_endpoint_hidden_in_static_mode() {
    let app = build_test_app(lazy_pool());

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/dev/token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    // Route should not exist in static mode.
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn dev_token_can_authenticate_requests() {
    let pool = common::setup_test_db().await;
    let (app, _) = build_dev_test_app(pool.clone());

    // Mint a token.
    let mint_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/dev/token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"tenant":"dev-int-test"}"#))
                .expect("request should build"),
        )
        .await
        .expect("mint request should complete");

    assert_eq!(mint_response.status(), StatusCode::OK);
    let body = to_bytes(mint_response.into_body(), usize::MAX).await.unwrap();
    let mint: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = mint["token"].as_str().unwrap();

    // Use the minted token to create a resource.
    let patient = serde_json::json!({
        "resourceType": "Patient",
        "active": true
    });

    let (status, _) = common::send_request(
        app,
        common::post_resource_with_token("Patient", &patient, token),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);

    // Clean up.
    common::clean_tenant(&pool, "dev-int-test").await;
}