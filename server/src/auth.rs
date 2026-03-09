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

    #[test]
    fn accepts_valid_token() {
        let token = encode(
            &Header::default(),
            &Claims {
                sub: Some("tenant-a".to_owned()),
                tenant: None,
                scope: Some("read write".to_owned()),
                resource_types: Some(vec!["Patient".to_owned()]),
                exp: Some(4_102_444_800),
            },
            &EncodingKey::from_secret("secret".as_bytes()),
        )
        .expect("token should encode");

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).expect("header should build"),
        );

        let access = extract_access_context(
            &headers,
            &AuthConfig {
                jwt_secret: "secret".to_owned(),
                allow_unauthenticated: false,
            },
        )
        .expect("token should decode");

        assert_eq!(access.tenant_id, "tenant-a");
        assert!(access.can_read);
        assert!(access.can_write);
        assert!(access.can_access_resource_type("Patient"));
        assert!(!access.can_access_resource_type("Observation"));
    }
}
