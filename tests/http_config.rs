mod common;

use axum::body::to_bytes;
use axum::{
    body::Body,
    http::{HeaderValue, Method, Request, StatusCode, header},
};
use common::{build_test_app, build_test_app_with_options};
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
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/docs")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response
            .headers()
            .get(header::LOCATION)
            .expect("docs redirect should set a location"),
        "/docs/"
    );
}

#[tokio::test]
async fn openapi_docs_advertise_bearer_auth_for_protected_routes() {
    let app = build_test_app_with_options(lazy_pool(), true, Vec::new());

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/docs/openapi.json")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("openapi body should be readable");
    let document: serde_json::Value =
        serde_json::from_slice(&body).expect("openapi document should be valid json");

    assert_eq!(
        document["components"]["securitySchemes"]["bearer_auth"]["type"],
        "http"
    );
    assert_eq!(
        document["components"]["securitySchemes"]["bearer_auth"]["scheme"],
        "bearer"
    );
    assert_eq!(
        document["paths"]["/fhir/{resource_type}"]["get"]["security"][0]["bearer_auth"],
        serde_json::json!([])
    );
}

#[tokio::test]
async fn docs_support_gzip_response_compression() {
    let app = build_test_app_with_options(lazy_pool(), true, Vec::new());

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/docs/openapi.json")
                .header(header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_ENCODING)
            .expect("gzip responses should advertise content encoding"),
        "gzip"
    );
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
                .header(
                    header::ACCESS_CONTROL_REQUEST_HEADERS,
                    "authorization,content-type",
                )
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
                .header(
                    header::ACCESS_CONTROL_REQUEST_HEADERS,
                    "authorization,content-type",
                )
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

#[tokio::test]
async fn dev_token_endpoint_is_not_available() {
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
