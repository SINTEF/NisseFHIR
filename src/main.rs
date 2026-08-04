use anyhow::Context;
use fhir_server::{
    AppState, auth::AuthConfig, build_router, config::AppConfig, store::PgStore,
    validation::FhirSchemaValidator,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = AppConfig::from_env()?;
    let mut db_url =
        url::Url::parse(&config.database_url).with_context(|| "failed to parse DATABASE_URL")?;
    db_url.query_pairs_mut().append_pair(
        "connect_timeout",
        &config.db_connect_timeout_secs.to_string(),
    );
    let statement_timeout_ms = config.db_statement_timeout_ms;
    let pool_cfg = config.db_pool;

    let mut pool_builder = PgPoolOptions::new()
        .min_connections(pool_cfg.min_connections)
        .max_connections(pool_cfg.max_connections)
        .acquire_timeout(Duration::from_secs(config.db_acquire_timeout_secs));
    // Only set timeouts when explicitly configured; otherwise keep the
    // library defaults (passing None would *disable* the default idle timeout).
    if let Some(secs) = pool_cfg.idle_timeout_secs {
        pool_builder = pool_builder.idle_timeout(Duration::from_secs(secs));
    }
    if let Some(secs) = pool_cfg.max_lifetime_secs {
        pool_builder = pool_builder.max_lifetime(Duration::from_secs(secs));
    }
    let pool = pool_builder
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

    // A token shared between the shutdown-signal watcher, the HTTP server,
    // and the background JWKS refresher. Cancelling it stops the background
    // work cleanly once a shutdown signal arrives.
    let shutdown_token = CancellationToken::new();

    // JWKS: fetch keys before accepting any requests, then start background refresh.
    if let AuthConfig::Jwks(ref jwks_cfg) = config.auth {
        fhir_server::jwks::initial_fetch(jwks_cfg).await?;
        fhir_server::jwks::spawn_refresh(jwks_cfg.clone(), shutdown_token.clone());
    }

    let state = AppState {
        store: PgStore::new(pool.clone()),
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

    info!(
        address = %config.bind_addr,
        shutdown_timeout_secs = config.shutdown_timeout_secs,
        "FHIR server listening"
    );

    // Stop accepting new connections and drain in-flight requests once a
    // shutdown signal fires. The drain deadline is measured from the moment
    // the signal arrives (not from server startup), so a wedged request can
    // never stall termination past the configured bound.
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_token.clone(), shutdown_tx))
        .into_future();
    tokio::pin!(server);

    let (serve_result, forced) = tokio::select! {
        res = &mut server => (res, false),
        _ = shutdown_rx => {
            // A shutdown signal arrived: the listener has stopped accepting
            // and in-flight requests are draining. Bound that drain so a
            // wedged request cannot stall termination past the deadline.
            match tokio::time::timeout(
                Duration::from_secs(config.shutdown_timeout_secs),
                &mut server,
            )
            .await
            {
                Ok(res) => (res, false),
                Err(_elapsed) => (Ok(()), true),
            }
        }
    };

    serve_result?;
    if forced {
        warn!(
            timeout_secs = config.shutdown_timeout_secs,
            "graceful shutdown deadline reached; forcing shutdown"
        );
    } else {
        info!("HTTP server shut down gracefully");
    }

    // Stop the background JWKS refresh task.
    shutdown_token.cancel();

    // Gracefully close the PostgreSQL pool. Bound this too so a connection
    // that cannot be returned cannot extend shutdown indefinitely.
    let close = pool.close();
    let _ = tokio::time::timeout(Duration::from_secs(5), close).await;

    Ok(())
}

/// Resolve when the process receives a shutdown signal (Ctrl-C or SIGTERM),
/// cancel the shared shutdown token so background work stops, and notify the
/// caller so it can start the bounded drain timer.
async fn shutdown_signal(token: CancellationToken, signal_tx: tokio::sync::oneshot::Sender<()>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("received Ctrl-C; starting graceful shutdown"),
        _ = terminate => info!("received SIGTERM; starting graceful shutdown"),
    }

    token.cancel();
    let _ = signal_tx.send(());
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fhir_server=info,tower_http=info".into()),
        )
        .init();
}
