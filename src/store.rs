use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use std::hash::{Hash, Hasher};
use std::time::Duration;

use crate::audit::MutationAuditContext;
use crate::error::AppError;
use crate::search_params::sql::{GeoSearchMode, SearchFilter};

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
    geo_mode: GeoSearchMode,
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

/// Outcome of an atomic version-aware delete (`If-Match`).
///
/// The delete-and-version-check is serialized inside a single PostgreSQL
/// transaction, so a stale client cannot delete a resource that another
/// writer has already updated.
#[derive(Debug, PartialEq, Eq)]
pub enum DeleteIfMatchOutcome {
    /// The resource matched the expected version and was deleted. Carries the
    /// next history version recorded as the tombstone.
    Deleted { new_version_id: i64 },
    /// The resource exists but its current version differs from the expected
    /// version.
    VersionMismatch,
    /// No such resource exists.
    NotFound,
}

pub struct SearchResults {
    pub total: i64,
    pub resources: Vec<StoredResource>,
    pub next_after_id: Option<String>,
}

/// Result of a `_sort`-driven search. Kept separate from [`SearchResults`]
/// because the pagination cursor for a sorted page is a vector of typed
/// column values (see [`crate::sort::SortCursorValue`]), not a single id.
pub struct SortedSearchResults {
    pub total: i64,
    pub resources: Vec<StoredResource>,
    pub next_cursor_values: Option<Vec<crate::sort::SortCursorValue>>,
}

pub struct HistoricalResource {
    pub id: String,
    pub version_id: i64,
    pub last_updated: DateTime<Utc>,
    pub deleted: bool,
    pub resource: Value,
}

/// A page of instance-history versions plus the cursor for the next page.
pub struct HistoryResults {
    pub exists: bool,
    pub versions: Vec<HistoricalResource>,
    pub next_after_version_id: Option<i64>,
}

impl PgStore {
    async fn record_success_and_commit(
        mut tx: TxExecutor<'_>,
        event: crate::audit::NewAuditEvent,
    ) -> Result<(), AppError> {
        if Self::append_audit_in_tx(&mut tx, event).await.is_err() {
            // The resource and history writes share this transaction with the
            // audit insert.  Do not surface 503 until rollback has completed.
            tx.rollback().await?;
            ::metrics::counter!("nissefhir_audit_persistence_failures_total").increment(1);
            return Err(AppError::ServiceUnavailable);
        }
        tx.commit().await?;
        Ok(())
    }

    pub fn new(pool: PgPool, geo_mode: GeoSearchMode) -> Self {
        Self { pool, geo_mode }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Cheap database-aware readiness check: run `SELECT 1` with a short
    /// bounded timeout. Returns `false` on any failure or timeout so that
    /// unready pods are taken out of service without a restart loop.
    pub async fn is_ready(&self) -> bool {
        tokio::time::timeout(Duration::from_secs(2), async {
            sqlx::query("SELECT 1").execute(&self.pool).await
        })
        .await
        .is_ok_and(|result| result.is_ok())
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

    /// Create a resource and its successful audit evidence atomically.
    pub async fn create_with_audit(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
        audit: &MutationAuditContext,
    ) -> Result<Option<StoredResource>, AppError> {
        let mut tx = self.pool.begin().await?;
        let stored = Self::create_in_tx(&mut tx, tenant_id, resource_type, id, resource).await?;
        let Some(stored) = stored else {
            tx.rollback().await?;
            return Ok(None);
        };
        Self::record_success_and_commit(
            tx,
            audit.success_event(
                "create",
                'C',
                resource_type,
                id,
                201,
                Some(stored.version_id),
                None,
            ),
        )
        .await?;
        Ok(Some(stored))
    }

    /// Upsert a resource and its successful audit evidence atomically.
    pub async fn upsert_with_audit(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
        audit: &MutationAuditContext,
    ) -> Result<UpsertResult, AppError> {
        let mut tx = self.pool.begin().await?;
        let result = Self::upsert_in_tx(&mut tx, tenant_id, resource_type, id, resource).await?;
        let status = if result.created { 201 } else { 200 };
        Self::record_success_and_commit(
            tx,
            audit.success_event(
                "update",
                'U',
                resource_type,
                id,
                status,
                Some(result.stored.version_id),
                None,
            ),
        )
        .await?;
        Ok(result)
    }

    /// Update conditionally and persist success evidence in the same
    /// transaction. A non-match is intentionally not audited as success.
    pub async fn update_if_version_matches_with_audit(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
        resource: Value,
        audit: &MutationAuditContext,
    ) -> Result<Option<StoredResource>, AppError> {
        let mut tx = self.pool.begin().await?;
        let stored = Self::update_if_version_matches_in_tx(
            &mut tx,
            tenant_id,
            resource_type,
            id,
            expected_version,
            resource,
        )
        .await?;
        let Some(stored) = stored else {
            tx.rollback().await?;
            return Ok(None);
        };
        Self::record_success_and_commit(
            tx,
            audit.success_event(
                "update",
                'U',
                resource_type,
                id,
                200,
                Some(stored.version_id),
                None,
            ),
        )
        .await?;
        Ok(Some(stored))
    }

    /// Update an existing resource and persist success evidence atomically.
    pub async fn update_existing_with_audit(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        resource: Value,
        audit: &MutationAuditContext,
    ) -> Result<Option<StoredResource>, AppError> {
        let mut tx = self.pool.begin().await?;
        let stored = Self::update_in_tx(&mut tx, tenant_id, resource_type, id, resource).await?;
        let Some(stored) = stored else {
            tx.rollback().await?;
            return Ok(None);
        };
        Self::record_success_and_commit(
            tx,
            audit.success_event(
                "update",
                'U',
                resource_type,
                id,
                200,
                Some(stored.version_id),
                None,
            ),
        )
        .await?;
        Ok(Some(stored))
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

    /// Delete a resource only if its current version matches `expected_version`.
    ///
    /// Returns [`DeleteIfMatchOutcome::Deleted`] (with the new history version)
    /// on a match, [`DeleteIfMatchOutcome::VersionMismatch`] when the resource
    /// exists at a different version, and [`DeleteIfMatchOutcome::NotFound`]
    /// when there is no such resource. Nothing is modified unless the version
    /// matches.
    pub async fn delete_if_version_matches(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
    ) -> Result<DeleteIfMatchOutcome, AppError> {
        let mut tx = self.pool.begin().await?;
        let outcome = PgStore::delete_if_version_matches_in_tx(
            &mut tx,
            tenant_id,
            resource_type,
            id,
            expected_version,
        )
        .await?;
        if matches!(outcome, DeleteIfMatchOutcome::Deleted { .. }) {
            tx.commit().await?;
        } else {
            tx.rollback().await?;
        }
        Ok(outcome)
    }

    pub async fn delete(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<Option<i64>, AppError> {
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
            return Ok(None);
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
        Ok(Some(version_id))
    }

    /// Delete (optionally version-checked) and write its successful audit row
    /// in the same transaction as the tombstone history row.
    pub async fn delete_with_audit(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: Option<i64>,
        audit: &MutationAuditContext,
    ) -> Result<DeleteIfMatchOutcome, AppError> {
        let mut tx = self.pool.begin().await?;
        let outcome = match expected_version {
            Some(version) => {
                Self::delete_if_version_matches_in_tx(
                    &mut tx,
                    tenant_id,
                    resource_type,
                    id,
                    version,
                )
                .await?
            }
            None => {
                if let Some(version) =
                    Self::delete_in_tx(&mut tx, tenant_id, resource_type, id).await?
                {
                    DeleteIfMatchOutcome::Deleted {
                        new_version_id: version,
                    }
                } else {
                    DeleteIfMatchOutcome::NotFound
                }
            }
        };
        let DeleteIfMatchOutcome::Deleted { new_version_id } = outcome else {
            tx.rollback().await?;
            return Ok(outcome);
        };
        Self::record_success_and_commit(
            tx,
            audit.success_event(
                "delete",
                'D',
                resource_type,
                id,
                204,
                Some(new_version_id),
                None,
            ),
        )
        .await?;
        Ok(outcome)
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
        crate::search_params::sql::push_search_filters(&mut total_query, filters, self.geo_mode)?;

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
        crate::search_params::sql::push_search_filters(
            &mut resource_query,
            filters,
            self.geo_mode,
        )?;

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

    /// Search under a client-requested `_sort` order. Kept as a separate
    /// method from [`PgStore::search`] rather than unifying the two: the
    /// default (no `_sort`) path is unchanged by task 040 by design, so its
    /// well-exercised query stays untouched, and this path owns the keyset
    /// predicate and multi-column `ORDER BY` a sorted page needs instead.
    ///
    /// `sort` must be the *effective* sort (see [`crate::sort::effective_sort`]),
    /// i.e. already including the `_id` tiebreak. `after` is the decoded
    /// cursor from the previous page, with one value per entry in `sort`.
    pub async fn search_sorted(
        &self,
        tenant_id: &str,
        resource_type: &str,
        filters: &[SearchFilter],
        limit: i64,
        sort: &[crate::sort::SortKey],
        after: Option<&[crate::sort::SortCursorValue]>,
    ) -> Result<SortedSearchResults, AppError> {
        let mut total_query: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT count(*) FROM fhir_resources WHERE tenant_id = ");
        total_query.push_bind(tenant_id);
        total_query.push(" AND resource_type = ");
        total_query.push_bind(resource_type);
        crate::search_params::sql::push_search_filters(&mut total_query, filters, self.geo_mode)?;

        let total = total_query
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await?;

        if limit <= 0 {
            return Ok(SortedSearchResults {
                total,
                resources: Vec::new(),
                next_cursor_values: None,
            });
        }

        let mut resource_query: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, version_id, last_updated, resource FROM fhir_resources WHERE tenant_id = ",
        );
        resource_query.push_bind(tenant_id);
        resource_query.push(" AND resource_type = ");
        resource_query.push_bind(resource_type);
        crate::search_params::sql::push_search_filters(
            &mut resource_query,
            filters,
            self.geo_mode,
        )?;

        if let Some(after) = after {
            crate::sort::push_keyset_predicate(&mut resource_query, sort, after);
        }

        crate::sort::push_order_by(&mut resource_query, sort);
        resource_query.push(" LIMIT ");
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

        let next_cursor_values = if resources.len() > limit as usize {
            resources.truncate(limit as usize);
            resources
                .last()
                .map(|resource| crate::sort::cursor_values_for(sort, resource))
        } else {
            None
        };

        Ok(SortedSearchResults {
            total,
            resources,
            next_cursor_values,
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
        crate::search_params::sql::push_search_filters(&mut query, filters, self.geo_mode)?;
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

    /// Conditional create plus its success audit row in one transaction.  The
    /// one-match path commits its audit row too, despite making no resource
    /// change, so the 200 result is durably evidenced before it is returned.
    #[allow(clippy::too_many_arguments)] // mirrors conditional_create_atomic + audit context
    pub async fn conditional_create_with_audit(
        &self,
        tenant_id: &str,
        resource_type: &str,
        filters: &[SearchFilter],
        lock_key: i64,
        id: &str,
        resource: Value,
        audit: &MutationAuditContext,
    ) -> Result<ConditionalCreateOutcome, AppError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut *tx)
            .await?;
        let mut query: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, version_id, last_updated, resource FROM fhir_resources WHERE tenant_id = ",
        );
        query.push_bind(tenant_id);
        query.push(" AND resource_type = ");
        query.push_bind(resource_type);
        crate::search_params::sql::push_search_filters(&mut query, filters, self.geo_mode)?;
        query.push(" ORDER BY id ASC LIMIT ");
        query.push_bind(2_i64);
        let rows = query.build().fetch_all(&mut *tx).await?;
        if rows.len() > 1 {
            tx.rollback().await?;
            return Ok(ConditionalCreateOutcome::MultipleMatches);
        }
        let (outcome, event) = if let Some(row) = rows.into_iter().next() {
            let stored = StoredResource {
                id: row.get("id"),
                version_id: row.get("version_id"),
                last_updated: row.get("last_updated"),
                resource: row.get("resource"),
            };
            let event = audit.success_event(
                "create",
                'C',
                resource_type,
                &stored.id,
                200,
                Some(stored.version_id),
                Some("existing"),
            );
            (ConditionalCreateOutcome::Existing(stored), event)
        } else {
            let stored = Self::create_in_tx(&mut tx, tenant_id, resource_type, id, resource)
                .await?
                .ok_or_else(|| {
                    AppError::Conflict("a resource with the generated id already exists".to_owned())
                })?;
            let event = audit.success_event(
                "create",
                'C',
                resource_type,
                id,
                201,
                Some(stored.version_id),
                Some("created"),
            );
            (ConditionalCreateOutcome::Created(stored), event)
        };
        Self::record_success_and_commit(tx, event).await?;
        Ok(outcome)
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

    /// Delete a resource inside a transaction, only if its current version
    /// matches `expected_version`. See [`PgStore::delete_if_version_matches`].
    pub async fn delete_if_version_matches_in_tx(
        tx: &mut TxExecutor<'_>,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        expected_version: i64,
    ) -> Result<DeleteIfMatchOutcome, AppError> {
        let deleted = sqlx::query(
            r#"
            DELETE FROM fhir_resources
            WHERE tenant_id = $1 AND resource_type = $2 AND id = $3 AND version_id = $4
            RETURNING version_id, resource
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .bind(expected_version)
        .fetch_optional(&mut **tx)
        .await?;

        let Some(row) = deleted else {
            // The version predicate did not match. Distinguish a version
            // mismatch from a missing resource by checking existence.
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT version_id FROM fhir_resources WHERE tenant_id = $1 AND resource_type = $2 AND id = $3",
            )
            .bind(tenant_id)
            .bind(resource_type)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
            return Ok(match exists {
                Some(_) => DeleteIfMatchOutcome::VersionMismatch,
                None => DeleteIfMatchOutcome::NotFound,
            });
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

        Ok(DeleteIfMatchOutcome::Deleted {
            new_version_id: version_id,
        })
    }

    /// Delete a resource inside a transaction.
    pub async fn delete_in_tx(
        tx: &mut TxExecutor<'_>,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<Option<i64>, AppError> {
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
            return Ok(None);
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

        Ok(Some(version_id))
    }

    pub async fn read_history(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
        limit: i64,
        after_version_id: Option<i64>,
    ) -> Result<HistoryResults, AppError> {
        if limit <= 0 {
            return Ok(HistoryResults {
                exists: false,
                versions: Vec::new(),
                next_after_version_id: None,
            });
        }

        let rows = if let Some(after) = after_version_id {
            sqlx::query(
                r#"
                SELECT version_id, last_updated, deleted, resource
                FROM fhir_resource_history
                WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
                  AND version_id < $4
                ORDER BY version_id DESC
                LIMIT $5
                "#,
            )
            .bind(tenant_id)
            .bind(resource_type)
            .bind(id)
            .bind(after)
            .bind(limit.saturating_add(1))
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT version_id, last_updated, deleted, resource
                FROM fhir_resource_history
                WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
                ORDER BY version_id DESC
                LIMIT $4
                "#,
            )
            .bind(tenant_id)
            .bind(resource_type)
            .bind(id)
            .bind(limit.saturating_add(1))
            .fetch_all(&self.pool)
            .await?
        };

        let mut versions = rows
            .into_iter()
            .map(|row| HistoricalResource {
                id: id.to_owned(),
                version_id: row.get::<i64, _>("version_id"),
                last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
                deleted: row.get::<bool, _>("deleted"),
                resource: row.get::<Value, _>("resource"),
            })
            .collect::<Vec<_>>();

        let next_after_version_id = if versions.len() > limit as usize {
            versions.truncate(limit as usize);
            versions.last().map(|version| version.version_id)
        } else {
            None
        };

        // A cursor can legitimately point past the oldest version. Only that
        // empty-page case needs a separate existence check to distinguish it
        // from a resource that has no history at all.
        let exists = if versions.is_empty() && after_version_id.is_some() {
            sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS (
                    SELECT 1 FROM fhir_resource_history
                    WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
                )
                "#,
            )
            .bind(tenant_id)
            .bind(resource_type)
            .bind(id)
            .fetch_one(&self.pool)
            .await?
        } else {
            !versions.is_empty()
        };

        Ok(HistoryResults {
            exists,
            versions,
            next_after_version_id,
        })
    }
}

/// Derive a deterministic PostgreSQL advisory lock key for a conditional
/// create.
///
/// The key is a stable function of the (tenant, resource type, canonical
/// condition) tuple. The condition is canonicalized by sorting parameter
/// occurrences and the OR values within them, so equivalent `If-None-Exist`
/// headers carrying terms in different orders produce the same key.
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
    // Repeated occurrences are AND terms and comma-separated values are OR
    // terms, so both levels can be sorted without changing their meaning.
    let mut canon = Vec::new();
    for f in filters {
        let mut values = f.values.clone();
        values.sort();
        canon.push((f.param.code, values));
    }
    canon.sort();

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    tenant_id.hash(&mut hasher);
    0u8.hash(&mut hasher);
    resource_type.hash(&mut hasher);
    0u8.hash(&mut hasher);
    for (code, values) in canon {
        code.hash(&mut hasher);
        1u8.hash(&mut hasher);
        values.hash(&mut hasher);
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
            values: vec![value.to_owned()],
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

    #[tokio::test]
    async fn is_ready_true_when_database_reachable() {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            return;
        };
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy(&database_url)
            .expect("lazy pool should build");
        let store = PgStore::new(pool, GeoSearchMode::EarthDistance);
        assert!(store.is_ready().await, "reachable database must be ready");
    }

    #[tokio::test]
    async fn is_ready_false_when_database_unreachable() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@127.0.0.1:1/postgres")
            .expect("lazy pool should build");
        let store = PgStore::new(pool, GeoSearchMode::EarthDistance);
        assert!(
            !store.is_ready().await,
            "unreachable database must not be ready"
        );
    }

    #[tokio::test]
    async fn is_ready_times_out_and_reports_unready() {
        // A non-routable address that drops packets forces the connection to
        // hang until the bounded timeout fires; either way the check must
        // return false without exceeding the bound.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@10.255.255.1:5432/postgres")
            .expect("lazy pool should build");
        let store = PgStore::new(pool, GeoSearchMode::EarthDistance);
        let start = std::time::Instant::now();
        assert!(
            !store.is_ready().await,
            "unresponsive database must not be ready"
        );
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "readiness check must be bounded by its timeout"
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
