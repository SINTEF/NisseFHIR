//! Privacy tests for the HTTP trace layer.
//!
//! The default tower-http trace span records the full request URI, which means
//! FHIR search query strings (names, identifiers, dates) would leak into the
//! logs. These tests capture every log line emitted while a request with PHI
//! in its query string is served, and prove the sensitive values never appear.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::Request;
use common::build_test_app;
use tower::ServiceExt;
use tracing::Level;
use tracing_subscriber::fmt;
use tracing_subscriber::util::SubscriberInitExt;

/// A writer that appends every formatted log line to a shared buffer.
#[derive(Clone)]
struct CaptureWriter(Arc<Mutex<Vec<String>>>);

impl Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(buf).to_string());
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn sensitive_query_values_never_reach_logs() {
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));

    // Install a global subscriber (once) that captures all log output. Other
    // tests do not install a subscriber, so this is safe to do here.
    let writer = CaptureWriter(Arc::clone(&captured));
    let _guard = fmt()
        .with_ansi(false)
        .with_max_level(Level::TRACE)
        .with_writer(move || writer.clone())
        .set_default();

    let app = build_test_app(common::lazy_pool());

    // A GET with PHI in the query string: patient name, birth date, and a
    // sensitive identifier. `/fhir/metadata` is served without touching the
    // database, so this stays a fast, hermetic test.
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/fhir/metadata?name=John+Doe&birthdate=1990-01-01&identifier=SSN-123-45-6789")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), 200);

    // Allow the subscriber to flush before inspecting.
    std::thread::sleep(std::time::Duration::from_millis(100));

    let logs = captured.lock().unwrap().join("\n");

    // The log schema must retain the privacy-safe fields.
    assert!(
        logs.contains("method=GET"),
        "logs should retain the method, got:\n{logs}"
    );
    assert!(
        logs.contains("route=/fhir/metadata"),
        "logs should retain the matched route template, got:\n{logs}"
    );
    assert!(
        logs.contains("correlation_id="),
        "logs should retain a correlation ID, got:\n{logs}"
    );
    assert!(
        logs.contains("status=200"),
        "logs should retain the response status, got:\n{logs}"
    );
    assert!(
        logs.contains("latency_ms="),
        "logs should retain latency, got:\n{logs}"
    );

    // PHI from the query string must never appear.
    for leak in [
        "John+Doe",
        "John",
        "Doe",
        "birthdate=1990-01-01",
        "1990-01-01",
        "SSN-123-45-6789",
        "identifier=",
        "/fhir/metadata?",
    ] {
        assert!(
            !logs.contains(leak),
            "sensitive value '{leak}' leaked into logs:\n{logs}"
        );
    }
}
