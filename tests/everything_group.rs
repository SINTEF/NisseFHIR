mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use common::{build_test_app, clean_tenant, send_request, setup_test_db, tenant_token};
use fhir_server::{search_params::sql::GeoSearchMode, store::PgStore};
use serde_json::json;

fn get(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn enumerated_person_group_reuses_patient_expansion_and_deduplicates_support() {
    let pool = setup_test_db().await;
    let tenant = "everything-group-basic";
    clean_tenant(&pool, tenant).await;
    let store = PgStore::new(pool.clone(), GeoSearchMode::EarthDistance);
    for id in ["p1", "p2"] {
        store
            .upsert(
                tenant,
                "Patient",
                id,
                json!({"resourceType":"Patient","id":id}),
            )
            .await
            .unwrap();
    }
    store
        .upsert(
            tenant,
            "Practitioner",
            "shared",
            json!({"resourceType":"Practitioner","id":"shared"}),
        )
        .await
        .unwrap();
    for (id, patient) in [("o1", "p1"), ("o2", "p2")] {
        store
            .upsert(
                tenant,
                "Observation",
                id,
                json!({
                    "resourceType":"Observation","id":id,"status":"final","code":{"text":"example"},
                    "subject":{"reference":format!("Patient/{patient}")},
                    "performer":[{"reference":"Practitioner/shared"}]
                }),
            )
            .await
            .unwrap();
    }
    store
        .upsert(
            tenant,
            "Group",
            "g1",
            json!({
                "resourceType":"Group","id":"g1","type":"person","membership":"enumerated",
                "member":[
                    {"entity":{"reference":"Patient/p1"}},
                    {"entity":{"reference":"Patient/p1"}},
                    {"entity":{"reference":"Patient/p2"}}
                ]
            }),
        )
        .await
        .unwrap();
    let token = tenant_token(tenant);
    let (status, body) = send_request(
        build_test_app(pool),
        get("/fhir/Group/g1/$everything", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 5);
    let practitioner_count = body["entry"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["resource"]["resourceType"] == "Practitioner")
        .count();
    assert_eq!(practitioner_count, 1);
}

#[tokio::test]
async fn definitional_or_non_person_group_is_rejected() {
    let pool = setup_test_db().await;
    let tenant = "everything-group-reject";
    clean_tenant(&pool, tenant).await;
    let store = PgStore::new(pool.clone(), GeoSearchMode::EarthDistance);
    store
        .upsert(
            tenant,
            "Group",
            "g1",
            json!({
                "resourceType":"Group","id":"g1","type":"person","membership":"definitional"
            }),
        )
        .await
        .unwrap();
    let token = tenant_token(tenant);
    let (status, body) = send_request(
        build_test_app(pool),
        get("/fhir/Group/g1/$everything", &token),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}
