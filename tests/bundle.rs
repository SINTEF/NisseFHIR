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
async fn transaction_delete_with_matching_if_match_succeeds() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-del-ifmatch-ok";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let create_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {"resourceType": "Patient", "id": "tx-del-ifmatch-pat"},
            "request": {"method": "POST", "url": "Patient"}
        }]
    });
    let (status, created) = send_request(app.clone(), bundle_request(&create_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["entry"][0]["resource"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Delete via transaction with a matching If-Match.
    let delete_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "request": {
                "method": "DELETE",
                "url": format!("Patient/{id}"),
                "ifMatch": "W/\"1\""
            }
        }]
    });
    let (status, body) = send_request(app.clone(), bundle_request(&delete_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "204 No Content");

    let (s, _) = send_request(app, get_resource_with_token("Patient", &id, &token)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn transaction_delete_with_stale_if_match_returns_412_without_deleting() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-del-ifmatch-stale";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let create_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {"resourceType": "Patient", "id": "tx-del-ifmatch-stale-pat"},
            "request": {"method": "POST", "url": "Patient"}
        }]
    });
    let (status, created) = send_request(app.clone(), bundle_request(&create_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["entry"][0]["resource"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Bump the version to 2 via PUT with If-Match.
    let update_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {"resourceType": "Patient", "id": id},
            "request": {"method": "PUT", "url": format!("Patient/{id}"), "ifMatch": "W/\"1\""}
        }]
    });
    let (status, body) = send_request(app.clone(), bundle_request(&update_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "200 OK");

    // Delete with the stale ETag.
    let delete_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "request": {
                "method": "DELETE",
                "url": format!("Patient/{id}"),
                "ifMatch": "W/\"1\""
            }
        }]
    });
    // A stale If-Match delete in a transaction fails and rolls back.
    let (status, body) = send_request(app.clone(), bundle_request(&delete_bundle, &token)).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "body: {body}");
    assert_eq!(body["resourceType"], "OperationOutcome");

    // Resource was not deleted (transaction rolled back).
    assert_eq!(count_resources(&pool, tenant).await, 1);
    let (s, _) = send_request(app, get_resource_with_token("Patient", &id, &token)).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn batch_delete_with_if_match_is_consistent_with_transaction() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-del-ifmatch";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    // Seed two resources so we can exercise both a matching and a stale delete
    // independently (a successful delete removes the resource entirely).
    let create_bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {"resource": {"resourceType": "Patient", "id": "batch-del-ifmatch-ok"}, "request": {"method": "POST", "url": "Patient"}},
            {"resource": {"resourceType": "Patient", "id": "batch-del-ifmatch-stale"}, "request": {"method": "POST", "url": "Patient"}}
        ]
    });
    let (status, created) = send_request(app.clone(), bundle_request(&create_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK);
    let ok_id = created["entry"][0]["resource"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let stale_id = created["entry"][1]["resource"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // A matching If-Match delete in batch mode succeeds.
    let ok_bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [{
            "request": {
                "method": "DELETE",
                "url": format!("Patient/{ok_id}"),
                "ifMatch": "W/\"1\""
            }
        }]
    });
    let (status, body) = send_request(app.clone(), bundle_request(&ok_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "204 No Content");

    // A stale If-Match delete in batch mode reports 412 and does not delete.
    let stale_bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [{
            "request": {
                "method": "DELETE",
                "url": format!("Patient/{stale_id}"),
                "ifMatch": "W/\"2\""
            }
        }]
    });
    let (status, body) = send_request(app.clone(), bundle_request(&stale_bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(
        body["entry"][0]["response"]["status"],
        "412 Precondition Failed"
    );
    assert_eq!(count_resources(&pool, tenant).await, 1);
    let (s, _) = send_request(app, get_resource_with_token("Patient", &stale_id, &token)).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn read_only_token_can_submit_get_only_transaction() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-get";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let write_token = tenant_token(tenant);

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
    let (status, created) =
        send_request(app.clone(), bundle_request(&create_bundle, &write_token)).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["entry"][0]["resource"]["id"].as_str().unwrap();

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "request": {"method": "GET", "url": format!("Patient/{id}")}
        }]
    });
    let read_token = read_only_token(tenant);
    let (status, body) = send_request(app, bundle_request(&bundle, &read_token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries[0]["response"]["status"], "200 OK");
    assert_eq!(entries[0]["resource"]["name"][0]["family"], "Readable");
}

#[tokio::test]
async fn read_only_transaction_rejects_write_entry() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-readonly-write";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = read_only_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "name": [{"family": "Forbidden"}]
            },
            "request": {"method": "POST", "url": "Patient"}
        }]
    });
    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;

    assert_eq!(status, StatusCode::FORBIDDEN, "body: {body}");
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(count_resources(&pool, tenant).await, 0);
}

// ──────────────────────────────────────────────────────────────────
// Batch tests
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn read_only_token_can_submit_get_only_batch() {
    let pool = setup_test_db().await;
    let tenant = "bundle-batch-readonly";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool);

    let patient = json!({
        "resourceType": "Patient",
        "name": [{"family": "BatchReadable"}]
    });
    let write_token = tenant_token(tenant);
    let (status, created) = send_request(
        app.clone(),
        post_resource_with_token("Patient", &patient, &write_token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [{
            "request": {"method": "GET", "url": format!("Patient/{id}")}
        }]
    });
    let read_token = read_only_token(tenant);
    let (status, body) = send_request(app, bundle_request(&bundle, &read_token)).await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "200 OK");
    assert_eq!(body["entry"][0]["resource"]["id"], id);
}

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
async fn read_only_batch_rejects_write_entry_inline() {
    let pool = setup_test_db().await;
    let tenant = "bundle-readonly-write";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = read_only_token(tenant);

    let body = json!({
        "resourceType": "Bundle",
        "type": "batch",
        "entry": [{
            "resource": {
                "resourceType": "Patient",
                "name": [{"family": "Forbidden"}]
            },
            "request": {"method": "POST", "url": "Patient"}
        }]
    });

    let (status, body) = send_request(app, bundle_request(&body, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["entry"][0]["response"]["status"], "403 Forbidden");
    assert_eq!(count_resources(&pool, tenant).await, 0);
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

// ──────────────────────────────────────────────────────────────────
// Transaction internal link resolution
// ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn transaction_resolves_urn_uuid_references_in_both_directions() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-links";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    // The Observation references a Patient defined *after* it, and the Patient
    // references the Observation back, so no entry order resolves this without
    // a planning pass.
    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "fullUrl": "urn:uuid:11111111-1111-1111-1111-111111111111",
                "resource": {
                    "resourceType": "Observation",
                    "status": "final",
                    "code": {"coding": [{"system": "http://loinc.org", "code": "1234-5"}]},
                    "subject": {"reference": "urn:uuid:22222222-2222-2222-2222-222222222222"},
                    "text": {
                        "status": "generated",
                        "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\"><a href=\"urn:uuid:22222222-2222-2222-2222-222222222222\">subject</a></div>"
                    }
                },
                "request": {"method": "POST", "url": "Observation"}
            },
            {
                "fullUrl": "urn:uuid:22222222-2222-2222-2222-222222222222",
                "resource": {
                    "resourceType": "Patient",
                    "name": [{"family": "Linked"}],
                    "generalPractitioner": [
                        {"reference": "urn:uuid:11111111-1111-1111-1111-111111111111#top"},
                        {"reference": "urn:uuid:99999999-9999-9999-9999-999999999999"},
                        {"reference": "http://example.org/fhir/Practitioner/7"}
                    ]
                },
                "request": {"method": "POST", "url": "Patient"}
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let observation_id = body["entry"][0]["resource"]["id"].as_str().unwrap();
    let patient_id = body["entry"][1]["resource"]["id"].as_str().unwrap();

    // Response entries carry the server's identity for each created resource.
    assert_eq!(
        body["entry"][1]["fullUrl"].as_str().unwrap(),
        format!("http://localhost:8080/fhir/Patient/{patient_id}")
    );
    assert_eq!(
        body["entry"][1]["response"]["location"].as_str().unwrap(),
        format!("Patient/{patient_id}")
    );

    // Read both back, to confirm the resolved links were persisted and not
    // merely echoed in the response.
    let (status, stored_observation) = send_request(
        app.clone(),
        get_resource_with_token("Observation", observation_id, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {stored_observation}");
    assert_eq!(
        stored_observation["subject"]["reference"].as_str().unwrap(),
        format!("Patient/{patient_id}")
    );
    assert!(
        stored_observation["text"]["div"]
            .as_str()
            .unwrap()
            .contains(&format!("href=\"Patient/{patient_id}\"")),
        "narrative link should be resolved: {}",
        stored_observation["text"]["div"]
    );

    let (status, stored_patient) =
        send_request(app, get_resource_with_token("Patient", patient_id, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {stored_patient}");
    let practitioners = stored_patient["generalPractitioner"].as_array().unwrap();
    // Backward reference, resolved with its fragment preserved.
    assert_eq!(
        practitioners[0]["reference"].as_str().unwrap(),
        format!("Observation/{observation_id}#top")
    );
    // No matching entry, and an external reference: both stored as sent.
    assert_eq!(
        practitioners[1]["reference"].as_str().unwrap(),
        "urn:uuid:99999999-9999-9999-9999-999999999999"
    );
    assert_eq!(
        practitioners[2]["reference"].as_str().unwrap(),
        "http://example.org/fhir/Practitioner/7"
    );
}

#[tokio::test]
async fn transaction_resolves_absolute_full_url_for_put_entry() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-absolute";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "fullUrl": "http://localhost:8080/fhir/Patient/absolute-pat",
                "resource": {"resourceType": "Patient", "name": [{"family": "Absolute"}]},
                "request": {"method": "PUT", "url": "Patient/absolute-pat"}
            },
            {
                "fullUrl": "urn:uuid:33333333-3333-3333-3333-333333333333",
                "resource": {
                    "resourceType": "Observation",
                    "status": "final",
                    "code": {"coding": [{"system": "http://loinc.org", "code": "1234-5"}]},
                    "subject": {"reference": "http://localhost:8080/fhir/Patient/absolute-pat"}
                },
                "request": {"method": "POST", "url": "Observation"}
            }
        ]
    });

    let (status, body) = send_request(app.clone(), bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let observation_id = body["entry"][1]["resource"]["id"].as_str().unwrap();
    let (status, stored) = send_request(
        app,
        get_resource_with_token("Observation", observation_id, &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {stored}");
    assert_eq!(
        stored["subject"]["reference"].as_str().unwrap(),
        "Patient/absolute-pat"
    );
}

#[tokio::test]
async fn transaction_rejects_duplicate_full_url_before_persisting() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-dup-fullurl";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "fullUrl": "urn:uuid:44444444-4444-4444-4444-444444444444",
                "resource": {"resourceType": "Patient", "name": [{"family": "One"}]},
                "request": {"method": "POST", "url": "Patient"}
            },
            {
                "fullUrl": "urn:uuid:44444444-4444-4444-4444-444444444444",
                "resource": {"resourceType": "Patient", "name": [{"family": "Two"}]},
                "request": {"method": "POST", "url": "Patient"}
            }
        ]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(count_resources(&pool, tenant).await, 0);
}

#[tokio::test]
async fn transaction_rejects_overlapping_target_identities() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-dup-target";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "resource": {"resourceType": "Patient", "name": [{"family": "One"}]},
                "request": {"method": "PUT", "url": "Patient/same-target"}
            },
            {
                "resource": {"resourceType": "Patient", "name": [{"family": "Two"}]},
                "request": {"method": "PUT", "url": "Patient/same-target"}
            }
        ]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
    assert_eq!(count_resources(&pool, tenant).await, 0);
}

#[tokio::test]
async fn transaction_rolls_back_resolved_links_when_a_later_entry_fails() {
    let pool = setup_test_db().await;
    let tenant = "bundle-tx-links-rollback";
    clean_tenant(&pool, tenant).await;
    let app = build_test_app(pool.clone());
    let token = tenant_token(tenant);

    let bundle = json!({
        "resourceType": "Bundle",
        "type": "transaction",
        "entry": [
            {
                "fullUrl": "urn:uuid:55555555-5555-5555-5555-555555555555",
                "resource": {"resourceType": "Patient", "name": [{"family": "Rollback"}]},
                "request": {"method": "POST", "url": "Patient"}
            },
            {
                "request": {"method": "DELETE", "url": "Observation/does-not-exist"}
            }
        ]
    });

    let (status, body) = send_request(app, bundle_request(&bundle, &token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    assert_eq!(count_resources(&pool, tenant).await, 0);
}
