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
    http::{Method, header},
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
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use utoipa::openapi::{
    Components,
    security::{Http, HttpAuthScheme, SecurityScheme},
};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::{Config as SwaggerUiConfig, SwaggerUi};
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
    let trace = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

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
