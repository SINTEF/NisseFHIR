use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};

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

pub struct SearchResults {
    pub total: i64,
    pub resources: Vec<StoredResource>,
}

#[derive(Debug)]
pub enum SearchFilter {
    PatientName(String),
    PatientBirthDate(String),
    PatientIdentifier {
        system: Option<String>,
        value: String,
    },
    ObservationCode(String),
    ObservationStatus(String),
    ObservationSubject(String),
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
            ON CONFLICT (resource_type, tenant_id, id)
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

    pub async fn delete(
        &self,
        tenant_id: &str,
        resource_type: &str,
        id: &str,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            r#"
            DELETE FROM fhir_resources
            WHERE tenant_id = $1 AND resource_type = $2 AND id = $3
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn search(
        &self,
        tenant_id: &str,
        resource_type: &str,
        filters: &[SearchFilter],
        limit: i64,
        offset: i64,
    ) -> Result<SearchResults, AppError> {
        let mut total_query: QueryBuilder<'_, Postgres> =
            QueryBuilder::new("SELECT count(*) FROM fhir_resources WHERE tenant_id = ");
        total_query.push_bind(tenant_id);
        total_query.push(" AND resource_type = ");
        total_query.push_bind(resource_type);
        push_search_filters(&mut total_query, filters);

        let total = total_query
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await?;

        let mut resource_query: QueryBuilder<'_, Postgres> = QueryBuilder::new(
            "SELECT version_id, last_updated, resource FROM fhir_resources WHERE tenant_id = ",
        );
        resource_query.push_bind(tenant_id);
        resource_query.push(" AND resource_type = ");
        resource_query.push_bind(resource_type);
        push_search_filters(&mut resource_query, filters);
        resource_query.push(" ORDER BY last_updated DESC, id ASC LIMIT ");
        resource_query.push_bind(limit);
        resource_query.push(" OFFSET ");
        resource_query.push_bind(offset);

        let rows = resource_query.build().fetch_all(&self.pool).await?;

        let resources = rows
            .into_iter()
            .map(|row| StoredResource {
                version_id: row.get::<i64, _>("version_id"),
                last_updated: row.get::<DateTime<Utc>, _>("last_updated"),
                resource: row.get::<Value, _>("resource"),
            })
            .collect();

        Ok(SearchResults { total, resources })
    }
}

fn push_search_filters(query: &mut QueryBuilder<'_, Postgres>, filters: &[SearchFilter]) {
    for filter in filters {
        match filter {
            SearchFilter::PatientName(value) => {
                let pattern = format!("%{}%", value.to_lowercase());
                query.push(
                    r#"
                    AND EXISTS (
                        SELECT 1
                        FROM jsonb_array_elements(COALESCE(resource->'name', '[]'::jsonb)) AS patient_name
                        WHERE lower(COALESCE(patient_name->>'family', '')) LIKE
                    "#,
                );
                query.push_bind(pattern.clone());
                query.push(
                    r#"
                        OR EXISTS (
                            SELECT 1
                            FROM jsonb_array_elements_text(COALESCE(patient_name->'given', '[]'::jsonb)) AS given_name(given)
                            WHERE lower(given_name.given) LIKE
                    "#,
                );
                query.push_bind(pattern);
                query.push(
                    r#"
                        )
                    )
                    "#,
                );
            }
            SearchFilter::PatientBirthDate(value) => {
                query.push(" AND resource->>'birthDate' = ");
                query.push_bind(value.clone());
            }
            SearchFilter::PatientIdentifier { system, value } => {
                query.push(
                    r#"
                    AND EXISTS (
                        SELECT 1
                        FROM jsonb_array_elements(COALESCE(resource->'identifier', '[]'::jsonb)) AS identifier
                        WHERE identifier->>'value' =
                    "#,
                );
                query.push_bind(value.clone());

                if let Some(system) = system {
                    query.push(" AND identifier->>'system' = ");
                    query.push_bind(system.clone());
                }

                query.push(
                    r#"
                    )
                    "#,
                );
            }
            SearchFilter::ObservationCode(value) => {
                query.push(
                    r#"
                    AND EXISTS (
                        SELECT 1
                        FROM jsonb_array_elements(COALESCE(resource->'code'->'coding', '[]'::jsonb)) AS coding
                        WHERE coding->>'code' =
                    "#,
                );
                query.push_bind(value.clone());
                query.push(
                    r#"
                    )
                    "#,
                );
            }
            SearchFilter::ObservationStatus(value) => {
                query.push(" AND resource->>'status' = ");
                query.push_bind(value.clone());
            }
            SearchFilter::ObservationSubject(value) => {
                query.push(" AND resource->'subject'->>'reference' = ");
                query.push_bind(value.clone());
            }
        }
    }
}
