mod auth;
mod capability;
mod config;
mod error;
mod fhir;
mod store;
mod validation;

use anyhow::Context;
use auth::AuthConfig;
use axum::Router;
use config::AppConfig;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use store::PgStore;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use validation::FhirSchemaValidator;

#[derive(Clone)]
pub struct AppState {
    pub store: PgStore,
    pub auth: AuthConfig,
    pub fhir_base_url: String,
    pub validator: Arc<FhirSchemaValidator>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = AppConfig::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .with_context(|| "failed to connect to PostgreSQL")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .with_context(|| "failed to run migrations")?;

    let state = AppState {
        store: PgStore::new(pool),
        auth: AuthConfig {
            jwt_secret: config.jwt_secret,
            allow_unauthenticated: config.allow_unauthenticated,
        },
        fhir_base_url: config.fhir_base_url,
        validator: Arc::new(FhirSchemaValidator::new()?),
    };

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .with_context(|| format!("failed to bind to {}", config.bind_addr))?;

    info!(address = %config.bind_addr, "FHIR server listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    fhir::routes()
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fhir_server=info,tower_http=info".into()),
        )
        .init();
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use std::sync::Arc;

    use crate::{
        AppState, auth::AuthConfig, build_router, store::PgStore, validation::FhirSchemaValidator,
    };

    #[tokio::test]
    async fn health_endpoint_is_up() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost/postgres")
            .expect("lazy pool should build");

        let app = build_router(AppState {
            store: PgStore::new(pool),
            auth: AuthConfig {
                jwt_secret: "secret".to_owned(),
                allow_unauthenticated: true,
            },
            fhir_base_url: "http://localhost:8080/fhir".to_owned(),
            validator: Arc::new(FhirSchemaValidator::new().expect("validator should load")),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
