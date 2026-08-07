//! Result metadata is bounded evidence, not a copy of the request or response.

mod common;

use axum::http::StatusCode;
use common::{
    build_test_app, clean_tenant, post_resource_conditional, post_resource_with_token,
    search_resource_with_token, send_request, setup_test_db, tenant_token,
};
use serde_json::json;
use sqlx::Row;

const TENANT: &str = "audit-result-metadata";

#[tokio::test]
async fn records_search_counts_and_conditional_create_disposition_without_request_content() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, TENANT).await;
    sqlx::query("DELETE FROM audit_events WHERE tenant_id = $1")
        .bind(TENANT)
        .execute(&pool)
        .await
        .unwrap();
    let app = build_test_app(pool.clone());
    let token = tenant_token(TENANT);
    let patient = json!({
        "resourceType": "Patient",
        "id": "metadata-patient",
        "active": true,
        "identifier": [{"system": "urn:private", "value": "patient-secret"}]
    });

    let (status, _) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = send_request(
        app.clone(),
        search_resource_with_token(
            "Patient",
            Some("identifier=urn:private|patient-secret"),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    let search = sqlx::query(
        "SELECT result_count, resource_version, to_jsonb(audit_events)::text AS evidence \
         FROM audit_events WHERE tenant_id = $1 AND interaction = 'search' \
         ORDER BY recorded_at DESC LIMIT 1",
    )
    .bind(TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(search.get::<Option<i64>, _>("result_count"), Some(1));
    assert_eq!(search.get::<Option<i64>, _>("resource_version"), None);
    let evidence: String = search.get("evidence");
    for forbidden in [
        "patient-secret",
        "urn:private",
        "metadata-patient",
        "identifier=",
    ] {
        assert!(
            !evidence.contains(forbidden),
            "audit evidence leaked {forbidden}"
        );
    }

    let conditional = json!({"resourceType": "Patient", "active": true,
        "identifier": [{"system": "urn:conditional", "value": "created"}]});
    let (status, _) = send_request(
        app.clone(),
        post_resource_conditional(
            "Patient",
            &conditional,
            &token,
            "identifier=urn:conditional|created",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = send_request(
        app,
        post_resource_conditional(
            "Patient",
            &conditional,
            &token,
            "identifier=urn:conditional|created",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let create = sqlx::query(
        "SELECT resource_version, conditional_create_disposition \
         FROM audit_events WHERE tenant_id = $1 AND interaction = 'create' \
         ORDER BY recorded_at ASC LIMIT 1",
    )
    .bind(TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(create.get::<Option<i64>, _>("resource_version"), Some(1));
    assert_eq!(
        create.get::<Option<String>, _>("conditional_create_disposition"),
        None
    );
    let conditional_dispositions = sqlx::query_scalar::<_, String>(
        "SELECT conditional_create_disposition FROM audit_events \
         WHERE tenant_id = $1 AND conditional_create_disposition IS NOT NULL \
         ORDER BY recorded_at ASC",
    )
    .bind(TENANT)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(conditional_dispositions, ["created", "existing"]);
}
