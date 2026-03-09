pub mod auth;
pub mod capability;
pub mod config;
pub mod dev;
pub mod error;
pub mod fhir;
pub mod jwks;
pub mod store;
pub mod validation;

use auth::AuthConfig;
use axum::{
    Router,
    http::{Method, header},
};
use std::sync::Arc;
use store::PgStore;
use tower_helmet::HelmetLayer;
use tower_http::{
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable as ScalarServable};
use validation::FhirSchemaValidator;

/// Maximum request body size: 10 MB.
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub store: PgStore,
    pub auth: AuthConfig,
    pub fhir_base_url: String,
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
    paths(
        fhir::healthz,
        fhir::get_metadata,
        fhir::search_resources,
        fhir::create_resource,
        fhir::read_resource,
        fhir::update_resource,
        fhir::patch_resource,
        fhir::delete_resource,
    )
)]
struct ApiDoc;

pub fn build_router(state: AppState) -> Router {
    let cors = build_cors_layer(&state.cors_allowed_origins);
    let trace = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    let mut router = fhir::routes();
    if state.serve_docs {
        router = router.merge(Scalar::with_url("/docs", ApiDoc::openapi()));
    }
    if matches!(&state.auth, AuthConfig::Dev(_)) {
        router = router.merge(dev::routes());
    }

    router
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
