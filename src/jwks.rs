use std::time::Duration;

use anyhow::{Context, Result};
use jsonwebtoken::jwk::JwkSet;
use tracing::{info, warn};

use crate::auth::JwksConfig;

/// Fetch the JWKS from the configured URI and populate the key store.
///
/// Called once at startup before the server begins accepting requests.
/// Returns an error if keys cannot be fetched, preventing the server from
/// starting without valid verification keys.
pub async fn initial_fetch(cfg: &JwksConfig) -> Result<()> {
    let client = build_client()?;
    let jwk_set = fetch_jwks(&client, &cfg.jwks_uri).await?;
    let count = jwk_set.keys.len();

    let mut store = cfg
        .key_store
        .write()
        .map_err(|_| anyhow::anyhow!("JWKS key store lock poisoned"))?;
    *store = jwk_set;

    info!(keys = count, uri = %cfg.jwks_uri, "JWKS loaded at startup");
    Ok(())
}

/// Spawn a background task that periodically refreshes the JWKS key store.
///
/// On failure the existing keys are retained and a warning is logged.
pub fn spawn_refresh(cfg: JwksConfig) {
    tokio::spawn(async move {
        let client = match build_client() {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "failed to build JWKS refresh HTTP client; background refresh disabled");
                return;
            }
        };

        let interval = Duration::from_secs(cfg.refresh_secs);

        loop {
            tokio::time::sleep(interval).await;

            match fetch_jwks(&client, &cfg.jwks_uri).await {
                Ok(jwk_set) => {
                    let count = jwk_set.keys.len();
                    match cfg.key_store.write() {
                        Ok(mut store) => {
                            *store = jwk_set;
                            info!(keys = count, "JWKS refreshed");
                        }
                        Err(_) => {
                            warn!("JWKS key store lock poisoned; cannot refresh");
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "JWKS refresh failed; keeping existing keys");
                }
            }
        }
    });
}

fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("failed to build HTTP client for JWKS")
}

async fn fetch_jwks(client: &reqwest::Client, uri: &str) -> Result<JwkSet> {
    let response = client
        .get(uri)
        .send()
        .await
        .with_context(|| format!("failed to fetch JWKS from {uri}"))?;

    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("JWKS endpoint {uri} returned HTTP {status}");
    }

    response
        .json::<JwkSet>()
        .await
        .with_context(|| format!("failed to parse JWKS response from {uri}"))
}
