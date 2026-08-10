mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use common::{
    build_test_app, clean_tenant, restricted_token, send_request, setup_test_db, tenant_token,
};
use fhir_server::{search_params::sql::GeoSearchMode, store::PgStore};
use serde_json::json;

fn get(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

async fn fixture(tenant: &str) -> (sqlx::PgPool, String) {
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    let store = PgStore::new(pool.clone(), GeoSearchMode::EarthDistance);
    store
        .upsert(
            tenant,
            "Patient",
            "p1",
            json!({"resourceType":"Patient","id":"p1"}),
        )
        .await
        .unwrap();
    store
        .upsert(
            tenant,
            "Practitioner",
            "pr1",
            json!({"resourceType":"Practitioner","id":"pr1"}),
        )
        .await
        .unwrap();
    store
        .upsert(
            tenant,
            "Observation",
            "o1",
            json!({
                "resourceType":"Observation","id":"o1","status":"final",
                "code":{"text":"example"},
                "subject":{"reference":"Patient/p1"},
                "performer":[{"reference":"Practitioner/pr1"}],
                "effectiveDateTime":"2025-02-15"
            }),
        )
        .await
        .unwrap();
    (pool, tenant_token(tenant))
}

#[tokio::test]
async fn patient_everything_returns_primary_and_support_with_modes() {
    let (pool, token) = fixture("everything-patient-basic").await;
    let (status, body) = send_request(
        build_test_app(pool),
        get("/fhir/Patient/p1/$everything", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["type"], "searchset");
    assert_eq!(body["total"], 3);
    let entries = body["entry"].as_array().unwrap();
    assert_eq!(entries[0]["resource"]["resourceType"], "Patient");
    let observation = entries
        .iter()
        .find(|entry| entry["resource"]["resourceType"] == "Observation")
        .unwrap();
    let practitioner = entries
        .iter()
        .find(|entry| entry["resource"]["resourceType"] == "Practitioner")
        .unwrap();
    assert_eq!(observation["search"]["mode"], "match");
    assert_eq!(practitioner["search"]["mode"], "include");
}

#[tokio::test]
async fn patient_everything_pages_and_rejects_tampered_cursor() {
    let (pool, token) = fixture("everything-patient-page").await;
    let app = build_test_app(pool);
    let (status, first) = send_request(
        app.clone(),
        get("/fhir/Patient/p1/$everything?_count=1", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["entry"].as_array().unwrap().len(), 1);
    let next = first["link"]
        .as_array()
        .unwrap()
        .iter()
        .find(|link| link["relation"] == "next")
        .unwrap()["url"]
        .as_str()
        .unwrap();
    let parsed = url::Url::parse(next).unwrap();
    let path = format!("{}?{}", parsed.path(), parsed.query().unwrap());
    let (status, second) = send_request(app.clone(), get(&path, &token)).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_ne!(first["entry"][0]["fullUrl"], second["entry"][0]["fullUrl"]);

    let tampered = path.replace("_cursor=", "_cursor=x");
    let (status, outcome) = send_request(app, get(&tampered, &token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{outcome}");
}

#[tokio::test]
async fn current_resource_updates_do_not_repeat_an_already_emitted_cursor_key() {
    let (pool, token) = fixture("everything-patient-cursor-update").await;
    let app = build_test_app(pool.clone());
    let (status, first) = send_request(
        app.clone(),
        get("/fhir/Patient/p1/$everything?_count=1", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["entry"][0]["resource"]["resourceType"], "Patient");

    // The current Patient gets a new resource version after it has appeared
    // on page one. A cursor must order current resources by their stable
    // identity, not their mutable current version number.
    PgStore::new(pool, GeoSearchMode::EarthDistance)
        .upsert(
            "everything-patient-cursor-update",
            "Patient",
            "p1",
            json!({"resourceType":"Patient","id":"p1","active":true}),
        )
        .await
        .unwrap();

    let next = first["link"]
        .as_array()
        .unwrap()
        .iter()
        .find(|link| link["relation"] == "next")
        .unwrap()["url"]
        .as_str()
        .unwrap();
    let parsed = url::Url::parse(next).unwrap();
    let path = format!("{}?{}", parsed.path(), parsed.query().unwrap());
    let (status, second) = send_request(app, get(&path, &token)).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert!(
        second["entry"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["resource"]["resourceType"] != "Patient")
    );
}

#[tokio::test]
async fn type_date_since_and_post_parameters_are_normalized() {
    let (pool, token) = fixture("everything-patient-filters").await;
    let app = build_test_app(pool);
    let (status, body) = send_request(
        app.clone(),
        get(
            "/fhir/Patient/p1/$everything?_type=Observation&start=2025-01-01&end=2025-12-31",
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let types: Vec<_> = body["entry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["resource"]["resourceType"].as_str().unwrap())
        .collect();
    assert_eq!(types, vec!["Patient", "Observation"]);

    let parameters = json!({"resourceType":"Parameters","parameter":[
        {"name":"_type","valueCode":"Observation"},
        {"name":"start","valueDate":"2030"}
    ]});
    let request = Request::builder()
        .method("POST")
        .uri("/fhir/Patient/p1/$everything")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/fhir+json")
        .body(Body::from(serde_json::to_vec(&parameters).unwrap()))
        .unwrap();
    let (status, body) = send_request(app, request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1, "the nominated Patient remains present");
}

#[tokio::test]
async fn patient_type_operation_fails_closed() {
    let (pool, token) = fixture("everything-patient-type").await;
    let (status, body) = send_request(
        build_test_app(pool),
        get("/fhir/Patient/$everything", &token),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}

#[tokio::test]
async fn requested_forbidden_type_fails_instead_of_looking_complete() {
    let (pool, _) = fixture("everything-patient-authz").await;
    let token = restricted_token(
        "everything-patient-authz",
        vec!["Patient".to_owned(), "Observation".to_owned()],
    );
    let (status, body) = send_request(
        build_test_app(pool),
        get("/fhir/Patient/p1/$everything?_type=Practitioner", &token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}

#[tokio::test]
async fn explicit_version_reference_resolves_history_and_marks_full_url() {
    let pool = setup_test_db().await;
    let tenant = "everything-patient-history";
    clean_tenant(&pool, tenant).await;
    let store = PgStore::new(pool.clone(), GeoSearchMode::EarthDistance);
    store
        .upsert(
            tenant,
            "Patient",
            "p1",
            json!({"resourceType":"Patient","id":"p1"}),
        )
        .await
        .unwrap();
    store
        .upsert(
            tenant,
            "Observation",
            "o1",
            json!({
                "resourceType":"Observation","id":"o1","status":"preliminary","code":{"text":"old"}
            }),
        )
        .await
        .unwrap();
    store
        .upsert(
            tenant,
            "Observation",
            "o1",
            json!({
                "resourceType":"Observation","id":"o1","status":"final","code":{"text":"current"}
            }),
        )
        .await
        .unwrap();
    store.upsert(tenant, "DiagnosticReport", "d1", json!({
        "resourceType":"DiagnosticReport","id":"d1","status":"final","code":{"text":"report"},
        "subject":{"reference":"Patient/p1"},
        "result":[{"reference":"Observation/o1/_history/1"}]
    })).await.unwrap();

    let token = tenant_token(tenant);
    let (status, body) = send_request(
        build_test_app(pool),
        get("/fhir/Patient/p1/$everything", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let historical = body["entry"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["resource"]["resourceType"] == "Observation")
        .unwrap();
    assert_eq!(historical["resource"]["status"], "preliminary");
    assert!(
        historical["fullUrl"]
            .as_str()
            .unwrap()
            .ends_with("/Observation/o1/_history/1")
    );
    assert_eq!(historical["search"]["mode"], "include");
}

#[tokio::test]
async fn all_patient_branches_and_attachment_binary_are_included() {
    let pool = setup_test_db().await;
    let tenant = "everything-patient-all-branches";
    clean_tenant(&pool, tenant).await;
    let store = PgStore::new(pool.clone(), GeoSearchMode::EarthDistance);
    for id in ["p-subject", "p-actor"] {
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
            "Binary",
            "b1",
            json!({"resourceType":"Binary","id":"b1","contentType":"text/plain","data":"eA=="}),
        )
        .await
        .unwrap();
    store
        .upsert(
            tenant,
            "Appointment",
            "a1",
            json!({
                "resourceType":"Appointment","id":"a1","status":"booked",
                "subject":{"reference":"Patient/p-subject"},
                "participant":[{"actor":{"reference":"Patient/p-actor"},"status":"accepted"}]
            }),
        )
        .await
        .unwrap();
    store
        .upsert(
            tenant,
            "DocumentReference",
            "d1",
            json!({
                "resourceType":"DocumentReference","id":"d1","status":"current","content":[
                    {"attachment":{"contentType":"text/plain","url":"Binary/b1"}}
                ],"subject":{"reference":"Patient/p-subject"}
            }),
        )
        .await
        .unwrap();
    let token = tenant_token(tenant);
    for patient in ["p-subject", "p-actor"] {
        let (status, body) = send_request(
            build_test_app(pool.clone()),
            get(&format!("/fhir/Patient/{patient}/$everything"), &token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(
            body["entry"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["resource"]["id"] == "a1")
        );
        if patient == "p-subject" {
            assert!(
                body["entry"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|entry| entry["resource"]["resourceType"] == "Binary")
            );
        }
    }
}

#[tokio::test]
async fn condition_care_period_spans_onset_to_abatement_and_since_is_applied() {
    let pool = setup_test_db().await;
    let tenant = "everything-condition-period";
    clean_tenant(&pool, tenant).await;
    let store = PgStore::new(pool.clone(), GeoSearchMode::EarthDistance);
    store
        .upsert(
            tenant,
            "Patient",
            "p1",
            json!({"resourceType":"Patient","id":"p1"}),
        )
        .await
        .unwrap();
    store
        .upsert(
            tenant,
            "Condition",
            "c1",
            json!({
                "resourceType":"Condition","id":"c1","subject":{"reference":"Patient/p1"},
                "clinicalStatus":{"coding":[{"code":"active"}]},"code":{"text":"example"},
                "onsetDateTime":"2020-01-01","abatementDateTime":"2025-01-01"
            }),
        )
        .await
        .unwrap();
    let token = tenant_token(tenant);
    let (status, body) = send_request(
        build_test_app(pool.clone()),
        get(
            "/fhir/Patient/p1/$everything?_type=Condition&start=2023-01-01&end=2023-12-31",
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 2);
    let (status, body) = send_request(
        build_test_app(pool),
        get(
            "/fhir/Patient/p1/$everything?_type=Condition&_since=2100-01-01T00:00:00Z",
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1);
}
