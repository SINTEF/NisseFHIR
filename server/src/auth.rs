use std::sync::{Arc, RwLock};

use anyhow::{Context, Result};
use axum::http::{HeaderMap, header};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Validation, decode, decode_header,
    jwk::{AlgorithmParameters, EllipticCurve, Jwk, JwkSet},
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

// ---------------------------------------------------------------------------
// Auth configuration variants
// ---------------------------------------------------------------------------

/// JWT verification configuration.
#[derive(Clone)]
pub enum AuthConfig {
    /// Single static key (HMAC secret or asymmetric PEM).
    Static(StaticKeyConfig),
    /// Keys fetched from a JWKS endpoint with background refresh.
    Jwks(JwksConfig),
    /// Development-only mode: random key with token-minting endpoint.
    Dev(DevKeyConfig),
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static(_) => f.write_str("AuthConfig::Static(…)"),
            Self::Jwks(_) => f.write_str("AuthConfig::Jwks(…)"),
            Self::Dev(_) => f.write_str("AuthConfig::Dev(…)"),
        }
    }
}

/// Convenience constructors that produce a `Static` variant (backward-compat).
impl AuthConfig {
    pub fn from_hmac_secret(algorithm: Algorithm, secret: &str) -> Self {
        let validation = build_validation(algorithm, None, None);
        Self::Static(StaticKeyConfig {
            decoding_key: Arc::new(DecodingKey::from_secret(secret.as_bytes())),
            validation: Arc::new(validation),
        })
    }

    pub fn from_rsa_pem(algorithm: Algorithm, pem: &str) -> Result<Self> {
        let validation = build_validation(algorithm, None, None);
        let key = DecodingKey::from_rsa_pem(pem.as_bytes())
            .context("failed to parse RSA public key PEM")?;
        Ok(Self::Static(StaticKeyConfig {
            decoding_key: Arc::new(key),
            validation: Arc::new(validation),
        }))
    }

    pub fn from_ec_pem(algorithm: Algorithm, pem: &str) -> Result<Self> {
        let validation = build_validation(algorithm, None, None);
        let key = DecodingKey::from_ec_pem(pem.as_bytes())
            .context("failed to parse EC public key PEM")?;
        Ok(Self::Static(StaticKeyConfig {
            decoding_key: Arc::new(key),
            validation: Arc::new(validation),
        }))
    }
}

// --- Static key config ---

#[derive(Clone)]
pub struct StaticKeyConfig {
    pub decoding_key: Arc<DecodingKey>,
    pub validation: Arc<Validation>,
}

impl StaticKeyConfig {
    pub fn new(algorithm: Algorithm, secret: &str, issuer: Option<&str>, audience: Option<&str>) -> Self {
        let validation = build_validation(algorithm, issuer, audience);
        Self {
            decoding_key: Arc::new(DecodingKey::from_secret(secret.as_bytes())),
            validation: Arc::new(validation),
        }
    }

    pub fn from_rsa_pem(pem: &str, validation: Validation) -> Result<Self> {
        let key = DecodingKey::from_rsa_pem(pem.as_bytes())
            .context("failed to parse RSA public key PEM")?;
        Ok(Self { decoding_key: Arc::new(key), validation: Arc::new(validation) })
    }

    pub fn from_ec_pem(pem: &str, validation: Validation) -> Result<Self> {
        let key = DecodingKey::from_ec_pem(pem.as_bytes())
            .context("failed to parse EC public key PEM")?;
        Ok(Self { decoding_key: Arc::new(key), validation: Arc::new(validation) })
    }
}

// --- JWKS config ---

#[derive(Clone)]
pub struct JwksConfig {
    pub key_store: Arc<RwLock<JwkSet>>,
    pub jwks_uri: String,
    pub refresh_secs: u64,
    pub issuer: Option<String>,
    pub audience: Option<String>,
}

// --- Dev config ---

#[derive(Clone)]
pub struct DevKeyConfig {
    pub encoding_key: Arc<EncodingKey>,
    pub decoding_key: Arc<DecodingKey>,
    pub validation: Arc<Validation>,
}

impl DevKeyConfig {
    pub fn new(secret: &str) -> Self {
        let validation = build_validation(Algorithm::HS256, None, None);
        Self {
            encoding_key: Arc::new(EncodingKey::from_secret(secret.as_bytes())),
            decoding_key: Arc::new(DecodingKey::from_secret(secret.as_bytes())),
            validation: Arc::new(validation),
        }
    }
}

// ---------------------------------------------------------------------------
// Claims & access context
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Claims {
    pub sub: Option<String>,
    pub tenant: Option<String>,
    pub scope: Option<String>,
    pub resource_types: Option<Vec<String>>,
    pub exp: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct AccessContext {
    pub tenant_id: String,
    pub can_read: bool,
    pub can_write: bool,
    pub resource_allow_list: Option<Vec<String>>,
}

impl AccessContext {
    pub fn can_access_resource_type(&self, resource_type: &str) -> bool {
        match &self.resource_allow_list {
            Some(allow) => allow.iter().any(|x| x.eq_ignore_ascii_case(resource_type)),
            None => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Token verification
// ---------------------------------------------------------------------------

pub fn extract_access_context(
    headers: &HeaderMap,
    cfg: &AuthConfig,
) -> Result<AccessContext, AppError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let claims = match cfg {
        AuthConfig::Static(sc) => decode::<Claims>(token, &sc.decoding_key, &sc.validation)
            .map_err(|_| AppError::Unauthorized)?
            .claims,
        AuthConfig::Jwks(jc) => verify_with_jwks(token, jc)?,
        AuthConfig::Dev(dc) => decode::<Claims>(token, &dc.decoding_key, &dc.validation)
            .map_err(|_| AppError::Unauthorized)?
            .claims,
    };

    let tenant_id = claims.tenant.or(claims.sub).ok_or(AppError::Unauthorized)?;
    let scope = claims.scope.unwrap_or_else(|| "read write".to_owned());
    let can_read = scope.split_whitespace().any(|s| s.eq_ignore_ascii_case("read"));
    let can_write = scope.split_whitespace().any(|s| s.eq_ignore_ascii_case("write"));

    Ok(AccessContext {
        tenant_id,
        can_read,
        can_write,
        resource_allow_list: claims.resource_types,
    })
}

// ---------------------------------------------------------------------------
// JWKS-specific verification
// ---------------------------------------------------------------------------

fn verify_with_jwks(token: &str, cfg: &JwksConfig) -> Result<Claims, AppError> {
    let header = decode_header(token).map_err(|_| AppError::Unauthorized)?;
    let kid = header.kid.as_deref().ok_or(AppError::Unauthorized)?;

    let key_store = cfg
        .key_store
        .read()
        .map_err(|_| AppError::Internal("JWKS key store lock poisoned".into()))?;
    let jwk = key_store.find(kid).ok_or(AppError::Unauthorized)?;

    let algorithm = algorithm_for_jwk(jwk).ok_or(AppError::Unauthorized)?;
    let decoding_key = DecodingKey::from_jwk(jwk).map_err(|_| AppError::Unauthorized)?;

    let mut validation = Validation::new(algorithm);
    validation.validate_exp = true;
    if let Some(ref iss) = cfg.issuer {
        validation.set_issuer(&[iss]);
    }
    if let Some(ref aud) = cfg.audience {
        validation.set_audience(&[aud]);
    }

    decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|_| AppError::Unauthorized)
        .map(|td| td.claims)
}

/// Derive the JWT algorithm from a JWK's metadata or key type.
fn algorithm_for_jwk(jwk: &Jwk) -> Option<Algorithm> {
    if let Some(ref key_alg) = jwk.common.key_algorithm {
        return key_algorithm_to_algorithm(key_alg);
    }
    match &jwk.algorithm {
        AlgorithmParameters::RSA(_) => Some(Algorithm::RS256),
        AlgorithmParameters::EllipticCurve(ec) => match ec.curve {
            EllipticCurve::P256 => Some(Algorithm::ES256),
            EllipticCurve::P384 => Some(Algorithm::ES384),
            _ => None,
        },
        AlgorithmParameters::OctetKey(_) => Some(Algorithm::HS256),
        AlgorithmParameters::OctetKeyPair(_) => Some(Algorithm::EdDSA),
    }
}

fn key_algorithm_to_algorithm(ka: &jsonwebtoken::jwk::KeyAlgorithm) -> Option<Algorithm> {
    use jsonwebtoken::jwk::KeyAlgorithm;
    match ka {
        KeyAlgorithm::HS256 => Some(Algorithm::HS256),
        KeyAlgorithm::HS384 => Some(Algorithm::HS384),
        KeyAlgorithm::HS512 => Some(Algorithm::HS512),
        KeyAlgorithm::RS256 => Some(Algorithm::RS256),
        KeyAlgorithm::RS384 => Some(Algorithm::RS384),
        KeyAlgorithm::RS512 => Some(Algorithm::RS512),
        KeyAlgorithm::ES256 => Some(Algorithm::ES256),
        KeyAlgorithm::ES384 => Some(Algorithm::ES384),
        KeyAlgorithm::PS256 => Some(Algorithm::PS256),
        KeyAlgorithm::PS384 => Some(Algorithm::PS384),
        KeyAlgorithm::PS512 => Some(Algorithm::PS512),
        KeyAlgorithm::EdDSA => Some(Algorithm::EdDSA),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn build_validation(algorithm: Algorithm, issuer: Option<&str>, audience: Option<&str>) -> Validation {
    let mut v = Validation::new(algorithm);
    v.validate_exp = true;
    if let Some(iss) = issuer {
        v.set_issuer(&[iss]);
    }
    if let Some(aud) = audience {
        v.set_audience(&[aud]);
    }
    v
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};
    use jsonwebtoken::{Algorithm, EncodingKey, Header, decode, encode};

    use super::{AuthConfig, Claims, extract_access_context};

    const TEST_SECRET: &str = "0123456789abcdef0123456789abcdef";

    fn make_config() -> AuthConfig {
        AuthConfig::from_hmac_secret(Algorithm::HS256, TEST_SECRET)
    }

    fn encode_claims(claims: &Claims) -> String {
        encode(
            &Header::default(),
            claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .expect("token should encode")
    }

    fn bearer_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).expect("header should build"),
        );
        headers
    }

    #[test]
    fn accepts_valid_token() {
        let token = encode_claims(&Claims {
            sub: Some("tenant-a".to_owned()),
            tenant: None,
            scope: Some("read write".to_owned()),
            resource_types: Some(vec!["Patient".to_owned()]),
            exp: Some(4_102_444_800),
        });

        let access = extract_access_context(&bearer_headers(&token), &make_config())
            .expect("token should decode");

        assert_eq!(access.tenant_id, "tenant-a");
        assert!(access.can_read);
        assert!(access.can_write);
        assert!(access.can_access_resource_type("Patient"));
        assert!(!access.can_access_resource_type("Observation"));
    }

    #[test]
    fn rejects_expired_token() {
        let token = encode_claims(&Claims {
            sub: Some("t".to_owned()),
            tenant: None,
            scope: Some("read".to_owned()),
            resource_types: None,
            exp: Some(0),
        });

        let result = extract_access_context(&bearer_headers(&token), &make_config());
        assert!(result.is_err());
    }

    #[test]
    fn rejects_wrong_secret() {
        let token = encode(
            &Header::default(),
            &Claims {
                sub: Some("t".to_owned()),
                tenant: None,
                scope: None,
                resource_types: None,
                exp: Some(4_102_444_800),
            },
            &EncodingKey::from_secret("wrong-secret-wrong-secret-wrong!!".as_bytes()),
        )
        .unwrap();

        let result = extract_access_context(&bearer_headers(&token), &make_config());
        assert!(result.is_err());
    }

    #[test]
    fn unauthenticated_returns_error() {
        let headers = HeaderMap::new();
        let result = extract_access_context(&headers, &make_config());
        assert!(result.is_err());
    }

    #[test]
    fn tenant_claim_overrides_sub() {
        let token = encode_claims(&Claims {
            sub: Some("sub-val".to_owned()),
            tenant: Some("tenant-val".to_owned()),
            scope: None,
            resource_types: None,
            exp: Some(4_102_444_800),
        });

        let access = extract_access_context(&bearer_headers(&token), &make_config())
            .expect("should decode");

        assert_eq!(access.tenant_id, "tenant-val");
    }

    #[test]
    fn no_tenant_and_no_sub_is_rejected() {
        let token = encode_claims(&Claims {
            sub: None,
            tenant: None,
            scope: Some("read".to_owned()),
            resource_types: None,
            exp: Some(4_102_444_800),
        });

        let result = extract_access_context(&bearer_headers(&token), &make_config());
        assert!(result.is_err());
    }

    #[test]
    fn scope_read_only() {
        let token = encode_claims(&Claims {
            sub: Some("t".to_owned()),
            tenant: None,
            scope: Some("read".to_owned()),
            resource_types: None,
            exp: Some(4_102_444_800),
        });

        let access = extract_access_context(&bearer_headers(&token), &make_config()).unwrap();
        assert!(access.can_read);
        assert!(!access.can_write);
    }

    #[test]
    fn scope_write_only() {
        let token = encode_claims(&Claims {
            sub: Some("t".to_owned()),
            tenant: None,
            scope: Some("write".to_owned()),
            resource_types: None,
            exp: Some(4_102_444_800),
        });

        let access = extract_access_context(&bearer_headers(&token), &make_config()).unwrap();
        assert!(!access.can_read);
        assert!(access.can_write);
    }

    #[test]
    fn missing_scope_defaults_to_read_write() {
        let token = encode_claims(&Claims {
            sub: Some("t".to_owned()),
            tenant: None,
            scope: None,
            resource_types: None,
            exp: Some(4_102_444_800),
        });

        let access = extract_access_context(&bearer_headers(&token), &make_config()).unwrap();
        assert!(access.can_read);
        assert!(access.can_write);
    }

    #[test]
    fn rejects_missing_exp_claim() {
        let token = encode_claims(&Claims {
            sub: Some("tenant-a".to_owned()),
            tenant: None,
            scope: Some("read write".to_owned()),
            resource_types: None,
            exp: None,
        });

        let result = extract_access_context(&bearer_headers(&token), &make_config());
        assert!(result.is_err());
    }

    #[test]
    fn resource_allow_list_case_insensitive() {
        let token = encode_claims(&Claims {
            sub: Some("t".to_owned()),
            tenant: None,
            scope: None,
            resource_types: Some(vec!["Patient".to_owned()]),
            exp: Some(4_102_444_800),
        });

        let access = extract_access_context(&bearer_headers(&token), &make_config()).unwrap();
        assert!(access.can_access_resource_type("patient"));
        assert!(access.can_access_resource_type("PATIENT"));
        assert!(access.can_access_resource_type("Patient"));
        assert!(!access.can_access_resource_type("Observation"));
    }

    #[test]
    fn no_resource_allow_list_allows_all() {
        let token = encode_claims(&Claims {
            sub: Some("t".to_owned()),
            tenant: None,
            scope: None,
            resource_types: None,
            exp: Some(4_102_444_800),
        });

        let access = extract_access_context(&bearer_headers(&token), &make_config()).unwrap();
        assert!(access.can_access_resource_type("Patient"));
        assert!(access.can_access_resource_type("Observation"));
        assert!(access.can_access_resource_type("AnythingElse"));
    }

    #[test]
    fn malformed_bearer_prefix_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Token some-value"),
        );

        let result = extract_access_context(&headers, &make_config());
        assert!(result.is_err());
    }

    #[test]
    fn jwks_rejects_token_without_kid() {
        use std::sync::{Arc, RwLock};
        use super::JwksConfig;
        use jsonwebtoken::jwk::JwkSet;

        let cfg = AuthConfig::Jwks(JwksConfig {
            key_store: Arc::new(RwLock::new(JwkSet { keys: vec![] })),
            jwks_uri: "https://example.com/.well-known/jwks.json".to_owned(),
            refresh_secs: 300,
            issuer: None,
            audience: None,
        });

        // Default Header has no `kid`, so JWKS lookup should fail.
        let token = encode_claims(&Claims {
            sub: Some("t".to_owned()),
            tenant: None,
            scope: None,
            resource_types: None,
            exp: Some(4_102_444_800),
        });

        let result = extract_access_context(&bearer_headers(&token), &cfg);
        assert!(result.is_err());
    }

    #[test]
    fn jwks_rejects_unknown_kid() {
        use std::sync::{Arc, RwLock};
        use super::JwksConfig;
        use jsonwebtoken::jwk::JwkSet;

        let cfg = AuthConfig::Jwks(JwksConfig {
            key_store: Arc::new(RwLock::new(JwkSet { keys: vec![] })),
            jwks_uri: "https://example.com/.well-known/jwks.json".to_owned(),
            refresh_secs: 300,
            issuer: None,
            audience: None,
        });

        // Use HS256 so we can sign with HMAC but include a kid in the header.
        let mut hdr = Header::new(Algorithm::HS256);
        hdr.kid = Some("nonexistent-kid".to_owned());

        let token = encode(
            &hdr,
            &Claims {
                sub: Some("t".to_owned()),
                tenant: None,
                scope: None,
                resource_types: None,
                exp: Some(4_102_444_800),
            },
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let result = extract_access_context(&bearer_headers(&token), &cfg);
        assert!(result.is_err());
    }

    #[test]
    fn dev_mode_accepts_own_tokens() {
        use super::DevKeyConfig;

        let dev = DevKeyConfig::new(TEST_SECRET);
        let cfg = AuthConfig::Dev(dev.clone());

        let token = encode(
            &Header::default(),
            &Claims {
                sub: Some("dev-tenant".to_owned()),
                tenant: None,
                scope: Some("read write".to_owned()),
                resource_types: None,
                exp: Some(4_102_444_800),
            },
            &dev.encoding_key,
        )
        .unwrap();

        let access = extract_access_context(&bearer_headers(&token), &cfg)
            .expect("dev token should verify");
        assert_eq!(access.tenant_id, "dev-tenant");
    }

    // -----------------------------------------------------------------------
    // Debug implementation
    // -----------------------------------------------------------------------

    #[test]
    fn debug_static_config() {
        let cfg = make_config();
        let debug = format!("{cfg:?}");
        assert!(debug.contains("Static"));
    }

    #[test]
    fn debug_jwks_config() {
        use super::JwksConfig;
        use jsonwebtoken::jwk::JwkSet;
        use std::sync::{Arc, RwLock};

        let cfg = AuthConfig::Jwks(JwksConfig {
            key_store: Arc::new(RwLock::new(JwkSet { keys: vec![] })),
            jwks_uri: "https://example.com/jwks".to_owned(),
            refresh_secs: 300,
            issuer: None,
            audience: None,
        });
        let debug = format!("{cfg:?}");
        assert!(debug.contains("Jwks"));
    }

    #[test]
    fn debug_dev_config() {
        use super::DevKeyConfig;
        let cfg = AuthConfig::Dev(DevKeyConfig::new(TEST_SECRET));
        let debug = format!("{cfg:?}");
        assert!(debug.contains("Dev"));
    }

    // -----------------------------------------------------------------------
    // RSA / EC PEM constructors
    // -----------------------------------------------------------------------

    #[test]
    fn from_rsa_pem_invalid_key_returns_error() {
        let result = AuthConfig::from_rsa_pem(Algorithm::RS256, "not-a-valid-pem");
        assert!(result.is_err());
    }

    #[test]
    fn from_ec_pem_invalid_key_returns_error() {
        let result = AuthConfig::from_ec_pem(Algorithm::ES256, "not-a-valid-pem");
        assert!(result.is_err());
    }

    const TEST_RSA_PUB_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAu1SU1LfVLPHCozMxH2Mo\n4lgOEePzNm0tRgeLezV6ffAt0gunVTLw7onLRnrq0/IzW7yWR7QkrmBL7jTKEn5u\n+qKhbwKfBstIs+bMY2Zkp18gnTxklLgs0gG+o0561MRpmHAXE5gTa5RiP3InC8gJ\ncLJPQGmNR/Vu36MThEoSjks2Lg2aaGr/sMbmGrUK5cYXWIFiAPxsWfRMUSN7YZFJ\nkdqPsm9Lo1E9jqnukEif+F61VBJ6mIUfpE6YiOvJim2rk+0M20IHXBQ/8v7U5I7r\nXIq6NKPHKxR3n9+niPKtkHQCb3TGpN/M5W7g7XyxAZ3RYMg+hWqcIGXflNiLt5Ei\nQwIDAQAB\n-----END PUBLIC KEY-----";

    const TEST_EC_PUB_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEoBGIlAqHWzG7aIgjDzY9a/dcCurC\np0+MjUBT8JKfj9aTLoYMYYS4YDYhzEGf4WC0w3xJP/M888igBJaOGtsk1g==\n-----END PUBLIC KEY-----";

    #[test]
    fn from_rsa_pem_valid_key_succeeds() {
        let result = AuthConfig::from_rsa_pem(Algorithm::RS256, TEST_RSA_PUB_PEM);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), AuthConfig::Static(_)));
    }

    #[test]
    fn from_ec_pem_valid_key_succeeds() {
        let result = AuthConfig::from_ec_pem(Algorithm::ES256, TEST_EC_PUB_PEM);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), AuthConfig::Static(_)));
    }

    #[test]
    fn static_key_config_from_rsa_pem_valid() {
        let result = super::StaticKeyConfig::from_rsa_pem(
            TEST_RSA_PUB_PEM,
            super::build_validation(Algorithm::RS256, None, None),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn static_key_config_from_ec_pem_valid() {
        let result = super::StaticKeyConfig::from_ec_pem(
            TEST_EC_PUB_PEM,
            super::build_validation(Algorithm::ES256, None, None),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn static_key_config_new() {
        let cfg = super::StaticKeyConfig::new(
            Algorithm::HS256,
            TEST_SECRET,
            Some("iss"),
            Some("aud"),
        );
        // Verify the config was constructed with issuer/audience validation.
        // A token with the matching iss and aud should succeed.
        let token = encode(
            &Header::default(),
            &serde_json::json!({
                "sub": "t",
                "exp": 4_102_444_800u64,
                "iss": "iss",
                "aud": "aud",
            }),
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let result = decode::<Claims>(&token, &cfg.decoding_key, &cfg.validation);
        assert!(result.is_ok());
    }

    #[test]
    fn static_key_config_from_rsa_pem_invalid() {
        let result = super::StaticKeyConfig::from_rsa_pem(
            "not-a-pem",
            super::build_validation(Algorithm::RS256, None, None),
        );
        assert!(result.is_err());
    }

    #[test]
    fn static_key_config_from_ec_pem_invalid() {
        let result = super::StaticKeyConfig::from_ec_pem(
            "not-a-pem",
            super::build_validation(Algorithm::ES256, None, None),
        );
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // algorithm_for_jwk
    // -----------------------------------------------------------------------

    #[test]
    fn algorithm_for_jwk_rsa_default() {
        use jsonwebtoken::jwk::{
            AlgorithmParameters, CommonParameters, Jwk, RSAKeyParameters,
        };

        let jwk = Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: None,
                key_id: None,
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::RSA(RSAKeyParameters {
                key_type: Default::default(),
                n: "fake_n".to_owned(),
                e: "fake_e".to_owned(),
            }),
        };

        assert_eq!(super::algorithm_for_jwk(&jwk), Some(Algorithm::RS256));
    }

    #[test]
    fn algorithm_for_jwk_ec_p256() {
        use jsonwebtoken::jwk::{
            AlgorithmParameters, CommonParameters, EllipticCurve,
            EllipticCurveKeyParameters, Jwk,
        };

        let jwk = Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: None,
                key_id: None,
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::EllipticCurve(EllipticCurveKeyParameters {
                key_type: Default::default(),
                curve: EllipticCurve::P256,
                x: "x".to_owned(),
                y: "y".to_owned(),
            }),
        };

        assert_eq!(super::algorithm_for_jwk(&jwk), Some(Algorithm::ES256));
    }

    #[test]
    fn algorithm_for_jwk_ec_p384() {
        use jsonwebtoken::jwk::{
            AlgorithmParameters, CommonParameters, EllipticCurve,
            EllipticCurveKeyParameters, Jwk,
        };

        let jwk = Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: None,
                key_id: None,
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::EllipticCurve(EllipticCurveKeyParameters {
                key_type: Default::default(),
                curve: EllipticCurve::P384,
                x: "x".to_owned(),
                y: "y".to_owned(),
            }),
        };

        assert_eq!(super::algorithm_for_jwk(&jwk), Some(Algorithm::ES384));
    }

    #[test]
    fn algorithm_for_jwk_octet_key() {
        use jsonwebtoken::jwk::{
            AlgorithmParameters, CommonParameters, Jwk, OctetKeyParameters,
        };

        let jwk = Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: None,
                key_id: None,
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::OctetKey(OctetKeyParameters {
                key_type: Default::default(),
                value: "secret".to_owned(),
            }),
        };

        assert_eq!(super::algorithm_for_jwk(&jwk), Some(Algorithm::HS256));
    }

    #[test]
    fn algorithm_for_jwk_uses_key_algorithm_when_present() {
        use jsonwebtoken::jwk::{
            AlgorithmParameters, CommonParameters, Jwk, KeyAlgorithm,
            RSAKeyParameters,
        };

        let jwk = Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: Some(KeyAlgorithm::RS384),
                key_id: None,
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::RSA(RSAKeyParameters {
                key_type: Default::default(),
                n: "n".to_owned(),
                e: "e".to_owned(),
            }),
        };

        // key_algorithm takes precedence
        assert_eq!(super::algorithm_for_jwk(&jwk), Some(Algorithm::RS384));
    }

    // -----------------------------------------------------------------------
    // key_algorithm_to_algorithm
    // -----------------------------------------------------------------------

    #[test]
    fn key_algorithm_to_algorithm_all_known() {
        use jsonwebtoken::jwk::KeyAlgorithm;

        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::HS256), Some(Algorithm::HS256));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::HS384), Some(Algorithm::HS384));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::HS512), Some(Algorithm::HS512));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::RS256), Some(Algorithm::RS256));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::RS384), Some(Algorithm::RS384));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::RS512), Some(Algorithm::RS512));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::ES256), Some(Algorithm::ES256));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::ES384), Some(Algorithm::ES384));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::PS256), Some(Algorithm::PS256));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::PS384), Some(Algorithm::PS384));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::PS512), Some(Algorithm::PS512));
        assert_eq!(super::key_algorithm_to_algorithm(&KeyAlgorithm::EdDSA), Some(Algorithm::EdDSA));
    }

    // -----------------------------------------------------------------------
    // build_validation helper
    // -----------------------------------------------------------------------

    #[test]
    fn build_validation_with_issuer_and_audience() {
        let v = super::build_validation(Algorithm::RS256, Some("iss"), Some("aud"));
        assert!(v.validate_exp);
        // The fact that it doesn't panic is the key check here.
    }

    #[test]
    fn build_validation_without_issuer_and_audience() {
        let v = super::build_validation(Algorithm::HS256, None, None);
        assert!(v.validate_exp);
    }

    // -----------------------------------------------------------------------
    // JWKS verification success path
    // -----------------------------------------------------------------------

    #[test]
    fn jwks_verifies_token_with_matching_kid() {
        use super::JwksConfig;
        use jsonwebtoken::jwk::{
            AlgorithmParameters, CommonParameters, Jwk, JwkSet, OctetKeyParameters,
        };
        use std::sync::{Arc, RwLock};

        // Use base64url-encoded secret for the OctetKey JWK.
        // "0123456789abcdef0123456789abcdef" base64url => MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY
        let secret_bytes = TEST_SECRET.as_bytes();
        let b64_secret = base64url_encode(secret_bytes);

        let jwk = Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: None,
                key_id: Some("test-kid-1".to_owned()),
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::OctetKey(OctetKeyParameters {
                key_type: Default::default(),
                value: b64_secret,
            }),
        };

        let jwk_set = JwkSet { keys: vec![jwk] };
        let cfg = AuthConfig::Jwks(JwksConfig {
            key_store: Arc::new(RwLock::new(jwk_set)),
            jwks_uri: "https://example.com/jwks".to_owned(),
            refresh_secs: 300,
            issuer: None,
            audience: None,
        });

        let mut hdr = Header::new(Algorithm::HS256);
        hdr.kid = Some("test-kid-1".to_owned());

        let token = encode(
            &hdr,
            &Claims {
                sub: Some("jwks-tenant".to_owned()),
                tenant: None,
                scope: Some("read write".to_owned()),
                resource_types: None,
                exp: Some(4_102_444_800),
            },
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let access = extract_access_context(&bearer_headers(&token), &cfg)
            .expect("JWKS verification should succeed");
        assert_eq!(access.tenant_id, "jwks-tenant");
        assert!(access.can_read);
        assert!(access.can_write);
    }

    #[test]
    fn jwks_verifies_with_issuer_and_audience() {
        use super::JwksConfig;
        use jsonwebtoken::jwk::{
            AlgorithmParameters, CommonParameters, Jwk, JwkSet, OctetKeyParameters,
        };
        use std::sync::{Arc, RwLock};

        let b64_secret = base64url_encode(TEST_SECRET.as_bytes());

        let jwk = Jwk {
            common: CommonParameters {
                public_key_use: None,
                key_operations: None,
                key_algorithm: None,
                key_id: Some("kid-iss-aud".to_owned()),
                x509_url: None,
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
            },
            algorithm: AlgorithmParameters::OctetKey(OctetKeyParameters {
                key_type: Default::default(),
                value: b64_secret,
            }),
        };

        let cfg = AuthConfig::Jwks(JwksConfig {
            key_store: Arc::new(RwLock::new(JwkSet { keys: vec![jwk] })),
            jwks_uri: "https://example.com/jwks".to_owned(),
            refresh_secs: 300,
            issuer: Some("https://issuer.example.com".to_owned()),
            audience: Some("my-audience".to_owned()),
        });

        let mut hdr = Header::new(Algorithm::HS256);
        hdr.kid = Some("kid-iss-aud".to_owned());

        let token = encode(
            &hdr,
            &serde_json::json!({
                "sub": "jwks-iss-aud-tenant",
                "scope": "read",
                "exp": 4_102_444_800u64,
                "iss": "https://issuer.example.com",
                "aud": "my-audience",
            }),
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let access = extract_access_context(&bearer_headers(&token), &cfg)
            .expect("JWKS with iss/aud should succeed");
        assert_eq!(access.tenant_id, "jwks-iss-aud-tenant");
        assert!(access.can_read);
        assert!(!access.can_write);
    }

    /// Simple base64url encoding without padding (for constructing JWK OctetKey values).
    fn base64url_encode(input: &[u8]) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        let url_safe = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

        // Use standard base64, then translate
        let chunks = input.chunks(3);
        for chunk in chunks {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let triple = (b0 << 16) | (b1 << 8) | b2;

            let _ = write!(out, "{}", url_safe.as_bytes()[((triple >> 18) & 0x3F) as usize] as char);
            let _ = write!(out, "{}", url_safe.as_bytes()[((triple >> 12) & 0x3F) as usize] as char);
            if chunk.len() > 1 {
                let _ = write!(out, "{}", url_safe.as_bytes()[((triple >> 6) & 0x3F) as usize] as char);
            }
            if chunk.len() > 2 {
                let _ = write!(out, "{}", url_safe.as_bytes()[(triple & 0x3F) as usize] as char);
            }
        }
        out
    }
}
