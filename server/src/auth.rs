use axum::http::{HeaderMap, header};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub allow_unauthenticated: bool,
}

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

pub fn extract_access_context(
    headers: &HeaderMap,
    cfg: &AuthConfig,
) -> Result<AccessContext, AppError> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    if let Some(token) = bearer {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        let decoded = decode::<Claims>(
            token,
            &DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|_| AppError::Unauthorized)?;

        let claims = decoded.claims;
        let tenant_id = claims.tenant.or(claims.sub).ok_or(AppError::Unauthorized)?;

        let scope = claims.scope.unwrap_or_else(|| "read write".to_owned());
        let can_read = scope
            .split_whitespace()
            .any(|s| s.eq_ignore_ascii_case("read"));
        let can_write = scope
            .split_whitespace()
            .any(|s| s.eq_ignore_ascii_case("write"));

        return Ok(AccessContext {
            tenant_id,
            can_read,
            can_write,
            resource_allow_list: claims.resource_types,
        });
    }

    if cfg.allow_unauthenticated {
        return Ok(AccessContext {
            tenant_id: "public".to_owned(),
            can_read: true,
            can_write: true,
            resource_allow_list: None,
        });
    }

    Err(AppError::Unauthorized)
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};
    use jsonwebtoken::{EncodingKey, Header, encode};

    use super::{AuthConfig, Claims, extract_access_context};

    fn make_config(allow_unauth: bool) -> AuthConfig {
        AuthConfig {
            jwt_secret: "secret".to_owned(),
            allow_unauthenticated: allow_unauth,
        }
    }

    fn encode_claims(claims: &Claims) -> String {
        encode(
            &Header::default(),
            claims,
            &EncodingKey::from_secret("secret".as_bytes()),
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

        let access = extract_access_context(&bearer_headers(&token), &make_config(false))
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
            exp: Some(0), // epoch = long expired
        });

        let result = extract_access_context(&bearer_headers(&token), &make_config(false));
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
            &EncodingKey::from_secret("wrong-secret".as_bytes()),
        )
        .unwrap();

        let result = extract_access_context(&bearer_headers(&token), &make_config(false));
        assert!(result.is_err());
    }

    #[test]
    fn unauthenticated_allowed_returns_public_tenant() {
        let headers = HeaderMap::new(); // no auth header
        let access = extract_access_context(&headers, &make_config(true))
            .expect("unauthenticated should be allowed");

        assert_eq!(access.tenant_id, "public");
        assert!(access.can_read);
        assert!(access.can_write);
        assert!(access.resource_allow_list.is_none());
    }

    #[test]
    fn unauthenticated_disallowed_returns_error() {
        let headers = HeaderMap::new();
        let result = extract_access_context(&headers, &make_config(false));
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

        let access = extract_access_context(&bearer_headers(&token), &make_config(false))
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

        let result = extract_access_context(&bearer_headers(&token), &make_config(false));
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

        let access = extract_access_context(&bearer_headers(&token), &make_config(false)).unwrap();
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

        let access = extract_access_context(&bearer_headers(&token), &make_config(false)).unwrap();
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

        let access = extract_access_context(&bearer_headers(&token), &make_config(false)).unwrap();
        assert!(access.can_read);
        assert!(access.can_write);
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

        let access = extract_access_context(&bearer_headers(&token), &make_config(false)).unwrap();
        assert!(access.can_access_resource_type("patient")); // lowercase
        assert!(access.can_access_resource_type("PATIENT")); // uppercase
        assert!(access.can_access_resource_type("Patient")); // exact
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

        let access = extract_access_context(&bearer_headers(&token), &make_config(false)).unwrap();
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

        let result = extract_access_context(&headers, &make_config(false));
        assert!(result.is_err());
    }
}
