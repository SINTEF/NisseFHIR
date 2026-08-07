//! Integration tests for the `_sort` search-result parameter (task 040).
//!
//! `_id` and `_lastUpdated` are the only sortable keys (see
//! `src/sort.rs` and `ARCHITECTURE.md`'s "Sorted search (`_sort`)" section).
//! Both map to `NOT NULL` columns, so there is no "resource missing the sort
//! element" case to cover here — that acceptance scenario does not apply to
//! this server's supported sort keys by design.

mod common;

use axum::http::StatusCode;
use serde_json::Value;
use sqlx::Row;
use url::Url;

use common::{
    build_test_app_auth_required, clean_tenant, post_resource_with_token,
    search_resource_with_token, send_request, setup_test_db, tenant_token, test_data,
};

fn entry_ids(body: &Value) -> Vec<String> {
    body["entry"]
        .as_array()
        .into_iter()
        .flatten()
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
    Some(
        parsed
            .query()
            .map_or(path.clone(), |query| format!("{path}?{query}")),
    )
}

fn self_link_url(body: &Value) -> &str {
    body["link"]
        .as_array()
        .unwrap()
        .iter()
        .find(|link| link["relation"] == "self")
        .expect("self link must be present")["url"]
        .as_str()
        .unwrap()
}

async fn create_patient(pool: &sqlx::PgPool, token: &str, id: &str) -> String {
    let mut patient = test_data::minimal_patient();
    patient["id"] = serde_json::json!(id);
    let app = build_test_app_auth_required(pool.clone());
    let (status, created) =
        send_request(app, post_resource_with_token("Patient", &patient, token)).await;
    assert_eq!(status, StatusCode::CREATED, "setup: create {id}");
    created["id"].as_str().unwrap().to_owned()
}

/// The order Postgres's default `TEXT` collation gives a set of ids under
/// `ORDER BY id ASC/DESC` is not always plain byte-order (locale collation
/// can weigh punctuation like `-` differently), and ids assigned by `POST`
/// are server-generated UUIDs the test does not control. Rather than assume
/// a Rust-side sort matches, ask the database directly for the order it
/// actually produces, so tests assert against reality instead of an
/// incorrect ASCII-ordering assumption.
async fn ids_ordered_by_id(
    pool: &sqlx::PgPool,
    tenant_id: &str,
    ids: &[String],
    descending: bool,
) -> Vec<String> {
    let rows = if descending {
        sqlx::query(
            "SELECT id FROM fhir_resources WHERE tenant_id = $1 AND resource_type = 'Patient' \
             AND id = ANY($2) ORDER BY id DESC",
        )
        .bind(tenant_id)
        .bind(ids)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT id FROM fhir_resources WHERE tenant_id = $1 AND resource_type = 'Patient' \
             AND id = ANY($2) ORDER BY id ASC",
        )
        .bind(tenant_id)
        .bind(ids)
        .fetch_all(pool)
        .await
    }
    .expect("order-by-id query should succeed");
    rows.into_iter()
        .map(|row| row.get::<String, _>("id"))
        .collect()
}

/// Force a resource's `last_updated` to an explicit value, bypassing the
/// server-generated `now()` default, so tests can construct exact ties and
/// exact orderings deterministically. `base` must be a single value shared
/// across every call meant to tie or order together — each call evaluating
/// its own `now()` (in SQL or in Rust) would only be microseconds apart, not
/// exactly equal, and silently turn an intended tie into a near-tie ordered
/// by `last_updated` instead of by the `_id` tiebreak the test means to
/// exercise.
async fn set_last_updated(
    pool: &sqlx::PgPool,
    tenant_id: &str,
    id: &str,
    base: chrono::DateTime<chrono::Utc>,
    seconds_offset: i64,
) {
    let last_updated = base + chrono::Duration::seconds(seconds_offset);
    sqlx::query(
        "UPDATE fhir_resources SET last_updated = $1 \
         WHERE tenant_id = $2 AND resource_type = 'Patient' AND id = $3",
    )
    .bind(last_updated)
    .bind(tenant_id)
    .bind(id)
    .execute(pool)
    .await
    .expect("last_updated backdoor update should succeed");
}

#[tokio::test]
async fn sort_by_last_updated_ascending_and_descending_are_exact_reverses() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "sort-lu-reverse").await;
    let token = tenant_token("sort-lu-reverse");

    let a = create_patient(&pool, &token, "patient-a").await;
    let b = create_patient(&pool, &token, "patient-b").await;
    let c = create_patient(&pool, &token, "patient-c").await;
    // Distinct, controlled timestamps sharing one base instant: no ties, so
    // ascending must be the exact reverse of descending.
    let base = chrono::Utc::now();
    set_last_updated(&pool, "sort-lu-reverse", &a, base, -300).await;
    set_last_updated(&pool, "sort-lu-reverse", &b, base, -200).await;
    set_last_updated(&pool, "sort-lu-reverse", &c, base, -100).await;

    let app = build_test_app_auth_required(pool.clone());
    let (status, ascending) = send_request(
        app,
        search_resource_with_token("Patient", Some("_sort=_lastUpdated"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let app = build_test_app_auth_required(pool);
    let (status, descending) = send_request(
        app,
        search_resource_with_token("Patient", Some("_sort=-_lastUpdated"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(entry_ids(&ascending), vec![a.clone(), b.clone(), c.clone()]);
    let mut reversed = entry_ids(&descending);
    reversed.reverse();
    assert_eq!(reversed, vec![a, b, c]);
}

#[tokio::test]
async fn sort_multi_key_orders_primary_then_explicit_secondary_key() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "sort-multi-key").await;
    let token = tenant_token("sort-multi-key");

    // Two ties on last_updated (x/y), one distinct later value (z).
    let x = create_patient(&pool, &token, "patient-x").await;
    let y = create_patient(&pool, &token, "patient-y").await;
    let z = create_patient(&pool, &token, "patient-z").await;
    let base = chrono::Utc::now();
    set_last_updated(&pool, "sort-multi-key", &x, base, -200).await;
    set_last_updated(&pool, "sort-multi-key", &y, base, -200).await;
    set_last_updated(&pool, "sort-multi-key", &z, base, -100).await;

    let tied = vec![x.clone(), y.clone()];
    let tied_desc = ids_ordered_by_id(&pool, "sort-multi-key", &tied, true).await;
    let tied_asc = ids_ordered_by_id(&pool, "sort-multi-key", &tied, false).await;
    assert_ne!(
        tied_desc, tied_asc,
        "test setup needs a tied pair whose id order actually differs by direction"
    );

    // Primary key breaks the x/y tie apart from z; explicit descending `_id`
    // as the secondary key must reverse the default (ascending) tiebreak
    // within the tied group, proving the second key is honored, not just an
    // automatically-appended ascending `_id`.
    let app = build_test_app_auth_required(pool.clone());
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("_sort=_lastUpdated,-_id"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(entry_ids(&body), [tied_desc, vec![z.clone()]].concat());

    // Flipping to ascending `_id` reverses the tiebreak but not the primary
    // grouping.
    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("_sort=_lastUpdated,_id"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(entry_ids(&body), [tied_asc, vec![z]].concat());
}

#[tokio::test]
async fn sort_paging_across_ties_returns_every_resource_exactly_once() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "sort-paging-ties").await;
    let token = tenant_token("sort-paging-ties");

    // Three ties at each of two distinct last_updated instants: 6 resources,
    // paged 2 at a time, so every page boundary falls inside a tied group.
    let mut ids = Vec::new();
    let base = chrono::Utc::now();
    for (i, offset) in [
        (0, -200),
        (1, -200),
        (2, -200),
        (3, -100),
        (4, -100),
        (5, -100),
    ] {
        let id = create_patient(&pool, &token, &format!("patient-tie-{i}")).await;
        set_last_updated(&pool, "sort-paging-ties", &id, base, offset).await;
        ids.push(id);
    }
    let expected_first_group =
        ids_ordered_by_id(&pool, "sort-paging-ties", &ids[0..3], false).await;
    let expected_second_group =
        ids_ordered_by_id(&pool, "sort-paging-ties", &ids[3..6], false).await;

    let mut collected = Vec::new();
    let mut query = Some("_sort=_lastUpdated&_count=2".to_owned());
    let mut hops = 0;
    while let Some(q) = query {
        let app = build_test_app_auth_required(pool.clone());
        let (status, body) =
            send_request(app, search_resource_with_token("Patient", Some(&q), &token)).await;
        assert_eq!(status, StatusCode::OK);
        collected.extend(entry_ids(&body));
        query = next_link_query(&body).map(|full| full.split_once('?').unwrap().1.to_owned());
        hops += 1;
        assert!(hops <= 10, "pagination should terminate");
    }

    assert_eq!(
        collected.len(),
        6,
        "every resource must appear exactly once"
    );
    assert_eq!(&collected[0..3], expected_first_group.as_slice());
    assert_eq!(&collected[3..6], expected_second_group.as_slice());
}

#[tokio::test]
async fn sort_self_and_next_links_preserve_sort_unchanged() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "sort-links").await;
    let token = tenant_token("sort-links");

    for id in ["patient-a", "patient-b", "patient-c"] {
        create_patient(&pool, &token, id).await;
    }

    let app = build_test_app_auth_required(pool.clone());
    let (status, first_page) = send_request(
        app,
        search_resource_with_token("Patient", Some("_sort=-_lastUpdated&_count=2"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(self_link_url(&first_page).contains("_sort=-_lastUpdated"));
    let next_query = next_link_query(&first_page).expect("next link expected");
    assert!(next_query.contains("_sort=-_lastUpdated"));

    // Following `next` must not require the client to re-supply `_sort`: the
    // link already carries it, and the second page's own links must still
    // carry it unchanged.
    let (path, query) = next_query.split_once('?').unwrap();
    assert_eq!(path, "/fhir/Patient");
    let app = build_test_app_auth_required(pool);
    let (status, second_page) = send_request(
        app,
        search_resource_with_token("Patient", Some(query), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(self_link_url(&second_page).contains("_sort=-_lastUpdated"));
}

#[tokio::test]
async fn sort_unsupported_and_unknown_keys_return_400() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "sort-unsupported").await;
    let token = tenant_token("sort-unsupported");
    create_patient(&pool, &token, "patient-a").await;

    for sort in ["status", "name", "nonexistent", "_id,status"] {
        let app = build_test_app_auth_required(pool.clone());
        let (status, body) = send_request(
            app,
            search_resource_with_token("Patient", Some(&format!("_sort={sort}")), &token),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "_sort={sort}");
        common::assert_operation_outcome(&body, "invalid");
    }
}

#[tokio::test]
async fn sort_repeated_parameter_returns_400() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "sort-repeated").await;
    let token = tenant_token("sort-repeated");
    create_patient(&pool, &token, "patient-a").await;

    let app = build_test_app_auth_required(pool);
    let (status, _) = send_request(
        app,
        search_resource_with_token("Patient", Some("_sort=_id&_sort=_lastUpdated"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sort_cursor_rejected_when_replayed_against_different_sort_or_filters() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "sort-cursor-mismatch").await;
    let token = tenant_token("sort-cursor-mismatch");

    for id in ["patient-a", "patient-b", "patient-c"] {
        create_patient(&pool, &token, id).await;
    }

    let app = build_test_app_auth_required(pool.clone());
    let (status, first_page) = send_request(
        app,
        search_resource_with_token("Patient", Some("_sort=_lastUpdated&_count=1"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let next_query = next_link_query(&first_page).expect("next link expected");
    let (_, query) = next_query.split_once('?').unwrap();
    let after_id = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("_after_id="))
        .expect("cursor expected")
        .to_owned();

    // Replay against a different `_sort`.
    let app = build_test_app_auth_required(pool.clone());
    let (status, _) = send_request(
        app,
        search_resource_with_token(
            "Patient",
            Some(&format!(
                "_sort=-_lastUpdated&_count=1&_after_id={after_id}"
            )),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Replay with the same `_sort` but a different filter set.
    let app = build_test_app_auth_required(pool);
    let (status, _) = send_request(
        app,
        search_resource_with_token(
            "Patient",
            Some(&format!(
                "_sort=_lastUpdated&_count=1&_after_id={after_id}&active=true"
            )),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sort_is_tenant_scoped() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "sort-tenant-a").await;
    clean_tenant(&pool, "sort-tenant-b").await;
    let token_a = tenant_token("sort-tenant-a");
    let token_b = tenant_token("sort-tenant-b");

    let a1 = create_patient(&pool, &token_a, "patient-a1").await;
    let a2 = create_patient(&pool, &token_a, "patient-a2").await;
    let b1 = create_patient(&pool, &token_b, "patient-b1").await;
    let base = chrono::Utc::now();
    set_last_updated(&pool, "sort-tenant-a", &a1, base, -300).await;
    set_last_updated(&pool, "sort-tenant-a", &a2, base, -100).await;
    set_last_updated(&pool, "sort-tenant-b", &b1, base, -200).await;

    let app = build_test_app_auth_required(pool);
    let (status, body) = send_request(
        app,
        search_resource_with_token("Patient", Some("_sort=_lastUpdated"), &token_a),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(entry_ids(&body), vec![a1, a2]);
    assert_eq!(body["total"], 2, "tenant b's resource must not be counted");
}

#[tokio::test]
async fn sort_by_last_updated_uses_an_index_not_a_sequential_scan() {
    let pool = setup_test_db().await;
    clean_tenant(&pool, "sort-explain").await;

    let mut tx = pool.begin().await.expect("begin tx");
    // Force the planner to prefer any usable index over a sequential scan.
    // On the tiny table an integration test creates, the planner would
    // otherwise legitimately prefer a seq scan on cost grounds alone; this
    // isolates the question this test actually cares about — whether an
    // index exists that can serve this WHERE + ORDER BY shape at all — from
    // a realistic-workload cost comparison, which belongs in a benchmark.
    sqlx::query("SET LOCAL enable_seqscan = off")
        .execute(&mut *tx)
        .await
        .expect("disable seqscan");

    let rows = sqlx::query(
        "EXPLAIN SELECT id, version_id, last_updated, resource FROM fhir_resources \
         WHERE tenant_id = $1 AND resource_type = 'Patient' \
         ORDER BY last_updated ASC, id ASC LIMIT 51",
    )
    .bind("sort-explain")
    .fetch_all(&mut *tx)
    .await
    .expect("explain should succeed");

    let plan: String = rows
        .iter()
        .map(|row| row.get::<String, _>("QUERY PLAN"))
        .collect::<Vec<_>>()
        .join("\n");
    tx.rollback().await.expect("rollback");

    assert!(
        !plan.contains("Seq Scan"),
        "expected the (tenant_id, last_updated, id) index to serve _sort=_lastUpdated, got plan:\n{plan}"
    );
    assert!(
        plan.contains("Index"),
        "expected an index scan in the plan, got:\n{plan}"
    );
}
