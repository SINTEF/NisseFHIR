mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use chrono::{TimeZone, Utc};
use common::{
    TestClaims, build_test_app_auth_required, clean_tenant, create_test_token, send_request,
    setup_test_db,
};
use serde_json::Value;
use url::{Url, form_urlencoded::Serializer};
use uuid::Uuid;

const TENANT: &str = "audit-search-pagination";
const ID_FILTER_TENANT: &str = "audit-search-id-filter";

fn auditlog_token(tenant: &str) -> String {
    create_test_token(&TestClaims {
        sub: Some("operator/a+b &?".to_owned()),
        tenant: Some(tenant.to_owned()),
        scope: Some("auditlog".to_owned()),
        resource_types: None,
        exp: 4_102_444_800,
    })
}

async fn seed_audit_event(
    pool: &sqlx::PgPool,
    tenant: &str,
    id: Uuid,
    subject_id: &str,
    resource_id: &str,
) {
    sqlx::query(
        "INSERT INTO audit_events (id, occurred_at, recorded_at, tenant_id, subject_id, correlation_id, interaction, action, resource_type, resource_id, http_status, outcome, row_kind) VALUES ($1, $2, $2, $3, $4, $5, 'read', 'R', 'Patient', $6, 200, 'success', 'standalone')",
    )
    .bind(id)
    .bind(Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap())
    .bind(tenant)
    .bind(subject_id)
    .bind(Uuid::new_v4())
    .bind(resource_id)
    .execute(pool)
    .await
    .unwrap();
}

fn audit_search_request(query: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(format!("/fhir/AuditEvent?{query}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn link(body: &Value, relation: &str) -> String {
    body["link"]
        .as_array()
        .unwrap()
        .iter()
        .find(|link| link["relation"] == relation)
        .and_then(|link| link["url"].as_str())
        .unwrap()
        .to_owned()
}

fn query_from_url(url: &str) -> String {
    let url = Url::parse(url).unwrap();
    url.query().unwrap().to_owned()
}

fn query_pairs(url: &str) -> Vec<(String, String)> {
    Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

#[tokio::test]
async fn audit_search_next_links_retain_and_encode_all_filters_across_pages() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, TENANT).await;
    sqlx::query("DELETE FROM audit_events WHERE tenant_id = $1")
        .bind(TENANT)
        .execute(&pool)
        .await
        .unwrap();

    let agent = "operator/a+b &?";
    for value in 1..=7_u128 {
        seed_audit_event(&pool, TENANT, Uuid::from_u128(value), agent, "target").await;
    }
    // This row would leak into later pages if the entity/agent filters were
    // omitted from the generated next link.
    seed_audit_event(
        &pool,
        TENANT,
        Uuid::from_u128(8),
        "different-agent",
        "other-target",
    )
    .await;

    let mut query = Serializer::new(String::new());
    query.append_pair("action", "R");
    query.append_pair("date", "ge2026-01-01T00:00:00Z");
    query.append_pair("date", "le2026-01-31T00:00:00Z");
    query.append_pair("code", "REST");
    query.append_pair("outcome", "success");
    query.append_pair("agent", agent);
    query.append_pair("entity", "Patient/target");
    query.append_pair("_count", "2");
    let original_query = query.finish();
    let expected_filters = vec![
        ("action".to_owned(), "R".to_owned()),
        ("date".to_owned(), "ge2026-01-01T00:00:00Z".to_owned()),
        ("date".to_owned(), "le2026-01-31T00:00:00Z".to_owned()),
        ("code".to_owned(), "REST".to_owned()),
        ("outcome".to_owned(), "success".to_owned()),
        ("agent".to_owned(), agent.to_owned()),
        ("entity".to_owned(), "Patient/target".to_owned()),
    ];
    let token = auditlog_token(TENANT);

    let mut next_query = original_query;
    let mut pages = 0;
    let mut ids = Vec::new();
    loop {
        let app = build_test_app_auth_required(pool.clone());
        let (status, body) = send_request(app, audit_search_request(&next_query, &token)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        pages += 1;
        ids.extend(
            body["entry"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| entry["resource"]["id"].as_str().unwrap().to_owned()),
        );

        let self_url = link(&body, "self");
        let self_pairs = query_pairs(&self_url);
        assert_eq!(self_pairs[0], ("_count".to_owned(), "2".to_owned()));
        let requested_after = url::form_urlencoded::parse(next_query.as_bytes())
            .find_map(|(key, value)| (key == "_after_id").then(|| value.into_owned()));
        let self_after = self_pairs
            .iter()
            .find_map(|(key, value)| (key == "_after_id").then_some(value.clone()));
        assert_eq!(self_after, requested_after);
        assert_eq!(
            self_pairs
                .iter()
                .filter(|(key, _)| key != "_count" && key != "_after_id")
                .cloned()
                .collect::<Vec<_>>(),
            expected_filters
        );
        assert_eq!(
            self_pairs.iter().filter(|(key, _)| key == "date").count(),
            2
        );
        assert!(
            self_pairs
                .iter()
                .any(|(key, value)| key == "agent" && value == agent)
        );
        assert!(
            self_pairs
                .iter()
                .any(|(key, value)| key == "entity" && value == "Patient/target")
        );
        for (key, value) in [("action", "R"), ("code", "REST"), ("outcome", "success")] {
            assert!(
                self_pairs
                    .iter()
                    .any(|(actual_key, actual_value)| actual_key == key && actual_value == value)
            );
        }

        let Some(next_url) = body["link"]
            .as_array()
            .unwrap()
            .iter()
            .find(|link| link["relation"] == "next")
            .and_then(|link| link["url"].as_str())
        else {
            break;
        };
        let next_pairs = query_pairs(next_url);
        assert_eq!(next_pairs[0], ("_count".to_owned(), "2".to_owned()));
        assert_eq!(next_pairs[1].0, "_after_id");
        assert_eq!(next_pairs[2..], expected_filters);
        assert_eq!(
            next_pairs.iter().filter(|(key, _)| key == "date").count(),
            2
        );
        assert!(
            next_pairs
                .iter()
                .any(|(key, value)| key == "agent" && value == agent)
        );
        assert!(
            next_pairs
                .iter()
                .any(|(key, value)| key == "entity" && value == "Patient/target")
        );
        for (key, value) in [("action", "R"), ("code", "REST"), ("outcome", "success")] {
            assert!(
                next_pairs
                    .iter()
                    .any(|(actual_key, actual_value)| actual_key == key && actual_value == value)
            );
        }
        next_query = query_from_url(next_url);
    }

    assert_eq!(pages, 4);
    assert_eq!(ids.len(), 7);
    assert!(ids.iter().all(|id| id != &Uuid::from_u128(8).to_string()));
}

#[tokio::test]
async fn audit_search_self_link_preserves_id_filter() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, ID_FILTER_TENANT).await;
    sqlx::query("DELETE FROM audit_events WHERE tenant_id = $1")
        .bind(ID_FILTER_TENANT)
        .execute(&pool)
        .await
        .unwrap();

    let id = Uuid::from_u128(100);
    seed_audit_event(&pool, ID_FILTER_TENANT, id, "operator", "target").await;
    let mut query = Serializer::new(String::new());
    query.append_pair("_id", &id.to_string());
    query.append_pair("_count", "1");

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        audit_search_request(&query.finish(), &auditlog_token(ID_FILTER_TENANT)),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1);
    assert_eq!(body["entry"][0]["resource"]["id"], id.to_string());
    assert_eq!(
        query_pairs(&link(&body, "self")),
        vec![
            ("_count".to_owned(), "1".to_owned()),
            ("_id".to_owned(), id.to_string()),
        ]
    );
}
