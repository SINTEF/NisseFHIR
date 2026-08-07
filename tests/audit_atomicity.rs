//! A failing success-audit insert must roll back the accompanying mutation.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use common::{
    build_test_app, clean_tenant, count_history_entries, count_resources,
    delete_resource_with_token, post_resource_conditional, post_resource_with_token,
    put_resource_with_token_if_match, send_request, setup_test_db, tenant_token,
};
use serde_json::json;
use sqlx::Row;

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

fn bundle_request(bundle: serde_json::Value, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/fhir")
        .header("content-type", "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(bundle.to_string()))
        .unwrap()
}

#[tokio::test]
async fn bundle_audits_link_children_and_summarize_mixed_batches() {
    let pool = setup_test_db().await;
    let tenant = "bundle-audit-linkage";
    clean_tenant(&pool, tenant).await;
    sqlx::query("DELETE FROM audit_events WHERE tenant_id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let transaction = json!({"resourceType":"Bundle","type":"transaction","entry":[
        {"resource":{"resourceType":"Patient","active":true},"request":{"method":"POST","url":"Patient"}},
        {"resource":{"resourceType":"Patient","active":true},"request":{"method":"POST","url":"Patient"}}
    ]});
    let (status, _) = send_request(app.clone(), bundle_request(transaction, &token)).await;
    assert_eq!(status, StatusCode::OK);
    let rows = sqlx::query("SELECT id, row_kind, entry_index, parent_audit_id, correlation_id FROM audit_events WHERE tenant_id=$1 AND row_kind LIKE 'bundle-%' ORDER BY row_kind, entry_index")
        .bind(tenant).fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 3);
    let parent = rows
        .iter()
        .find(|r| r.get::<String, _>("row_kind") == "bundle-parent")
        .unwrap();
    let correlation: uuid::Uuid = parent.get("correlation_id");
    let children: Vec<_> = rows
        .iter()
        .filter(|r| r.get::<String, _>("row_kind") == "bundle-entry")
        .collect();
    assert_eq!(
        children
            .iter()
            .map(|r| r.get::<i32, _>("entry_index"))
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert!(
        children
            .iter()
            .all(|r| r.get::<uuid::Uuid, _>("parent_audit_id")
                == parent.get::<uuid::Uuid, _>("id"))
    );
    assert!(
        children
            .iter()
            .all(|r| r.get::<uuid::Uuid, _>("correlation_id") == correlation)
    );

    let batch = json!({"resourceType":"Bundle","type":"batch","entry":[
        {"resource":{"resourceType":"Patient","active":true},"request":{"method":"POST","url":"Patient"}},
        {"request":{"method":"GET","url":"Patient/missing"}}
    ]});
    let (status, body) = send_request(app, bundle_request(batch, &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["entry"][1]["response"]["status"], "404 Not Found");
    let parent = sqlx::query("SELECT outcome, reason_code FROM audit_events WHERE tenant_id=$1 AND row_kind='bundle-parent' ORDER BY recorded_at DESC LIMIT 1")
        .bind(tenant).fetch_one(&pool).await.unwrap();
    assert_eq!(parent.get::<String, _>("outcome"), "minor-failure");
    assert_eq!(
        parent.get::<String, _>("reason_code"),
        "bundle-entry-failed"
    );
}

#[tokio::test]
async fn failed_transaction_records_failing_and_rolled_back_children() {
    let pool = setup_test_db().await;
    let tenant = "bundle-audit-rollback";
    clean_tenant(&pool, tenant).await;
    sqlx::query("DELETE FROM audit_events WHERE tenant_id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);
    let bundle = json!({"resourceType":"Bundle","type":"transaction","entry":[
        {"resource":{"resourceType":"Patient","active":true},"request":{"method":"POST","url":"Patient"}},
        {"request":{"method":"GET","url":"Patient/missing"}}
    ]});
    let (status, _) = send_request(app, bundle_request(bundle, &token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(count_resources(&pool, tenant).await, 0);
    let rows = sqlx::query("SELECT entry_index, outcome, reason_code FROM audit_events WHERE tenant_id=$1 AND row_kind='bundle-entry' ORDER BY entry_index")
        .bind(tenant).fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<i32, _>("entry_index"), 0);
    assert_eq!(rows[0].get::<String, _>("reason_code"), "rolled-back");
    assert_eq!(rows[1].get::<i32, _>("entry_index"), 1);
    assert_eq!(rows[1].get::<String, _>("reason_code"), "not-found");
}

#[tokio::test]
async fn batch_child_audit_failure_rolls_back_only_that_entry_and_continues() {
    let pool = setup_test_db().await;
    let tenant = "bundle-audit-child-failure";
    clean_tenant(&pool, tenant).await;
    sqlx::query("DELETE FROM audit_events WHERE tenant_id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
        CREATE OR REPLACE FUNCTION fail_first_bundle_success_audit_test() RETURNS trigger AS $$
        BEGIN
            IF NEW.tenant_id = 'bundle-audit-child-failure'
               AND NEW.row_kind = 'bundle-entry'
               AND NEW.entry_index = 0
               AND NEW.outcome = 'success'
            THEN RAISE EXCEPTION 'forced child audit failure'; END IF;
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
    "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DROP TRIGGER IF EXISTS fail_first_bundle_success_audit_test ON audit_events")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_first_bundle_success_audit_test BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION fail_first_bundle_success_audit_test()")
        .execute(&pool).await.unwrap();

    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);
    let bundle = json!({"resourceType":"Bundle","type":"batch","entry":[
        {"resource":{"resourceType":"Patient","active":true},"request":{"method":"POST","url":"Patient"}},
        {"resource":{"resourceType":"Patient","active":true},"request":{"method":"POST","url":"Patient"}}
    ]});
    let (status, body) = send_request(app, bundle_request(bundle, &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["entry"][0]["response"]["status"],
        "500 Internal Server Error"
    );
    assert_eq!(body["entry"][1]["response"]["status"], "201 Created");
    assert_eq!(count_resources(&pool, tenant).await, 1);
    let rows = sqlx::query("SELECT entry_index, outcome, http_status FROM audit_events WHERE tenant_id=$1 AND row_kind='bundle-entry' ORDER BY entry_index")
        .bind(tenant).fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<i16, _>("http_status"), 500);
    assert_eq!(rows[0].get::<String, _>("outcome"), "serious-failure");
    assert_eq!(rows[1].get::<String, _>("outcome"), "success");
    let parent = sqlx::query(
        "SELECT outcome FROM audit_events WHERE tenant_id=$1 AND row_kind='bundle-parent'",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(parent.get::<String, _>("outcome"), "serious-failure");

    sqlx::query("DROP TRIGGER IF EXISTS fail_first_bundle_success_audit_test ON audit_events")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION IF EXISTS fail_first_bundle_success_audit_test()")
        .execute(&pool)
        .await
        .unwrap();
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
