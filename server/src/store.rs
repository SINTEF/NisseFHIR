use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};

use crate::error::AppError;
use crate::search_params::sql::SearchFilter;

#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

pub struct StoredResource {
    pub id: String,
    pub version_id: i64,
    pub last_updated: DateTime<Utc>,
    pub resource: Value,
}

pub struct SearchResults {
    pub total: i64,
    pub resources: Vec<StoredResource>,
    pub next_after_id: Option<String>,
}

pub struct HistoricalResource {
    pub id: String,
    pub version_id: i64,
    pub last_updated: DateTime<Utc>,
    pub deleted: bool,
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
            id: id.to_owned(),
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
            WITH next_version AS (
                SELECT COALESCE(
                    (
                        SELECT version_id + 1
                        FROM fhir_resources
                        WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
                    ),
                    (
                        SELECT MAX(version_id) + 1
                        FROM fhir_resource_history
                        WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
                    ),
                    1
                ) AS version_id
            ),
            upserted AS (
                INSERT INTO fhir_resources (tenant_id, resource_type, id, version_id, resource)
                SELECT $1, $2, $3, next_version.version_id, $4
                FROM next_version
                ON CONFLICT (resource_type, tenant_id, id)
                DO UPDATE SET
                    resource = EXCLUDED.resource,
                    version_id = fhir_resources.version_id + 1,
                    last_updated = now()
                RETURNING version_id, last_updated, resource
            )
            INSERT INTO fhir_resource_history (
                tenant_id,
                resource_type,
                id,
                version_id,
                last_updated,
                deleted,
                resource
            )
            SELECT $1, $2, $3, version_id, last_updated, FALSE, resource
            FROM upserted
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
            id: id.to_owned(),
            version_id: row.get::<i64, _>("version_id"),
            last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
            resource: row.get::<Value, _>("resource"),
        })
    }

    pub async fn update_if_version_matches(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
        resource: Value,
    ) -> Result<Option<StoredResource>, AppError> {
        let mut tx = self.pool.begin().await?;

        let updated = sqlx::query(
            r#"
            UPDATE fhir_resources
            SET resource = $4,
                version_id = version_id + 1,
                last_updated = now()
            WHERE tenant_id = $1
              AND resource_type = $2
              AND id = $3
              AND version_id = $5
            RETURNING version_id, last_updated, resource
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(resource)
        .bind(expected_version)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(updated_row) = updated else {
            tx.rollback().await?;
            return Ok(None);
        };

        let new_version_id = updated_row.get::<i64, _>("version_id");
        let last_updated = updated_row.get::<DateTime<Utc>, _>("last_updated");
        let updated_resource = updated_row.get::<Value, _>("resource");

        sqlx::query(
            r#"
            INSERT INTO fhir_resource_history (
                tenant_id,
                resource_type,
                id,
                version_id,
                last_updated,
                deleted,
                resource
            )
            VALUES ($1, $2, $3, $4, $5, FALSE, $6)
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(new_version_id)
        .bind(last_updated)
        .bind(updated_resource.clone())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(Some(StoredResource {
            id: id.to_owned(),
            version_id: new_version_id,
            last_updated,
            resource: updated_resource,
        }))
    }

    pub async fn update_existing(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<Option<StoredResource>, AppError> {
        let mut tx = self.pool.begin().await?;

        let updated = sqlx::query(
            r#"
            UPDATE fhir_resources
            SET resource = $4,
                version_id = version_id + 1,
                last_updated = now()
            WHERE tenant_id = $1
              AND resource_type = $2
              AND id = $3
            RETURNING version_id, last_updated, resource
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(resource)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(updated_row) = updated else {
            tx.rollback().await?;
            return Ok(None);
        };

        let new_version_id = updated_row.get::<i64, _>("version_id");
        let last_updated = updated_row.get::<DateTime<Utc>, _>("last_updated");
        let updated_resource = updated_row.get::<Value, _>("resource");

        sqlx::query(
            r#"
            INSERT INTO fhir_resource_history (
                tenant_id,
                resource_type,
                id,
                version_id,
                last_updated,
                deleted,
                resource
            )
            VALUES ($1, $2, $3, $4, $5, FALSE, $6)
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(new_version_id)
        .bind(last_updated)
        .bind(updated_resource.clone())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(Some(StoredResource {
            id: id.to_owned(),
            version_id: new_version_id,
            last_updated,
            resource: updated_resource,
        }))
    }

    pub async fn delete(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<bool, AppError> {
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query(
            r#"
            DELETE FROM fhir_resources
            WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
            RETURNING version_id, resource
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(row) = deleted else {
            tx.rollback().await?;
            return Ok(false);
        };

        let version_id = row.get::<i64, _>("version_id") + 1;
        let resource = row.get::<Value, _>("resource");

        sqlx::query(
            r#"
            INSERT INTO fhir_resource_history (
                tenant_id,
                resource_type,
                id,
                version_id,
                last_updated,
                deleted,
                resource
            )
            VALUES ($1, $2, $3, $4, now(), TRUE, $5)
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(version_id)
        .bind(resource)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(true)
    }

    pub async fn search(
        &self,
        tenant_id: &str,
        resource_type: &str,
        filters: &[SearchFilter],
        limit: i64,
        after_id: Option<&str>,
    ) -> Result<SearchResults, AppError> {
        let mut total_query: QueryBuilder<'_, Postgres> =
            QueryBuilder::new("SELECT count(*) FROM fhir_resources WHERE tenant_id = ");
        total_query.push_bind(tenant_id);
        total_query.push(" AND resource_type = ");
        total_query.push_bind(resource_type);
        crate::search_params::sql::push_search_filters(&mut total_query, filters);

        let total = total_query
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await?;

        if limit <= 0 {
            return Ok(SearchResults {
                total,
                resources: Vec::new(),
                next_after_id: None,
            });
        }

        let mut resource_query: QueryBuilder<'_, Postgres> = QueryBuilder::new(
            "SELECT id, version_id, last_updated, resource FROM fhir_resources WHERE tenant_id = ",
        );
        resource_query.push_bind(tenant_id);
        resource_query.push(" AND resource_type = ");
        resource_query.push_bind(resource_type);
        crate::search_params::sql::push_search_filters(&mut resource_query, filters);

        if let Some(after_id) = after_id {
            resource_query.push(" AND id > ");
            resource_query.push_bind(after_id);
        }

        resource_query.push(" ORDER BY id ASC LIMIT ");
        resource_query.push_bind(limit.saturating_add(1));

        let rows = resource_query.build().fetch_all(&self.pool).await?;

        let mut resources = rows
            .into_iter()
            .map(|row| StoredResource {
                id: row.get::<String, _>("id"),
                version_id: row.get::<i64, _>("version_id"),
                last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
                resource: row.get::<Value, _>("resource"),
            })
            .collect::<Vec<_>>();

        let next_after_id = if resources.len() > limit as usize {
            resources.truncate(limit as usize);
            resources.last().map(|resource| resource.id.clone())
        } else {
            None
        };

        Ok(SearchResults {
            total,
            resources,
            next_after_id,
        })
    }

    pub async fn read_history(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<Vec<HistoricalResource>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT version_id, last_updated, deleted, resource
            FROM fhir_resource_history
            WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
            ORDER BY version_id DESC
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| HistoricalResource {
                id: id.to_owned(),
                version_id: row.get::<i64, _>("version_id"),
                last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
                deleted: row.get::<bool, _>("deleted"),
                resource: row.get::<Value, _>("resource"),
            })
            .collect())
    }
}
