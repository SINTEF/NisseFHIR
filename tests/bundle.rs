mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::{Value, json};

use common::{
    build_test_app, clean_tenant, get_resource_with_token, read_only_token, send_request,
    setup_test_db, tenant_token,
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

    // Verify both resources exist
    let (s, _) = send_request(
        app.clone(),
        get_resource_with_token("Patient", "tx-pat-1", &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let (s, _) = send_request(
        app,
        get_resource_with_token("Observation", "tx-obs-1", &token),
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
    let (status, _) = send_request(app.clone(), bundle_request(&create_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK);

    // Now update via PUT in a transaction
    let update_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "id": "tx-put-pat",
                "name": [{"family": "Updated"}]
            },
            "request": {
                "method": "PUT",
                "url": "Patient/tx-put-pat"
            }
        }]
    });
    let (status, body) = send_request(app.clone(), bundle_request(&update_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "200 OK");

    // Read back to verify update
    let (s, patient) = send_request(
        app,
        get_resource_with_token("Patient", "tx-put-pat", &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(patient["name"][0]["family"], "Updated");
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
    let (status, _) = send_request(app.clone(), bundle_request(&create_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK);

    // Delete via transaction
    let delete_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "request": {
                "method": "DELETE",
                "url": "Patient/tx-del-pat"
            }
        }]
    });
    let (status, body) = send_request(app.clone(), bundle_request(&delete_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "204 No Content");

    // Verify deleted
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", "tx-del-pat", &token),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn transaction_with_get_reads_resource() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-get";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    // Create, then read via GET inside a transaction
    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "tx-get-pat",
                    "name": [{"family": "Readable"}]
                },
                "request": {
                    "method": "POST",
                    "url": "Patient"
                }
            },
            {
                "request": {
                    "method": "GET",
                    "url": "Patient/tx-get-pat"
                }
            }
        ]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries[0]["response"]["status"], "201 Created");
    assert_eq!(entries[1]["response"]["status"], "200 OK");
    assert_eq!(entries[1]["resource"]["name"][0]["family"], "Readable");
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
    assert_eq!(entries[1]["resource"]["resourceType"], "OperationOutcome");

    // Good patient should exist despite the middle failure
    let (s, _) = send_request(
        app,
        get_resource_with_token("Patient", "batch-ok-pat", &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
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
    let (s, _) = send_request(app.clone(), bundle_request(&setup, &token)).await;
    assert_eq!(s, StatusCode::OK);

    // Now do a mixed transaction: update one, delete the other, create a new one
    let mixed = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "resource": {
                    "resourceType": "Patient",
                    "id": "mix-keep",
                    "name": [{"family": "Updated"}]
                },
                "request": { "method": "PUT", "url": "Patient/mix-keep" }
            },
            {
                "request": { "method": "DELETE", "url": "Patient/mix-delete" }
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

    // Verify state
    let (s, pat) = send_request(
        app.clone(),
        get_resource_with_token("Patient", "mix-keep", &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(pat["name"][0]["family"], "Updated");

    let (s, _) = send_request(
        app.clone(),
        get_resource_with_token("Patient", "mix-delete", &token),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, _) = send_request(
        app,
        get_resource_with_token("Observation", "mix-obs", &token),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
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
    assert!(
        entry["response"]["location"]
            .as_str()
            .unwrap()
            .contains("Patient/etag-pat")
    );
    assert!(entry["response"]["lastModified"].as_str().is_some());
}
