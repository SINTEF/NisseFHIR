//! AuditEvent `date` search prefix semantics.
//!
//! These tests exercise the precision-aware, half-open `[lower, upper)`
//! recorded_at interval that `parse_audit_date` builds from the shared FHIR
//! date parser. They specifically guard the fix for the bug where `gt` was
//! collapsed into `ge` and `le` into `lt`, by asserting the strict/inclusive
//! boundary at the exact supplied timestamp.

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
use uuid::Uuid;

/// 2026-01-15T12:00:00Z — the boundary timestamp used across most tests.
fn boundary() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap()
}

fn auditlog_token(tenant: &str) -> String {
    create_test_token(&TestClaims {
        sub: Some("operator".to_owned()),
        tenant: Some(tenant.to_owned()),
        scope: Some("auditlog".to_owned()),
        resource_types: None,
        exp: 4_102_444_800,
    })
}

/// Drop any prior audit rows for this tenant. `audit_events` is a global,
/// append-only table shared across tests, so each test owns an isolated
/// tenant and uses fresh random UUIDs to avoid cross-test primary-key
/// collisions when tests run in parallel.
async fn clean_audit(pool: &sqlx::PgPool, tenant: &str) {
    sqlx::query("DELETE FROM audit_events WHERE tenant_id = $1")
        .bind(tenant)
        .execute(pool)
        .await
        .unwrap();
}

async fn seed_at(pool: &sqlx::PgPool, tenant: &str, id: Uuid, recorded_at: chrono::DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO audit_events (id, occurred_at, recorded_at, tenant_id, subject_id, correlation_id, interaction, action, resource_type, resource_id, http_status, outcome, row_kind) \
         VALUES ($1, $2, $2, $3, 'operator', $4, 'read', 'R', 'Patient', 'target', 200, 'success', 'standalone')",
    )
    .bind(id)
    .bind(recorded_at)
    .bind(tenant)
    .bind(Uuid::new_v4())
    .execute(pool)
    .await
    .unwrap();
}

fn audit_search_request(query: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(format!("/fhir/AuditEvent?{query}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::ACCEPT, "application/fhir+json")
        .body(Body::empty())
        .unwrap()
}

/// Run a search and return (status, sorted vector of returned resource ids).
async fn search_ids(query: &str, token: &str, pool: sqlx::PgPool) -> (StatusCode, Vec<String>) {
    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(app, audit_search_request(query, token)).await;
    let ids = body["entry"]
        .as_array()
        .map(|entries| {
            let mut v: Vec<String> = entries
                .iter()
                .map(|e| e["resource"]["id"].as_str().unwrap().to_owned())
                .collect();
            v.sort();
            v
        })
        .unwrap_or_default();
    (status, ids)
}

/// Seed the canonical three boundary events: one second before, exactly at,
/// and one second after the supplied timestamp. Returns a name->id map.
async fn seed_boundary_trio(pool: &sqlx::PgPool, tenant: &str) -> Vec<(String, Uuid)> {
    let before = Uuid::new_v4();
    let exact = Uuid::new_v4();
    let after = Uuid::new_v4();
    seed_at(
        pool,
        tenant,
        before,
        boundary() - chrono::Duration::seconds(1),
    )
    .await;
    seed_at(pool, tenant, exact, boundary()).await;
    seed_at(
        pool,
        tenant,
        after,
        boundary() + chrono::Duration::seconds(1),
    )
    .await;
    vec![
        ("before".to_owned(), before),
        ("exact".to_owned(), exact),
        ("after".to_owned(), after),
    ]
}

fn ids_for(names: &[&str], map: &[(String, Uuid)]) -> Vec<String> {
    let mut v: Vec<String> = map
        .iter()
        .filter(|(n, _)| names.contains(&n.as_str()))
        .map(|(_, id)| id.to_string())
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn date_prefixes_differ_at_exact_boundary() {
    let tenant = "audit-date-boundary";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    clean_audit(&pool, tenant).await;
    let map = seed_boundary_trio(&pool, tenant).await;
    let token = auditlog_token(tenant);
    let boundary = "2026-01-15T12:00:00Z";

    let (status, ids) =
        search_ids(&format!("action=R&date=ge{boundary}"), &token, pool.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids, ids_for(&["exact", "after"], &map), "ge");

    let (status, ids) =
        search_ids(&format!("action=R&date=gt{boundary}"), &token, pool.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids, ids_for(&["after"], &map), "gt excludes the boundary");

    let (status, ids) =
        search_ids(&format!("action=R&date=lt{boundary}"), &token, pool.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids, ids_for(&["before"], &map), "lt excludes the boundary");

    let (status, ids) =
        search_ids(&format!("action=R&date=le{boundary}"), &token, pool.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids, ids_for(&["before", "exact"], &map), "le");

    let exact_id = map
        .iter()
        .find(|(n, _)| n == "exact")
        .unwrap()
        .1
        .to_string();
    for prefix in ["gt", "lt"] {
        let (_s, ids) = search_ids(
            &format!("action=R&date={prefix}{boundary}"),
            &token,
            pool.clone(),
        )
        .await;
        assert!(!ids.contains(&exact_id), "{prefix} must exclude boundary");
    }
    for prefix in ["ge", "le"] {
        let (_s, ids) = search_ids(
            &format!("action=R&date={prefix}{boundary}"),
            &token,
            pool.clone(),
        )
        .await;
        assert!(ids.contains(&exact_id), "{prefix} must include boundary");
    }
}

#[tokio::test]
async fn eq_uses_precision_interval() {
    let tenant = "audit-date-eq";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    clean_audit(&pool, tenant).await;
    let map = seed_boundary_trio(&pool, tenant).await;
    let token = auditlog_token(tenant);

    let (status, ids) =
        search_ids("action=R&date=2026-01-15T12:00:00Z", &token, pool.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids, ids_for(&["exact"], &map));

    let (status, ids) = search_ids("action=R&date=2026", &token, pool.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids, ids_for(&["before", "exact", "after"], &map));

    let (status, ids) = search_ids("action=R&date=2025", &token, pool.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ids.is_empty());
}

#[tokio::test]
async fn timezone_offset_is_normalized_to_utc() {
    let tenant = "audit-date-offset";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    clean_audit(&pool, tenant).await;
    let map = seed_boundary_trio(&pool, tenant).await;
    let token = auditlog_token(tenant);
    // 2026-01-15T14:00:00+02:00 == 2026-01-15T12:00:00Z.
    let (_s, ids_offset) = search_ids(
        "action=R&date=ge2026-01-15T14:00:00%2B02:00",
        &token,
        pool.clone(),
    )
    .await;
    let (_s, ids_z) =
        search_ids("action=R&date=ge2026-01-15T12:00:00Z", &token, pool.clone()).await;
    assert_eq!(ids_offset, ids_z);
    assert_eq!(ids_offset, ids_for(&["exact", "after"], &map));
}

#[tokio::test]
async fn fractional_seconds_accepted_and_compared_at_second_precision() {
    let tenant = "audit-date-frac";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    clean_audit(&pool, tenant).await;
    let map = seed_boundary_trio(&pool, tenant).await;
    let token = auditlog_token(tenant);
    let (status, ids_ge) = search_ids(
        "action=R&date=ge2026-01-15T12:00:00.500Z",
        &token,
        pool.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids_ge, ids_for(&["exact", "after"], &map));
    let (status, ids_gt) = search_ids(
        "action=R&date=gt2026-01-15T12:00:00.500Z",
        &token,
        pool.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids_gt, ids_for(&["after"], &map));
}

#[tokio::test]
async fn repeated_bounds_intersect_not_overwrite() {
    let tenant = "audit-date-intersect";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    clean_audit(&pool, tenant).await;
    let token = auditlog_token(tenant);
    let d10 = Uuid::new_v4();
    let d15 = Uuid::new_v4();
    let d20 = Uuid::new_v4();
    seed_at(
        &pool,
        tenant,
        d10,
        Utc.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
    )
    .await;
    seed_at(
        &pool,
        tenant,
        d15,
        Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap(),
    )
    .await;
    seed_at(
        &pool,
        tenant,
        d20,
        Utc.with_ymd_and_hms(2026, 1, 20, 0, 0, 0).unwrap(),
    )
    .await;

    // Lower bounds: ge day 12 intersected with gt day 08 keeps the stricter
    // >= day 12. A naive "last wins" overwrite would keep gt day 09 and
    // include the day-10 event.
    let (status, ids) = search_ids(
        "action=R&date=ge2026-01-12&date=gt2026-01-08",
        &token,
        pool.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut expected = vec![d15.to_string(), d20.to_string()];
    expected.sort();
    assert_eq!(ids, expected);

    // Upper bounds: le day 18 intersected with lt day 25 keeps the earlier
    // < day 19. "Last wins" would keep lt day 25 and include day-20.
    let (status, ids) = search_ids(
        "action=R&date=le2026-01-18&date=lt2026-01-25",
        &token,
        pool.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut expected = vec![d10.to_string(), d15.to_string()];
    expected.sort();
    assert_eq!(ids, expected);
}

#[tokio::test]
async fn contradictory_ranges_fail_closed() {
    let tenant = "audit-date-contradictory";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    clean_audit(&pool, tenant).await;
    let token = auditlog_token(tenant);
    seed_at(&pool, tenant, Uuid::new_v4(), boundary()).await;

    for query in [
        "action=R&date=gt2026-01-15&date=lt2026-01-15",
        "action=R&date=ge2026&date=lt2026",
        "action=R&date=le2026-01-10&date=gt2026-01-10",
        "action=R&date=le2026-01-15T12:00:00Z&date=gt2026-01-15T12:00:00Z",
    ] {
        let (status, body) = send_request(
            build_test_app_auth_required(pool.clone()),
            audit_search_request(query, &token),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "query '{query}' should be 400: {body}"
        );
    }
}

#[tokio::test]
async fn unsupported_prefixes_are_rejected() {
    let tenant = "audit-date-unsupported";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    clean_audit(&pool, tenant).await;
    let token = auditlog_token(tenant);
    seed_at(&pool, tenant, Uuid::new_v4(), boundary()).await;

    for query in [
        "action=R&date=ne2026",
        "action=R&date=sa2026",
        "action=R&date=eb2026",
        "action=R&date=ap2026",
    ] {
        let (status, body) = send_request(
            build_test_app_auth_required(pool.clone()),
            audit_search_request(query, &token),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "query '{query}' should be rejected: {body}"
        );
    }
}

#[tokio::test]
async fn malformed_dates_are_rejected() {
    let tenant = "audit-date-malformed";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    clean_audit(&pool, tenant).await;
    let token = auditlog_token(tenant);
    seed_at(&pool, tenant, Uuid::new_v4(), boundary()).await;

    for query in [
        "action=R&date=not-a-date",
        "action=R&date=ge2026-13-01",
        "action=R&date=ge2026-01-15T25:00:00Z",
        "action=R&date=ge",
    ] {
        let (status, body) = send_request(
            build_test_app_auth_required(pool.clone()),
            audit_search_request(query, &token),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "query '{query}' should be 400: {body}"
        );
    }
}

#[tokio::test]
async fn date_filtered_result_set_paginates_with_boundary() {
    let tenant = "audit-date-pagination";
    let pool = setup_test_db().await;
    clean_tenant(&pool, tenant).await;
    clean_audit(&pool, tenant).await;
    let token = auditlog_token(tenant);
    // Five events at 12:00:00 .. 12:00:04; gt the first timestamp excludes
    // the boundary event on every page.
    let mut all = Vec::new();
    let mut boundary_id = None;
    for i in 0..5u128 {
        let id = Uuid::new_v4();
        if i == 0 {
            boundary_id = Some(id.to_string());
        }
        seed_at(
            &pool,
            tenant,
            id,
            boundary() + chrono::Duration::seconds(i64::try_from(i).unwrap()),
        )
        .await;
        all.push(id.to_string());
    }
    all.sort();
    let boundary_id = boundary_id.unwrap();

    let mut page_query = "action=R&date=gt2026-01-15T12:00:00Z&_count=2".to_owned();
    let mut collected = Vec::new();
    let mut pages = 0;
    loop {
        let app = build_test_app_auth_required(pool.clone());
        let (status, body) = send_request(app, audit_search_request(&page_query, &token)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        pages += 1;
        assert_eq!(
            body["total"], 4,
            "gt boundary excludes the first event: {body}"
        );
        for entry in body["entry"].as_array().unwrap() {
            let id = entry["resource"]["id"].as_str().unwrap().to_owned();
            assert_ne!(id, boundary_id, "boundary event must never appear");
            collected.push(id);
        }
        let Some(next_url) = body["link"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["relation"] == "next")
            .and_then(|l| l["url"].as_str())
        else {
            break;
        };
        let pairs: Vec<(String, String)> = url::Url::parse(next_url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        assert!(
            pairs
                .iter()
                .any(|(k, v)| k == "date" && v == "gt2026-01-15T12:00:00Z")
        );
        page_query = url::Url::parse(next_url)
            .unwrap()
            .query()
            .unwrap()
            .to_owned();
    }
    collected.sort();
    let expected: Vec<String> = all.into_iter().filter(|id| id != &boundary_id).collect();
    assert_eq!(collected, expected);
    assert_eq!(pages, 2);
}
