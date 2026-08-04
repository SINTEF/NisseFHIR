//! Runtime test verifying signal-driven graceful shutdown.
//!
//! Spawns the real server binary against a real PostgreSQL database, waits
//! for it to accept connections, sends `SIGTERM`, and asserts the process
//! exits cleanly within the configured (bounded) grace period. This exercises
//! the full shutdown path in `src/main.rs` rather than just the router.

#![cfg(unix)]

mod common;

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tokio::net::TcpStream;

/// Find a free TCP port by binding to port 0 and reading the assigned port.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);
    addr.port()
}

/// Poll TCP connect until the server accepts (or the deadline expires).
async fn wait_until_accepting(port: u16) -> bool {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    false
}

/// Read whatever the (already exited) child wrote to stderr, for diagnostics.
fn read_stderr(child: &mut Child) -> String {
    let mut buf = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut buf);
    }
    buf
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sigterm_triggers_graceful_shutdown() {
    let port = free_port();
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1/fhir_test".to_owned());

    let bin = env!("CARGO_BIN_EXE_fhir_server");

    let mut child = Command::new(bin)
        .env("BIND_ADDR", format!("127.0.0.1:{port}"))
        .env("DATABASE_URL", database_url)
        .env("JWT_MODE", "static")
        .env("JWT_ALGORITHM", "HS256")
        .env("JWT_SECRET", common::TEST_JWT_SECRET)
        .env("SHUTDOWN_TIMEOUT_SECS", "2")
        .env("RUST_LOG", "fhir_server=info")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server binary should spawn");

    if !wait_until_accepting(port).await {
        let _ = child.kill();
        let _ = child.wait();
        panic!(
            "server did not start listening; stderr:\n{}",
            read_stderr(&mut child)
        );
    }

    // The drain deadline must be measured from the shutdown *signal*, not from
    // server startup. With SHUTDOWN_TIMEOUT_SECS=2, the server must still be
    // alive well past 2 seconds of uptime while no signal has been sent.
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(
        child.try_wait().expect("try_wait").is_none(),
        "server must not self-terminate while idle; the drain timer must only start on signal"
    );

    // Send SIGTERM and assert it is delivered.
    let pid = child.id() as i32;
    let rc = unsafe { libc::kill(pid, libc::SIGTERM) };
    assert_eq!(rc, 0, "failed to send SIGTERM to pid {pid}");

    // The process must exit cleanly within the bounded grace window.
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => break status,
            None if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "server did not exit after SIGTERM; stderr:\n{}",
                    read_stderr(&mut child)
                );
            }
            None => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    };

    assert!(
        status.success(),
        "server exited with failure after graceful shutdown; stderr:\n{}",
        read_stderr(&mut child)
    );
}

/// Poll TCP connect and assert the port stays closed for the whole window.
async fn assert_not_accepting(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            panic!("port {port} unexpectedly accepted a connection");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Perform a minimal HTTP/1.1 GET over a raw TCP socket and return the body.
async fn http_get_body(port: u16, path: &str) -> Option<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.ok()?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await.ok()?;
    let mut buf = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut buf))
        .await
        .ok()?;
    Some(String::from_utf8_lossy(&buf).to_string())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sigterm_graceful_shutdown_with_metrics_enabled() {
    let port = free_port();
    let metrics_port = free_port();
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1/fhir_test".to_owned());

    let bin = env!("CARGO_BIN_EXE_fhir_server");

    let mut child = Command::new(bin)
        .env("BIND_ADDR", format!("127.0.0.1:{port}"))
        .env("METRICS_ENABLED", "true")
        .env("METRICS_BIND_ADDR", format!("127.0.0.1:{metrics_port}"))
        .env("DATABASE_URL", database_url)
        .env("JWT_MODE", "static")
        .env("JWT_ALGORITHM", "HS256")
        .env("JWT_SECRET", common::TEST_JWT_SECRET)
        .env("SHUTDOWN_TIMEOUT_SECS", "2")
        .env("RUST_LOG", "fhir_server=info")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server binary should spawn");

    // Both the main HTTP listener and the telemetry listener must come up.
    if !wait_until_accepting(port).await || !wait_until_accepting(metrics_port).await {
        let _ = child.kill();
        let _ = child.wait();
        panic!(
            "server did not start both listeners; stderr:\n{}",
            read_stderr(&mut child)
        );
    }

    // The telemetry listener actually serves a Prometheus exposition. First
    // generate a little traffic on the main listener so the HTTP metric
    // families exist in the output.
    let main_body = http_get_body(port, "/fhir/metadata")
        .await
        .expect("main listener should answer");
    assert!(
        main_body.starts_with("HTTP/1.1 200"),
        "main listener should serve /fhir/metadata, got:\n{main_body}"
    );

    let body = http_get_body(metrics_port, "/metrics")
        .await
        .expect("telemetry listener should answer /metrics");
    assert!(
        body.contains("nissefhir_http_requests_total"),
        "telemetry listener must serve HTTP metrics, got:\n{body}"
    );
    assert!(
        body.contains("route=\"/fhir/metadata\""),
        "metrics must carry the normalized route template:\n{body}"
    );

    // A non-metrics path on the telemetry listener must 404.
    let body = http_get_body(metrics_port, "/")
        .await
        .expect("telemetry listener should answer");
    assert!(
        body.starts_with("HTTP/1.1 404"),
        "telemetry listener must 404 non-metrics paths, got:\n{body}"
    );

    // Send SIGTERM and assert the process (both listeners) drains cleanly.
    let pid = child.id() as i32;
    let rc = unsafe { libc::kill(pid, libc::SIGTERM) };
    assert_eq!(rc, 0, "failed to send SIGTERM to pid {pid}");

    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => break status,
            None if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "server did not exit after SIGTERM; stderr:\n{}",
                    read_stderr(&mut child)
                );
            }
            None => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    };

    assert!(
        status.success(),
        "server exited with failure after graceful shutdown; stderr:\n{}",
        read_stderr(&mut child)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metrics_disabled_opens_no_telemetry_listener() {
    let port = free_port();
    let metrics_port = free_port();
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1/fhir_test".to_owned());

    let bin = env!("CARGO_BIN_EXE_fhir_server");

    let mut child = Command::new(bin)
        .env("BIND_ADDR", format!("127.0.0.1:{port}"))
        .env("METRICS_ENABLED", "false")
        .env("METRICS_BIND_ADDR", format!("127.0.0.1:{metrics_port}"))
        .env("DATABASE_URL", database_url)
        .env("JWT_MODE", "static")
        .env("JWT_ALGORITHM", "HS256")
        .env("JWT_SECRET", common::TEST_JWT_SECRET)
        .env("SHUTDOWN_TIMEOUT_SECS", "2")
        .env("RUST_LOG", "fhir_server=info")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server binary should spawn");

    // The main listener comes up, but the telemetry listener must stay closed.
    if !wait_until_accepting(port).await {
        let _ = child.kill();
        let _ = child.wait();
        panic!(
            "server did not start main listener; stderr:\n{}",
            read_stderr(&mut child)
        );
    }
    assert_not_accepting(metrics_port).await;

    let pid = child.id() as i32;
    let rc = unsafe { libc::kill(pid, libc::SIGTERM) };
    assert_eq!(rc, 0, "failed to send SIGTERM to pid {pid}");

    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => break status,
            None if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("server did not exit after SIGTERM");
            }
            None => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    };
    assert!(
        status.success(),
        "server exited with failure after graceful shutdown"
    );
}
