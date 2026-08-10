mod common;

use common::{clean_tenant, setup_test_db};
use fhir_server::{search_params::sql::GeoSearchMode, store::PgStore};
use serde_json::json;
use sqlx::Row;

#[tokio::test]
async fn reference_index_is_atomically_replaced_and_deleted() {
    let pool = setup_test_db().await;
    let tenant = "everything-reference-lifecycle";
    clean_tenant(&pool, tenant).await;
    let store = PgStore::new(pool.clone(), GeoSearchMode::EarthDistance)
        .with_fhir_base_url("http://localhost:8080/fhir")
        .unwrap();
    store
        .upsert(
            tenant,
            "Observation",
            "o1",
            json!({
                "resourceType":"Observation","id":"o1","status":"final","code":{"text":"x"},
                "subject":{"reference":"Patient/old"}
            }),
        )
        .await
        .unwrap();
    let second_upsert = store
        .upsert(
            tenant,
            "Observation",
            "o1",
            json!({
                "resourceType":"Observation","id":"o1","status":"final","code":{"text":"x"},
                "subject":{"reference":"http://localhost:8080/fhir/Patient/new"},
                "performer":[{"reference":"https://external.example/fhir/Practitioner/nope"}]
            }),
        )
        .await
        .unwrap();

    let rows = sqlx::query(
        "SELECT target_type, target_id, source_version_id FROM fhir_resource_references WHERE tenant_id=$1 AND source_type='Observation' AND source_id='o1'"
    ).bind(tenant).fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("target_type"), "Patient");
    assert_eq!(rows[0].get::<String, _>("target_id"), "new");
    assert_eq!(
        rows[0].get::<i64, _>("source_version_id"),
        second_upsert.stored.version_id
    );

    store.delete(tenant, "Observation", "o1").await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fhir_resource_references WHERE tenant_id=$1 AND source_id='o1'",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn rolled_back_resource_write_leaves_no_reference_rows() {
    let pool = setup_test_db().await;
    let tenant = "everything-reference-rollback";
    clean_tenant(&pool, tenant).await;
    let mut tx = pool.begin().await.unwrap();
    PgStore::upsert_in_tx(
        &mut tx,
        tenant,
        "Observation",
        "o1",
        json!({
            "resourceType":"Observation","id":"o1","status":"final","code":{"text":"x"},
            "subject":{"reference":"Patient/p1"}
        }),
        None,
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM fhir_resource_references WHERE tenant_id=$1")
            .bind(tenant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}
