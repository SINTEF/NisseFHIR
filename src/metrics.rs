//! Privacy-safe Prometheus metrics.
//!
//! NisseFHIR exposes a small, explicitly documented set of low-cardinality
//! metrics on a dedicated telemetry listener so operators can measure request
//! rate, error rate, latency, saturation, and process health without deriving
//! them from logs.
//!
//! # Privacy
//!
//! FHIR requests can carry protected health information (PHI) in resource IDs,
//! search parameters, headers, and bodies. Metrics are therefore subject to
//! the same PHI policy as request logs:
//!
//! * the only request-derived label is the normalized Axum route template
//!   (`/fhir/{resource_type}/{id}`, never a concrete path);
//! * the raw URI, query string, resource ID, patient/subject ID, tenant ID,
//!   authorization information, body, database statement, and error message
//!   are never used as labels;
//! * no per-resource-type, search-parameter, user, client, issuer, or token
//!   claim labels exist. Any future label MUST have a documented, enforced
//!   finite value set;
//! * no global labels (pod name, namespace, environment, Git SHA) are added
//!   here — Prometheus discovery/relabeling should supply deployment metadata.
//!
//! # Implementation notes
//!
//! This module owns the [`metrics`] facade and a locally written HTTP
//! middleware instead of `axum-prometheus`, so one recorder serves HTTP,
//! database, authentication, and (future) background task metrics and the
//! allowed labels stay explicit in review. The exporter's self-hosted
//! listener is deliberately not used: only its recorder is installed and the
//! returned [`PrometheusHandle`] is served from an application-owned Axum
//! listener so startup errors and graceful shutdown remain under NisseFHIR's
//! control.

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use axum::{
    Router,
    extract::{MatchedPath, Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use metrics_process::Collector;
use sqlx::PgPool;

/// Content type mandated by the Prometheus text exposition format 0.0.4.
pub const METRICS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

/// Explicit histogram buckets tuned for an HTTP service (seconds).
const HTTP_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Runtime state backing the telemetry listener.
///
/// Holds the Prometheus render handle, the process metric collector, and the
/// PostgreSQL pool used to sample occupancy gauges immediately before each
/// scrape (no database query is performed).
#[derive(Clone)]
pub struct TelemetryState {
    handle: PrometheusHandle,
    process: Arc<Collector>,
    pool: PgPool,
}

impl TelemetryState {
    /// Install the process-global recorder and initialize every metric.
    ///
    /// Must be called exactly once per process, before any instrumented work
    /// begins. Registers descriptions for the application metric families,
    /// installs the `metrics-exporter-prometheus` recorder, describes the
    /// conventional `process_*` series, and initializes the startup gauges
    /// (build info and pool capacity).
    ///
    /// # Errors
    ///
    /// Fails if the recorder is already installed or the histogram buckets
    /// are invalid.
    pub fn install(pool: PgPool, max_connections: u32) -> Result<Self> {
        let builder = PrometheusBuilder::new()
            .set_buckets(HTTP_BUCKETS)
            .map_err(|e| anyhow!("invalid metrics histogram buckets: {e}"))?;
        let handle = builder
            .install_recorder()
            .context("failed to install metrics recorder")?;
        let process = Arc::new(Collector::default());
        process.describe();
        // Register metric descriptions only after the recorder is installed; the
        // `describe_*` macros route to the global recorder, so calling them any
        // earlier would send the metadata to a no-op and the HELP lines would be
        // lost from the rendered exposition.
        describe_all();
        // Initialize gauges once at startup.
        metrics::gauge!("nissefhir_db_pool_max_connections").set(f64::from(max_connections));
        metrics::gauge!("nissefhir_build_info", "version" => env!("CARGO_PKG_VERSION")).set(1.0);
        Ok(Self {
            handle,
            process,
            pool,
        })
    }

    /// Sample the pool occupancy gauges, refresh `process_*` metrics, and
    /// render the full Prometheus text exposition.
    ///
    /// The idle and in-use gauges always sum to the current pool size; a
    /// clamped idle snapshot guards against transient disagreement between the
    /// pool's two independent atomic reads.
    pub fn render(&self) -> String {
        let size = self.pool.size() as usize;
        let idle = self.pool.num_idle().min(size);
        let in_use = size - idle;
        metrics::gauge!("nissefhir_db_pool_connections", "state" => "idle").set(idle as f64);
        metrics::gauge!("nissefhir_db_pool_connections", "state" => "in_use").set(in_use as f64);
        self.process.collect();
        self.handle.render()
    }
}

/// Build the telemetry router.
///
/// Serves `GET /metrics` with the Prometheus text exposition. All other paths
/// fall through to Axum's default `404 Not Found`.
pub fn telemetry_router(state: TelemetryState) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}

async fn metrics_handler(State(state): State<TelemetryState>) -> Response {
    let body = state.render();
    ([(header::CONTENT_TYPE, METRICS_CONTENT_TYPE)], body).into_response()
}

/// Register descriptions and units for the application metric families.
///
/// Called once during recorder installation.
fn describe_all() {
    metrics::describe_counter!(
        "nissefhir_http_requests_total",
        metrics::Unit::Count,
        "Completed requests by HTTP method, normalized Axum route template, and numeric response status"
    );
    metrics::describe_histogram!(
        "nissefhir_http_request_duration_seconds",
        metrics::Unit::Seconds,
        "End-to-end request duration in seconds"
    );
    metrics::describe_gauge!(
        "nissefhir_http_requests_in_flight",
        metrics::Unit::Count,
        "Requests currently executing, including requests that eventually fail"
    );
    metrics::describe_gauge!(
        "nissefhir_db_pool_connections",
        metrics::Unit::Count,
        "Current SQLx pool occupancy, by state (idle or in_use)"
    );
    metrics::describe_gauge!(
        "nissefhir_db_pool_max_connections",
        metrics::Unit::Count,
        "Configured SQLx pool capacity"
    );
    metrics::describe_gauge!(
        "nissefhir_build_info",
        metrics::Unit::Count,
        "NisseFHIR build information"
    );
}

/// Privacy-safe HTTP metrics middleware.
///
/// Records the completed-request counter and duration histogram using only
/// the HTTP method, the normalized Axum route template, and the numeric
/// status — never the raw URI, query string, or any PHI-bearing input. An
/// in-flight gauge is incremented on entry and decremented via a drop guard,
/// so cancellation and early returns cannot leak the count.
pub async fn http_metrics_middleware(request: Request, next: Next) -> Response {
    let method = normalize_method(request.method().as_str()).to_owned();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| UNMATCHED_ROUTE.to_owned());

    // Increment on construction; the drop guard guarantees the gauge returns
    // to zero even if the downstream future is cancelled.
    let _in_flight = InFlightGuard::new();

    let start = Instant::now();
    let response = next.run(request).await;
    let status = response.status().as_u16().to_string();
    let duration = start.elapsed();

    metrics::counter!(
        "nissefhir_http_requests_total",
        "method" => method.clone(),
        "route" => route.clone(),
        "status" => status,
    )
    .increment(1);

    metrics::histogram!(
        "nissefhir_http_request_duration_seconds",
        "method" => method,
        "route" => route,
    )
    .record(duration.as_secs_f64());

    response
}

/// Label value used when no Axum matched-path template is available.
const UNMATCHED_ROUTE: &str = "<unmatched>";

/// Normalize an HTTP method to a small, fixed set of labels.
///
/// HTTP allows arbitrary extension methods, so recording the method verbatim
/// would let clients create unbounded Prometheus series. Only the supported
/// methods are emitted as their own label; everything else maps to `OTHER`.
fn normalize_method(method: &str) -> &'static str {
    match method {
        "GET" => "GET",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "OPTIONS" => "OPTIONS",
        "HEAD" => "HEAD",
        _ => "OTHER",
    }
}

/// Drop guard backing the in-flight gauge so cancellation cannot leak it.
struct InFlightGuard;

impl InFlightGuard {
    fn new() -> Self {
        metrics::gauge!("nissefhir_http_requests_in_flight").increment(1.0);
        Self
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        metrics::gauge!("nissefhir_http_requests_in_flight").decrement(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_method;

    #[test]
    fn supported_methods_map_to_themselves() {
        for m in ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"] {
            assert_eq!(normalize_method(m), m, "{m} must keep its own label");
        }
    }

    #[test]
    fn extension_methods_map_to_other() {
        for m in ["X-0001", "X-0002", "PROPFIND", "MKCOL", "LINK", "BREW"] {
            assert_eq!(normalize_method(m), "OTHER", "{m} must collapse to OTHER");
        }
    }

    #[test]
    fn method_normalization_is_case_sensitive_like_http() {
        // HTTP methods are case-sensitive; lowercase is an extension token.
        assert_eq!(normalize_method("get"), "OTHER");
        assert_eq!(normalize_method("Get"), "OTHER");
    }
}
