#![allow(dead_code)]

pub mod test_data;

use std::sync::{Arc, LazyLock};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::Value;
use sqlx::{PgPool, Row};
use tower::ServiceExt;

use fhir_server::SearchConfig;
use fhir_server::auth::AuthConfig;
use fhir_server::validation::FhirSchemaValidator;

/// Shared validator instance — parsing the 3.8 MB FHIR schema once instead of
/// per-test saves significant CPU time.
static SHARED_VALIDATOR: LazyLock<Arc<FhirSchemaValidator>> =
    LazyLock::new(|| Arc::new(FhirSchemaValidator::new().expect("validator should load")));

pub const TEST_JWT_SECRET: &str = "test-secret-0123456789abcdef012345";

/// Claims for JWT test tokens.
#[derive(serde::Serialize)]
pub struct TestClaims {
    pub sub: Option<String>,
    pub tenant: Option<String>,
    pub scope: Option<String>,
    pub resource_types: Option<Vec<String>>,
    pub exp: u64,
}

/// Build a fully-wired application router connected to a real PostgreSQL pool.
pub fn build_test_app(pool: PgPool) -> Router {
    build_test_app_with_options(pool, false, Vec::new())
}

/// Build a test app with authentication enforced (unauthenticated requests are rejected).
pub fn build_test_app_auth_required(pool: PgPool) -> Router {
    build_test_app_with_options(pool, false, Vec::new())
}

pub fn build_test_app_with_options(
    pool: PgPool,
    serve_docs: bool,
    cors_allowed_origins: Vec<axum::http::HeaderValue>,
) -> Router {
    use fhir_server::store::PgStore;
    use fhir_server::{AppState, build_router};

    let state = AppState {
        store: PgStore::new(pool),
        auth: AuthConfig::from_hmac_secret(jsonwebtoken::Algorithm::HS256, TEST_JWT_SECRET),
        fhir_base_url: "http://localhost:8080/fhir".to_owned(),
        search: SearchConfig {
            default_count: 50,
            max_count: 500,
        },
        validator: Arc::clone(&SHARED_VALIDATOR),
        cors_allowed_origins,
        serve_docs,
    };

    build_router(state)
}

/// Create a JWT token for testing with given claims.
pub fn create_test_token(claims: &TestClaims) -> String {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .expect("token encoding should succeed")
}

/// Create a default read-write token for a given tenant.
pub fn tenant_token(tenant: &str) -> String {
    create_test_token(&TestClaims {
        sub: Some(tenant.to_owned()),
        tenant: None,
        scope: Some("read write".to_owned()),
        resource_types: None,
        exp: 4_102_444_800, // year 2100
    })
}

/// Create a read-only token for a given tenant.
pub fn read_only_token(tenant: &str) -> String {
    create_test_token(&TestClaims {
        sub: Some(tenant.to_owned()),
        tenant: None,
        scope: Some("read".to_owned()),
        resource_types: None,
        exp: 4_102_444_800,
    })
}

/// Create a write-only token for a given tenant.
pub fn write_only_token(tenant: &str) -> String {
    create_test_token(&TestClaims {
        sub: Some(tenant.to_owned()),
        tenant: None,
        scope: Some("write".to_owned()),
        resource_types: None,
        exp: 4_102_444_800,
    })
}

/// Create a token restricted to specific resource types.
pub fn restricted_token(tenant: &str, resource_types: Vec<String>) -> String {
    create_test_token(&TestClaims {
        sub: Some(tenant.to_owned()),
        tenant: None,
        scope: Some("read write".to_owned()),
        resource_types: Some(resource_types),
        exp: 4_102_444_800,
    })
}

/// Create an expired token.
pub fn expired_token(tenant: &str) -> String {
    create_test_token(&TestClaims {
        sub: Some(tenant.to_owned()),
        tenant: None,
        scope: Some("read write".to_owned()),
        resource_types: None,
        exp: 0, // Unix epoch = already expired
    })
}

/// Acquire a connection pool pointed at the test database and run migrations.
///
/// Each call returns a pool connected to the same database. Tests running in
/// parallel share the database but use the default "public" tenant (when
/// unauthenticated) or specific tenant identifiers, keeping data isolated.
///
/// Each test gets its own pool to avoid cross-runtime lifetime issues
/// (each #[tokio::test] has its own runtime). The expensive part — parsing
/// the FHIR schema — is shared via SHARED_VALIDATOR above.
pub async fn setup_test_db() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1/fhir_test".to_owned());
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("failed to connect to test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations");

    pool
}

/// Clean all data for a specific tenant.
pub async fn clean_tenant(pool: &PgPool, tenant_id: &str) {
    sqlx::query("DELETE FROM fhir_resource_history WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(pool)
        .await
        .expect("failed to clean tenant history");

    sqlx::query("DELETE FROM fhir_resources WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(pool)
        .await
        .expect("failed to clean tenant data");
}

/// Send a request and return (status, body_value).
pub async fn send_request(app: Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(req).await.expect("request should complete");

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let value: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

    (status, value)
}

/// Build a POST request to create a FHIR resource.
pub fn post_resource(resource_type: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/fhir/{resource_type}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .expect("request should build")
}

/// Build an authenticated POST request.
pub fn post_resource_with_token(resource_type: &str, body: &Value, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/fhir/{resource_type}"))
        .header("content-type", "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .expect("request should build")
}

/// Build a GET request to read a FHIR resource.
pub fn get_resource(resource_type: &str, id: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(format!("/fhir/{resource_type}/{id}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build an authenticated GET request.
pub fn get_resource_with_token(resource_type: &str, id: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(format!("/fhir/{resource_type}/{id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build an authenticated GET request for instance history.
pub fn get_resource_history_with_token(
    resource_type: &str,
    id: &str,
    token: &str,
) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(format!("/fhir/{resource_type}/{id}/_history"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build an authenticated GET request for instance history with a query string.
pub fn get_resource_history_with_query(
    resource_type: &str,
    id: &str,
    query: &str,
    token: &str,
) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(format!("/fhir/{resource_type}/{id}/_history?{query}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build a GET request to search a FHIR resource collection.
pub fn search_resource(resource_type: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(format!("/fhir/{resource_type}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build an authenticated GET request to search a FHIR resource collection.
pub fn search_resource_with_token(
    resource_type: &str,
    query: Option<&str>,
    token: &str,
) -> Request<Body> {
    let uri = match query {
        Some(query) => format!("/fhir/{resource_type}?{query}"),
        None => format!("/fhir/{resource_type}"),
    };

    Request::builder()
        .method("GET")
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build a PUT request to update a FHIR resource.
pub fn put_resource(resource_type: &str, id: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(format!("/fhir/{resource_type}/{id}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .expect("request should build")
}

/// Build an authenticated PUT request.
pub fn put_resource_with_token(
    resource_type: &str,
    id: &str,
    body: &Value,
    token: &str,
) -> Request<Body> {
    put_resource_with_token_if_match(resource_type, id, body, token, "W/\"1\"")
}

/// Build an authenticated PUT request with an explicit If-Match precondition.
pub fn put_resource_with_token_if_match(
    resource_type: &str,
    id: &str,
    body: &Value,
    token: &str,
    if_match: &str,
) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(format!("/fhir/{resource_type}/{id}"))
        .header("content-type", "application/json")
        .header(header::IF_MATCH, if_match)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .expect("request should build")
}

/// Assert that a response body is a valid OperationOutcome.
pub fn assert_operation_outcome(value: &Value, expected_code: &str) {
    assert_eq!(
        value["resourceType"], "OperationOutcome",
        "response must be OperationOutcome, got: {value}"
    );
    assert!(
        value["issue"].is_array(),
        "OperationOutcome must have issues array"
    );
    let issues = value["issue"].as_array().unwrap();
    assert!(
        !issues.is_empty(),
        "OperationOutcome must have at least one issue"
    );
    assert_eq!(
        issues[0]["code"], expected_code,
        "expected issue code '{expected_code}', got: {}",
        issues[0]["code"]
    );
}

/// Build a DELETE request to delete a FHIR resource.
pub fn delete_resource(resource_type: &str, id: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(format!("/fhir/{resource_type}/{id}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build an authenticated DELETE request.
pub fn delete_resource_with_token(resource_type: &str, id: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(format!("/fhir/{resource_type}/{id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build an authenticated DELETE request with an explicit If-Match precondition.
pub fn delete_resource_with_token_if_match(
    resource_type: &str,
    id: &str,
    token: &str,
    if_match: &str,
) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(format!("/fhir/{resource_type}/{id}"))
        .header(header::IF_MATCH, if_match)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request should build")
}

/// Build an authenticated POST request with an If-None-Exist header for conditional create.
pub fn post_resource_conditional(
    resource_type: &str,
    body: &Value,
    token: &str,
    if_none_exist: &str,
) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/fhir/{resource_type}"))
        .header("content-type", "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("If-None-Exist", if_none_exist)
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .expect("request should build")
}

/// Count rows in fhir_resources for a given tenant.
pub async fn count_resources(pool: &PgPool, tenant_id: &str) -> i64 {
    sqlx::query("SELECT count(*) as cnt FROM fhir_resources WHERE tenant_id = $1")
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .expect("count query should succeed")
        .get::<i64, _>("cnt")
}

/// Count rows in fhir_resource_history for a given tenant.
pub async fn count_history_entries(pool: &PgPool, tenant_id: &str) -> i64 {
    sqlx::query("SELECT count(*) as cnt FROM fhir_resource_history WHERE tenant_id = $1")
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .expect("history count query should succeed")
        .get::<i64, _>("cnt")
}
