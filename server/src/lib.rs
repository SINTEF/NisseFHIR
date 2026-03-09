pub mod auth;
pub mod capability;
pub mod config;
pub mod error;
pub mod fhir;
pub mod store;
pub mod validation;

use axum::Router;
use std::sync::Arc;
use store::PgStore;
use auth::AuthConfig;
use tower_helmet::HelmetLayer;
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable as ScalarServable};
use validation::FhirSchemaValidator;

/// Maximum request body size: 50 MB.
const MAX_BODY_SIZE: usize = 50 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub store: PgStore,
    pub auth: AuthConfig,
    pub fhir_base_url: String,
    pub validator: Arc<FhirSchemaValidator>,
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
    ),
)]
struct ApiDoc;

pub fn build_router(state: AppState) -> Router {
    fhir::routes()
        .merge(Scalar::with_url("/docs", ApiDoc::openapi()))
        .layer(RequestBodyLimitLayer::new(MAX_BODY_SIZE))
        .layer(HelmetLayer::with_defaults())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
