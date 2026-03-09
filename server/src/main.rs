use anyhow::Context;
use fhir_server::{
    AppState, auth::AuthConfig, build_router, config::AppConfig, store::PgStore,
    validation::FhirSchemaValidator,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = AppConfig::from_env()?;
    let mut db_url = url::Url::parse(&config.database_url)
        .with_context(|| "failed to parse DATABASE_URL")?;
    db_url
        .query_pairs_mut()
        .append_pair("connect_timeout", &config.db_connect_timeout_secs.to_string());
    let statement_timeout_ms = config.db_statement_timeout_ms;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(config.db_acquire_timeout_secs))
        .after_connect(move |conn, _meta| {
            Box::pin(async move {
                sqlx::query("SELECT set_config('statement_timeout', $1, false)")
                    .bind(statement_timeout_ms.to_string())
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(db_url.as_str())
        .await
        .with_context(|| "failed to connect to PostgreSQL")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .with_context(|| "failed to run migrations")?;

    // JWKS: fetch keys before accepting any requests, then start background refresh.
    if let AuthConfig::Jwks(ref jwks_cfg) = config.auth {
        fhir_server::jwks::initial_fetch(jwks_cfg).await?;
        fhir_server::jwks::spawn_refresh(jwks_cfg.clone());
    }

    let state = AppState {
        store: PgStore::new(pool),
        auth: config.auth.clone(),
        fhir_base_url: config.fhir_base_url,
        search: config.search,
        validator: Arc::new(FhirSchemaValidator::new()?),
        cors_allowed_origins: config.cors_allowed_origins.clone(),
        serve_docs: config.serve_docs,
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
