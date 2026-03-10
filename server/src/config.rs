use std::{
    env, fs,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result, anyhow, bail};
use http::HeaderValue;
use jsonwebtoken::Algorithm;
use jsonwebtoken::jwk::JwkSet;
use tracing::warn;

use crate::auth::{AuthConfig, JwksConfig, StaticKeyConfig, build_validation};
use crate::{DEFAULT_MAX_SEARCH_PAGE_COUNT, DEFAULT_SEARCH_PAGE_COUNT, SearchConfig};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub db_connect_timeout_secs: u64,
    pub db_acquire_timeout_secs: u64,
    pub db_statement_timeout_ms: u64,
    pub auth: AuthConfig,
    pub fhir_base_url: String,
    pub search: SearchConfig,
    pub cors_allowed_origins: Vec<HeaderValue>,
    pub serve_docs: bool,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned());
        let database_url = load_env_or_file("DATABASE_URL")?
            .context("missing DATABASE_URL or DATABASE_URL_FILE")?;
        let db_connect_timeout_secs = parse_u64_env_var("DB_CONNECT_TIMEOUT_SECS")?.unwrap_or(5);
        let db_acquire_timeout_secs = parse_u64_env_var("DB_ACQUIRE_TIMEOUT_SECS")?.unwrap_or(5);
        let db_statement_timeout_ms =
            parse_u64_env_var("DB_STATEMENT_TIMEOUT_MS")?.unwrap_or(10_000);

        if db_connect_timeout_secs == 0 {
            bail!("DB_CONNECT_TIMEOUT_SECS must be greater than 0");
        }
        if db_acquire_timeout_secs == 0 {
            bail!("DB_ACQUIRE_TIMEOUT_SECS must be greater than 0");
        }
        if db_statement_timeout_ms == 0 {
            bail!("DB_STATEMENT_TIMEOUT_MS must be greater than 0");
        }

        let fhir_base_url =
            env::var("FHIR_BASE_URL").unwrap_or_else(|_| "http://localhost:8080/fhir".to_owned());
        let search = load_search_config()?;
        let auth = load_auth_config()?;
        let cors_allowed_origins = parse_cors_allowed_origins()?;
        let serve_docs = env_flag("SERVE_DOCS");

        Ok(Self {
            bind_addr,
            database_url,
            db_connect_timeout_secs,
            db_acquire_timeout_secs,
            db_statement_timeout_ms,
            auth,
            fhir_base_url,
            search,
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
        other => bail!("unsupported JWT_MODE '{other}'; expected 'static' or 'jwks'"),
    }
}

fn load_search_config() -> Result<SearchConfig> {
    let default_count =
        parse_u32_env_var("SEARCH_DEFAULT_COUNT")?.unwrap_or(DEFAULT_SEARCH_PAGE_COUNT);
    let max_count = parse_u32_env_var("SEARCH_MAX_COUNT")?.unwrap_or(DEFAULT_MAX_SEARCH_PAGE_COUNT);

    if default_count == 0 {
        bail!("SEARCH_DEFAULT_COUNT must be greater than 0");
    }

    if max_count == 0 {
        bail!("SEARCH_MAX_COUNT must be greater than 0");
    }

    if default_count > max_count {
        bail!("SEARCH_DEFAULT_COUNT must be less than or equal to SEARCH_MAX_COUNT");
    }

    Ok(SearchConfig {
        default_count,
        max_count,
    })
}

// --- static mode ---

fn load_static_auth(issuer: Option<&str>, audience: Option<&str>) -> Result<AuthConfig> {
    let algorithm =
        parse_algorithm(&env::var("JWT_ALGORITHM").unwrap_or_else(|_| "HS256".to_owned()))?;
    let validation = build_validation(algorithm, issuer, audience);

    match algorithm {
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
            let secret = load_env_or_file("JWT_SECRET")?.with_context(|| {
                format!("missing JWT_SECRET or JWT_SECRET_FILE for {algorithm:?} verification")
            })?;
            validate_hmac_secret(&secret)?;
            maybe_warn_on_development_secret(&secret);
            Ok(AuthConfig::Static(StaticKeyConfig {
                decoding_key: Arc::new(jsonwebtoken::DecodingKey::from_secret(secret.as_bytes())),
                validation: Arc::new(validation),
            }))
        }
        Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => {
            let pem = load_public_key_pem()?;
            Ok(AuthConfig::Static(StaticKeyConfig::from_rsa_pem(
                &pem, validation,
            )?))
        }
        Algorithm::ES256 | Algorithm::ES384 => {
            let pem = load_public_key_pem()?;
            Ok(AuthConfig::Static(StaticKeyConfig::from_ec_pem(
                &pem, validation,
            )?))
        }
        unsupported => bail!("unsupported JWT_ALGORITHM '{unsupported:?}'"),
    }
}

// --- jwks mode ---

fn load_jwks_auth(issuer: Option<String>, audience: Option<String>) -> Result<AuthConfig> {
    let jwks_uri =
        env::var("JWT_JWKS_URI").context("JWT_JWKS_URI is required when JWT_MODE=jwks")?;

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
        warn!(
            "JWT_SECRET appears to be a development value; do not reuse it outside local testing"
        );
    }
}

fn load_env_or_file(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => return Ok(Some(value)),
        Ok(_) | Err(env::VarError::NotPresent) => {}
        Err(env::VarError::NotUnicode(_)) => {
            return Err(anyhow!("{name} contains invalid unicode"));
        }
    }

    let file_var = format!("{name}_FILE");
    match env::var(&file_var) {
        Ok(path) if !path.trim().is_empty() => {
            let value = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {name} from {path}"))?;
            let value = value.trim_end_matches(['\r', '\n']).to_owned();
            if value.is_empty() {
                bail!("{file_var} points to an empty file");
            }
            Ok(Some(value))
        }
        Ok(_) | Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(anyhow!("{file_var} contains invalid unicode")),
    }
}

fn load_public_key_pem() -> Result<String> {
    match (
        env::var("JWT_PUBLIC_KEY_PEM"),
        env::var("JWT_PUBLIC_KEY_PATH"),
    ) {
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

fn parse_u32_env_var(name: &str) -> Result<Option<u32>> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .parse::<u32>()
                .with_context(|| format!("{name} must be an unsigned integer"))
        })
        .transpose()
}

fn parse_u64_env_var(name: &str) -> Result<Option<u64>> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .with_context(|| format!("{name} must be an unsigned integer"))
        })
        .transpose()
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Guard to serialize env-mutating tests (env vars are process-global).
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// Helper: run a closure with specific env vars set, then restore them.
    fn with_env_vars<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        let _lock = ENV_MUTEX.lock().unwrap();
        let mut saved: Vec<(&str, Option<String>)> = Vec::new();
        for &(key, val) in vars {
            saved.push((key, env::var(key).ok()));
            // SAFETY: tests are serialized via ENV_MUTEX so no concurrent access.
            unsafe {
                match val {
                    Some(v) => env::set_var(key, v),
                    None => env::remove_var(key),
                }
            }
        }
        f();
        for (key, old_val) in saved {
            // SAFETY: tests are serialized via ENV_MUTEX so no concurrent access.
            unsafe {
                match old_val {
                    Some(v) => env::set_var(key, v),
                    None => env::remove_var(key),
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // parse_algorithm
    // -----------------------------------------------------------------------

    #[test]
    fn parse_algorithm_all_supported() {
        assert_eq!(parse_algorithm("HS256").unwrap(), Algorithm::HS256);
        assert_eq!(parse_algorithm("HS384").unwrap(), Algorithm::HS384);
        assert_eq!(parse_algorithm("HS512").unwrap(), Algorithm::HS512);
        assert_eq!(parse_algorithm("RS256").unwrap(), Algorithm::RS256);
        assert_eq!(parse_algorithm("RS384").unwrap(), Algorithm::RS384);
        assert_eq!(parse_algorithm("RS512").unwrap(), Algorithm::RS512);
        assert_eq!(parse_algorithm("ES256").unwrap(), Algorithm::ES256);
        assert_eq!(parse_algorithm("ES384").unwrap(), Algorithm::ES384);
    }

    #[test]
    fn parse_algorithm_case_insensitive() {
        assert_eq!(parse_algorithm("hs256").unwrap(), Algorithm::HS256);
        assert_eq!(parse_algorithm("  Rs384  ").unwrap(), Algorithm::RS384);
    }

    #[test]
    fn parse_algorithm_unsupported() {
        let err = parse_algorithm("NONE").unwrap_err();
        assert!(err.to_string().contains("unsupported JWT_ALGORITHM"));
    }

    // -----------------------------------------------------------------------
    // validate_hmac_secret
    // -----------------------------------------------------------------------

    #[test]
    fn validate_hmac_secret_accepts_long() {
        assert!(validate_hmac_secret("01234567890123456789012345678901").is_ok());
    }

    #[test]
    fn validate_hmac_secret_rejects_short() {
        let err = validate_hmac_secret("too-short").unwrap_err();
        assert!(err.to_string().contains("at least 32 characters"));
    }

    // -----------------------------------------------------------------------
    // maybe_warn_on_development_secret (smoke test — just ensure no panic)
    // -----------------------------------------------------------------------

    #[test]
    fn warn_on_development_secret_does_not_panic() {
        maybe_warn_on_development_secret("change-me-secret-0123456789abcd");
        maybe_warn_on_development_secret("dev-secret-0123456789abcdefghij");
        maybe_warn_on_development_secret("example-0123456789abcdefghijklm");
        maybe_warn_on_development_secret("test-secret-0123456789abcdefghi");
        maybe_warn_on_development_secret("perfect-production-secret-000000");
    }

    #[test]
    fn load_env_or_file_prefers_env_var() {
        let tmp = "/tmp/__test_preferred_env_secret.txt";
        std::fs::write(tmp, "from-file").expect("write tmp file");
        with_env_vars(
            &[
                ("__TEST_SECRET", Some("from-env")),
                ("__TEST_SECRET_FILE", Some(tmp)),
            ],
            || {
                let value = load_env_or_file("__TEST_SECRET").unwrap();
                assert_eq!(value.as_deref(), Some("from-env"));
            },
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn load_env_or_file_reads_file() {
        let tmp = "/tmp/__test_secret_from_file.txt";
        std::fs::write(tmp, "from-file\n").expect("write tmp file");
        with_env_vars(
            &[("__TEST_SECRET", None), ("__TEST_SECRET_FILE", Some(tmp))],
            || {
                let value = load_env_or_file("__TEST_SECRET").unwrap();
                assert_eq!(value.as_deref(), Some("from-file"));
            },
        );
        let _ = std::fs::remove_file(tmp);
    }

    // -----------------------------------------------------------------------
    // env_flag
    // -----------------------------------------------------------------------

    #[test]
    fn env_flag_truthy_values() {
        with_env_vars(&[("__TEST_FLAG", Some("1"))], || {
            assert!(env_flag("__TEST_FLAG"));
        });
        with_env_vars(&[("__TEST_FLAG", Some("true"))], || {
            assert!(env_flag("__TEST_FLAG"));
        });
        with_env_vars(&[("__TEST_FLAG", Some("TRUE"))], || {
            assert!(env_flag("__TEST_FLAG"));
        });
        with_env_vars(&[("__TEST_FLAG", Some("yes"))], || {
            assert!(env_flag("__TEST_FLAG"));
        });
        with_env_vars(&[("__TEST_FLAG", Some("YES"))], || {
            assert!(env_flag("__TEST_FLAG"));
        });
    }

    #[test]
    fn env_flag_falsy_values() {
        with_env_vars(&[("__TEST_FLAG", Some("0"))], || {
            assert!(!env_flag("__TEST_FLAG"));
        });
        with_env_vars(&[("__TEST_FLAG", Some("false"))], || {
            assert!(!env_flag("__TEST_FLAG"));
        });
        with_env_vars(&[("__TEST_FLAG", None)], || {
            assert!(!env_flag("__TEST_FLAG"));
        });
    }

    // -----------------------------------------------------------------------
    // parse_cors_allowed_origins
    // -----------------------------------------------------------------------

    #[test]
    fn parse_cors_empty_when_unset() {
        with_env_vars(&[("CORS_ALLOWED_ORIGINS", None)], || {
            let origins = parse_cors_allowed_origins().unwrap();
            assert!(origins.is_empty());
        });
    }

    #[test]
    fn parse_cors_single_origin() {
        with_env_vars(
            &[("CORS_ALLOWED_ORIGINS", Some("https://example.com"))],
            || {
                let origins = parse_cors_allowed_origins().unwrap();
                assert_eq!(origins.len(), 1);
                assert_eq!(origins[0].to_str().unwrap(), "https://example.com");
            },
        );
    }

    #[test]
    fn parse_cors_multiple_origins() {
        with_env_vars(
            &[(
                "CORS_ALLOWED_ORIGINS",
                Some("https://a.com, https://b.com , https://c.com"),
            )],
            || {
                let origins = parse_cors_allowed_origins().unwrap();
                assert_eq!(origins.len(), 3);
            },
        );
    }

    // -----------------------------------------------------------------------
    // load_auth_config (integration-level, env-driven)
    // -----------------------------------------------------------------------

    #[test]
    fn load_static_hmac_from_env() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", Some("a]very-long-secret-at-least-32-chars!")),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let auth = load_auth_config().unwrap();
                assert!(matches!(auth, AuthConfig::Static(_)));
            },
        );
    }

    #[test]
    fn load_static_hmac_missing_secret() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", None),
                ("JWT_SECRET_FILE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let result = load_auth_config();
                assert!(result.is_err());
                assert!(result.unwrap_err().to_string().contains("JWT_SECRET"));
            },
        );
    }

    #[test]
    fn load_static_hmac_secret_too_short() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", Some("short")),
                ("JWT_SECRET_FILE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let result = load_auth_config();
                assert!(result.is_err());
                assert!(result.unwrap_err().to_string().contains("at least 32"));
            },
        );
    }

    #[test]
    fn load_unsupported_mode() {
        with_env_vars(&[("JWT_MODE", Some("bogus"))], || {
            let result = load_auth_config();
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("unsupported JWT_MODE")
            );
        });
    }

    #[test]
    fn load_jwks_mode_from_env() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("jwks")),
                (
                    "JWT_JWKS_URI",
                    Some("https://example.com/.well-known/jwks.json"),
                ),
                ("JWT_JWKS_REFRESH_SECS", None),
                ("JWT_ISSUER", Some("https://issuer.example.com")),
                ("JWT_AUDIENCE", Some("my-audience")),
            ],
            || {
                let auth = load_auth_config().unwrap();
                assert!(matches!(auth, AuthConfig::Jwks(_)));
            },
        );
    }

    #[test]
    fn load_jwks_mode_missing_uri() {
        with_env_vars(
            &[("JWT_MODE", Some("jwks")), ("JWT_JWKS_URI", None)],
            || {
                let result = load_auth_config();
                assert!(result.is_err());
                assert!(result.unwrap_err().to_string().contains("JWT_JWKS_URI"));
            },
        );
    }

    #[test]
    fn load_jwks_mode_invalid_uri() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("jwks")),
                ("JWT_JWKS_URI", Some("not a url")),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
            ],
            || {
                let result = load_auth_config();
                assert!(result.is_err());
                assert!(result.unwrap_err().to_string().contains("not a valid URL"));
            },
        );
    }

    #[test]
    fn load_static_rsa_missing_key() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("RS256")),
                ("JWT_SECRET", None),
                ("JWT_SECRET_FILE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let result = load_auth_config();
                assert!(result.is_err());
                assert!(result.unwrap_err().to_string().contains("JWT public key"));
            },
        );
    }

    #[test]
    fn load_static_ec_missing_key() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("ES256")),
                ("JWT_SECRET", None),
                ("JWT_SECRET_FILE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let result = load_auth_config();
                assert!(result.is_err());
                assert!(result.unwrap_err().to_string().contains("JWT public key"));
            },
        );
    }

    // The RSA public key PEM used for testing (2048-bit).
    const TEST_RSA_PUB_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAu1SU1LfVLPHCozMxH2Mo\n4lgOEePzNm0tRgeLezV6ffAt0gunVTLw7onLRnrq0/IzW7yWR7QkrmBL7jTKEn5u\n+qKhbwKfBstIs+bMY2Zkp18gnTxklLgs0gG+o0561MRpmHAXE5gTa5RiP3InC8gJ\ncLJPQGmNR/Vu36MThEoSjks2Lg2aaGr/sMbmGrUK5cYXWIFiAPxsWfRMUSN7YZFJ\nkdqPsm9Lo1E9jqnukEif+F61VBJ6mIUfpE6YiOvJim2rk+0M20IHXBQ/8v7U5I7r\nXIq6NKPHKxR3n9+niPKtkHQCb3TGpN/M5W7g7XyxAZ3RYMg+hWqcIGXflNiLt5Ei\nQwIDAQAB\n-----END PUBLIC KEY-----";

    // The EC public key PEM used for testing (P-256).
    const TEST_EC_PUB_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEoBGIlAqHWzG7aIgjDzY9a/dcCurC\np0+MjUBT8JKfj9aTLoYMYYS4YDYhzEGf4WC0w3xJP/M888igBJaOGtsk1g==\n-----END PUBLIC KEY-----";

    #[test]
    fn load_static_rsa_with_pem() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("RS256")),
                ("JWT_SECRET", None),
                ("JWT_SECRET_FILE", None),
                ("JWT_PUBLIC_KEY_PEM", Some(TEST_RSA_PUB_PEM)),
                ("JWT_PUBLIC_KEY_PATH", None),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
            ],
            || {
                let auth = load_auth_config().unwrap();
                assert!(matches!(auth, AuthConfig::Static(_)));
            },
        );
    }

    #[test]
    fn load_static_ec_with_pem() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("ES256")),
                ("JWT_SECRET", None),
                ("JWT_SECRET_FILE", None),
                ("JWT_PUBLIC_KEY_PEM", Some(TEST_EC_PUB_PEM)),
                ("JWT_PUBLIC_KEY_PATH", None),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
            ],
            || {
                let auth = load_auth_config().unwrap();
                assert!(matches!(auth, AuthConfig::Static(_)));
            },
        );
    }

    #[test]
    fn from_env_missing_database_url() {
        with_env_vars(&[("DATABASE_URL", None), ("DATABASE_URL_FILE", None)], || {
            let result = AppConfig::from_env();
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("DATABASE_URL"));
        });
    }

    #[test]
    fn load_static_hmac_from_secret_file() {
        let tmp = "/tmp/__test_jwt_secret.txt";
        std::fs::write(tmp, "a-very-long-secret-from-file-at-least-32-chars!!\n")
            .expect("write tmp file");
        with_env_vars(
            &[
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", None),
                ("JWT_SECRET_FILE", Some(tmp)),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let auth = load_auth_config().unwrap();
                assert!(matches!(auth, AuthConfig::Static(_)));
            },
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn from_env_reads_database_url_from_file() {
        let tmp = "/tmp/__test_database_url.txt";
        std::fs::write(tmp, "postgres://localhost/test-from-file\n").expect("write tmp file");
        with_env_vars(
            &[
                ("DATABASE_URL", None),
                ("DATABASE_URL_FILE", Some(tmp)),
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", Some("a-very-long-secret-at-least-32-chars!!")),
                ("JWT_SECRET_FILE", None),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let config = AppConfig::from_env().unwrap();
                assert_eq!(config.database_url, "postgres://localhost/test-from-file");
            },
        );
        let _ = std::fs::remove_file(tmp);
    }

    // -----------------------------------------------------------------------
    // load_public_key_pem
    // -----------------------------------------------------------------------

    #[test]
    fn load_public_key_from_inline_pem() {
        with_env_vars(
            &[
                (
                    "JWT_PUBLIC_KEY_PEM",
                    Some("-----BEGIN PUBLIC KEY-----\nfake\n-----END PUBLIC KEY-----"),
                ),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let pem = load_public_key_pem().unwrap();
                assert!(pem.contains("BEGIN PUBLIC KEY"));
            },
        );
    }

    #[test]
    fn load_public_key_from_file() {
        // Write a temporary file
        let tmp = "/tmp/__test_jwt_public_key.pem";
        std::fs::write(
            tmp,
            "-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----",
        )
        .expect("write tmp file");
        with_env_vars(
            &[
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", Some(tmp)),
            ],
            || {
                let pem = load_public_key_pem().unwrap();
                assert!(pem.contains("BEGIN PUBLIC KEY"));
            },
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn load_public_key_missing_both() {
        with_env_vars(
            &[("JWT_PUBLIC_KEY_PEM", None), ("JWT_PUBLIC_KEY_PATH", None)],
            || {
                let result = load_public_key_pem();
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn load_public_key_empty_pem_falls_through() {
        with_env_vars(
            &[
                ("JWT_PUBLIC_KEY_PEM", Some("   ")),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let result = load_public_key_pem();
                assert!(result.is_err());
            },
        );
    }

    // -----------------------------------------------------------------------
    // Full AppConfig::from_env with valid HMAC config
    // -----------------------------------------------------------------------

    #[test]
    fn from_env_with_full_hmac_config() {
        with_env_vars(
            &[
                ("DATABASE_URL", Some("postgres://localhost/test")),
                ("DB_CONNECT_TIMEOUT_SECS", Some("6")),
                ("DB_ACQUIRE_TIMEOUT_SECS", Some("7")),
                ("DB_STATEMENT_TIMEOUT_MS", Some("8000")),
                ("BIND_ADDR", Some("127.0.0.1:9090")),
                ("FHIR_BASE_URL", Some("http://localhost:9090/fhir")),
                ("SEARCH_DEFAULT_COUNT", Some("15")),
                ("SEARCH_MAX_COUNT", Some("60")),
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", Some("a-very-long-secret-at-least-32-chars!!")),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
                ("CORS_ALLOWED_ORIGINS", Some("https://example.com")),
                ("SERVE_DOCS", Some("true")),
            ],
            || {
                let config = AppConfig::from_env().unwrap();
                assert_eq!(config.bind_addr, "127.0.0.1:9090");
                assert_eq!(config.database_url, "postgres://localhost/test");
                assert_eq!(config.db_connect_timeout_secs, 6);
                assert_eq!(config.db_acquire_timeout_secs, 7);
                assert_eq!(config.db_statement_timeout_ms, 8000);
                assert_eq!(config.fhir_base_url, "http://localhost:9090/fhir");
                assert_eq!(config.search.default_count, 15);
                assert_eq!(config.search.max_count, 60);
                assert!(config.serve_docs);
                assert_eq!(config.cors_allowed_origins.len(), 1);
            },
        );
    }

    #[test]
    fn from_env_rejects_invalid_search_page_config() {
        with_env_vars(
            &[
                ("DATABASE_URL", Some("postgres://localhost/test")),
                ("SEARCH_DEFAULT_COUNT", Some("70")),
                ("SEARCH_MAX_COUNT", Some("60")),
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", Some("a-very-long-secret-at-least-32-chars!!")),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let error = AppConfig::from_env().unwrap_err();
                assert!(error.to_string().contains(
                    "SEARCH_DEFAULT_COUNT must be less than or equal to SEARCH_MAX_COUNT"
                ));
            },
        );
    }

    #[test]
    fn from_env_uses_default_db_timeouts_when_unset() {
        with_env_vars(
            &[
                ("DATABASE_URL", Some("postgres://localhost/test")),
                ("DB_CONNECT_TIMEOUT_SECS", None),
                ("DB_ACQUIRE_TIMEOUT_SECS", None),
                ("DB_STATEMENT_TIMEOUT_MS", None),
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", Some("a-very-long-secret-at-least-32-chars!!")),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let config = AppConfig::from_env().unwrap();
                assert_eq!(config.db_connect_timeout_secs, 5);
                assert_eq!(config.db_acquire_timeout_secs, 5);
                assert_eq!(config.db_statement_timeout_ms, 10_000);
            },
        );
    }

    #[test]
    fn from_env_rejects_zero_db_statement_timeout() {
        with_env_vars(
            &[
                ("DATABASE_URL", Some("postgres://localhost/test")),
                ("DB_STATEMENT_TIMEOUT_MS", Some("0")),
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", Some("a-very-long-secret-at-least-32-chars!!")),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let error = AppConfig::from_env().unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains("DB_STATEMENT_TIMEOUT_MS must be greater than 0")
                );
            },
        );
    }

    // -----------------------------------------------------------------------
    // AuthConfig Debug implementation
    // -----------------------------------------------------------------------

    #[test]
    fn auth_config_debug_static() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", Some("a-very-long-secret-at-least-32-chars!!")),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
            ],
            || {
                let auth = load_auth_config().unwrap();
                let debug_str = format!("{auth:?}");
                assert!(debug_str.contains("Static"));
            },
        );
    }

    #[test]
    fn auth_config_debug_jwks() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("jwks")),
                (
                    "JWT_JWKS_URI",
                    Some("https://example.com/.well-known/jwks.json"),
                ),
                ("JWT_ISSUER", None),
                ("JWT_AUDIENCE", None),
            ],
            || {
                let auth = load_auth_config().unwrap();
                let debug_str = format!("{auth:?}");
                assert!(debug_str.contains("Jwks"));
            },
        );
    }

    #[test]
    fn load_static_with_issuer_and_audience() {
        with_env_vars(
            &[
                ("JWT_MODE", Some("static")),
                ("JWT_ALGORITHM", Some("HS256")),
                ("JWT_SECRET", Some("a-very-long-secret-at-least-32-chars!!")),
                ("JWT_ISSUER", Some("https://issuer.example.com")),
                ("JWT_AUDIENCE", Some("my-audience")),
                ("JWT_PUBLIC_KEY_PEM", None),
                ("JWT_PUBLIC_KEY_PATH", None),
            ],
            || {
                let auth = load_auth_config().unwrap();
                assert!(matches!(auth, AuthConfig::Static(_)));
            },
        );
    }
}
