use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use crate::error::AppError;
use crate::search_params::sql::SearchFilter;

/// Outcome of an atomic conditional create (`If-None-Exist`).
///
/// The match-and-create decision is serialized inside a single PostgreSQL
/// transaction guarded by a transaction-scoped advisory lock, so two
/// concurrent identical conditional creates cannot both observe zero matches
/// and produce duplicate logical resources. See
/// [`PgStore::conditional_create_atomic`].
#[derive(Debug)]
pub enum ConditionalCreateOutcome {
    /// A new resource was created.
    Created(StoredResource),
    /// Exactly one existing resource matched the condition and is returned
    /// unchanged.
    Existing(StoredResource),
    /// More than one existing resource matched the condition.
    MultipleMatches,
}

/// A database executor that can be either a pool or an active transaction.
/// Used by the Bundle transaction/batch handler to share store logic.
pub type TxExecutor<'a> = sqlx::Transaction<'a, Postgres>;

#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

#[derive(Debug)]
pub struct StoredResource {
    pub id: String,
    pub version_id: i64,
    pub last_updated: DateTime<Utc>,
    pub resource: Value,
}

pub struct UpsertResult {
    pub stored: StoredResource,
    pub created: bool,
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
    ) -> Result<UpsertResult, AppError> {
        let row = sqlx::query(
            r#"
            WITH existing AS (
                SELECT EXISTS (
                    SELECT 1
                    FROM fhir_resources
                    WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
                ) AS existed
            ),
            next_version AS (
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
            RETURNING version_id, last_updated, resource,
                NOT (SELECT existed FROM existing) AS created
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(resource)
        .fetch_one(&self.pool)
        .await?;

        Ok(UpsertResult {
            stored: StoredResource {
                id: id.to_owned(),
                version_id: row.get::<i64, _>("version_id"),
                last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
                resource: row.get::<Value, _>("resource"),
            },
            created: row.get::<bool, _>("created"),
        })
    }

    /// Create a resource without ever changing an existing logical resource.
    ///
    /// Returns `None` if the generated logical id is already present. The
    /// current-row insert and initial history insert are one SQL statement, so
    /// concurrent attempts for the same tenant, type, and id cannot both win.
    pub async fn create(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<Option<StoredResource>, AppError> {
        let row = sqlx::query(
            r#"
            WITH created AS (
                INSERT INTO fhir_resources (
                    tenant_id, resource_type, id, version_id, resource
                )
                SELECT $1, $2, $3, 1, $4
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM fhir_resource_history
                    WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
                )
                ON CONFLICT (resource_type, tenant_id, id) DO NOTHING
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
            FROM created
            RETURNING version_id, last_updated, resource
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(resource)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| StoredResource {
            id: id.to_owned(),
            version_id: row.get::<i64, _>("version_id"),
            last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
            resource: row.get::<Value, _>("resource"),
        }))
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
        let mut total_query: QueryBuilder<Postgres> =
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

        let mut resource_query: QueryBuilder<Postgres> = QueryBuilder::new(
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

    /// Begin a database transaction for Bundle transaction processing.
    pub async fn begin_tx(&self) -> Result<TxExecutor<'_>, AppError> {
        Ok(self.pool.begin().await?)
    }

    /// Atomically evaluate a conditional create (`If-None-Exist`).
    ///
    /// # Locking strategy
    ///
    /// The whole match-and-create decision runs inside one PostgreSQL
    /// transaction guarded by a transaction-scoped advisory lock
    /// (`pg_advisory_xact_lock`) keyed on `lock_key`. The lock key MUST be a
    /// deterministic function of the tenant, resource type, and the canonical
    /// form of the decoded search condition (see
    /// [`conditional_create_lock_key`]).
    ///
    /// Two concurrent requests carrying equivalent `If-None-Exist` headers
    /// therefore acquire the same lock and are serialized: the first one
    /// observes zero matches and creates the resource; the second one — once
    /// it acquires the lock — observes the freshly inserted row and returns
    /// the existing resource (HTTP 200).
    ///
    /// Unrelated tenants, unrelated resource types, and unrelated conditions
    /// hash to different lock keys, so they do not serialize against each
    /// other. False key collisions only cause extra serialization (no
    /// correctness impact); true collisions are impossible because identical
    /// keys are hashed deterministically.
    ///
    /// # Multiple matches
    ///
    /// If the condition matches more than one existing resource, no resource
    /// is created and the transaction rolls back. The caller surfaces `412
    /// Precondition Failed` per the FHIR specification.
    pub async fn conditional_create_atomic(
        &self,
        tenant_id: &str,
        resource_type: &str,
        filters: &[SearchFilter],
        lock_key: i64,
        id: &str,
        resource: Value,
    ) -> Result<ConditionalCreateOutcome, AppError> {
        let mut tx = self.pool.begin().await?;

        // Transaction-scoped advisory lock: released automatically on
        // commit/rollback. Serializes concurrent identical conditional creates.
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut *tx)
            .await?;

        // Search for existing matches under the lock. LIMIT 2 is enough to
        // distinguish zero, one, and more-than-one matches.
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, version_id, last_updated, resource FROM fhir_resources WHERE tenant_id = ",
        );
        query.push_bind(tenant_id);
        query.push(" AND resource_type = ");
        query.push_bind(resource_type);
        crate::search_params::sql::push_search_filters(&mut query, filters);
        query.push(" ORDER BY id ASC LIMIT ");
        query.push_bind(2_i64);

        let rows = query.build().fetch_all(&mut *tx).await?;

        if rows.len() > 1 {
            tx.rollback().await?;
            return Ok(ConditionalCreateOutcome::MultipleMatches);
        }

        if let Some(row) = rows.into_iter().next() {
            tx.rollback().await?;
            return Ok(ConditionalCreateOutcome::Existing(StoredResource {
                id: row.get::<String, _>("id"),
                version_id: row.get::<i64, _>("version_id"),
                last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
                resource: row.get::<Value, _>("resource"),
            }));
        }

        // Zero matches: create the new resource within the same transaction.
        let created = Self::create_in_tx(&mut tx, tenant_id, resource_type, id, resource)
            .await?
            .ok_or_else(|| {
                AppError::Conflict("a resource with the generated id already exists".to_owned())
            })?;

        tx.commit().await?;

        Ok(ConditionalCreateOutcome::Created(created))
    }

    /// Create a resource inside an existing transaction without overwriting.
    pub async fn create_in_tx(
        tx: &mut TxExecutor<'_>,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<Option<StoredResource>, AppError> {
        let row = sqlx::query(
            r#"
            WITH created AS (
                INSERT INTO fhir_resources (
                    tenant_id, resource_type, id, version_id, resource
                )
                SELECT $1, $2, $3, 1, $4
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM fhir_resource_history
                    WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
                )
                ON CONFLICT (resource_type, tenant_id, id) DO NOTHING
                RETURNING version_id, last_updated, resource
            )
            INSERT INTO fhir_resource_history (
                tenant_id, resource_type, id, version_id, last_updated, deleted, resource
            )
            SELECT $1, $2, $3, version_id, last_updated, FALSE, resource
            FROM created
            RETURNING version_id, last_updated, resource
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(resource)
        .fetch_optional(&mut **tx)
        .await?;

        Ok(row.map(|row| StoredResource {
            id: id.to_owned(),
            version_id: row.get::<i64, _>("version_id"),
            last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
            resource: row.get::<Value, _>("resource"),
        }))
    }

    /// Upsert a resource inside an existing transaction.
    pub async fn upsert_in_tx(
        tx: &mut TxExecutor<'_>,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<UpsertResult, AppError> {
        let row = sqlx::query(
            r#"
            WITH existing AS (
                SELECT EXISTS (
                    SELECT 1
                    FROM fhir_resources
                    WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
                ) AS existed
            ),
            next_version AS (
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
                tenant_id, resource_type, id, version_id, last_updated, deleted, resource
            )
            SELECT $1, $2, $3, version_id, last_updated, FALSE, resource
            FROM upserted
            RETURNING version_id, last_updated, resource,
                NOT (SELECT existed FROM existing) AS created
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(resource)
        .fetch_one(&mut **tx)
        .await?;

        Ok(UpsertResult {
            stored: StoredResource {
                id: id.to_owned(),
                version_id: row.get::<i64, _>("version_id"),
                last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
                resource: row.get::<Value, _>("resource"),
            },
            created: row.get::<bool, _>("created"),
        })
    }

    /// Read a resource inside an existing transaction.
    pub async fn read_in_tx(
        tx: &mut TxExecutor<'_>,
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
        .fetch_optional(&mut **tx)
        .await?;

        Ok(row.map(|r| StoredResource {
            id: id.to_owned(),
            version_id: r.get::<i64, _>("version_id"),
            last_updated: r.get::<DateTime<Utc>, _>("last_updated"),
            resource: r.get::<Value, _>("resource"),
        }))
    }

    /// Update an existing resource inside a transaction.
    pub async fn update_in_tx(
        tx: &mut TxExecutor<'_>,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
    ) -> Result<Option<StoredResource>, AppError> {
        let updated = sqlx::query(
            r#"
            UPDATE fhir_resources
            SET resource = $4,
                version_id = version_id + 1,
                last_updated = now()
            WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
            RETURNING version_id, last_updated, resource
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(&resource)
        .fetch_optional(&mut **tx)
        .await?;

        let Some(updated_row) = updated else {
            return Ok(None);
        };

        let new_version_id = updated_row.get::<i64, _>("version_id");
        let last_updated = updated_row.get::<DateTime<Utc>, _>("last_updated");
        let updated_resource = updated_row.get::<Value, _>("resource");

        sqlx::query(
            r#"
            INSERT INTO fhir_resource_history (
                tenant_id, resource_type, id, version_id, last_updated, deleted, resource
            )
            VALUES ($1, $2, $3, $4, $5, FALSE, $6)
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(new_version_id)
        .bind(last_updated)
        .bind(&updated_resource)
        .execute(&mut **tx)
        .await?;

        Ok(Some(StoredResource {
            id: id.to_owned(),
            version_id: new_version_id,
            last_updated,
            resource: updated_resource,
        }))
    }

    /// Conditionally update an existing resource inside a transaction.
    pub async fn update_if_version_matches_in_tx(
        tx: &mut TxExecutor<'_>,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
        resource: Value,
    ) -> Result<Option<StoredResource>, AppError> {
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
        .bind(&resource)
        .bind(expected_version)
        .fetch_optional(&mut **tx)
        .await?;

        let Some(updated_row) = updated else {
            return Ok(None);
        };

        let new_version_id = updated_row.get::<i64, _>("version_id");
        let last_updated = updated_row.get::<DateTime<Utc>, _>("last_updated");
        let updated_resource = updated_row.get::<Value, _>("resource");

        sqlx::query(
            r#"
            INSERT INTO fhir_resource_history (
                tenant_id, resource_type, id, version_id, last_updated, deleted, resource
            )
            VALUES ($1, $2, $3, $4, $5, FALSE, $6)
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(new_version_id)
        .bind(last_updated)
        .bind(&updated_resource)
        .execute(&mut **tx)
        .await?;

        Ok(Some(StoredResource {
            id: id.to_owned(),
            version_id: new_version_id,
            last_updated,
            resource: updated_resource,
        }))
    }

    /// Delete a resource inside a transaction.
    pub async fn delete_in_tx(
        tx: &mut TxExecutor<'_>,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<bool, AppError> {
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
        .fetch_optional(&mut **tx)
        .await?;

        let Some(row) = deleted else {
            return Ok(false);
        };

        let version_id = row.get::<i64, _>("version_id") + 1;
        let resource = row.get::<Value, _>("resource");

        sqlx::query(
            r#"
            INSERT INTO fhir_resource_history (
                tenant_id, resource_type, id, version_id, last_updated, deleted, resource
            )
            VALUES ($1, $2, $3, $4, now(), TRUE, $5)
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(version_id)
        .bind(resource)
        .execute(&mut **tx)
        .await?;

        Ok(true)
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

/// Derive a deterministic PostgreSQL advisory lock key for a conditional
/// create.
///
/// The key is a stable function of the (tenant, resource type, canonical
/// condition) tuple. The condition is canonicalized by sorting its decoded
/// search filters by parameter code, so two equivalent `If-None-Exist`
/// headers carrying parameters in different orders produce the same key.
///
/// The returned `i64` is fed to `pg_advisory_xact_lock(bigint)`. Hash
/// collisions across unrelated (tenant, type, condition) tuples only cause
/// extra serialization — never incorrect results — because identical
/// conditions hash to identical keys.
pub fn conditional_create_lock_key(
    tenant_id: &str,
    resource_type: &str,
    filters: &[SearchFilter],
) -> i64 {
    // Build the canonical condition: (param_code, value) pairs sorted by
    // parameter code. The last write wins on duplicate codes — current
    // query parsing already collapses repeated keys.
    let mut canon: BTreeMap<&'static str, &str> = BTreeMap::new();
    for f in filters {
        canon.insert(f.param.code, f.value.as_str());
    }

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    tenant_id.hash(&mut hasher);
    0u8.hash(&mut hasher);
    resource_type.hash(&mut hasher);
    0u8.hash(&mut hasher);
    for (code, value) in canon {
        code.hash(&mut hasher);
        1u8.hash(&mut hasher);
        value.hash(&mut hasher);
        2u8.hash(&mut hasher);
    }
    hasher.finish() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patient_filter(code: &str, value: &str) -> SearchFilter {
        let param = crate::search_params::search_params_for("Patient")
            .iter()
            .find(|p| p.code == code)
            .unwrap_or_else(|| panic!("no Patient search param for code '{code}'"));
        SearchFilter {
            param,
            value: value.to_owned(),
        }
    }

    #[test]
    fn lock_key_is_stable_for_same_inputs() {
        let a = conditional_create_lock_key(
            "tenant-a",
            "Patient",
            &[
                patient_filter("identifier", "http://example.org|abc"),
                patient_filter("name", "smith"),
            ],
        );
        let b = conditional_create_lock_key(
            "tenant-a",
            "Patient",
            &[
                patient_filter("identifier", "http://example.org|abc"),
                patient_filter("name", "smith"),
            ],
        );
        assert_eq!(a, b, "equivalent input should hash to the same key");
    }

    #[test]
    fn lock_key_ignores_filter_order() {
        let a = conditional_create_lock_key(
            "tenant-a",
            "Patient",
            &[
                patient_filter("identifier", "http://example.org|abc"),
                patient_filter("name", "smith"),
            ],
        );
        let b = conditional_create_lock_key(
            "tenant-a",
            "Patient",
            &[
                patient_filter("name", "smith"),
                patient_filter("identifier", "http://example.org|abc"),
            ],
        );
        assert_eq!(
            a, b,
            "reordered equivalent conditions must share the same lock key"
        );
    }

    #[test]
    fn lock_key_differs_for_unrelated_tenants() {
        let a = conditional_create_lock_key(
            "tenant-a",
            "Patient",
            &[patient_filter("identifier", "x")],
        );
        let b = conditional_create_lock_key(
            "tenant-b",
            "Patient",
            &[patient_filter("identifier", "x")],
        );
        assert_ne!(a, b, "different tenants must produce different lock keys");
    }

    #[test]
    fn lock_key_differs_for_unrelated_conditions() {
        let a = conditional_create_lock_key(
            "tenant-a",
            "Patient",
            &[patient_filter("identifier", "x")],
        );
        let b =
            conditional_create_lock_key("tenant-a", "Patient", &[patient_filter("name", "smith")]);
        assert_ne!(
            a, b,
            "different conditions must produce different lock keys"
        );
    }

    #[test]
    fn lock_key_differs_for_unrelated_resource_types() {
        let a = conditional_create_lock_key(
            "tenant-a",
            "Patient",
            &[patient_filter("identifier", "x")],
        );
        let b = conditional_create_lock_key(
            "tenant-a",
            "Observation",
            &[patient_filter("identifier", "x")],
        );
        // If Observation happens not to define an `identifier` param the
        // lookup in the helper would panic, but Observation DOES define one.
        assert_ne!(
            a, b,
            "different resource types must produce different lock keys"
        );
    }
}
