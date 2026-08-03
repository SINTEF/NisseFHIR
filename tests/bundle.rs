mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::{Value, json};

use common::{
    build_test_app, clean_tenant, count_resources, get_resource_with_token,
    post_resource_with_token, read_only_token, restricted_token, send_request, setup_test_db,
    tenant_token, write_only_token,
};

fn bundle_request(body: &Value, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/fhir")
        .header("content-type", "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .expect("request should build")
}

// ──────────────────────────────────────────────────────────────────
// Transaction tests
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn transaction_creates_multiple_resources_atomically() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-create";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "tx-pat-1",
                    "name": [{"family": "Smith"}]
                },
                "request": {
                    "method": "POST",
                    "url": "Patient"
                }
            },
            {
                "resource": {
                    "resourceType": "Observation",
                    "id": "tx-obs-1",
                    "status": "final",
                    "code": {"coding": [{"system": "http://loinc.org", "code": "1234-5"}]},
                    "subject": {"reference": "Patient/tx-pat-1"}
                },
                "request": {
                    "method": "POST",
                    "url": "Observation"
                }
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["resourceType"], "Bundle");
    assert_eq!(body["type"], "transaction-response");
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["response"]["status"], "201 Created");
    assert_eq!(entries[1]["response"]["status"], "201 Created");
    let patient_id = entries[0]["resource"]["id"].as_str().unwrap();
    let observation_id = entries[1]["resource"]["id"].as_str().unwrap();
    assert_ne!(patient_id, "tx-pat-1");
    assert_ne!(observation_id, "tx-obs-1");

    // Verify both resources exist
    let (s, _) = send_request(
        app.clone(),
        get_resource_with_token("Patient", patient_id, &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let (s, _) = send_request(
        app,
        get_resource_with_token("Observation", observation_id, &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn transaction_rolls_back_on_failure() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-rollback";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "tx-rollback-pat",
                    "name": [{"family": "Jones"}]
                },
                "request": {
                    "method": "POST",
                    "url": "Patient"
                }
            },
            {
                "request": {
                    "method": "PUT",
                    "url": "Observation/nonexist-obs-999"
                }
                // Missing resource — should fail
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(count_resources(&pool, tenant).await, 0);

    // Patient should NOT exist because transaction was rolled back
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", "tx-rollback-pat", &token),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn transaction_with_put_updates_existing() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-put";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    // First create a Patient
    let create_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "id": "tx-put-pat",
                "name": [{"family": "Original"}]
            },
            "request": {
                "method": "POST",
                "url": "Patient"
            }
        }]
    });
    let (status, created) = send_request(app.clone(), bundle_request(&create_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["entry"][0]["resource"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Now update via PUT in a transaction
    let update_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "id": id,
                "name": [{"family": "Updated"}]
            },
            "request": {
                "method": "PUT",
                "url": format!("Patient/{id}")
            }
        }]
    });
    let (status, body) = send_request(app.clone(), bundle_request(&update_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "200 OK");
    assert_eq!(body["entry"][0]["response"]["etag"], "W/\"2\"");
    assert_eq!(
        body["entry"][0]["response"]["location"],
        format!("Patient/{id}/_history/2")
    );

    // Read back to verify update
    let (s, patient) = send_request(app, get_resource_with_token("Patient", &id, &token)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(patient["name"][0]["family"], "Updated");
}

#[tokio::test]
async fn transaction_put_creates_missing_resource() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-put-create";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "id": "tx-client-id",
                "active": true
            },
            "request": {"method": "PUT", "url": "Patient/tx-client-id"}
        }]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "201 Created");
    assert_eq!(body["entry"][0]["response"]["etag"], "W/\"1\"");
    assert_eq!(
        body["entry"][0]["response"]["location"],
        "Patient/tx-client-id/_history/1"
    );

    let (status, patient) = send_request(
        app,
        get_resource_with_token("Patient", "tx-client-id", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(patient["active"], true);
}

#[tokio::test]
async fn transaction_put_with_if_match_does_not_create_missing_resource() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-put-if-match-missing";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {"resourceType": "Patient", "id": "tx-if-match-missing"},
            "request": {
                "method": "PUT",
                "url": "Patient/tx-if-match-missing",
                "ifMatch": "W/\"1\""
            }
        }]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "body: {body}");
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(count_resources(&pool, tenant).await, 0);
}

#[tokio::test]
async fn transaction_rolls_back_put_create_when_later_entry_fails() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-put-create-rollback";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "resource": {"resourceType": "Patient", "id": "rolled-back-put"},
                "request": {"method": "PUT", "url": "Patient/rolled-back-put"}
            },
            {
                "request": {"method": "PUT", "url": "Observation/missing-body"}
            }
        ]
    });

    let (status, _) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(count_resources(&pool, tenant).await, 0);

    let (status, _) = send_request(
        app,
        get_resource_with_token("Patient", "rolled-back-put", &token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn transaction_with_delete_removes_resource() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-delete";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    // Create a Patient first
    let create_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "id": "tx-del-pat",
                "name": [{"family": "ToDelete"}]
            },
            "request": {
                "method": "POST",
                "url": "Patient"
            }
        }]
    });
    let (status, created) = send_request(app.clone(), bundle_request(&create_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["entry"][0]["resource"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Delete via transaction
    let delete_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "request": {
                "method": "DELETE",
                "url": format!("Patient/{id}")
            }
        }]
    });
    let (status, body) = send_request(app.clone(), bundle_request(&delete_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "204 No Content");

    // Verify deleted
    let (s, _) = send_request(app, get_resource_with_token("Patient", &id, &token)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn transaction_with_get_reads_resource() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-get";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let create_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "id": "tx-get-pat",
                "name": [{"family": "Readable"}]
            },
            "request": {"method": "POST", "url": "Patient"}
        }]
    });
    let (status, created) = send_request(app.clone(), bundle_request(&create_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["entry"][0]["resource"]["id"].as_str().unwrap();

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "request": {"method": "GET", "url": format!("Patient/{id}")}
        }]
    });
    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries[0]["response"]["status"], "200 OK");
    assert_eq!(entries[0]["resource"]["name"][0]["family"], "Readable");
}

// ──────────────────────────────────────────────────────────────────
// Batch tests
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_creates_multiple_resources_independently() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-create";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "batch-pat-1",
                    "name": [{"family": "BatchOne"}]
                },
                "request": {
                    "method": "POST",
                    "url": "Patient"
                }
            },
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "batch-pat-2",
                    "name": [{"family": "BatchTwo"}]
                },
                "request": {
                    "method": "POST",
                    "url": "Patient"
                }
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["resourceType"], "Bundle");
    assert_eq!(body["type"], "batch-response");
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["response"]["status"], "201 Created");
    assert_eq!(entries[1]["response"]["status"], "201 Created");
}

#[tokio::test]
async fn batch_continues_on_individual_failure() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-partial";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "batch-ok-pat",
                    "name": [{"family": "Good"}]
                },
                "request": {
                    "method": "POST",
                    "url": "Patient"
                }
            },
            {
                // Missing resource for PUT - should produce inline error
                "request": {
                    "method": "PUT",
                    "url": "Observation/no-resource"
                }
            },
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "batch-ok-pat-2",
                    "name": [{"family": "AlsoGood"}]
                },
                "request": {
                    "method": "POST",
                    "url": "Patient"
                }
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    // First and third succeed
    assert_eq!(entries[0]["response"]["status"], "201 Created");
    assert_eq!(entries[2]["response"]["status"], "201 Created");
    // Second failed with OperationOutcome inline
    assert_eq!(entries[1]["response"]["status"], "400 Bad Request");
    assert_eq!(
        entries[1]["response"]["outcome"]["resourceType"],
        "OperationOutcome"
    );

    // Good patient should exist despite the middle failure
    let (s, _) = send_request(
        app,
        get_resource_with_token(
            "Patient",
            entries[0]["resource"]["id"].as_str().unwrap(),
            &token,
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn batch_put_creates_missing_resource_and_isolates_failure() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-put-create";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [
            {
                "resource": {"resourceType": "Patient", "id": "batch-client-id"},
                "request": {"method": "PUT", "url": "Patient/batch-client-id"}
            },
            {
                "request": {"method": "PUT", "url": "Observation/missing-body"}
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "201 Created");
    assert_eq!(body["entry"][0]["response"]["etag"], "W/\"1\"");
    assert_eq!(body["entry"][1]["response"]["status"], "400 Bad Request");
    assert_eq!(count_resources(&pool, tenant).await, 1);

    let (status, patient) = send_request(
        app,
        get_resource_with_token("Patient", "batch-client-id", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(patient["id"], "batch-client-id");
}

#[tokio::test]
async fn batch_put_with_if_match_does_not_create_missing_resource() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-put-if-match-missing";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [{
            "resource": {"resourceType": "Patient", "id": "batch-if-match-missing"},
            "request": {
                "method": "PUT",
                "url": "Patient/batch-if-match-missing",
                "ifMatch": "W/\"1\""
            }
        }]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(
        body["entry"][0]["response"]["status"],
        "412 Precondition Failed"
    );
    assert_eq!(
        body["entry"][0]["response"]["outcome"]["resourceType"],
        "OperationOutcome"
    );
    assert_eq!(count_resources(&pool, tenant).await, 0);
}

// ──────────────────────────────────────────────────────────────────
// Validation and auth tests
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn bundle_rejects_non_bundle_resource_type() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let token = tenant_token("bundle-val");

    let body = json!({
        "resourceType": "Patient",
        "name": [{"family": "NotABundle"}]
    });

    let (status, body) = send_request(app, bundle_request(&body, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn bundle_rejects_unsupported_bundle_type() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let token = tenant_token("bundle-val");

    let body = json!({
        "resourceType": "Bundle",
        "type": "searchset"
    });

    let (status, body) = send_request(app, bundle_request(&body, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn bundle_requires_write_scope() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let token = read_only_token("bundle-readonly");

    let body = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": []
    });

    let (status, body) = send_request(app, bundle_request(&body, &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn bundle_empty_entries_returns_empty_response() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let token = tenant_token("bundle-empty");

    let body = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": []
    });

    let (status, body) = send_request(app, bundle_request(&body, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["type"], "transaction-response");
    assert_eq!(body["entry"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn batch_empty_entries_returns_empty_response() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let token = tenant_token("bundle-empty-batch");

    let body = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": []
    });

    let (status, body) = send_request(app, bundle_request(&body, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["type"], "batch-response");
    assert_eq!(body["entry"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn transaction_entry_missing_request_fails() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let token = tenant_token("bundle-no-request");

    let body = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "id": "no-req-pat"
            }
        }]
    });

    let (status, body) = send_request(app, bundle_request(&body, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn transaction_validates_resource_schema() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-schema";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool);
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "id": "schema-fail-pat",
                "birthDate": "not-a-date"
            },
            "request": {
                "method": "POST",
                "url": "Patient"
            }
        }]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn transaction_mixed_operations() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-mixed";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    // Create two patients first
    let setup = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "mix-keep",
                    "name": [{"family": "Keep"}]
                },
                "request": { "method": "POST", "url": "Patient" }
            },
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "mix-delete",
                    "name": [{"family": "Delete"}]
                },
                "request": { "method": "POST", "url": "Patient" }
            }
        ]
    });
    let (s, setup_response) = send_request(app.clone(), bundle_request(&setup, &token)).await;
    assert_eq!(s, StatusCode::OK);
    let keep_id = setup_response["entry"][0]["resource"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let delete_id = setup_response["entry"][1]["resource"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Now do a mixed transaction: update one, delete the other, create a new one
    let mixed = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": keep_id,
                    "name": [{"family": "Updated"}]
                },
                "request": { "method": "PUT", "url": format!("Patient/{keep_id}") }
            },
            {
                "request": { "method": "DELETE", "url": format!("Patient/{delete_id}") }
            },
            {
                "resource": {
                    "resourceType": "Observation",
                    "id": "mix-obs",
                    "status": "final",
                    "code": {"coding": [{"system": "http://loinc.org", "code": "5678-9"}]}
                },
                "request": { "method": "POST", "url": "Observation" }
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&mixed, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["response"]["status"], "200 OK");
    assert_eq!(entries[1]["response"]["status"], "204 No Content");
    assert_eq!(entries[2]["response"]["status"], "201 Created");
    let observation_id = entries[2]["resource"]["id"].as_str().unwrap();

    // Verify state
    let (s, pat) = send_request(
        app.clone(),
        get_resource_with_token("Patient", &keep_id, &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(pat["name"][0]["family"], "Updated");

    let (s, _) = send_request(
        app.clone(),
        get_resource_with_token("Patient", &delete_id, &token),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, _) = send_request(
        app,
        get_resource_with_token("Observation", observation_id, &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

// ──────────────────────────────────────────────────────────────────
// Authorization tests: per-entry resource-type and interaction checks
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn transaction_rolls_back_when_entry_resource_type_forbidden() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-authz";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = restricted_token(tenant, vec!["Patient".to_owned()]);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "tx-authz-pat",
                    "name": [{"family": "Allowed"}]
                },
                "request": { "method": "POST", "url": "Patient" }
            },
            {
                "resource": {
                    "resourceType": "Observation",
                    "id": "tx-authz-obs",
                    "status": "final",
                    "code": {"coding": [{"system": "http://loinc.org", "code": "1234-5"}]}
                },
                "request": { "method": "POST", "url": "Observation" }
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {body}");
    assert_eq!(body["resourceType"], "OperationOutcome");

    // The whole transaction must be rolled back: the allowed Patient must not exist.
    let full_token = tenant_token(tenant);
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", "tx-authz-pat", &full_token),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn transaction_rejects_get_entry_for_write_only_token() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-writeonly";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = write_only_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "request": { "method": "GET", "url": "Patient/tx-wo-pat" }
        }]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {body}");
    assert_eq!(body["resourceType"], "OperationOutcome");
}

#[tokio::test]
async fn batch_returns_403_for_forbidden_entries_and_continues() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-authz";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = restricted_token(tenant, vec!["Patient".to_owned()]);
    let seed = json!({
        "resourceType": "Patient",
        "id": "batch-authz-seed",
        "name": [{"family": "Seed"}]
    });
    let (seed_status, seeded) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &seed, &token),
    )
    .await;
    assert_eq!(seed_status, StatusCode::CREATED);
    let patient_id = seeded["id"].as_str().unwrap();

    let observation = json!({
        "resourceType": "Observation",
        "id": "batch-authz-obs",
        "status": "final",
        "code": {"coding": [{"system": "http://loinc.org", "code": "1234-5"}]}
    });

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [
            // Allowed Patient entries covering all four methods.
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "batch-authz-pat",
                    "name": [{"family": "Allowed"}]
                },
                "request": { "method": "POST", "url": "Patient" }
            },
            {
                "request": { "method": "GET", "url": format!("Patient/{patient_id}") }
            },
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": patient_id,
                    "name": [{"family": "Updated"}]
                },
                "request": { "method": "PUT", "url": format!("Patient/{patient_id}") }
            },
            // Forbidden Observation entries covering all four methods.
            {
                "resource": observation,
                "request": { "method": "POST", "url": "Observation" }
            },
            {
                "request": { "method": "GET", "url": "Observation/batch-authz-obs" }
            },
            {
                "resource": observation,
                "request": { "method": "PUT", "url": "Observation/batch-authz-obs" }
            },
            {
                "request": { "method": "DELETE", "url": "Observation/batch-authz-obs" }
            },
            // Allowed delete of the Patient created above.
            {
                "request": { "method": "DELETE", "url": format!("Patient/{patient_id}") }
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries.len(), 8);

    // Allowed Patient entries succeed.
    assert_eq!(entries[0]["response"]["status"], "201 Created");
    assert_eq!(entries[1]["response"]["status"], "200 OK");
    assert_eq!(entries[2]["response"]["status"], "200 OK");
    assert_eq!(
        entries[2]["response"]["location"],
        format!("Patient/{patient_id}/_history/2")
    );
    assert_eq!(entries[7]["response"]["status"], "204 No Content");

    // Forbidden Observation entries get an inline 403 OperationOutcome.
    for i in [3, 4, 5, 6] {
        assert_eq!(
            entries[i]["response"]["status"], "403 Forbidden",
            "entry {i} must be forbidden"
        );
        assert_eq!(
            entries[i]["response"]["outcome"]["resourceType"],
            "OperationOutcome"
        );
    }

    let full_token = tenant_token(tenant);

    // The forbidden Observation must never have been persisted.
    let (s, _) = send_request(
        app.clone(),
        get_resource_with_token("Observation", "batch-authz-obs", &full_token),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // The allowed Patient was deleted by the final entry.
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", patient_id, &full_token),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn batch_get_entry_requires_read_scope() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-writeonly";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = write_only_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "batch-wo-pat",
                    "name": [{"family": "WriteOnly"}]
                },
                "request": { "method": "POST", "url": "Patient" }
            },
            {
                "request": { "method": "GET", "url": "Patient/batch-wo-pat" }
            }
        ]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries[0]["response"]["status"], "201 Created");
    assert_eq!(entries[1]["response"]["status"], "403 Forbidden");
    assert_eq!(
        entries[1]["response"]["outcome"]["resourceType"],
        "OperationOutcome"
    );
}

// ──────────────────────────────────────────────────────────────────
// Accurate entry status coverage (400/403/404/412) in batch & transaction
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_get_missing_resource_returns_inline_404() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-404";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [{
            "request": { "method": "GET", "url": "Patient/does-not-exist" }
        }]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entry = &body["entry"][0];
    assert_eq!(entry["response"]["status"], "404 Not Found");
    assert_eq!(
        entry["response"]["outcome"]["resourceType"],
        "OperationOutcome"
    );
    assert_eq!(
        entry["response"]["outcome"]["issue"][0]["code"],
        "not-found"
    );
    assert!(entry.get("resource").is_none());
}

#[tokio::test]
async fn batch_delete_missing_resource_returns_inline_404() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-delete-404";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool);
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [{
            "request": { "method": "DELETE", "url": "Patient/never-existed" }
        }]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entry = &body["entry"][0];
    assert_eq!(entry["response"]["status"], "404 Not Found");
    assert_eq!(
        entry["response"]["outcome"]["issue"][0]["code"],
        "not-found"
    );
}

#[tokio::test]
async fn transaction_rolls_back_with_404_when_entry_resource_missing() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-404";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "tx-404-pat",
                    "name": [{"family": "ShouldRollBack"}]
                },
                "request": { "method": "POST", "url": "Patient" }
            },
            {
                "request": { "method": "GET", "url": "Patient/no-such-resource" }
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(body["issue"][0]["code"], "not-found");
    // Whole transaction rolled back: the Patient must not exist.
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", "tx-404-pat", &token),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn batch_validates_resource_schema_per_entry() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-validation";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "batch-valid-ok",
                    "name": [{"family": "Good"}]
                },
                "request": { "method": "POST", "url": "Patient" }
            },
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "batch-valid-bad",
                    "birthDate": "not-a-date"
                },
                "request": { "method": "POST", "url": "Patient" }
            }
        ]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries[0]["response"]["status"], "201 Created");
    assert_eq!(entries[1]["response"]["status"], "400 Bad Request");
    assert_eq!(
        entries[1]["response"]["outcome"]["resourceType"],
        "OperationOutcome"
    );
    assert!(
        !entries[1]["response"]["outcome"]["issue"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(entries[1].get("resource").is_none());
}

#[tokio::test]
async fn batch_put_ifmatch_missing_returns_inline_412_outcome() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-412";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool);
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [{
            "resource": {"resourceType": "Patient", "id": "batch-412-missing"},
            "request": {
                "method": "PUT",
                "url": "Patient/batch-412-missing",
                "ifMatch": "W/\"1\""
            }
        }]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entry = &body["entry"][0];
    assert_eq!(entry["response"]["status"], "412 Precondition Failed");
    assert_eq!(
        entry["response"]["outcome"]["resourceType"],
        "OperationOutcome"
    );
    assert_eq!(entry["response"]["outcome"]["issue"][0]["code"], "conflict");
    assert!(entry.get("resource").is_none());
}

#[tokio::test]
async fn bundle_response_includes_etag_and_location() {
    let pool = setup_test_db().await;
    let tenant = "bundle-etag";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool);
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "id": "etag-pat",
                "name": [{"family": "ETag"}]
            },
            "request": { "method": "POST", "url": "Patient" }
        }]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entry = &body["entry"][0];
    assert!(
        entry["response"]["etag"]
            .as_str()
            .unwrap()
            .starts_with("W/\"")
    );
    let id = entry["resource"]["id"].as_str().unwrap();
    assert_ne!(id, "etag-pat");
    assert_eq!(
        entry["response"]["location"].as_str().unwrap(),
        format!("Patient/{id}")
    );
    assert!(entry["response"]["lastModified"].as_str().is_some());
}
