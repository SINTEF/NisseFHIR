use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use jsonwebtoken::{Header, encode};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::{AuthConfig, Claims};

#[derive(Deserialize)]
pub struct MintRequest {
    /// Tenant identifier (defaults to "dev-tenant").
    pub tenant: Option<String>,
    /// Space-separated scopes (defaults to "read write").
    pub scope: Option<String>,
    /// Allowed resource types (defaults to all).
    pub resource_types: Option<Vec<String>>,
    /// Token lifetime in seconds (defaults to 3600, max 86400).
    pub expires_in: Option<u64>,
}

#[derive(Serialize)]
pub struct MintResponse {
    pub token: String,
    pub expires_in: u64,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/dev/token", post(mint_token))
}

async fn mint_token(
    State(state): State<AppState>,
    Json(req): Json<MintRequest>,
) -> Result<Json<MintResponse>, StatusCode> {
    let dev_cfg = match &state.auth {
        AuthConfig::Dev(cfg) => cfg,
        _ => return Err(StatusCode::NOT_FOUND),
    };

    let expires_in = req.expires_in.unwrap_or(3600).min(86400);
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs()
        + expires_in;

    let claims = Claims {
        sub: None,
        tenant: Some(req.tenant.unwrap_or_else(|| "dev-tenant".to_owned())),
        scope: Some(req.scope.unwrap_or_else(|| "read write".to_owned())),
        resource_types: req.resource_types,
        exp: Some(exp),
    };

    let token = encode(&Header::default(), &claims, &dev_cfg.encoding_key)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(MintResponse { token, expires_in }))
}
