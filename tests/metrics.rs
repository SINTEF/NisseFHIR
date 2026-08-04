//! Integration tests for the privacy-safe Prometheus metrics endpoint.
//!
//! The metrics-rs recorder is process-global, so the recorder is installed
//! exactly once per test process (guarded by a mutex + OnceLock). All metric
//! assertions run inside a single `#[tokio::test]` so the shared in-flight
//! gauge is not perturbed by parallel tests within this binary.

mod common;

use std::sync::{Arc, OnceLock};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    middleware,
    routing::get,
};
use common::{build_test_app, lazy_pool, setup_test_db};
use fhir_server::metrics::{
    METRICS_CONTENT_TYPE, TelemetryState, http_metrics_middleware, telemetry_router,
};
use tower::ServiceExt;

static INIT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static TELEMETRY: OnceLock<TelemetryState> = OnceLock::new();

/// Install the process-global recorder exactly once, backed by a real
/// connected pool so the pool-occupancy gauges are meaningful.
async fn telemetry_state() -> &'static TelemetryState {
    if TELEMETRY.get().is_none() {
        let _guard = INIT.lock().await;
        if TELEMETRY.get().is_none() {
            let pool = setup_test_db().await;
            let state = TelemetryState::install(pool, 5).expect("recorder install");
            let _ = TELEMETRY.set(state);
        }
    }
    TELEMETRY.get().expect("telemetry initialized")
}

fn telemetry_app(state: &'static TelemetryState) -> Router {
    telemetry_router(state.clone())
}

/// Extract the numeric value of a rendered metric line whose prefix matches
/// `needle` (e.g. `nissefhir_http_requests_in_flight` or
/// `nissefhir_db_pool_connections{state="idle"}`).
fn metric_value(body: &str, needle: &str) -> f64 {
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix(needle) {
            let value = rest.trim().trim_start_matches('{').trim().trim_start();
            let value = value.rsplit(' ').next().unwrap_or("").trim();
            return value
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("bad value in '{line}'"));
        }
    }
    panic!("metric '{needle}' not found in:\n{body}");
}

#[tokio::test(flavor = "multi_thread")]
async fn metrics_end_to_end() {
    let state = telemetry_state().await;
    let telemetry = telemetry_app(state);

    let app = build_test_app(lazy_pool());

    // --- representative traffic ---
    // Successful request (no DB needed).
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/fhir/metadata")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Failing request: authenticated route without a token -> 401.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/fhir/Patient")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Concrete FHIR resource ID in the path (would be PHI if leaked).
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/fhir/Patient/SUPER-SECRET-ID-12345")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // PHI in the query string.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/fhir/Patient?name=John+Doe&identifier=SSN-42-9876&birthdate=1990-01-01")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Unmatched path -> 404.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/totally/unmatched/route")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Arbitrary HTTP extension methods must be normalized, never recorded
    // verbatim (otherwise clients could create unbounded Prometheus series).
    for custom in ["X-0001", "X-0002", "PROPFIND"] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(custom)
                    .uri("/fhir/metadata")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "custom method {custom} should be rejected by the route"
        );
    }

    // --- scrape the telemetry endpoint ---
    let response = telemetry
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        METRICS_CONTENT_TYPE,
        "metrics endpoint must use the Prometheus text exposition content type"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8_lossy(&body_bytes).to_string();

    // --- all six required application families are present ---
    for family in [
        "nissefhir_http_requests_total",
        "nissefhir_http_request_duration_seconds",
        "nissefhir_http_requests_in_flight",
        "nissefhir_db_pool_connections",
        "nissefhir_db_pool_max_connections",
        "nissefhir_build_info",
    ] {
        assert!(
            body.contains(family),
            "missing metric family '{family}' in:\n{body}"
        );
    }

    // Conventional process_* series from metrics-process.
    assert!(
        body.contains("process_cpu_seconds_total") || body.contains("process_start_time_seconds"),
        "process_* metrics missing in:\n{body}"
    );

    // Build info carries the package version.
    assert!(
        body.contains("nissefhir_build_info{version=\""),
        "build_info must carry the version label:\n{body}"
    );

    // Metric descriptions (HELP lines) are registered after the recorder is
    // installed and must be present in the rendered exposition.
    assert!(
        body.contains("# HELP nissefhir_http_requests_total"),
        "application metrics must carry HELP metadata:\n{body}"
    );
    assert!(
        body.contains("# HELP nissefhir_db_pool_connections"),
        "pool metrics must carry HELP metadata:\n{body}"
    );

    // --- normalized route templates, never concrete paths ---
    assert!(
        body.contains("method=\"GET\",route=\"/fhir/metadata\",status=\"200\""),
        "successful request must be recorded with the normalized route:\n{body}"
    );
    assert!(
        body.contains("route=\"/fhir/{resource_type}\""),
        "failing request must be recorded with the route template:\n{body}"
    );
    assert!(
        body.contains("status=\"401\""),
        "failing request status must be recorded:\n{body}"
    );
    assert!(
        body.contains("route=\"<unmatched>\""),
        "unmatched request must use the <unmatched> route label:\n{body}"
    );
    assert!(
        body.contains("status=\"404\""),
        "unmatched request status must be recorded:\n{body}"
    );

    // Extension methods are collapsed to a single OTHER label with the
    // bounded supported set, and the raw method tokens never leak.
    assert!(
        body.contains("method=\"OTHER\""),
        "extension methods must be recorded as OTHER:\n{body}"
    );
    for custom in ["X-0001", "X-0002", "PROPFIND"] {
        assert!(
            !body.contains(custom),
            "custom method '{custom}' must not appear as a label:\n{body}"
        );
    }

    // --- in-flight gauge returns to zero ---
    assert_eq!(
        metric_value(&body, "nissefhir_http_requests_in_flight"),
        0.0,
        "in-flight gauge must return to zero after all requests complete"
    );

    // Cancellation must also release the drop guard. Use a handler that stays
    // pending until its task is aborted, observe one in-flight request, then
    // verify the gauge returns to zero after cancellation.
    let entered = Arc::new(tokio::sync::Notify::new());
    let handler_entered = entered.clone();
    let blocking_app = Router::new()
        .route(
            "/block",
            get(move || {
                let handler_entered = handler_entered.clone();
                async move {
                    handler_entered.notify_one();
                    std::future::pending::<()>().await;
                    StatusCode::OK
                }
            }),
        )
        .layer(middleware::from_fn(http_metrics_middleware));
    let request_task = tokio::spawn(
        blocking_app.oneshot(
            Request::builder()
                .method("GET")
                .uri("/block")
                .body(Body::empty())
                .unwrap(),
        ),
    );
    tokio::time::timeout(std::time::Duration::from_secs(2), entered.notified())
        .await
        .expect("blocking handler must start");

    let in_flight_response = telemetry
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let in_flight_body = String::from_utf8_lossy(
        &axum::body::to_bytes(in_flight_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .to_string();
    assert_eq!(
        metric_value(&in_flight_body, "nissefhir_http_requests_in_flight"),
        1.0,
        "pending request must increment the in-flight gauge"
    );

    request_task.abort();
    let _ = request_task.await;
    let cancelled_response = telemetry
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cancelled_body = String::from_utf8_lossy(
        &axum::body::to_bytes(cancelled_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .to_string();
    assert_eq!(
        metric_value(&cancelled_body, "nissefhir_http_requests_in_flight"),
        0.0,
        "cancelling a request must release the in-flight gauge guard"
    );

    // --- database pool gauges add up to the current pool size ---
    let idle = metric_value(&body, "nissefhir_db_pool_connections{state=\"idle\"}");
    let in_use = metric_value(&body, "nissefhir_db_pool_connections{state=\"in_use\"}");
    let size = idle + in_use;
    assert!(
        size > 0.0,
        "a connected pool should report non-zero occupancy, got idle={idle} in_use={in_use}"
    );
    // The pool was built with max_connections = 5; occupancy cannot exceed it.
    let max = metric_value(&body, "nissefhir_db_pool_max_connections");
    assert_eq!(
        max, 5.0,
        "max connections gauge must reflect configured capacity"
    );
    assert!(
        size <= max,
        "pool occupancy ({size}) cannot exceed max connections ({max})"
    );

    // --- privacy: no PHI leaks into the rendered exposition ---
    for leaked in [
        "SUPER-SECRET-ID-12345",
        "John",
        "Doe",
        "SSN-42-9876",
        "1990-01-01",
        "name=",
        "identifier=",
        "birthdate=",
        "Patient/",
    ] {
        assert!(
            !body.contains(leaked),
            "privacy leak: '{leaked}' found in metrics output:\n{body}"
        );
    }

    // --- repeated scrapes must not accumulate gauge values ---
    let response = telemetry
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body2 = String::from_utf8_lossy(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .to_string();
    assert_eq!(
        metric_value(&body2, "nissefhir_http_requests_in_flight"),
        0.0,
        "in-flight gauge must stay at zero across repeated scrapes"
    );
    let idle2 = metric_value(&body2, "nissefhir_db_pool_connections{state=\"idle\"}");
    let in_use2 = metric_value(&body2, "nissefhir_db_pool_connections{state=\"in_use\"}");
    assert!(
        (idle2 + in_use2 - (idle + in_use)).abs() < f64::EPSILON,
        "pool occupancy must be re-sampled, not accumulated across scrapes"
    );

    // --- other telemetry paths return 404 ---
    for path in ["/metrics/extra", "/health", "/"] {
        let resp = telemetry
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "telemetry listener must 404 on '{path}'"
        );
    }
}

// --- unit-level checks that need no recorder ---

/// The configured max-connections gauge must equal the pool capacity passed at
/// install time (checked indirectly by the end-to-end test above, but kept as
/// a cheap regression guard).
#[test]
fn content_type_is_prometheus_text() {
    assert_eq!(
        METRICS_CONTENT_TYPE,
        "text/plain; version=0.0.4; charset=utf-8"
    );
}
