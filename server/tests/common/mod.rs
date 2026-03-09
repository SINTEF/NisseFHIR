pub mod test_data;

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    Router,
};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::Value;
use sqlx::{PgPool, Row};
use tower::ServiceExt;

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
///
/// Uses a specific tenant for isolation. The app allows unauthenticated
/// requests that default to the "public" tenant—for per-test isolation,
/// prefer `build_test_app_for_tenant`.
pub fn build_test_app(pool: PgPool) -> Router {
    use fhir_server::{AppState, build_router};
    use fhir_server::auth::AuthConfig;
    use fhir_server::store::PgStore;
    use fhir_server::validation::FhirSchemaValidator;

    let state = AppState {
        store: PgStore::new(pool),
        auth: AuthConfig {
            jwt_secret: "test-secret".to_owned(),
            allow_unauthenticated: true,
        },
        fhir_base_url: "http://localhost:8080/fhir".to_owned(),
        validator: Arc::new(FhirSchemaValidator::new().expect("validator should load")),
    };

    build_router(state)
}

/// Build a test app with authentication enforced (unauthenticated requests are rejected).
pub fn build_test_app_auth_required(pool: PgPool) -> Router {
    use fhir_server::{AppState, build_router};
    use fhir_server::auth::AuthConfig;
    use fhir_server::store::PgStore;
    use fhir_server::validation::FhirSchemaValidator;

    let state = AppState {
        store: PgStore::new(pool),
        auth: AuthConfig {
            jwt_secret: "test-secret".to_owned(),
            allow_unauthenticated: false,
        },
        fhir_base_url: "http://localhost:8080/fhir".to_owned(),
        validator: Arc::new(FhirSchemaValidator::new().expect("validator should load")),
    };

    build_router(state)
}

/// Create a JWT token for testing with given claims.
pub fn create_test_token(claims: &TestClaims) -> String {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret("test-secret".as_bytes()),
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
/// The table is cleaned once initially if the `CLEAN_TEST_DB` flag is set,
/// but individual tests should not rely on an empty database — use unique
/// resource IDs or specific tenant tokens to avoid collisions.
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
    sqlx::query("DELETE FROM fhir_resources WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(pool)
        .await
        .expect("failed to clean tenant data");
}

/// Send a request and return (status, body_value).
pub async fn send_request(app: Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = app
        .oneshot(req)
        .await
        .expect("request should complete");

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
    Request::builder()
        .method("PUT")
        .uri(format!("/fhir/{resource_type}/{id}"))
        .header("content-type", "application/json")
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
    assert!(!issues.is_empty(), "OperationOutcome must have at least one issue");
    assert_eq!(
        issues[0]["code"], expected_code,
        "expected issue code '{expected_code}', got: {}",
        issues[0]["code"]
    );
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
