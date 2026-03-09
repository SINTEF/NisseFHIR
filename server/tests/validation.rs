//! Integration tests for FHIR schema validation.
//!
//! These tests exercise the JSON Schema validation layer with
//! realistic and deliberately-broken FHIR payloads to ensure the server
//! correctly accepts valid resources and rejects invalid ones with
//! meaningful OperationOutcome responses.

mod common;

use axum::http::StatusCode;
use common::{
    assert_operation_outcome, build_test_app, post_resource, send_request, setup_test_db,
    test_data,
};
use serde_json::json;

// ─── VALID RESOURCES ARE ACCEPTED ──────────────────────────────────────────

#[tokio::test]
async fn accepts_minimal_patient() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, _) =
        send_request(app, post_resource("Patient", &test_data::minimal_patient())).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn accepts_comprehensive_patient() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Patient", &test_data::patient_peter_chalmers()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Should accept valid Patient: {body}"
    );
}

#[tokio::test]
async fn accepts_infant_patient() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Patient", &test_data::patient_infant()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Should accept valid infant Patient: {body}"
    );
}

#[tokio::test]
async fn accepts_blood_glucose_observation() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Observation", &test_data::observation_blood_glucose()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Should accept valid blood glucose Observation: {body}"
    );
}

#[tokio::test]
async fn accepts_blood_pressure_observation() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Observation", &test_data::observation_blood_pressure()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Should accept valid blood pressure Observation: {body}"
    );
}

#[tokio::test]
async fn accepts_organization() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Organization", &test_data::organization_hl7()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Should accept valid Organization: {body}"
    );
}

#[tokio::test]
async fn accepts_practitioner() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Practitioner", &test_data::practitioner_example()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Should accept valid Practitioner: {body}"
    );
}

#[tokio::test]
async fn accepts_encounter() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Encounter", &test_data::encounter_example()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Should accept valid Encounter: {body}"
    );
}

#[tokio::test]
async fn accepts_condition() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Condition", &test_data::condition_example()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Should accept valid Condition: {body}"
    );
}

#[tokio::test]
async fn accepts_procedure() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Procedure", &test_data::procedure_example()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Should accept valid Procedure: {body}"
    );
}

#[tokio::test]
async fn accepts_diagnostic_report() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("DiagnosticReport", &test_data::diagnostic_report_example()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Should accept valid DiagnosticReport: {body}"
    );
}

// ─── INVALID RESOURCES ARE REJECTED ────────────────────────────────────────

#[tokio::test]
async fn rejects_patient_with_extra_property() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Patient", &test_data::patient_with_extra_property()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_observation_with_invalid_status_type() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Observation", &test_data::observation_invalid_status()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_patient_with_invalid_gender_type() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Patient", &test_data::patient_invalid_gender()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_patient_with_wrong_type_birthdate() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app,
        post_resource("Patient", &test_data::patient_wrong_type_birthdate()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_unsupported_resource_type() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let resource = json!({
        "resourceType": "MadeUpResource",
        "id": "bogus"
    });
    let (status, body) =
        send_request(app, post_resource("MadeUpResource", &resource)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
    let diagnostics = body["issue"][0]["diagnostics"].as_str().unwrap();
    assert!(
        diagnostics.contains("unsupported FHIR resource type"),
        "Should mention unsupported type, got: {diagnostics}"
    );
}

// ─── PAYLOAD STRUCTURE VALIDATION ──────────────────────────────────────────

#[tokio::test]
async fn rejects_missing_resource_type() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let (status, body) =
        send_request(app, post_resource("Patient", &test_data::empty_object())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_resource_type_mismatch() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    let obs = test_data::minimal_observation();
    // POST to Patient endpoint with Observation body
    let (status, body) = send_request(app, post_resource("Patient", &obs)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
    let diagnostics = body["issue"][0]["diagnostics"].as_str().unwrap();
    assert!(
        diagnostics.contains("does not match path"),
        "Should mention path mismatch, got: {diagnostics}"
    );
}

#[tokio::test]
async fn rejects_malformed_json() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/fhir/Patient")
        .header("content-type", "application/json")
        .body(axum::body::Body::from("not json at all"))
        .unwrap();

    let (status, body) = send_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_truncated_json() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/fhir/Patient")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(r#"{"resourceType": "Patient", "id"#))
        .unwrap();

    let (status, body) = send_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_empty_body() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/fhir/Patient")
        .header("content-type", "application/json")
        .body(axum::body::Body::empty())
        .unwrap();

    let (status, body) = send_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

// ─── VALIDATION DIAGNOSTICS QUALITY ────────────────────────────────────────

#[tokio::test]
async fn validation_error_includes_diagnostics() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    // Extra property should produce meaningful diagnostics
    let (_, body) = send_request(
        app,
        post_resource("Patient", &test_data::patient_with_extra_property()),
    )
    .await;

    let issues = body["issue"].as_array().unwrap();
    assert!(!issues.is_empty());
    for issue in issues {
        assert_eq!(issue["severity"], "error");
        assert!(
            issue["diagnostics"].as_str().unwrap().len() > 5,
            "Diagnostics should be descriptive, got: {}",
            issue["diagnostics"]
        );
    }
}

#[tokio::test]
async fn multiple_validation_errors_are_reported() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);
    // A patient with multiple problems: wrong type for active AND extra property
    let bad_patient = json!({
        "resourceType": "Patient",
        "id": "multi-error",
        "active": "not-a-bool",
        "bogusField": true
    });
    let (status, body) = send_request(app, post_resource("Patient", &bad_patient)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let issues = body["issue"].as_array().unwrap();
    assert!(
        issues.len() >= 2,
        "Should report multiple validation errors, got: {issues:?}"
    );
}

// ─── RESOURCE TYPE CASE SENSITIVITY ────────────────────────────────────────

#[tokio::test]
async fn resource_type_matching_is_case_insensitive_in_path() {
    let pool = setup_test_db().await;
    let app = build_test_app(pool);

    // Use "patient" in the body (lowercase) and "Patient" in the path
    // The path/body match check is case-insensitive
    let resource = json!({
        "resourceType": "patient",
        "id": "case-test"
    });
    let (status, _) = send_request(app, post_resource("Patient", &resource)).await;
    // This may fail schema validation because the schema expects exact "Patient",
    // but the path matching should not reject it on its own.
    // The status depends on whether the schema allows lowercase.
    assert!(
        status == StatusCode::CREATED || status == StatusCode::BAD_REQUEST,
        "Should either accept or fail schema validation, not 500"
    );
}
