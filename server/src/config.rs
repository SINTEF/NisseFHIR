use std::{env, fs, sync::{Arc, RwLock}};

use anyhow::{Context, Result, anyhow, bail};
use http::HeaderValue;
use jsonwebtoken::Algorithm;
use jsonwebtoken::jwk::JwkSet;
use tracing::warn;

use crate::auth::{AuthConfig, DevKeyConfig, JwksConfig, StaticKeyConfig, build_validation};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub auth: AuthConfig,
    pub fhir_base_url: String,
    pub cors_allowed_origins: Vec<HeaderValue>,
    pub serve_docs: bool,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned());
        let database_url = env::var("DATABASE_URL").context("missing DATABASE_URL")?;
        let fhir_base_url =
            env::var("FHIR_BASE_URL").unwrap_or_else(|_| "http://localhost:8080/fhir".to_owned());
        let auth = load_auth_config()?;
        let cors_allowed_origins = parse_cors_allowed_origins()?;
        let serve_docs = env_flag("SERVE_DOCS");

        Ok(Self {
            bind_addr,
            database_url,
            auth,
            fhir_base_url,
            cors_allowed_origins,
            serve_docs,
        })
    }
}

// ---------------------------------------------------------------------------
// JWT mode dispatch
// ---------------------------------------------------------------------------

fn load_auth_config() -> Result<AuthConfig> {
    let mode = env::var("JWT_MODE")
        .unwrap_or_else(|_| "static".to_owned())
        .to_ascii_lowercase();
    let issuer = env::var("JWT_ISSUER").ok().filter(|s| !s.is_empty());
    let audience = env::var("JWT_AUDIENCE").ok().filter(|s| !s.is_empty());

    match mode.as_str() {
        "static" => load_static_auth(issuer.as_deref(), audience.as_deref()),
        "jwks" => load_jwks_auth(issuer, audience),
        "dev" => Ok(load_dev_auth()),
        other => bail!("unsupported JWT_MODE '{other}'; expected 'static', 'jwks', or 'dev'"),
    }
}

// --- static mode ---

fn load_static_auth(issuer: Option<&str>, audience: Option<&str>) -> Result<AuthConfig> {
    let algorithm = parse_algorithm(
        &env::var("JWT_ALGORITHM").unwrap_or_else(|_| "HS256".to_owned()),
    )?;
    let validation = build_validation(algorithm, issuer, audience);

    match algorithm {
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
            let secret = env::var("JWT_SECRET")
                .with_context(|| format!("missing JWT_SECRET for {algorithm:?} verification"))?;
            validate_hmac_secret(&secret)?;
            maybe_warn_on_development_secret(&secret);
            Ok(AuthConfig::Static(StaticKeyConfig {
                decoding_key: Arc::new(jsonwebtoken::DecodingKey::from_secret(secret.as_bytes())),
                validation: Arc::new(validation),
            }))
        }
        Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => {
            let pem = load_public_key_pem()?;
            Ok(AuthConfig::Static(StaticKeyConfig::from_rsa_pem(&pem, validation)?))
        }
        Algorithm::ES256 | Algorithm::ES384 => {
            let pem = load_public_key_pem()?;
            Ok(AuthConfig::Static(StaticKeyConfig::from_ec_pem(&pem, validation)?))
        }
        unsupported => bail!("unsupported JWT_ALGORITHM '{unsupported:?}'"),
    }
}

// --- jwks mode ---

fn load_jwks_auth(issuer: Option<String>, audience: Option<String>) -> Result<AuthConfig> {
    let jwks_uri = env::var("JWT_JWKS_URI")
        .context("JWT_JWKS_URI is required when JWT_MODE=jwks")?;

    url::Url::parse(&jwks_uri)
        .with_context(|| format!("JWT_JWKS_URI '{jwks_uri}' is not a valid URL"))?;

    let refresh_secs: u64 = env::var("JWT_JWKS_REFRESH_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);

    // Start with an empty key store; main.rs performs the initial fetch before
    // serving requests.
    let key_store = Arc::new(RwLock::new(JwkSet { keys: vec![] }));

    Ok(AuthConfig::Jwks(JwksConfig {
        key_store,
        jwks_uri,
        refresh_secs,
        issuer,
        audience,
    }))
}

// --- dev mode ---

fn load_dev_auth() -> AuthConfig {
    let secret = format!(
        "{}{}",
        uuid::Uuid::new_v4().as_simple(),
        uuid::Uuid::new_v4().as_simple(),
    );

    warn!("──────────────────────────────────────────────────────────────");
    warn!("JWT_MODE=dev: using a randomly generated secret per startup");
    warn!("A token-minting endpoint is available at POST /dev/token");
    warn!("DO NOT use dev mode in production!");
    warn!("──────────────────────────────────────────────────────────────");

    AuthConfig::Dev(DevKeyConfig::new(&secret))
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn validate_hmac_secret(secret: &str) -> Result<()> {
    if secret.len() < 32 {
        bail!("JWT_SECRET must be at least 32 characters for HMAC JWT verification");
    }
    Ok(())
}

fn maybe_warn_on_development_secret(secret: &str) {
    let normalized = secret.to_ascii_lowercase();
    if normalized.contains("change-me")
        || normalized.contains("dev-secret")
        || normalized.contains("example")
        || normalized.contains("test-secret")
    {
        warn!("JWT_SECRET appears to be a development value; do not reuse it outside local testing");
    }
}

fn load_public_key_pem() -> Result<String> {
    match (env::var("JWT_PUBLIC_KEY_PEM"), env::var("JWT_PUBLIC_KEY_PATH")) {
        (Ok(pem), _) if !pem.trim().is_empty() => Ok(pem),
        (_, Ok(path)) if !path.trim().is_empty() => fs::read_to_string(&path)
            .with_context(|| format!("failed to read JWT public key from {path}")),
        _ => Err(anyhow!(
            "missing JWT public key; set JWT_PUBLIC_KEY_PEM or JWT_PUBLIC_KEY_PATH"
        )),
    }
}

fn parse_algorithm(value: &str) -> Result<Algorithm> {
    match value.trim().to_ascii_uppercase().as_str() {
        "HS256" => Ok(Algorithm::HS256),
        "HS384" => Ok(Algorithm::HS384),
        "HS512" => Ok(Algorithm::HS512),
        "RS256" => Ok(Algorithm::RS256),
        "RS384" => Ok(Algorithm::RS384),
        "RS512" => Ok(Algorithm::RS512),
        "ES256" => Ok(Algorithm::ES256),
        "ES384" => Ok(Algorithm::ES384),
        other => bail!("unsupported JWT_ALGORITHM '{other}'"),
    }
}

fn parse_cors_allowed_origins() -> Result<Vec<HeaderValue>> {
    env::var("CORS_ALLOWED_ORIGINS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(|origin| {
                    HeaderValue::from_str(origin)
                        .with_context(|| format!("invalid CORS origin header value '{origin}'"))
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()
        .map(|origins| origins.unwrap_or_default())
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}
