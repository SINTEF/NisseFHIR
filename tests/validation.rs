//! Integration tests for FHIR schema validation.
//!
//! These tests exercise the JSON Schema validation layer with
//! realistic and deliberately-broken FHIR payloads to ensure the server
//! correctly accepts valid resources and rejects invalid ones with
//! meaningful OperationOutcome responses.

mod common;

use axum::http::StatusCode;
use common::{
    assert_operation_outcome, build_test_app_auth_required, post_resource_with_token, send_request,
    setup_test_db, tenant_token, test_data,
};
use serde_json::json;

async fn setup(tenant: &str) -> (axum::Router, String) {
    let pool = setup_test_db().await;
    (build_test_app_auth_required(pool), tenant_token(tenant))
}

// ─── VALID RESOURCES ARE ACCEPTED ──────────────────────────────────────────

#[tokio::test]
async fn accepts_minimal_patient() {
    let (app, token) = setup("validation-minimal-patient").await;
    let (status, _) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn accepts_comprehensive_patient() {
    let (app, token) = setup("validation-comprehensive-patient").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_peter_chalmers(), &token),
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
    let (app, token) = setup("validation-infant-patient").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_infant(), &token),
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
    let (app, token) = setup("validation-blood-glucose").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token(
            "Observation",
            &test_data::observation_blood_glucose(),
            &token,
        ),
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
    let (app, token) = setup("validation-blood-pressure").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token(
            "Observation",
            &test_data::observation_blood_pressure(),
            &token,
        ),
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
    let (app, token) = setup("validation-organization").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token("Organization", &test_data::organization_hl7(), &token),
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
    let (app, token) = setup("validation-practitioner").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token("Practitioner", &test_data::practitioner_example(), &token),
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
    let (app, token) = setup("validation-encounter").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token("Encounter", &test_data::encounter_example(), &token),
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
    let (app, token) = setup("validation-condition").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token("Condition", &test_data::condition_example(), &token),
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
    let (app, token) = setup("validation-procedure").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token("Procedure", &test_data::procedure_example(), &token),
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
    let (app, token) = setup("validation-diagnostic-report").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token(
            "DiagnosticReport",
            &test_data::diagnostic_report_example(),
            &token,
        ),
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
    let (app, token) = setup("validation-extra-property").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_with_extra_property(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_observation_with_invalid_status_type() {
    let (app, token) = setup("validation-invalid-status").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token(
            "Observation",
            &test_data::observation_invalid_status(),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_patient_with_invalid_gender_type() {
    let (app, token) = setup("validation-invalid-gender").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_invalid_gender(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_patient_with_wrong_type_birthdate() {
    let (app, token) = setup("validation-wrong-birthdate").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token(
            "Patient",
            &test_data::patient_wrong_type_birthdate(),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_patient_with_invalid_calendar_birthdate() {
    let (app, token) = setup("validation-invalid-calendar-date").await;
    let patient = json!({
        "resourceType": "Patient",
        "birthDate": "2024-02-30"
    });

    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_patient_with_invalid_positive_int_extension() {
    let (app, token) = setup("validation-invalid-positive-int").await;
    let patient = json!({
        "resourceType": "Patient",
        "extension": [{
            "url": "http://example.org/fhir/StructureDefinition/test-positive-int",
            "valuePositiveInt": 0
        }]
    });

    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_patient_with_invalid_identifier_system_uri() {
    let (app, token) = setup("validation-invalid-uri").await;
    let patient = json!({
        "resourceType": "Patient",
        "identifier": [{
            "system": "http://[::1",
            "value": "12345"
        }]
    });

    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_patient_with_contact_point_value_without_system() {
    let (app, token) = setup("validation-contact-point-system").await;
    let patient = json!({
        "resourceType": "Patient",
        "telecom": [{
            "value": "555-0100"
        }]
    });

    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_observation_with_quantity_code_without_system() {
    let (app, token) = setup("validation-quantity-system").await;
    let observation = json!({
        "resourceType": "Observation",
        "status": "final",
        "code": {
            "coding": [{
                "system": "http://loinc.org",
                "code": "15074-8"
            }]
        },
        "valueQuantity": {
            "value": 6.3,
            "code": "mmol/L"
        }
    });

    let (status, body) = send_request(
        app,
        post_resource_with_token("Observation", &observation, &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_observation_with_effective_period_start_after_end() {
    let (app, token) = setup("validation-period-order").await;
    let observation = json!({
        "resourceType": "Observation",
        "status": "final",
        "code": {
            "coding": [{
                "system": "http://loinc.org",
                "code": "15074-8"
            }]
        },
        "effectivePeriod": {
            "start": "2024-03-02T10:00:00Z",
            "end": "2024-03-01T10:00:00Z"
        }
    });

    let (status, body) = send_request(
        app,
        post_resource_with_token("Observation", &observation, &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_unsupported_resource_type() {
    let (app, token) = setup("validation-unsupported-type").await;
    let resource = json!({
        "resourceType": "MadeUpResource",
        "id": "bogus"
    });
    let (status, body) = send_request(
        app,
        post_resource_with_token("MadeUpResource", &resource, &token),
    )
    .await;
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
    let (app, token) = setup("validation-missing-type").await;
    let (status, body) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::empty_object(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_resource_type_mismatch() {
    let (app, token) = setup("validation-type-mismatch").await;
    let obs = test_data::minimal_observation();
    let (status, body) = send_request(app, post_resource_with_token("Patient", &obs, &token)).await;
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
    let (app, token) = setup("validation-malformed-json").await;

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/fhir/Patient")
        .header("content-type", "application/json")
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(axum::body::Body::from("not json at all"))
        .unwrap();

    let (status, body) = send_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_truncated_json() {
    let (app, token) = setup("validation-truncated-json").await;

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/fhir/Patient")
        .header("content-type", "application/json")
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(axum::body::Body::from(r#"{"resourceType": "Patient", "id"#))
        .unwrap();

    let (status, body) = send_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

#[tokio::test]
async fn rejects_empty_body() {
    let (app, token) = setup("validation-empty-body").await;

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/fhir/Patient")
        .header("content-type", "application/json")
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(axum::body::Body::empty())
        .unwrap();

    let (status, body) = send_request(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_operation_outcome(&body, "invalid");
}

// ─── VALIDATION DIAGNOSTICS QUALITY ────────────────────────────────────────

#[tokio::test]
async fn validation_error_includes_diagnostics() {
    let (app, token) = setup("validation-diagnostics").await;
    let (_, body) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_with_extra_property(), &token),
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
    let (app, token) = setup("validation-multiple-errors").await;
    let bad_patient = json!({
        "resourceType": "Patient",
        "id": "multi-error",
        "active": "not-a-bool",
        "bogusField": true
    });
    let (status, body) = send_request(
        app,
        post_resource_with_token("Patient", &bad_patient, &token),
    )
    .await;
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
    let (app, token) = setup("validation-case-insensitive-path").await;
    let resource = json!({
        "resourceType": "patient",
        "id": "case-test"
    });
    let (status, _) =
        send_request(app, post_resource_with_token("Patient", &resource, &token)).await;
    assert!(
        status == StatusCode::CREATED || status == StatusCode::BAD_REQUEST,
        "Should either accept or fail schema validation, not 500"
    );
}
