use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};

use crate::error::AppError;

#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

pub struct StoredResource {
    pub version_id: i64,
    pub last_updated: DateTime<Utc>,
    pub resource: Value,
}

impl PgStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn read(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<Option<StoredResource>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT version_id, last_updated, resource
            FROM fhir_resources
            WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| StoredResource {
            version_id: r.get::<i64, _>("version_id"),
            last_updated: r.get::<DateTime<Utc>, _>("last_updated"),
            resource: r.get::<Value, _>("resource"),
        }))
    }

    pub async fn upsert(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<StoredResource, AppError> {
        let row = sqlx::query(
            r#"
            INSERT INTO fhir_resources (tenant_id, resource_type, id, version_id, resource)
            VALUES ($1, $2, $3, 1, $4)
            ON CONFLICT (tenant_id, resource_type, id)
            DO UPDATE SET
                resource = EXCLUDED.resource,
                version_id = fhir_resources.version_id + 1,
                last_updated = now()
            RETURNING version_id, last_updated, resource
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(resource)
        .fetch_one(&self.pool)
        .await?;

        Ok(StoredResource {
            version_id: row.get::<i64, _>("version_id"),
            last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
            resource: row.get::<Value, _>("resource"),
        })
    }
}
