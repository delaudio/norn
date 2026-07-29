//! Minimal self-hosted process boundary for the shared review service.
//!
//! This module deliberately owns only process startup, persistence readiness,
//! and an offline smoke path. Provider ingress and real review execution remain
//! behind their public contracts and are not substituted by this deployment
//! wrapper.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::operational_telemetry::OperationalTelemetry;
use crate::review_event::{
    PullRequestEventActor, PullRequestReviewEvent, PullRequestReviewEventKind,
    PullRequestReviewEventProvider, PullRequestReviewEventSchemaVersion, PullRequestRevision,
};
use crate::review_job::{
    ReviewConcurrencyLimits, ReviewJobCoordinator, ReviewJobExecution, ReviewJobExecutor,
    SqliteReviewJobStore,
};
use crate::review_storage;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const MAX_HTTP_REQUEST_LINE_BYTES: usize = 8 * 1024;
const HTTP_CONNECTION_READ_TIMEOUT: Duration = Duration::from_secs(2);
const HTTP_WORKER_COUNT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceConfig {
    pub data_dir: PathBuf,
    pub bind_addr: SocketAddr,
}

impl ServiceConfig {
    pub fn from_env() -> Result<Self, String> {
        let data_dir = std::env::var_os("LACHESI_SERVICE_DATA_DIR")
            .map(PathBuf::from)
            .ok_or_else(|| {
                "`LACHESI_SERVICE_DATA_DIR` must name a persistent volume".to_string()
            })?;
        if !data_dir.is_absolute() {
            return Err("`LACHESI_SERVICE_DATA_DIR` must be an absolute path".to_string());
        }
        let bind_addr = std::env::var("LACHESI_SERVICE_BIND_ADDR")
            .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string())
            .parse::<SocketAddr>()
            .map_err(|_| {
                "`LACHESI_SERVICE_BIND_ADDR` must be an IP address and port".to_string()
            })?;
        if bind_addr.port() == 0 {
            return Err("`LACHESI_SERVICE_BIND_ADDR` must use a non-zero port".to_string());
        }
        Ok(Self {
            data_dir,
            bind_addr,
        })
    }

    pub fn prepare(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.data_dir).map_err(|error| {
            format!(
                "Failed to create service data directory {}: {error}",
                self.data_dir.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.data_dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| format!("Failed to secure service data directory: {error}"))?;
        }
        std::env::set_var("LACHESI_REVIEW_DATA_DIR", &self.data_dir);
        review_storage::initialize_database()
    }
}

pub fn run(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match args {
        [command] if command == "run" => run_server(stdout, stderr),
        [command] if command == "smoke" => run_smoke(stdout, stderr),
        [command] if command == "healthcheck" => run_healthcheck(stdout, stderr),
        [command] if command == "--help" || command == "-h" => {
            let _ = writeln!(stdout, "{}", usage());
            0
        }
        _ => {
            let _ = writeln!(
                stderr,
                "{}\n\n{}",
                "Usage: lachesi service <run|smoke|healthcheck>",
                usage()
            );
            2
        }
    }
}

pub fn usage() -> &'static str {
    "Usage:\n  lachesi service run\n  lachesi service smoke\n  lachesi service healthcheck\n\nEnvironment:\n  LACHESI_SERVICE_DATA_DIR  Absolute persistent data-volume path (required)\n  LACHESI_SERVICE_BIND_ADDR HTTP bind address (default: 0.0.0.0:8080)"
}

fn prepare_from_env(stderr: &mut dyn Write) -> Result<ServiceConfig, i32> {
    let config = ServiceConfig::from_env().map_err(|error| {
        let _ = writeln!(stderr, "Service configuration is invalid: {error}");
        2
    })?;
    config.prepare().map_err(|error| {
        let _ = writeln!(stderr, "Service is not ready: {error}");
        7
    })?;
    Ok(config)
}

fn run_server(stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let config = match prepare_from_env(stderr) {
        Ok(config) => config,
        Err(code) => return code,
    };
    let listener = match TcpListener::bind(config.bind_addr) {
        Ok(listener) => listener,
        Err(error) => {
            let _ = writeln!(
                stderr,
                "Service is not ready: failed to bind {}: {error}",
                config.bind_addr
            );
            return 7;
        }
    };
    let _ = writeln!(
        stdout,
        "Lachesi self-hosted service listening on {}",
        config.bind_addr
    );
    let available_connection_slots = Arc::new(AtomicUsize::new(HTTP_WORKER_COUNT));
    let telemetry = OperationalTelemetry::default();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if !try_acquire_connection_slot(&available_connection_slots) {
                    let mut stream = stream;
                    let _ = stream.set_write_timeout(Some(HTTP_CONNECTION_READ_TIMEOUT));
                    let _ =
                        write_response(&mut stream, 503, "{\"status\":\"service_unavailable\"}");
                    continue;
                }
                let available_connection_slots = Arc::clone(&available_connection_slots);
                let telemetry = telemetry.clone();
                std::thread::spawn(move || {
                    let _ = respond(stream, &telemetry);
                    available_connection_slots.fetch_add(1, Ordering::Release);
                });
            }
            Err(error) => {
                let _ = writeln!(stderr, "HTTP listener error: {error}");
            }
        }
    }
    0
}

fn try_acquire_connection_slot(slots: &AtomicUsize) -> bool {
    let mut available = slots.load(Ordering::Acquire);
    loop {
        if available == 0 {
            return false;
        }
        match slots.compare_exchange_weak(
            available,
            available - 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return true,
            Err(current) => available = current,
        }
    }
}

fn run_healthcheck(stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let config = match ServiceConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            let _ = writeln!(stderr, "Service healthcheck is invalid: {error}");
            return 2;
        }
    };
    let loopback = SocketAddr::new(
        healthcheck_ip(config.bind_addr.ip()),
        config.bind_addr.port(),
    );
    let mut stream = match TcpStream::connect_timeout(&loopback, HTTP_CONNECTION_READ_TIMEOUT) {
        Ok(stream) => stream,
        Err(error) => {
            let _ = writeln!(stderr, "Service is not ready: {error}");
            return 7;
        }
    };
    let result = (|| -> Result<(), String> {
        stream
            .set_read_timeout(Some(HTTP_CONNECTION_READ_TIMEOUT))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(HTTP_CONNECTION_READ_TIMEOUT))
            .map_err(|error| error.to_string())?;
        stream
            .write_all(b"GET /readyz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .map_err(|error| error.to_string())?;
        let mut response = String::new();
        BufReader::new(stream)
            .read_line(&mut response)
            .map_err(|error| error.to_string())?;
        if response.starts_with("HTTP/1.1 200 ") {
            Ok(())
        } else {
            Err("readiness endpoint did not return HTTP 200".to_string())
        }
    })();
    match result {
        Ok(()) => {
            let _ = writeln!(stdout, "Service is ready.");
            0
        }
        Err(error) => {
            let _ = writeln!(stderr, "Service is not ready: {error}");
            7
        }
    }
}

fn healthcheck_ip(bind_ip: IpAddr) -> IpAddr {
    match bind_ip {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    }
}

fn respond(mut stream: TcpStream, telemetry: &OperationalTelemetry) -> Result<(), String> {
    stream
        .set_read_timeout(Some(HTTP_CONNECTION_READ_TIMEOUT))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(HTTP_CONNECTION_READ_TIMEOUT))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|error| error.to_string())?);
    let mut request_line = Vec::with_capacity(MAX_HTTP_REQUEST_LINE_BYTES + 1);
    let read = reader
        .by_ref()
        .take((MAX_HTTP_REQUEST_LINE_BYTES + 1) as u64)
        .read_until(b'\n', &mut request_line)
        .map_err(|error| error.to_string())?;
    if read > MAX_HTTP_REQUEST_LINE_BYTES || !request_line.ends_with(b"\n") {
        return write_response(&mut stream, 414, "{\"status\":\"request_uri_too_long\"}");
    }
    let line = match std::str::from_utf8(&request_line) {
        Ok(line) => line,
        Err(_) => return write_response(&mut stream, 400, "{\"status\":\"bad_request\"}"),
    };
    let mut parts = line.split_whitespace();
    let method = parts.next();
    let path = parts.next();
    let version = parts.next();
    if parts.next().is_some() || !version.is_some_and(|value| value.starts_with("HTTP/")) {
        return write_response(&mut stream, 400, "{\"status\":\"bad_request\"}");
    }
    if method != Some("GET") {
        return write_response(&mut stream, 405, "{\"status\":\"method_not_allowed\"}");
    }
    let (status, body) = match path {
        Some("/healthz") => (200, "{\"status\":\"ok\"}"),
        Some("/readyz") => (200, "{\"status\":\"ready\"}"),
        Some("/metrics") => return write_metrics_response(&mut stream, &telemetry.prometheus()),
        _ => (404, "{\"status\":\"not_found\"}"),
    };
    write_response(&mut stream, status, body)
}

fn write_metrics_response(stream: &mut TcpStream, body: &str) -> Result<(), String> {
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
        .map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())
}

fn write_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        414 => "URI Too Long",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    write!(stream, "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
        .map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())
}

fn run_smoke(stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if let Err(code) = prepare_from_env(stderr) {
        return code;
    }
    let telemetry = OperationalTelemetry::default();
    let coordinator = match ReviewJobCoordinator::new(
        SqliteReviewJobStore,
        OfflineSmokeExecutor,
        ReviewConcurrencyLimits::default(),
    )
    .map(|coordinator| coordinator.with_telemetry(telemetry))
    {
        Ok(coordinator) => coordinator,
        Err(error) => {
            let _ = writeln!(stderr, "Smoke setup failed: {error}");
            return 7;
        }
    };
    if let Err(error) = coordinator.accept_event(&smoke_event()) {
        let _ = writeln!(stderr, "Smoke event was not accepted: {error}");
        return 7;
    }
    match coordinator.run_next() {
        Ok(Some(job)) if matches!(job.status, crate::review_job::ReviewJobStatus::Completed) => {
            let _ = writeln!(stdout, "Self-hosted smoke test completed offline.");
            0
        }
        Ok(Some(job)) => {
            let _ = writeln!(stderr, "Smoke job did not complete: {:?}", job.status);
            7
        }
        Ok(None) => {
            let _ = writeln!(stderr, "Smoke job was not claimable.");
            7
        }
        Err(error) => {
            let _ = writeln!(stderr, "Smoke job failed: {error}");
            7
        }
    }
}

struct OfflineSmokeExecutor;

impl ReviewJobExecutor for OfflineSmokeExecutor {
    fn execute(&self, request: &crate::review_job::ReviewJobRequest) -> ReviewJobExecution {
        ReviewJobExecution::Completed {
            run_id: format!("offline-smoke-{}", request.id),
        }
    }
}

fn smoke_event() -> PullRequestReviewEvent {
    PullRequestReviewEvent {
        schema_version: PullRequestReviewEventSchemaVersion::V1,
        kind: PullRequestReviewEventKind::Opened,
        provider: PullRequestReviewEventProvider::Github,
        tenant_id: "smoke-tenant".to_string(),
        workspace: "smoke-org".to_string(),
        repository: "smoke-repository".to_string(),
        pull_request_id: 1,
        base: PullRequestRevision {
            ref_name: "main".to_string(),
            sha: "1111111111111111111111111111111111111111".to_string(),
        },
        head: PullRequestRevision {
            ref_name: "smoke".to_string(),
            sha: "2222222222222222222222222222222222222222".to_string(),
        },
        provider_updated_at_ms: Some(1),
        draft: false,
        closed_outcome: None,
        actor: PullRequestEventActor {
            id: "smoke".to_string(),
            login: "smoke".to_string(),
            display_name: None,
        },
        delivery_id: "self-hosted-smoke-delivery-v1".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::io::Read;

    struct ServiceEnvironmentGuard {
        service_data_dir: Option<OsString>,
        review_data_dir: Option<OsString>,
    }

    impl ServiceEnvironmentGuard {
        fn capture() -> Self {
            Self {
                service_data_dir: std::env::var_os("LACHESI_SERVICE_DATA_DIR"),
                review_data_dir: std::env::var_os("LACHESI_REVIEW_DATA_DIR"),
            }
        }
    }

    impl Drop for ServiceEnvironmentGuard {
        fn drop(&mut self) {
            restore_env("LACHESI_SERVICE_DATA_DIR", self.service_data_dir.take());
            restore_env("LACHESI_REVIEW_DATA_DIR", self.review_data_dir.take());
        }
    }

    fn restore_env(name: &str, value: Option<OsString>) {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }

    #[test]
    fn service_requires_an_absolute_persistent_volume() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .expect("lock");
        let _environment = ServiceEnvironmentGuard::capture();
        std::env::set_var("LACHESI_SERVICE_DATA_DIR", "relative");
        assert!(ServiceConfig::from_env().is_err());
    }

    #[test]
    fn smoke_runs_without_provider_or_ai_network_access() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .expect("lock");
        let _environment = ServiceEnvironmentGuard::capture();
        let directory = tempfile::tempdir().expect("tempdir");
        std::env::set_var("LACHESI_SERVICE_DATA_DIR", directory.path());
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            run(&["smoke".to_string()], &mut out, &mut err),
            0,
            "{}",
            String::from_utf8_lossy(&err)
        );
        assert!(String::from_utf8(out)
            .expect("utf8")
            .contains("completed offline"));
    }

    #[test]
    fn restart_keeps_completed_job_and_review_cursor_on_the_volume() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .expect("lock");
        let _environment = ServiceEnvironmentGuard::capture();
        let directory = tempfile::tempdir().expect("tempdir");
        std::env::set_var("LACHESI_SERVICE_DATA_DIR", directory.path());
        let config = ServiceConfig::from_env().expect("config");
        config.prepare().expect("first startup");
        let coordinator = ReviewJobCoordinator::new(
            SqliteReviewJobStore,
            OfflineSmokeExecutor,
            ReviewConcurrencyLimits::default(),
        )
        .expect("coordinator");
        let queued = coordinator.accept_event(&smoke_event()).expect("enqueue");
        let job_id = match queued {
            crate::review_job::ReviewJobEnqueueOutcome::Queued(job) => job.request.id,
            _ => panic!("expected queued smoke job"),
        };
        assert!(matches!(
            coordinator.run_next().expect("run job"),
            Some(job) if job.status == crate::review_job::ReviewJobStatus::Completed
        ));

        config.prepare().expect("restart migration boundary");
        assert!(matches!(
            review_storage::get_shared_review_job(&job_id).expect("persisted job"),
            Some(job) if job.status == crate::review_job::ReviewJobStatus::Completed
        ));
        let cursor = crate::review_storage::get_review_cursor(
            &crate::review_storage::ReviewCursorIdentity {
                tenant_id: "smoke-tenant".to_string(),
                provider: PullRequestReviewEventProvider::Github,
                workspace: "smoke-org".to_string(),
                repo: "smoke-repository".to_string(),
                pr_id: 1,
            },
        )
        .expect("persisted cursor");
        assert!(matches!(
            cursor,
            crate::review_storage::ReviewCursorState::Reviewed(_)
        ));
    }

    #[test]
    fn health_routes_have_no_secret_or_provider_dependency() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("address");
        let worker = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            respond(stream, &OperationalTelemetry::default()).expect("respond");
        });
        let mut stream = TcpStream::connect(address).expect("connect");
        stream
            .write_all(b"GET /readyz HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("write");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read");
        worker.join().expect("join");
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("{\"status\":\"ready\"}"));
    }

    #[test]
    fn metrics_endpoint_is_machine_readable_and_redacted() {
        let telemetry = OperationalTelemetry::default();
        telemetry.record_queued(
            PullRequestReviewEventProvider::Github,
            "delivery-with-secret-material",
            "job-42",
        );
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("address");
        let worker = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            respond(stream, &telemetry).expect("respond");
        });
        let mut stream = TcpStream::connect(address).expect("connect");
        stream
            .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("write");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read");
        worker.join().expect("join");
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("Content-Type: text/plain; version=0.0.4"));
        assert!(response.contains("lachesi_review_jobs_total"));
        assert!(!response.contains("delivery-with-secret-material"));
        assert!(!response.contains("job-42"));
    }

    #[test]
    fn oversized_request_line_is_rejected_with_a_fixed_read_cap() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("address");
        let worker = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            respond(stream, &OperationalTelemetry::default()).expect("respond");
        });
        let mut stream = TcpStream::connect(address).expect("connect");
        let request = format!(
            "GET /{} HTTP/1.1\r\n",
            "a".repeat(MAX_HTTP_REQUEST_LINE_BYTES)
        );
        stream.write_all(request.as_bytes()).expect("write");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read");
        worker.join().expect("join");
        assert!(response.starts_with("HTTP/1.1 414"));
    }

    #[test]
    fn readiness_accepts_only_well_formed_get_requests() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("address");
        let worker = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            respond(stream, &OperationalTelemetry::default()).expect("respond");
        });
        let mut stream = TcpStream::connect(address).expect("connect");
        stream
            .write_all(b"POST /readyz HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("write");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read");
        worker.join().expect("join");
        assert!(response.starts_with("HTTP/1.1 405"));
    }

    #[test]
    fn healthcheck_uses_loopback_for_wildcard_binds() {
        assert_eq!(
            healthcheck_ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        );
        assert_eq!(
            healthcheck_ip(IpAddr::V6(Ipv6Addr::UNSPECIFIED)),
            IpAddr::V6(Ipv6Addr::LOCALHOST)
        );
        assert_eq!(
            healthcheck_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)),
            IpAddr::V6(Ipv6Addr::LOCALHOST)
        );
    }
}
