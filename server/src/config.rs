use std::env;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub jwt_secret: String,
    pub allow_unauthenticated: bool,
    pub fhir_base_url: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned());
        let database_url = env::var("DATABASE_URL").context("missing DATABASE_URL")?;
        let jwt_secret =
            env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me".to_owned());
        let allow_unauthenticated = env::var("ALLOW_UNAUTHENTICATED")
            .ok()
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);
        let fhir_base_url =
            env::var("FHIR_BASE_URL").unwrap_or_else(|_| "http://localhost:8080/fhir".to_owned());

        Ok(Self {
            bind_addr,
            database_url,
            jwt_secret,
            allow_unauthenticated,
            fhir_base_url,
        })
    }
}
