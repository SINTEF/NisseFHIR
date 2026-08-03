mod common;

use axum::http::StatusCode;
use serde_json::Value;
use url::Url;

use common::{
    build_test_app_auth_required, clean_tenant, post_resource_with_token, restricted_token,
    search_resource_with_token, send_request, setup_test_db, tenant_token, test_data,
};

fn entry_ids(body: &Value) -> Vec<String> {
    let Some(entries) = body["entry"].as_array() else {
        return Vec::new();
    };

    entries
        .iter()
        .filter_map(|entry| entry["resource"]["id"].as_str().map(ToOwned::to_owned))
        .collect()
}

fn next_link_query(body: &Value) -> Option<String> {
    let next_url = body["link"].as_array()?.iter().find_map(|link| {
        (link["relation"].as_str() == Some("next"))
            .then(|| link["url"].as_str())
            .flatten()
    })?;

    let parsed = Url::parse(next_url).ok()?;
    let path = parsed.path().to_owned();

    parsed
        .query()
        .map_or(Some(path.clone()), |query| Some(format!("{path}?{query}")))
}

#[tokio::test]
async fn search_returns_searchset_bundle() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-bundle").await;
    let token = tenant_token("search-bundle");

    let app = build_test_app_auth_required(pool.clone());
    let (status, created) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool);
    let (status, body) =
        send_request(app, search_resource_with_token("Patient", None, &token)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "Bundle");
    assert_eq!(body["type"], "searchset");
    assert_eq!(body["total"], 1);
    assert_eq!(body["entry"][0]["resource"]["id"], created["id"]);
    assert_eq!(body["entry"][0]["search"]["mode"], "match");
}

#[tokio::test]
async fn search_uses_cursor_pagination() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-paging").await;
    let token = tenant_token("search-paging");

    let mut created_ids = Vec::new();
    for id in ["patient-a", "patient-b", "patient-c"] {
        let mut patient = test_data::minimal_patient();
        patient["id"] = serde_json::json!(id);

        let app = build_test_app_auth_required(pool.clone());
        let (status, created) =
            send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
        assert_eq!(status, StatusCode::CREATED);
        created_ids.push(created["id"].as_str().unwrap().to_owned());
    }
    created_ids.sort();

    let app = build_test_app_auth_required(pool.clone());
    let (status, first_page) = send_request(
        app,
        search_resource_with_token("Patient", Some("_count=2"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_page["total"], 3);
    assert_eq!(entry_ids(&first_page), created_ids[..2]);
    assert_eq!(
        first_page["link"][0]["url"],
        "http://localhost:8080/fhir/Patient?_count=2"
    );
    assert_eq!(first_page["link"][1]["relation"], "next");
    assert_eq!(
        first_page["link"][1]["url"],
        format!(
            "http://localhost:8080/fhir/Patient?_count=2&_after_id={}",
            created_ids[1]
        )
    );

    let app = build_test_app_auth_required(pool);
    let (status, second_page) = send_request(
        app,
        search_resource_with_token(
            "Patient",
            Some(&format!("_count=2&_after_id={}", created_ids[1])),
            &token,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(entry_ids(&second_page), created_ids[2..]);
    assert_eq!(second_page["link"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn search_cursor_pagination_traverses_many_filtered_results() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-many-pages").await;
    let token = tenant_token("search-many-pages");

    let mut expected_ids = Vec::new();
    for index in 0..25 {
        let mut patient = test_data::patient_infant();
        patient["id"] = serde_json::json!(format!("patient-{index:03}"));
        patient["name"][0]["family"] = serde_json::json!("Cursor");

        let app = build_test_app_auth_required(pool.clone());
        let (status, created) =
            send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
        assert_eq!(status, StatusCode::CREATED);
        expected_ids.push(created["id"].as_str().unwrap().to_owned());
    }

    for index in 0..4 {
        let mut patient = test_data::patient_infant();
        patient["id"] = serde_json::json!(format!("noise-{index:03}"));
        patient["name"][0]["family"] = serde_json::json!("Other");

        let app = build_test_app_auth_required(pool.clone());
        let (status, _) =
            send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let mut query = Some("name=cursor&_count=7".to_owned());
    let mut collected_ids = Vec::new();
    let mut page_count = 0;

    while let Some(current_query) = query.take() {
        page_count += 1;
        let app = build_test_app_auth_required(pool.clone());
        let (status, body) = send_request(
            app,
            search_resource_with_token("Patient", Some(&current_query), &token),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        collected_ids.extend(entry_ids(&body));

        query = next_link_query(&body).map(|path_and_query| {
            path_and_query
                .split_once('?')
                .map(|(_, query)| query.to_owned())
                .expect("next link should include a query string")
        });
    }

    expected_ids.sort();

    assert_eq!(page_count, 4);
    assert_eq!(collected_ids, expected_ids);
}

#[tokio::test]
async fn search_respects_tenant_isolation() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-tenant-a").await;
    clean_tenant(&pool, "search-tenant-b").await;

    let token_a = tenant_token("search-tenant-a");
    let token_b = tenant_token("search-tenant-b");

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::minimal_patient(), &token_a),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (status, body) =
        send_request(app, search_resource_with_token("Patient", None, &token_b)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert_eq!(body["entry"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn search_rejects_forbidden_resource_type() {
    let pool = setup_test_db().await;
    let token = restricted_token("search-restricted", vec!["Observation".to_owned()]);

    let app = build_test_app_auth_required(pool);
    let (status, body) =
        send_request(app, search_resource_with_token("Patient", None, &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["issue"][0]["code"], "forbidden");
}

#[tokio::test]
async fn search_rejects_count_above_limit() {
    let pool = setup_test_db().await;
    let token = tenant_token("search-limit");

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("_count=1000000"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");
}

#[tokio::test]
async fn patient_search_filters_by_name() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-patient-name").await;
    let token = tenant_token("search-patient-name");

    let app = build_test_app_auth_required(pool.clone());
    let (status, peter) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_peter_chalmers(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_infant(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("name=peter"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    assert_eq!(body["entry"][0]["resource"]["id"], peter["id"]);
}

#[tokio::test]
async fn patient_search_filters_by_birthdate() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-patient-birthdate").await;
    let token = tenant_token("search-patient-birthdate");

    let app = build_test_app_auth_required(pool.clone());
    let (status, peter) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_peter_chalmers(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_infant(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("birthdate=1974-12-25"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    assert_eq!(body["entry"][0]["resource"]["id"], peter["id"]);
}

#[tokio::test]
async fn patient_search_filters_by_identifier_value_and_system() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-patient-identifier").await;
    let token = tenant_token("search-patient-identifier");

    let app = build_test_app_auth_required(pool.clone());
    let (status, peter) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_peter_chalmers(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token(
            "Patient",
            Some("identifier=urn:oid:1.2.36.146.595.217.0.1|12345"),
            &token,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    assert_eq!(body["entry"][0]["resource"]["id"], peter["id"]);
}

#[tokio::test]
async fn observation_search_filters_by_code_status_and_subject() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-observation-filters").await;
    let token = tenant_token("search-observation-filters");

    let app = build_test_app_auth_required(pool.clone());
    let (status, glucose) = send_request(
        app,
        post_resource_with_token(
            "Observation",
            &test_data::observation_blood_glucose(),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        post_resource_with_token(
            "Observation",
            &test_data::observation_blood_pressure(),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token(
            "Observation",
            Some("code=15074-8&status=final&subject=Patient/example"),
            &token,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    assert_eq!(body["entry"][0]["resource"]["id"], glucose["id"]);
}

#[tokio::test]
async fn search_preserves_filters_in_self_and_next_links() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-links").await;
    let token = tenant_token("search-links");

    let mut created_ids = Vec::new();
    for id in ["patient-a", "patient-b", "patient-c"] {
        let mut patient = test_data::patient_infant();
        patient["id"] = serde_json::json!(id);
        patient["name"][0]["family"] = serde_json::json!("Smith");

        let app = build_test_app_auth_required(pool.clone());
        let (status, created) =
            send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
        assert_eq!(status, StatusCode::CREATED);
        created_ids.push(created["id"].as_str().unwrap().to_owned());
    }
    created_ids.sort();

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("name=smith&_count=2"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["link"][0]["url"],
        "http://localhost:8080/fhir/Patient?_count=2&name=smith"
    );
    assert_eq!(
        body["link"][1]["url"],
        format!(
            "http://localhost:8080/fhir/Patient?_count=2&_after_id={}&name=smith",
            created_ids[1]
        )
    );
}

#[tokio::test]
async fn search_rejects_legacy_offset_parameter() {
    let pool = setup_test_db().await;
    let token = tenant_token("search-legacy-offset");

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("_offset=10"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");
    assert!(
        body["issue"][0]["diagnostics"]
            .as_str()
            .expect("diagnostics should be present")
            .contains("_offset is no longer supported")
    );
}

#[tokio::test]
async fn search_rejects_unsupported_resource_specific_parameter() {
    let pool = setup_test_db().await;
    let token = tenant_token("search-invalid-param");

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Organization", Some("bogus-nonexistent-param=hl7"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");
}

#[tokio::test]
async fn search_rejects_malformed_identifier_filter() {
    let pool = setup_test_db().await;
    let token = tenant_token("search-invalid-identifier");

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("identifier=urn:test|"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");
}

#[tokio::test]
async fn search_rejects_empty_identifier_value() {
    let pool = setup_test_db().await;
    let token = tenant_token("search-empty-identifier");

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("identifier="), &token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");
}

#[tokio::test]
async fn search_rejects_identifier_with_empty_system() {
    let pool = setup_test_db().await;
    let token = tenant_token("search-empty-sys-ident");

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("identifier=|value"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");
}
