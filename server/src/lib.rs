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
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use validation::FhirSchemaValidator;

#[derive(Clone)]
pub struct AppState {
    pub store: PgStore,
    pub auth: AuthConfig,
    pub fhir_base_url: String,
    pub validator: Arc<FhirSchemaValidator>,
}

pub fn build_router(state: AppState) -> Router {
    fhir::routes()
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
