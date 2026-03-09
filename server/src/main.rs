use anyhow::Context;
use fhir_server::{AppState, auth::AuthConfig, build_router, config::AppConfig, store::PgStore, validation::FhirSchemaValidator};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::info;

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

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fhir_server=info,tower_http=info".into()),
        )
        .init();
}
