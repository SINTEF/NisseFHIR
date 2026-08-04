use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use jsonwebtoken::jwk::JwkSet;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::auth::JwksConfig;

/// Unix timestamp (seconds) of the last successful JWKS load/refresh, or 0
/// if none has succeeded yet. Read by the metrics renderer to expose how
/// stale the key store is.
static LAST_REFRESH_UNIX_SECS: AtomicU64 = AtomicU64::new(0);

/// Fetch the JWKS from the configured URI and populate the key store.
///
/// Called once at startup before the server begins accepting requests.
/// Returns an error if keys cannot be fetched — including a set with no
/// usable verification key — preventing the server from starting without
/// valid verification keys (fails closed).
pub async fn initial_fetch(cfg: &JwksConfig) -> Result<()> {
    let client = build_client()?;
    let jwk_set = fetch_jwks(&client, &cfg.jwks_uri).await?;
    if !has_usable_key(&jwk_set) {
        anyhow::bail!(
            "JWKS endpoint {} returned no usable verification keys; refusing to start without verification keys",
            cfg.jwks_uri
        );
    }
    let count = jwk_set.keys.len();

    let mut store = cfg
        .key_store
        .write()
        .map_err(|_| anyhow::anyhow!("JWKS key store lock poisoned"))?;
    *store = jwk_set;
    note_success(count);

    info!(keys = count, uri = %cfg.jwks_uri, "JWKS loaded at startup");
    Ok(())
}

/// Spawn a background task that periodically refreshes the JWKS key store.
///
/// On failure — a network error, an empty set, or an unusable response — the
/// existing keys are retained and a warning is logged, so a transient bad
/// response can never wipe out working verification keys. The task exits
/// promptly once `shutdown` is cancelled, so the server can stop the
/// background work cleanly during graceful shutdown.
pub fn spawn_refresh(cfg: JwksConfig, shutdown: CancellationToken) {
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
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("JWKS background refresh stopped");
                    return;
                }
                _ = tokio::time::sleep(interval) => {
                    match fetch_jwks(&client, &cfg.jwks_uri).await {
                        Ok(jwk_set) => {
                            if apply_fetch(&cfg.key_store, Ok(jwk_set)) {
                                info!("JWKS refreshed");
                            } else {
                                warn!(
                                    "JWKS refresh returned an unusable key set; keeping existing keys"
                                );
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "JWKS refresh failed; keeping existing keys");
                        }
                    }
                }
            }
        }
    });
}

/// Apply the outcome of a JWKS fetch to the shared key store, retaining
/// known-good keys whenever the result is unusable.
///
/// Returns `true` if the store was updated, `false` if it was left untouched
/// (fetch failed, lock poisoned, or an empty set arrived against a non-empty
/// store). Any fetch error is treated the same way: the error payload is
/// discarded here and logged by the caller.
fn apply_fetch(store: &RwLock<JwkSet>, result: Result<JwkSet, anyhow::Error>) -> bool {
    let fetched = match result {
        Ok(set) => set,
        Err(_) => return false,
    };

    let mut guard = match store.write() {
        Ok(g) => g,
        Err(_) => return false,
    };

    match merge_fetched(&guard, fetched) {
        Some(new_set) => {
            let count = new_set.keys.len();
            *guard = new_set;
            note_success(count);
            true
        }
        None => false,
    }
}

/// Decide whether a freshly fetched set may replace the current keys,
/// returning the set to install.
///
/// A fetched set with no usable key is rejected whenever the store already
/// holds keys, so a transient empty, malformed, or unsupported response cannot
/// wipe out a working set. An unusable set is accepted only when the store is
/// empty too (which only happens at startup, where `initial_fetch` instead
/// fails closed).
fn merge_fetched(current: &JwkSet, fetched: JwkSet) -> Option<JwkSet> {
    if !has_usable_key(&fetched) && !current.keys.is_empty() {
        None
    } else {
        Some(fetched)
    }
}

/// Returns `true` if the set contains at least one JWK that can actually be
/// used for verification (a supported algorithm with decodable key material).
fn has_usable_key(set: &JwkSet) -> bool {
    set.keys.iter().any(crate::auth::jwk_is_usable)
}

/// Record a successful load/refresh: stamp the current time and publish the
/// key-count gauge. Gauge writes are no-ops when the metrics recorder is not
/// installed (metrics disabled).
fn note_success(key_count: usize) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    LAST_REFRESH_UNIX_SECS.store(now, Ordering::Relaxed);
    metrics::gauge!("nissefhir_jwks_keys").set(key_count as f64);
}

/// Age in seconds since the last successful JWKS load/refresh, or `None` if
/// no successful load has happened yet. Consumed by the metrics renderer.
pub fn last_refresh_age_secs() -> Option<f64> {
    let last = LAST_REFRESH_UNIX_SECS.load(Ordering::Relaxed);
    if last == 0 {
        return None;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(last);
    Some(now.saturating_sub(last) as f64)
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use jsonwebtoken::jwk::{AlgorithmParameters, CommonParameters, Jwk, OctetKeyParameters};

    fn store_with(keys: Vec<Jwk>) -> Arc<RwLock<JwkSet>> {
        Arc::new(RwLock::new(JwkSet { keys }))
    }

    fn dummy_jwk(kid: &str) -> Jwk {
        Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: None,
                key_id: Some(kid.to_owned()),
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::OctetKey(OctetKeyParameters {
                key_type: Default::default(),
                value: "c2VjcmV0".to_owned(),
            }),
        }
    }

    /// An EC key on an unsupported curve (P-521): no supported algorithm, so
    /// it can never be used for verification.
    fn unsupported_curve_jwk(kid: &str) -> Jwk {
        use jsonwebtoken::jwk::{AlgorithmParameters, EllipticCurve, EllipticCurveKeyParameters};
        Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: None,
                key_id: Some(kid.to_owned()),
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::EllipticCurve(EllipticCurveKeyParameters {
                key_type: Default::default(),
                curve: EllipticCurve::P521,
                x: "x".to_owned(),
                y: "y".to_owned(),
            }),
        }
    }

    /// An RSA key with malformed `n` material: it maps to RS256 but fails to
    /// decode into a `DecodingKey`.
    fn malformed_rsa_jwk(kid: &str) -> Jwk {
        use jsonwebtoken::jwk::{AlgorithmParameters, RSAKeyParameters};
        Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: None,
                key_id: Some(kid.to_owned()),
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::RSA(RSAKeyParameters {
                key_type: Default::default(),
                n: "!!!not-base64url!!!".to_owned(),
                e: "AQAB".to_owned(),
            }),
        }
    }

    /// An RSA key whose `alg` metadata claims `HS256`. Both the algorithm and
    /// the decoding key decode successfully, but their families disagree (HMAC
    /// vs RSA), so token verification would always fail with `InvalidAlgorithm`.
    fn mismatched_alg_kty_jwk(kid: &str) -> Jwk {
        use jsonwebtoken::jwk::{AlgorithmParameters, KeyAlgorithm, RSAKeyParameters};
        Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: Some(KeyAlgorithm::HS256),
                key_id: Some(kid.to_owned()),
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::RSA(RSAKeyParameters {
                key_type: Default::default(),
                n: "AQAB".to_owned(),
                e: "AQAB".to_owned(),
            }),
        }
    }

    #[test]
    fn failed_fetch_retains_existing_keys() {
        let store = store_with(vec![dummy_jwk("old")]);
        let applied = apply_fetch(&store, Err(anyhow::anyhow!("network down")));
        assert!(!applied);
        assert_eq!(store.read().unwrap().keys.len(), 1);
        assert!(store.read().unwrap().find("old").is_some());
    }

    #[test]
    fn empty_fetch_does_not_replace_working_set() {
        let store = store_with(vec![dummy_jwk("old")]);
        let applied = apply_fetch(&store, Ok(JwkSet { keys: vec![] }));
        assert!(!applied);
        assert_eq!(store.read().unwrap().keys.len(), 1);
        assert!(store.read().unwrap().find("old").is_some());
    }

    #[test]
    fn empty_fetch_applies_when_store_is_empty() {
        let store = store_with(vec![]);
        let applied = apply_fetch(&store, Ok(JwkSet { keys: vec![] }));
        assert!(applied);
        assert!(store.read().unwrap().keys.is_empty());
    }

    #[test]
    fn recovery_after_bad_responses() {
        let store = store_with(vec![dummy_jwk("old")]);
        // A failed fetch keeps the working key.
        assert!(!apply_fetch(&store, Err(anyhow::anyhow!("down"))));
        // An empty response also keeps the working key.
        assert!(!apply_fetch(&store, Ok(JwkSet { keys: vec![] })));
        assert_eq!(store.read().unwrap().keys.len(), 1);

        // Once the endpoint recovers, the new set is installed.
        let applied = apply_fetch(
            &store,
            Ok(JwkSet {
                keys: vec![dummy_jwk("recovered")],
            }),
        );
        assert!(applied);
        assert_eq!(store.read().unwrap().keys.len(), 1);
        assert!(store.read().unwrap().find("recovered").is_some());
    }

    #[test]
    fn unusable_keys_do_not_replace_working_set() {
        let store = store_with(vec![dummy_jwk("old")]);
        // Unsupported algorithm only.
        assert!(!apply_fetch(
            &store,
            Ok(JwkSet {
                keys: vec![unsupported_curve_jwk("bad-alg")],
            }),
        ));
        // Supported algorithm but malformed key material.
        assert!(!apply_fetch(
            &store,
            Ok(JwkSet {
                keys: vec![malformed_rsa_jwk("bad-mat")],
            }),
        ));
        assert_eq!(store.read().unwrap().keys.len(), 1);
        assert!(store.read().unwrap().find("old").is_some());
    }

    #[test]
    fn mismatched_alg_kty_does_not_replace_working_set() {
        // Sanity: the key decodes and the algorithm is "supported" in isolation,
        // but they disagree, so it is unusable for verification.
        let jwk = mismatched_alg_kty_jwk("bad");
        assert!(!crate::auth::jwk_is_usable(&jwk));

        let store = store_with(vec![dummy_jwk("old")]);
        assert!(!apply_fetch(&store, Ok(JwkSet { keys: vec![jwk] }),));
        assert_eq!(store.read().unwrap().keys.len(), 1);
        assert!(store.read().unwrap().find("old").is_some());
    }

    #[test]
    fn mixed_set_with_one_usable_key_replaces() {
        let store = store_with(vec![dummy_jwk("old")]);
        let applied = apply_fetch(
            &store,
            Ok(JwkSet {
                keys: vec![malformed_rsa_jwk("bad"), dummy_jwk("good")],
            }),
        );
        assert!(applied);
        assert!(store.read().unwrap().find("good").is_some());
    }

    #[test]
    fn has_usable_key_detects_unusable_sets() {
        assert!(!has_usable_key(&JwkSet { keys: vec![] }));
        assert!(!has_usable_key(&JwkSet {
            keys: vec![unsupported_curve_jwk("a"), malformed_rsa_jwk("b")],
        }));
        assert!(!has_usable_key(&JwkSet {
            keys: vec![mismatched_alg_kty_jwk("c")],
        }));
        assert!(has_usable_key(&JwkSet {
            keys: vec![dummy_jwk("a")],
        }));
    }

    #[test]
    fn rotated_keys_replace_old() {
        let store = store_with(vec![dummy_jwk("old")]);
        let applied = apply_fetch(
            &store,
            Ok(JwkSet {
                keys: vec![dummy_jwk("new"), dummy_jwk("new-2")],
            }),
        );
        assert!(applied);
        let snapshot = store.read().unwrap();
        assert_eq!(snapshot.keys.len(), 2);
        assert!(snapshot.find("new").is_some());
        assert!(snapshot.find("new-2").is_some());
        assert!(snapshot.find("old").is_none());
    }

    #[test]
    fn note_success_then_age_is_available() {
        note_success(3);
        assert!(last_refresh_age_secs().is_some());
    }
}
