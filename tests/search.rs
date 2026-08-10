mod common;

use axum::http::StatusCode;
use serde_json::Value;
use url::{Url, form_urlencoded::Serializer};

use common::{
    build_test_app_auth_required, build_test_app_with_geo_mode, clean_tenant,
    post_resource_with_token, restricted_token, search_resource_with_token, send_request,
    setup_test_db, tenant_token, test_data,
};
use fhir_server::search_params::sql::GeoSearchMode;

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
    let next_query = next_link_query(&first_page)
        .and_then(|path_and_query| {
            path_and_query
                .split_once('?')
                .map(|(_, query)| query.to_owned())
        })
        .expect("first page must include a next link with a query string");
    let next_params: Vec<_> = url::form_urlencoded::parse(next_query.as_bytes()).collect();
    assert!(next_params.contains(&("_count".into(), "2".into())));
    assert!(next_params.iter().any(|(key, value)| {
        key == "_after_id" && !value.is_empty() && value.starts_with("v1\u{1f}")
    }));

    let app = build_test_app_auth_required(pool);
    let (status, second_page) = send_request(
        app,
        search_resource_with_token("Patient", Some(&next_query), &token),
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
async fn patient_string_search_is_prefix_based_and_phonetic_or_modifiers_are_rejected() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-patient-string-contract").await;
    let token = tenant_token("search-patient-string-contract");

    let app = build_test_app_auth_required(pool.clone());
    let (status, peter) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_peter_chalmers(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let app = build_test_app_auth_required(pool.clone());
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("given=pet"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        entry_ids(&body),
        vec![peter["id"].as_str().unwrap().to_owned()]
    );

    // `eter` is a substring but not a FHIR-default prefix match.
    let app = build_test_app_auth_required(pool.clone());
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("given=eter"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);

    for query in ["phonetic=peter", "given:exact=Peter", "given:contains=eter"] {
        let app = build_test_app_auth_required(pool.clone());
        let (status, body) = send_request(
            app,
            search_resource_with_token("Patient", Some(query), &token),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {body}");
    }
}

#[tokio::test]
async fn patient_search_supports_standard_id_and_representative_parameter_types() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-patient-parameter-types").await;
    let token = tenant_token("search-patient-parameter-types");

    let app = build_test_app_auth_required(pool.clone());
    let (status, peter) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_peter_chalmers(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let peter_id = peter["id"].as_str().unwrap();

    for query in [
        format!("_id={peter_id}"),
        "email=Jim@example.org".to_owned(),
        "address-city=Plea".to_owned(),
        "organization=Organization/1".to_owned(),
        "birthdate=ge1974".to_owned(),
    ] {
        let app = build_test_app_auth_required(pool.clone());
        let (status, body) = send_request(
            app,
            search_resource_with_token("Patient", Some(&query), &token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(
            entry_ids(&body),
            vec![peter_id.to_owned()],
            "{query}: {body}"
        );
    }

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("_id=not|a-token"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn patient_deceased_and_death_date_searches_have_boolean_and_date_semantics() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-patient-deceased").await;
    let token = tenant_token("search-patient-deceased");

    let mut living = test_data::minimal_patient();
    living["deceasedBoolean"] = serde_json::json!(false);
    let app = build_test_app_auth_required(pool.clone());
    let (status, living) =
        send_request(app, post_resource_with_token("Patient", &living, &token)).await;
    assert_eq!(status, StatusCode::CREATED);

    let mut deceased = test_data::minimal_patient();
    deceased["deceasedDateTime"] = serde_json::json!("2020-05-01");
    let app = build_test_app_auth_required(pool.clone());
    let (status, deceased) =
        send_request(app, post_resource_with_token("Patient", &deceased, &token)).await;
    assert_eq!(status, StatusCode::CREATED);

    for (query, expected_id) in [
        ("deceased=true", deceased["id"].as_str().unwrap()),
        ("deceased=false", living["id"].as_str().unwrap()),
        ("death-date=2020-05-01", deceased["id"].as_str().unwrap()),
    ] {
        let app = build_test_app_auth_required(pool.clone());
        let (status, body) = send_request(
            app,
            search_resource_with_token("Patient", Some(query), &token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(
            entry_ids(&body),
            vec![expected_id.to_owned()],
            "{query}: {body}"
        );
    }
}

#[tokio::test]
async fn filtered_last_updated_sort_paginates_without_duplicates_or_gaps() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-filtered-sorted-pages").await;
    let token = tenant_token("search-filtered-sorted-pages");

    let mut expected_ids = Vec::new();
    for gender in ["female", "male", "female", "female"] {
        let mut patient = test_data::minimal_patient();
        patient["gender"] = serde_json::json!(gender);
        let app = build_test_app_auth_required(pool.clone());
        let (status, created) =
            send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
        assert_eq!(status, StatusCode::CREATED);
        if gender == "female" {
            expected_ids.push(created["id"].as_str().unwrap().to_owned());
        }
    }

    let mut query = Some("gender=female&_sort=-_lastUpdated&_count=1".to_owned());
    let mut found_ids = Vec::new();
    while let Some(current_query) = query.take() {
        let app = build_test_app_auth_required(pool.clone());
        let (status, body) = send_request(
            app,
            search_resource_with_token("Patient", Some(&current_query), &token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{current_query}: {body}");
        assert_eq!(body["total"], 3);
        found_ids.extend(entry_ids(&body));
        query = next_link_query(&body).and_then(|path_and_query| {
            path_and_query
                .split_once('?')
                .map(|(_, query)| query.to_owned())
        });
    }

    expected_ids.sort();
    found_ids.sort();
    assert_eq!(found_ids, expected_ids);
}

#[tokio::test]
async fn search_injection_canary_is_bound_and_cannot_change_the_query() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "search-injection-canary").await;
    let token = tenant_token("search-injection-canary");

    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        post_resource_with_token("Patient", &test_data::patient_peter_chalmers(), &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let payload = "peter%' OR TRUE; DROP TABLE fhir_resources; --";
    let mut serializer = Serializer::new(String::new());
    serializer.append_pair("name", payload);
    let query = serializer.finish();
    let app = build_test_app_auth_required(pool.clone());
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some(&query), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "canary search failed: {body}");
    assert_eq!(body["total"], 0);

    // A normal search still works, proving the payload neither broadened the
    // predicate nor executed its attempted statement.
    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("name=peter"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
}

#[tokio::test]
async fn search_rejects_registry_entries_without_an_executable_sql_path() {
    let pool = setup_test_db().await;
    let token = tenant_token("search-fail-closed");
    let app = build_test_app_auth_required(pool);

    let (status, body) = send_request(
        app,
        search_resource_with_token(
            "QuestionnaireResponse",
            Some("item-subject=Patient/example"),
            &token,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert!(
        body["issue"][0]["diagnostics"]
            .as_str()
            .unwrap()
            .contains("unsupported")
    );
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

    for id in ["patient-a", "patient-b", "patient-c"] {
        let mut patient = test_data::patient_infant();
        patient["id"] = serde_json::json!(id);
        patient["name"][0]["family"] = serde_json::json!("Smith");

        let app = build_test_app_auth_required(pool.clone());
        let (status, _) =
            send_request(app, post_resource_with_token("Patient", &patient, &token)).await;
        assert_eq!(status, StatusCode::CREATED);
    }

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
    let next_query = next_link_query(&body)
        .and_then(|path_and_query| {
            path_and_query
                .split_once('?')
                .map(|(_, query)| query.to_owned())
        })
        .expect("first page must include a next link with a query string");
    let next_params: Vec<_> = url::form_urlencoded::parse(next_query.as_bytes()).collect();
    assert!(next_params.contains(&("_count".into(), "2".into())));
    assert!(next_params.contains(&("name".into(), "smith".into())));
    assert!(next_params.iter().any(|(key, value)| {
        key == "_after_id" && !value.is_empty() && value.starts_with("v1\u{1f}")
    }));
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

/// Create a Location at the given coordinates and return its server id.
async fn create_location(
    pool: &sqlx::PgPool,
    tenant: &str,
    name: &str,
    lat: f64,
    lon: f64,
) -> String {
    let app = build_test_app_auth_required(pool.clone());
    let token = tenant_token(tenant);
    let body = serde_json::json!({
        "resourceType": "Location",
        "name": name,
        "position": { "latitude": lat, "longitude": lon }
    });
    let (status, created) =
        send_request(app, post_resource_with_token("Location", &body, &token)).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {created}");
    created["id"].as_str().unwrap().to_owned()
}

/// `near` search must filter by proximity in both geospatial modes: the
/// indexed `earthdistance` path and the pure-SQL haversine fallback. Both
/// modes run against the same real database, so a passing test proves the
/// haversine SQL executes on Postgres and returns correct results.
async fn near_search_filters_by_proximity(pool: sqlx::PgPool, geo_mode: GeoSearchMode) {
    let tenant = "search-near";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // Boston and a point ~111 km east; both must be stored.
    let boston = create_location(&pool, tenant, "boston", 42.36, -71.06).await;
    let far = create_location(&pool, tenant, "far", 42.36, -70.06).await;

    let app = build_test_app_with_geo_mode(pool, false, Vec::new(), geo_mode);
    // 50 km radius around Boston: only Boston qualifies, "far" is excluded.
    let (status, body) = send_request(
        app,
        search_resource_with_token("Location", Some("near=42.36|-71.06|50|km"), &token),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "near search failed: {body}");
    let ids = entry_ids(&body);
    assert!(
        ids.contains(&boston),
        "expected boston ({boston}) in results, got {ids:?}"
    );
    assert!(
        !ids.contains(&far),
        "expected far ({far}) excluded by 50km radius, got {ids:?}"
    );
}

#[tokio::test]
async fn near_search_filters_by_proximity_in_earthdistance_mode() {
    let pool = setup_test_db().await;
    near_search_filters_by_proximity(pool, GeoSearchMode::EarthDistance).await;
}

#[tokio::test]
async fn near_search_filters_by_proximity_in_haversine_mode() {
    let pool = setup_test_db().await;
    near_search_filters_by_proximity(pool, GeoSearchMode::Haversine).await;
}

#[tokio::test]
async fn near_search_earthdistance_predicate_uses_gist_index() {
    let pool = setup_test_db().await;
    let mut tx = pool.begin().await.expect("begin planner-test transaction");

    // Small test tables normally favor a sequential scan. Disabling it only
    // for this transaction lets EXPLAIN tell us whether the predicate is
    // compatible with the functional GiST index created by the migration.
    sqlx::query("SET LOCAL enable_seqscan = off")
        .execute(&mut *tx)
        .await
        .expect("disable sequential scans for planner test");

    let plan: Value = sqlx::query_scalar(
        r#"
        EXPLAIN (FORMAT JSON)
        SELECT id
        FROM fhir_res_location
        WHERE resource->'position' IS NOT NULL
          AND earth_box(ll_to_earth($1, $2), $3) @> ll_to_earth(
                (resource->'position'->>'latitude')::float8,
                (resource->'position'->>'longitude')::float8
              )
          AND earth_distance(
                ll_to_earth(
                  (resource->'position'->>'latitude')::float8,
                  (resource->'position'->>'longitude')::float8
                ),
                ll_to_earth($1, $2)
              ) <= $3
        "#,
    )
    .bind(42.36_f64)
    .bind(-71.06_f64)
    .bind(50_000.0_f64)
    .fetch_one(&mut *tx)
    .await
    .expect("explain indexed near-search predicate");

    let plan = plan.to_string();
    assert!(
        plan.contains("idx_fhir_res_location_position"),
        "expected geospatial GiST index in plan: {plan}"
    );
    assert!(
        plan.contains("Index Cond"),
        "expected an indexed bounding-box condition: {plan}"
    );
}
