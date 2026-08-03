//! Integration tests for FHIR date search semantics.
//!
//! These exercise the precision-aware date search implemented in
//! `src/search_params/date.rs` end-to-end against a real PostgreSQL
//! instance, covering:
//!
//! - The nine FHIR comparator prefixes (`eq`, `ne`, `gt`, `ge`, `lt`, `le`,
//!   `sa`, `eb`, `ap`).
//! - Precision expansion for year, year-month, date, and dateTime inputs.
//! - Timezone normalisation for dateTimes with explicit offsets and Zulu.
//! - Boundary behaviour at half-open period edges.
//! - Rejection of malformed dates and unsupported prefixes with `400`.
//!
//! Each test uses its own tenant so they can run in parallel without
//! interfering with one another.

mod common;

use axum::http::StatusCode;
use serde_json::{Value, json};

use common::{
    build_test_app_auth_required, clean_tenant, post_resource_with_token,
    search_resource_with_token, send_request, setup_test_db, tenant_token,
};

/// Build a Patient with the given `birthDate` string. We use Patient because
/// the registry declares `birthdate` as a date search parameter (single
/// field path `["birthDate"]`), so the full precision machinery in
/// `push_date_filter` runs on the value.
fn patient_with_birthdate(birth_date: &str) -> Value {
    json!({
        "resourceType": "Patient",
        "name": [{"family": "Test", "given": ["Test"]}],
        "birthDate": birth_date,
    })
}

/// Build a Condition with `onsetDateTime` as a dateTime search parameter.
/// This lets us exercise full-instant precision instead of date-only.
fn condition_with_onset(onset: &str) -> Value {
    json!({
        "resourceType": "Condition",
        "clinicalStatus": {
            "coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/condition-clinical",
                "code": "active",
            }]
        },
        "code": {
            "coding": [{"system": "http://snomed.info/sct", "code": "386661006"}]
        },
        "subject": {"reference": "Patient/example"},
        "onsetDateTime": onset,
    })
}

/// Create a Patient, returning the server-assigned id.
async fn create_patient(pool: &sqlx::PgPool, token: &str, patient: Value) -> String {
    let app = build_test_app_auth_required(pool.clone());
    let (status, body) =
        send_request(app, post_resource_with_token("Patient", &patient, token)).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body}");
    body["id"].as_str().map(ToOwned::to_owned).unwrap()
}

async fn create_condition(pool: &sqlx::PgPool, token: &str, cond: Value) -> String {
    let app = build_test_app_auth_required(pool.clone());
    let (status, body) =
        send_request(app, post_resource_with_token("Condition", &cond, token)).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body}");
    body["id"].as_str().map(ToOwned::to_owned).unwrap()
}

async fn create_encounter(pool: &sqlx::PgPool, token: &str, enc: Value) -> String {
    let app = build_test_app_auth_required(pool.clone());
    let (status, body) =
        send_request(app, post_resource_with_token("Encounter", &enc, token)).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body}");
    body["id"].as_str().map(ToOwned::to_owned).unwrap()
}

async fn search(pool: &sqlx::PgPool, token: &str, query: &str) -> (StatusCode, Value) {
    let app = build_test_app_auth_required(pool.clone());
    send_request(
        app,
        search_resource_with_token("Patient", Some(query), token),
    )
    .await
}

async fn search_condition(pool: &sqlx::PgPool, token: &str, query: &str) -> (StatusCode, Value) {
    let app = build_test_app_auth_required(pool.clone());
    send_request(
        app,
        search_resource_with_token("Condition", Some(query), token),
    )
    .await
}

async fn search_encounter(pool: &sqlx::PgPool, token: &str, query: &str) -> (StatusCode, Value) {
    let app = build_test_app_auth_required(pool.clone());
    send_request(
        app,
        search_resource_with_token("Encounter", Some(query), token),
    )
    .await
}

/// Extract the list of patient ids returned by a search bundle.
fn entry_ids(body: &Value) -> Vec<String> {
    body["entry"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| e["resource"]["id"].as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Section A: prefix semantics against a single known birthdate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eq_full_date_matches_exact_birthdate() {
    let pool = setup_test_db().await;
    let tenant = "date-eq-exact";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_eq_exact_p1 =
        create_patient(&pool, &token, patient_with_birthdate("1974-12-25")).await;

    let (status, body) = search(&pool, &token, "birthdate=1974-12-25").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["total"], 1);
    assert_eq!(entry_ids(&body), [date_eq_exact_p1]);

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn eq_year_precision_matches_any_day_in_that_year() {
    let pool = setup_test_db().await;
    let tenant = "date-eq-year";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_eq_year_a = create_patient(&pool, &token, patient_with_birthdate("1974-01-01")).await;
    let date_eq_year_b = create_patient(&pool, &token, patient_with_birthdate("1974-12-31")).await;
    let date_eq_year_c = create_patient(&pool, &token, patient_with_birthdate("1975-01-01")).await;

    let (status, body) = search(&pool, &token, "birthdate=1974").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_eq_year_a));
    assert!(ids.contains(&date_eq_year_b));
    assert!(!ids.contains(&date_eq_year_c));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn eq_year_month_precision_matches_dates_in_that_month() {
    let pool = setup_test_db().await;
    let tenant = "date-eq-month";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_eq_month_a = create_patient(&pool, &token, patient_with_birthdate("1974-12-01")).await;
    let date_eq_month_b = create_patient(&pool, &token, patient_with_birthdate("1974-12-31")).await;
    let date_eq_month_c = create_patient(&pool, &token, patient_with_birthdate("1975-01-01")).await;

    let (status, body) = search(&pool, &token, "birthdate=1974-12").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_eq_month_a));
    assert!(ids.contains(&date_eq_month_b));
    assert!(!ids.contains(&date_eq_month_c));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn ne_excludes_dates_overlapping_search_period() {
    let pool = setup_test_db().await;
    let tenant = "date-ne";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_ne_in = create_patient(&pool, &token, patient_with_birthdate("1974-12-25")).await;
    let date_ne_out = create_patient(&pool, &token, patient_with_birthdate("1980-06-15")).await;

    let (status, body) = search(&pool, &token, "birthdate=ne1974-12-25").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(!ids.contains(&date_ne_in));
    assert!(ids.contains(&date_ne_out));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn gt_returns_patients_strictly_after_search_end() {
    let pool = setup_test_db().await;
    let tenant = "date-gt";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // Search end is 1975-01-01T00:00:00Z (year-precision boundary).
    let date_gt_before = create_patient(&pool, &token, patient_with_birthdate("1974-06-15")).await;
    let date_gt_after = create_patient(&pool, &token, patient_with_birthdate("1975-01-02")).await;

    let (status, body) = search(&pool, &token, "birthdate=gt1974").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_gt_after));
    assert!(!ids.contains(&date_gt_before));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn ge_includes_dates_at_or_after_search_start() {
    let pool = setup_test_db().await;
    let tenant = "date-ge";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_ge_before = create_patient(&pool, &token, patient_with_birthdate("1973-12-31")).await;
    let date_ge_start = create_patient(&pool, &token, patient_with_birthdate("1974-01-01")).await;
    let date_ge_after = create_patient(&pool, &token, patient_with_birthdate("1974-12-31")).await;

    let (status, body) = search(&pool, &token, "birthdate=ge1974").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_ge_start));
    assert!(ids.contains(&date_ge_after));
    assert!(!ids.contains(&date_ge_before));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn lt_returns_patients_strictly_before_search_start() {
    let pool = setup_test_db().await;
    let tenant = "date-lt";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_lt_before = create_patient(&pool, &token, patient_with_birthdate("1973-12-31")).await;
    let date_lt_after = create_patient(&pool, &token, patient_with_birthdate("1974-06-15")).await;

    let (status, body) = search(&pool, &token, "birthdate=lt1974").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_lt_before));
    assert!(!ids.contains(&date_lt_after));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn le_includes_dates_at_or_before_search_end() {
    let pool = setup_test_db().await;
    let tenant = "date-le";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_le_before = create_patient(&pool, &token, patient_with_birthdate("1973-12-31")).await;
    let date_le_end = create_patient(&pool, &token, patient_with_birthdate("1974-12-31")).await;
    let date_le_after = create_patient(&pool, &token, patient_with_birthdate("1975-01-02")).await;

    let (status, body) = search(&pool, &token, "birthdate=le1974").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_le_before));
    assert!(ids.contains(&date_le_end));
    assert!(!ids.contains(&date_le_after));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn sa_requires_resource_start_after_search_end() {
    let pool = setup_test_db().await;
    let tenant = "date-sa";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_sa_before = create_patient(&pool, &token, patient_with_birthdate("1974-06-15")).await;
    let date_sa_after = create_patient(&pool, &token, patient_with_birthdate("1975-01-02")).await;

    let (status, body) = search(&pool, &token, "birthdate=sa1974").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_sa_after));
    assert!(!ids.contains(&date_sa_before));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn eb_requires_resource_end_before_search_start() {
    let pool = setup_test_db().await;
    let tenant = "date-eb";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_eb_before = create_patient(&pool, &token, patient_with_birthdate("1973-12-31")).await;
    let date_eb_after = create_patient(&pool, &token, patient_with_birthdate("1974-06-15")).await;

    let (status, body) = search(&pool, &token, "birthdate=eb1974").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_eb_before));
    assert!(!ids.contains(&date_eb_after));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn ap_matches_approximately_the_same_year() {
    let pool = setup_test_db().await;
    let tenant = "date-ap";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // ap2000 widens by ±10% of one year (~36.5 days), so [1999-11-25,
    // 2001-02-06) approximately. A patient born at end of 2000 and one in
    // early 2001 should not match.
    let date_ap_in = create_patient(&pool, &token, patient_with_birthdate("2000-01-15")).await;
    let date_ap_far = create_patient(&pool, &token, patient_with_birthdate("2001-06-15")).await;

    let (status, body) = search(&pool, &token, "birthdate=ap2000").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_ap_in));
    assert!(!ids.contains(&date_ap_far));

    clean_tenant(&pool, tenant).await;
}

// ---------------------------------------------------------------------------
// Section B: timezone normalisation for dateTime search parameters
// ---------------------------------------------------------------------------

#[tokio::test]
async fn datetime_search_with_zulu_matches_utc_instant() {
    let pool = setup_test_db().await;
    let tenant = "date-tz-zulu";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let _date_tz_zulu_c1 =
        create_condition(&pool, &token, condition_with_onset("2024-06-15T10:30:00Z")).await;

    let (status, body) = search_condition(&pool, &token, "onset-date=2024-06-15T10:30:00Z").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["total"], 1);

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn datetime_search_with_offset_normalises_to_utc() {
    let pool = setup_test_db().await;
    let tenant = "date-tz-offset";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // Search with +05:00 offset → 10:30 +05:00 == 05:30 UTC. Searching UTC
    // for the same instant must match.
    let _date_tz_offset_c1 =
        create_condition(&pool, &token, condition_with_onset("2024-06-15T05:30:00Z")).await;

    let (status, body) =
        search_condition(&pool, &token, "onset-date=2024-06-15T10:30:00%2B05:00").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn datetime_search_with_negative_offset_normalises_to_utc() {
    let pool = setup_test_db().await;
    let tenant = "date-tz-neg";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // 10:30 -08:00 == 18:30 UTC.
    let _date_tz_neg_c1 =
        create_condition(&pool, &token, condition_with_onset("2024-06-15T18:30:00Z")).await;

    let (status, body) =
        search_condition(&pool, &token, "onset-date=2024-06-15T10:30:00-08:00").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn datetime_search_across_day_boundary_with_offset() {
    let pool = setup_test_db().await;
    let tenant = "date-tz-bd";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // 23:30 +05:00 == 18:30 UTC of the previous day.
    let _date_tz_bd_c1 =
        create_condition(&pool, &token, condition_with_onset("2024-06-14T18:30:00Z")).await;

    let (status, body) =
        search_condition(&pool, &token, "onset-date=2024-06-14T23:30:00%2B05:00").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn fractional_seconds_are_ignored_consistently() {
    let pool = setup_test_db().await;
    let tenant = "date-fractional";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let condition_id = create_condition(
        &pool,
        &token,
        condition_with_onset("2024-06-15T10:30:00.500Z"),
    )
    .await;

    let (status, body) =
        search_condition(&pool, &token, "onset-date=2024-06-15T10:30:00.500Z").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert!(entry_ids(&body).contains(&condition_id));

    clean_tenant(&pool, tenant).await;
}

// ---------------------------------------------------------------------------
// Section C: precision mismatch (stored date precision vs search precision)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn year_precision_search_matches_datetime_resource_in_that_year() {
    let pool = setup_test_db().await;
    let tenant = "date-prec-ymatch";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let _date_prec_ymatch_c1 =
        create_condition(&pool, &token, condition_with_onset("2024-06-15T10:30:00Z")).await;

    let (status, body) = search_condition(&pool, &token, "onset-date=2024").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn month_precision_search_matches_datetime_resource_in_that_month() {
    let pool = setup_test_db().await;
    let tenant = "date-prec-mmatch";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_prec_mmatch_in =
        create_condition(&pool, &token, condition_with_onset("2024-06-15T10:30:00Z")).await;
    let date_prec_mmatch_out =
        create_condition(&pool, &token, condition_with_onset("2024-07-01T00:00:00Z")).await;

    let (status, body) = search_condition(&pool, &token, "onset-date=2024-06").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_prec_mmatch_in));
    assert!(!ids.contains(&date_prec_mmatch_out));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn date_precision_search_does_not_match_next_day_instant() {
    let pool = setup_test_db().await;
    let tenant = "date-prec-day";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // Resource instant is at 2024-06-16T00:00:00Z, which is the *exclusive*
    // upper bound of the period [2024-06-15, 2024-06-16). A date search for
    // 2024-06-15 must NOT match.
    let _date_prec_day_edge =
        create_condition(&pool, &token, condition_with_onset("2024-06-16T00:00:00Z")).await;

    let (status, body) = search_condition(&pool, &token, "onset-date=2024-06-15").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["total"], 0,
        "half-open upper bound should exclude 00:00:00Z of the next day"
    );

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn broad_resource_precision_distinguishes_range_prefixes() {
    let pool = setup_test_db().await;
    let tenant = "date-prec-broad-resource";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // A resource value of `1974` represents the full year. A day-precision
    // search range does not contain that resource range, but the resource has
    // portions both below and above the searched day. This is the case that
    // distinguishes gt/lt from sa/eb and verifies eq/ne containment.
    let patient_id = create_patient(&pool, &token, patient_with_birthdate("1974")).await;

    for (query, should_match) in [
        ("birthdate=eq1974-12-25", false),
        ("birthdate=ne1974-12-25", true),
        ("birthdate=gt1974-12-25", true),
        ("birthdate=ge1974-12-25", true),
        ("birthdate=lt1974-12-25", true),
        ("birthdate=le1974-12-25", true),
        ("birthdate=sa1974-12-25", false),
        ("birthdate=eb1974-12-25", false),
    ] {
        let (status, body) = search(&pool, &token, query).await;
        assert_eq!(status, StatusCode::OK, "query {query}: {body}");
        assert_eq!(
            entry_ids(&body).contains(&patient_id),
            should_match,
            "unexpected range result for {query}: {body}"
        );
    }

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn complete_period_uses_explicit_resource_bounds() {
    let pool = setup_test_db().await;
    let tenant = "date-period-complete";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let encounter = json!({
        "resourceType": "Encounter",
        "status": "completed",
        "class": [{
            "coding": [{"system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "code": "AMB"}]
        }],
        "actualPeriod": {
            "start": "2024-06-01",
            "end": "2024-06-30"
        },
        "subject": {"reference": "Patient/example"},
    });
    let encounter_id = create_encounter(&pool, &token, encounter).await;

    for (query, should_match) in [
        ("date=eq2024-06", true),
        ("date=eq2024-06-15", false),
        ("date=gt2024-06-15", true),
        ("date=lt2024-06-15", true),
        ("date=sa2024-06-15", false),
        ("date=eb2024-06-15", false),
    ] {
        let (status, body) = search_encounter(&pool, &token, query).await;
        assert_eq!(status, StatusCode::OK, "query {query}: {body}");
        assert_eq!(
            entry_ids(&body).contains(&encounter_id),
            should_match,
            "unexpected Period result for {query}: {body}"
        );
    }

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn date_choice_parameter_matches_period_alternative() {
    let pool = setup_test_db().await;
    let tenant = "date-choice-period";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let condition = json!({
        "resourceType": "Condition",
        "clinicalStatus": {
            "coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/condition-clinical",
                "code": "active"
            }]
        },
        "code": {
            "coding": [{"system": "http://snomed.info/sct", "code": "386661006"}]
        },
        "subject": {"reference": "Patient/example"},
        "onsetPeriod": {"start": "2024-06-01", "end": "2024-06-30"}
    });
    let condition_id = create_condition(&pool, &token, condition).await;

    let (status, body) = search_condition(&pool, &token, "onset-date=eq2024-06").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert!(entry_ids(&body).contains(&condition_id));

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn date_choice_parameter_matches_timing_events() {
    let pool = setup_test_db().await;
    let tenant = "date-choice-timing";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let observation = json!({
        "resourceType": "Observation",
        "status": "final",
        "code": {
            "coding": [{"system": "http://loinc.org", "code": "8867-4"}]
        },
        "subject": {"reference": "Patient/example"},
        "effectiveTiming": {
            "event": ["2024-06-15T10:30:00Z", "2024-06-16T10:30:00Z"]
        }
    });
    let app = build_test_app_auth_required(pool.clone());
    let (status, created) = send_request(
        app,
        post_resource_with_token("Observation", &observation, &token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {created}");
    let observation_id = created["id"].as_str().unwrap().to_owned();

    for (query, should_match) in [
        ("date=eq2024-06", true),
        ("date=eq2024-06-16", false),
        ("date=gt2024-06-15", true),
    ] {
        let app = build_test_app_auth_required(pool.clone());
        let (status, body) = send_request(
            app,
            search_resource_with_token("Observation", Some(query), &token),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "query {query}: {body}");
        assert_eq!(
            entry_ids(&body).contains(&observation_id),
            should_match,
            "unexpected Timing result for {query}: {body}"
        );
    }

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn open_ended_period_uses_infinite_upper_bound() {
    let pool = setup_test_db().await;
    let tenant = "date-period-open";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let encounter = json!({
        "resourceType": "Encounter",
        "status": "in-progress",
        "class": [{
            "coding": [{"system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "code": "AMB"}]
        }],
        "actualPeriod": {"start": "2024-06-15"},
        "subject": {"reference": "Patient/example"},
    });
    let encounter_id = create_encounter(&pool, &token, encounter).await;

    for (query, should_match) in [
        ("date=eq2024", false),
        ("date=ge2024", true),
        ("date=le2024", true),
        ("date=sa2024-06-01", true),
        ("date=eb2024-06-01", false),
    ] {
        let (status, body) = search_encounter(&pool, &token, query).await;
        assert_eq!(status, StatusCode::OK, "query {query}: {body}");
        assert_eq!(
            entry_ids(&body).contains(&encounter_id),
            should_match,
            "unexpected open Period result for {query}: {body}"
        );
    }

    clean_tenant(&pool, tenant).await;
}

// ---------------------------------------------------------------------------
// Section D: half-open boundary behaviour
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eq_year_boundary_excludes_january_first_of_next_year() {
    let pool = setup_test_db().await;
    let tenant = "date-bdy-year";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // 1974 → [1974-01-01, 1975-01-01). A birthdate of 1975-01-01 lies outside.
    let _date_bdy_year_1975 =
        create_patient(&pool, &token, patient_with_birthdate("1975-01-01")).await;

    let (status, body) = search(&pool, &token, "birthdate=eq1974").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn eq_month_boundary_excludes_first_of_next_month() {
    let pool = setup_test_db().await;
    let tenant = "date-bdy-month";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let _date_bdy_month_next =
        create_patient(&pool, &token, patient_with_birthdate("1974-07-01")).await;

    let (status, body) = search(&pool, &token, "birthdate=eq1974-06").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn date_precision_is_stable_across_dst_transition() {
    let pool = setup_test_db().await;
    let tenant = "date-dst";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let patient_id = create_patient(&pool, &token, patient_with_birthdate("2024-03-31")).await;
    let (status, body) = search(&pool, &token, "birthdate=eq2024-03-31").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert!(entry_ids(&body).contains(&patient_id));

    clean_tenant(&pool, tenant).await;
}

// ---------------------------------------------------------------------------
// Section E: invalid input → 400 Bad Request
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invalid_month_returns_400() {
    let pool = setup_test_db().await;
    let tenant = "date-bad-month";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let (status, body) = search(&pool, &token, "birthdate=2024-13").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(body["issue"][0]["code"], "invalid");

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn invalid_day_returns_400() {
    let pool = setup_test_db().await;
    let tenant = "date-bad-day";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let (status, body) = search(&pool, &token, "birthdate=2024-02-30").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn invalid_timezone_offset_returns_400() {
    let pool = setup_test_db().await;
    let tenant = "date-bad-tz";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    let (status, body) = search(&pool, &token, "birthdate=2024-01-01T10:30:00+99:00").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");

    clean_tenant(&pool, tenant).await;
}

#[tokio::test]
async fn malformed_date_returns_400() {
    let pool = setup_test_db().await;
    let tenant = "date-malformed";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    for value in [
        "abc",
        "2024-13",
        "2024-02-31",
        "xx2024-01-01",
        "2024-01-01T",
        "2024-01-01T10:30:Z",
    ] {
        let (status, body) = search(&pool, &token, &format!("birthdate={value}")).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "expected 400 for malformed date '{value}', got body: {body}"
        );
        assert_eq!(body["issue"][0]["code"], "invalid");
    }

    clean_tenant(&pool, tenant).await;
}

// ---------------------------------------------------------------------------
// Section F: backward compatibility — the default (no-prefix) value behaves
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_prefix_defaults_to_eq() {
    let pool = setup_test_db().await;
    let tenant = "date-default";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let _date_default_p1 =
        create_patient(&pool, &token, patient_with_birthdate("1974-12-25")).await;

    // Without an explicit prefix, the request must behave the same as `eq`.
    let (status, body) = search(&pool, &token, "birthdate=1974").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);

    clean_tenant(&pool, tenant).await;
}

// ---------------------------------------------------------------------------
// Section G: OR-combined date search values honour precision per term
// ---------------------------------------------------------------------------

#[tokio::test]
async fn or_combined_date_values_match_per_term() {
    let pool = setup_test_db().await;
    let tenant = "date-or";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);
    let date_or_1974 = create_patient(&pool, &token, patient_with_birthdate("1974-12-25")).await;
    let date_or_1990 = create_patient(&pool, &token, patient_with_birthdate("1990-06-15")).await;
    let date_or_2000 = create_patient(&pool, &token, patient_with_birthdate("2000-01-01")).await;

    let (status, body) = search(&pool, &token, "birthdate=1974,1990").await;
    assert_eq!(status, StatusCode::OK);
    let ids = entry_ids(&body);
    assert!(ids.contains(&date_or_1974));
    assert!(ids.contains(&date_or_1990));
    assert!(!ids.contains(&date_or_2000));

    clean_tenant(&pool, tenant).await;
}

// ---------------------------------------------------------------------------
// Section H: nested date field (Encounter.actualPeriod.start) coverage
// ---------------------------------------------------------------------------

#[tokio::test]
async fn nested_date_field_uses_precision_aware_predicate() {
    let pool = setup_test_db().await;
    let tenant = "date-nested";
    clean_tenant(&pool, tenant).await;
    let token = tenant_token(tenant);

    // Encounter defines `date-start` as a Date search parameter with the
    // nested path `actualPeriod.start` — exercising the nested branch of
    // push_date_filter with year-precision search semantics.
    let encounter = json!({
        "resourceType": "Encounter",
        "status": "completed",
        "class": [{
            "coding": [{"system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "code": "AMB"}]
        }],
        "actualPeriod": {"start": "2024-06-15T10:30:00Z"},
        "subject": {"reference": "Patient/example"},
    });

    let _enc_id = create_encounter(&pool, &token, encounter).await;

    // `ge2024-01-01` includes the encounter whose start is 2024-06-15T10:30:00Z.
    let app = build_test_app_auth_required(pool.clone());
    let (status, body) = send_request(
        app,
        search_resource_with_token("Encounter", Some("date-start=ge2024-01-01"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert!(
        body["total"].as_i64().unwrap_or(0) >= 1,
        "expected nested-date `ge` search to match, got {body}"
    );

    // `lt2024-01-01` excludes anything from 2024-06-15 onwards.
    let app = build_test_app_auth_required(pool.clone());
    let (status, body) = send_request(
        app,
        search_resource_with_token("Encounter", Some("date-start=lt2024-01-01"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(
        body["total"], 0,
        "expected lt search to exclude the encounter"
    );

    clean_tenant(&pool, tenant).await;
}
