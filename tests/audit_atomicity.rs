//! A failing success-audit insert must roll back the accompanying mutation.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::{
    build_test_app, clean_tenant, count_history_entries, count_resources,
    delete_resource_with_token, post_resource_conditional, post_resource_with_token,
    put_resource_with_token_if_match, send_request, setup_test_db, tenant_token,
};
use serde_json::json;

const TENANT: &str = "audit-atomicity";

async fn seed_patient(pool: &sqlx::PgPool, id: &str) {
    let resource = json!({"resourceType":"Patient", "id":id, "active":true,
        "identifier":[{"system":"urn:audit-atomicity", "value":id}]});
    sqlx::query("INSERT INTO fhir_resources (tenant_id, resource_type, id, version_id, resource) VALUES ($1, 'Patient', $2, 1, $3)")
        .bind(TENANT).bind(id).bind(&resource).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO fhir_resource_history (tenant_id, resource_type, id, version_id, last_updated, deleted, resource) VALUES ($1, 'Patient', $2, 1, now(), FALSE, $3)")
        .bind(TENANT).bind(id).bind(resource).execute(pool).await.unwrap();
}

fn patch_request(id: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(format!("/fhir/Patient/{id}"))
        .header("content-type", "application/json-patch+json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"[{"op":"replace","path":"/active","value":false}]"#,
        ))
        .unwrap()
}

#[tokio::test]
async fn failed_success_audits_roll_back_all_standalone_mutations() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, TENANT).await;
    // This trigger is intentionally tenant-scoped, so parallel tests retain
    // their normal audit behavior. It simulates an unavailable audit sink.
    sqlx::query(r#"
        CREATE OR REPLACE FUNCTION fail_audit_atomicity_test() RETURNS trigger AS $$
        BEGIN
            IF NEW.tenant_id = 'audit-atomicity' THEN RAISE EXCEPTION 'forced audit failure'; END IF;
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
    "#).execute(&pool).await.unwrap();
    sqlx::query("DROP TRIGGER IF EXISTS fail_audit_atomicity_test ON audit_events")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_audit_atomicity_test BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION fail_audit_atomicity_test()")
        .execute(&pool).await.unwrap();

    let app = build_test_app(pool.clone());
    let token = tenant_token(TENANT);
    let new_patient = json!({"resourceType":"Patient", "active":true});

    let (status, _) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &new_patient, &token),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(count_resources(&pool, TENANT).await, 0);
    assert_eq!(count_history_entries(&pool, TENANT).await, 0);

    let (status, _) = send_request(
        app.clone(),
        post_resource_conditional(
            "Patient",
            &json!({"resourceType":"Patient", "identifier":[{"system":"urn:test", "value":"new"}]}),
            &token,
            "identifier=urn:test|new",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(count_resources(&pool, TENANT).await, 0);

    // A conditional-create match has no resource write, but its success audit
    // is still part of the response contract and must be durable first.
    seed_patient(&pool, "existing").await;
    let (status, _) = send_request(
        app.clone(),
        post_resource_conditional(
            "Patient",
            &json!({"resourceType":"Patient", "id":"ignored"}),
            &token,
            "identifier=urn:audit-atomicity|existing",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(count_resources(&pool, TENANT).await, 1);

    for (id, request) in [
        (
            "update",
            put_resource_with_token_if_match(
                "Patient",
                "update",
                &json!({"resourceType":"Patient", "id":"update", "active":false}),
                &token,
                "W/\"1\"",
            ),
        ),
        ("patch", patch_request("patch", &token)),
        (
            "delete",
            delete_resource_with_token("Patient", "delete", &token),
        ),
    ] {
        seed_patient(&pool, id).await;
        let history_before = count_history_entries(&pool, TENANT).await;
        let (status, _) = send_request(app.clone(), request).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{id}");
        assert_eq!(
            count_history_entries(&pool, TENANT).await,
            history_before,
            "{id}"
        );
    }

    sqlx::query("DROP TRIGGER IF EXISTS fail_audit_atomicity_test ON audit_events")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION IF EXISTS fail_audit_atomicity_test()")
        .execute(&pool)
        .await
        .unwrap();
    clean_tenant(&pool, TENANT).await;
}
