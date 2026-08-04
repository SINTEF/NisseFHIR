pub mod auth;
pub mod bundle;
pub mod capability;
pub mod config;
pub mod error;
pub mod fhir;
pub mod jwks;
pub mod media_type;
pub mod search;
pub mod search_params;
pub mod store;
pub mod validation;

pub const DEFAULT_SEARCH_PAGE_COUNT: u32 = 128;
pub const DEFAULT_MAX_SEARCH_PAGE_COUNT: u32 = 2048;
pub const MAX_SEARCH_PARAMETER_OCCURRENCES: usize = 128;
pub const MAX_SEARCH_OR_VALUES_PER_OCCURRENCE: usize = 128;
pub const MAX_SEARCH_TOTAL_VALUES: usize = 512;
pub const MAX_SEARCH_QUERY_BYTES: usize = 64 * 1024;

use auth::AuthConfig;
use axum::{
    Router,
    extract::MatchedPath,
    http::{Method, Request, Response, header},
    middleware::{self, Next},
    response::IntoResponse,
};
use std::sync::Arc;
use store::PgStore;
use tower_helmet::HelmetLayer;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    trace::{MakeSpan, OnResponse, TraceLayer},
};
use tracing::Span;
use utoipa::openapi::{
    Components,
    security::{Http, HttpAuthScheme, SecurityScheme},
};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::{Config as SwaggerUiConfig, SwaggerUi};
use uuid::Uuid;
use validation::FhirSchemaValidator;

/// Maximum request body size: 10 MB.
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct SearchConfig {
    pub default_count: u32,
    pub max_count: u32,
}

#[derive(Clone)]
pub struct AppState {
    pub store: PgStore,
    pub auth: AuthConfig,
    pub fhir_base_url: String,
    pub search: SearchConfig,
    pub validator: Arc<FhirSchemaValidator>,
    pub cors_allowed_origins: Vec<header::HeaderValue>,
    pub serve_docs: bool,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "FHIR R6 Server",
        description = "Lightweight Rust FHIR 6.0 server",
        version = "0.1.0",
    ),
    modifiers(&SecurityAddon),
    paths(
        fhir::healthz,
        fhir::readyz,
        fhir::get_metadata,
        fhir::process_bundle,
        fhir::search_resources,
        fhir::create_resource,
        fhir::read_resource,
        fhir::read_resource_history,
        fhir::update_resource,
        fhir::patch_resource,
        fhir::delete_resource,
    )
)]
struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        openapi
            .components
            .get_or_insert_with(Components::new)
            .add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
    }
}

pub fn build_router(state: AppState) -> Router {
    let cors = build_cors_layer(&state.cors_allowed_origins);
    // Privacy-safe trace schema: never record request URIs (which may carry
    // PHI in the query string), headers, or bodies. Only method, matched route
    // template, correlation ID, status, and latency are logged.
    let trace = TraceLayer::new_for_http()
        .make_span_with(PrivacyMakeSpan)
        .on_request(())
        .on_body_chunk(())
        .on_eos(())
        .on_response(PrivacyOnResponse);

    let mut router = fhir::routes();
    if state.serve_docs {
        router = router.merge(
            SwaggerUi::new("/docs")
                .url("/docs/openapi.json", ApiDoc::openapi())
                .config(
                    SwaggerUiConfig::new(["/docs/openapi.json"])
                        .try_it_out_enabled(true)
                        .persist_authorization(true),
                ),
        );
    }

    router
        .layer(middleware::from_fn(fhir_content_type))
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(MAX_BODY_SIZE))
        .layer(HelmetLayer::with_defaults())
        .layer(trace)
        .layer(cors)
        .with_state(state)
}

/// Privacy-safe request span.
///
/// Records only the HTTP method, the matched route template, and a fresh
/// correlation ID. The raw request URI is deliberately *never* recorded,
/// because FHIR search query strings can contain names, identifiers, dates,
/// and other protected health information.
///
/// Redaction policy for resource IDs and tenant identifiers: both can appear
/// in request paths and query strings, so the policy is to never log the raw
/// URI. The matched route template uses `{id}`-style placeholders, so concrete
/// resource IDs and tenant IDs never reach the logs. If tenant-level
/// correlation is ever needed, add an explicitly *hashed* tenant field here.
#[derive(Clone, Debug)]
struct PrivacyMakeSpan;

impl<B> MakeSpan<B> for PrivacyMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        let method = request.method().to_string();
        let route = request
            .extensions()
            .get::<MatchedPath>()
            .map(|p| p.as_str().to_owned())
            .unwrap_or_else(|| "<unmatched>".to_owned());
        let correlation_id = Uuid::new_v4().to_string();
        tracing::info_span!(
            "http_request",
            method = %method,
            route = %route,
            correlation_id = %correlation_id,
            status = tracing::field::Empty,
            latency_ms = tracing::field::Empty,
        )
    }
}

/// Privacy-safe completion event.
///
/// Records status and latency into the request span and emits an INFO event.
#[derive(Clone, Debug)]
struct PrivacyOnResponse;

impl<B> OnResponse<B> for PrivacyOnResponse {
    fn on_response(self, response: &Response<B>, latency: std::time::Duration, span: &Span) {
        let status = response.status().as_u16();
        let latency_ms = latency.as_millis() as u64;
        span.record("status", status);
        span.record("latency_ms", latency_ms);
        tracing::info!(
            parent: span,
            status,
            latency_ms,
            "request completed"
        );
    }
}

fn build_cors_layer(allowed_origins: &[header::HeaderValue]) -> CorsLayer {
    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::ACCEPT,
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::IF_MATCH,
            header::HeaderName::from_static("if-none-exist"),
        ])
        .expose_headers([
            header::ETAG,
            header::LOCATION,
            header::HeaderName::from_static("last-modified"),
        ]);

    if allowed_origins.is_empty() {
        cors
    } else {
        cors.allow_origin(allowed_origins.to_vec())
    }
}

/// Middleware that negotiates the `Accept` header and sets
/// `Content-Type: application/fhir+json` on JSON responses from FHIR
/// endpoints.
async fn fhir_content_type(
    request: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let is_fhir_path =
        request.uri().path().starts_with("/fhir") || request.uri().path() == "/metadata";
    if is_fhir_path && let Err(err) = crate::media_type::validate_accept(request.headers()) {
        let mut response = err.into_response();
        set_fhir_content_type(&mut response);
        return response;
    }
    let mut response = next.run(request).await;
    if is_fhir_path {
        set_fhir_content_type(&mut response);
    }
    response
}

/// Set `Content-Type: application/fhir+json` on JSON responses containing a
/// FHIR resource.
fn set_fhir_content_type(response: &mut axum::response::Response) {
    if let Some(ct) = response.headers().get(header::CONTENT_TYPE)
        && ct.as_bytes().starts_with(b"application/json")
    {
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/fhir+json; charset=utf-8"),
        );
    }
}
