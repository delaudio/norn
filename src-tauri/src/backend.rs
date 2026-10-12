//! Persistent Rust stdio backend for the OpenTUI workspace (epic #294, issue
//! #298).
//!
//! Reads newline-delimited protocol frames from standard input and writes only
//! protocol frames to standard output; diagnostics go to standard error. Input
//! frames are bounded, the writer is serialized through a bounded queue, and
//! blocking work (Git, operations) runs on its own bounded threads so
//! cancel/status/shutdown stay responsive.
//!
//! Review execution is intentionally not wired here: the first slice collects the
//! target snapshot and drives the operation lifecycle. The review/agent engine
//! is connected in later steps (#301, #303).

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;

use crate::browser_diff::{open_browser_url, WebDiffServer, WebDiffState, WebDiffTargetKind};
use crate::config;
use crate::protocol::{
    self, ClientMessage, OperationEvent, OperationEventFrame, OperationState, ProtocolError,
    ProtocolErrorCode, ProviderKind, Ready, Request, Response, ServerMessage, TargetIdentity,
    MAX_FRAME_BYTES, PROTOCOL_VERSION,
};
use crate::services::review::{
    cancel_local_inline_review_native, get_ai_review_fix_state_native,
    get_local_ai_review_run_state_native, start_ai_review_commit_native,
    start_ai_review_fix_native, start_ai_review_push_native, start_local_inline_review_native,
    validate_fix_commit_paths, AiReviewFixPhase, AiReviewFixStatus, AiReviewFixStore,
    AiReviewRunStatus, AiReviewRunStore,
};
use crate::services::review_prompt::{build_review_payload, resolve_review_prompt};

const WRITER_QUEUE_BOUND: usize = 256;
const EXIT_PROTOCOL_MISMATCH: i32 = 3;
const EXIT_USAGE: i32 = 2;
const DEFAULT_OPERATION_DELAY_MS: u64 = 5;
const MAX_DIFF_BYTES: usize = 256 * 1024;
const MAX_SYNTHETIC_FILE_BYTES: u64 = 512 * 1024;
const MAX_PREVIEW_BYTES: u64 = 2 * 1024 * 1024;
const GIT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CONCURRENT_DIFFS: usize = 4;
const MAX_CONCURRENT_FILES: usize = 4;
const MAX_CONCURRENT_BROWSER_OPENS: usize = 1;
const MAX_CONCURRENT_PREVIEWS: usize = 4;
const MAX_ACTIVE_OPERATIONS: usize = 8;

/// Which review engine executes `review.start`. The shared headless engine is
/// the default; `NORN_REVIEW_ENGINE=simulated` keeps the deterministic
/// placeholder and `fake` is a fast deterministic engine for tests.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReviewEngine {
    Simulated,
    Fake,
    Real,
}

fn review_engine() -> ReviewEngine {
    match std::env::var("NORN_REVIEW_ENGINE").as_deref() {
        Ok("simulated") => ReviewEngine::Simulated,
        Ok("fake") => ReviewEngine::Fake,
        _ => ReviewEngine::Real,
    }
}

/// Every operation the backend tracks while it is running.
struct Operation {
    cancel: Arc<AtomicBool>,
    state: OperationState,
    sequence: u64,
    error: Option<ProtocolError>,
}

type Operations = Arc<Mutex<HashMap<String, Operation>>>;

#[derive(Clone)]
struct ReviewRuntime {
    engine: ReviewEngine,
    snapshots: Arc<crate::services::provider_snapshot::Manager>,
    provider_reads: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
}

struct Backend {
    control: Sender<ServerMessage>,
    events: SyncSender<ServerMessage>,
    operations: Operations,
    next_operation: AtomicU64,
    shutdown: Arc<AtomicBool>,
    active_diffs: Arc<AtomicUsize>,
    active_files: Arc<AtomicUsize>,
    active_operations: Arc<AtomicUsize>,
    active_browser: Arc<AtomicUsize>,
    active_previews: Arc<AtomicUsize>,
    active_settings: Arc<AtomicUsize>,
    active_workspace: Arc<AtomicUsize>,
    active_provider_reads: Arc<AtomicUsize>,
    provider_snapshots: Arc<crate::services::provider_snapshot::Manager>,
    active_publication: Arc<AtomicUsize>,
    active_publication_reads: Arc<AtomicUsize>,
    publication: Arc<crate::services::draft_publication::Manager>,
    local_snapshots: Arc<crate::services::local_snapshot::Manager>,
    active_local: Arc<AtomicUsize>,
    provider_approval: Arc<crate::services::provider_approval::Manager>,
    active_approval: Arc<AtomicUsize>,
    browser: Arc<Mutex<Option<WebDiffServer>>>,
    fix_approvals: Arc<Mutex<HashMap<String, FixApproval>>>,
}

impl Backend {
    fn operation_delay(&self) -> Duration {
        let millis = std::env::var("NORN_BACKEND_OPERATION_DELAY_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_OPERATION_DELAY_MS);
        Duration::from_millis(millis)
    }

    /// Control responses (handshake, request results, shutdown) must never block
    /// the read loop, so they use an unbounded, prioritized channel.
    fn send_control(&self, message: ServerMessage) -> bool {
        self.control.send(message).is_ok()
    }

    fn respond(&self, id: &str, result: serde_json::Value) -> bool {
        self.send_control(ServerMessage::Response(Response {
            id: id.to_string(),
            ok: true,
            result: Some(result),
            error: None,
        }))
    }

    fn fail(&self, id: &str, error: ProtocolError) -> bool {
        self.send_control(ServerMessage::Response(Response {
            id: id.to_string(),
            ok: false,
            result: None,
            error: Some(error),
        }))
    }
}

/// Run the backend until stdin reaches EOF or a `shutdown` request arrives.
pub fn run() -> Result<(), i32> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();

    if !handshake(&mut reader)? {
        return Ok(());
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let (control, control_receiver) = channel::<ServerMessage>();
    let (events, event_receiver) = sync_channel::<ServerMessage>(WRITER_QUEUE_BOUND);
    let writer_thread = thread::spawn({
        let shutdown = shutdown.clone();
        move || writer_loop(control_receiver, event_receiver, shutdown)
    });

    let active_provider_reads = Arc::new(AtomicUsize::new(0));
    let backend = Backend {
        control,
        events,
        operations: Arc::new(Mutex::new(HashMap::new())),
        next_operation: AtomicU64::new(1),
        shutdown: shutdown.clone(),
        active_diffs: Arc::new(AtomicUsize::new(0)),
        active_files: Arc::new(AtomicUsize::new(0)),
        active_browser: Arc::new(AtomicUsize::new(0)),
        active_previews: Arc::new(AtomicUsize::new(0)),
        active_settings: Arc::new(AtomicUsize::new(0)),
        active_workspace: Arc::new(AtomicUsize::new(0)),
        active_provider_reads: active_provider_reads.clone(),
        provider_snapshots: Arc::new(crate::services::provider_snapshot::Manager::with_runtime(
            active_provider_reads.clone(),
            Arc::new(AtomicBool::new(false)),
        )),
        local_snapshots: Arc::new(crate::services::local_snapshot::Manager::new(
            std::env::var_os("NORN_BACKEND_REPO_PATH")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
        )),
        active_local: Arc::new(AtomicUsize::new(0)),
        provider_approval: Arc::new(crate::services::provider_approval::Manager::new(
            shutdown.clone(),
        )),
        active_approval: Arc::new(AtomicUsize::new(0)),
        active_publication: Arc::new(AtomicUsize::new(0)),
        active_publication_reads: Arc::new(AtomicUsize::new(0)),
        publication: Arc::new(crate::services::draft_publication::Manager::new(
            shutdown.clone(),
        )),
        browser: Arc::new(Mutex::new(None)),
        fix_approvals: Arc::new(Mutex::new(HashMap::new())),
        active_operations: Arc::new(AtomicUsize::new(0)),
    };

    backend.send_control(ServerMessage::Ready(Ready {
        protocol_version: PROTOCOL_VERSION,
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: vec![
            "workspace-data".to_string(),
            "workspace-records".to_string(),
            "workspace-scopes".to_string(),
            "provider-pr-reads".to_string(),
            "provider-snapshots".to_string(),
            "provider-evidence-reviews".to_string(),
            "provider-external-browser".to_string(),
            "provider-pr-approval".to_string(),
            "provider-snapshot-preview".to_string(),
            "draft-staging".to_string(),
            "draft-publication".to_string(),
            "remote-comments".to_string(),
            "workspace-readiness".to_string(),
            "settings".to_string(),
            "auth".to_string(),
            "repository".to_string(),
            "diff".to_string(),
            "review".to_string(),
            "review-profile-catalog".to_string(),
            "local-snapshots".to_string(),
            "local-evidence-reviews".to_string(),
        ],
    }));

    read_loop(&mut reader, &backend);
    backend.provider_snapshots.stop();

    if let Ok(operations) = backend.operations.lock() {
        for operation in operations.values() {
            operation.cancel.store(true, Ordering::SeqCst);
        }
    }
    backend.local_snapshots.stop();
    // Stop spawning new children, terminate the ones this backend owns, and let
    // the writer flush and exit.
    shutting_down().store(true, Ordering::SeqCst);
    kill_git_children();
    let deadline = Instant::now() + Duration::from_secs(15);
    while (backend.active_operations.load(Ordering::SeqCst) > 0
        || backend.active_local.load(Ordering::SeqCst) > 0)
        && Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(25));
    }
    backend.shutdown.store(true, Ordering::SeqCst);
    let _ = writer_thread.join();
    Ok(())
}

fn shutting_down() -> &'static AtomicBool {
    static FLAG: OnceLock<AtomicBool> = OnceLock::new();
    FLAG.get_or_init(|| AtomicBool::new(false))
}

/// Read one bounded newline-delimited frame. Oversized frames are truncated to
/// `MAX_FRAME_BYTES + 1` and drained to the next newline so decoding rejects
/// them without unbounded allocation.
fn read_frame(reader: &mut impl BufRead) -> std::io::Result<Option<Vec<u8>>> {
    let mut buffer = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok((!buffer.is_empty()).then_some(buffer));
        }
        let newline = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|pos| pos + 1);
        let consume = newline.unwrap_or(available.len());
        if buffer.len() + consume > MAX_FRAME_BYTES + 1 {
            let remaining = MAX_FRAME_BYTES + 1 - buffer.len();
            buffer.extend_from_slice(&available[..remaining]);
            let had_newline = newline.is_some();
            reader.consume(consume);
            if !had_newline {
                loop {
                    let chunk = reader.fill_buf()?;
                    if chunk.is_empty() {
                        break;
                    }
                    if let Some(pos) = chunk.iter().position(|byte| *byte == b'\n') {
                        reader.consume(pos + 1);
                        break;
                    }
                    let len = chunk.len();
                    reader.consume(len);
                }
            }
            return Ok(Some(buffer));
        }
        buffer.extend_from_slice(&available[..consume]);
        reader.consume(consume);
        if newline.is_some() {
            return Ok(Some(buffer));
        }
    }
}

fn handshake(reader: &mut impl BufRead) -> Result<bool, i32> {
    let frame = read_frame(reader).map_err(|error| {
        eprintln!("norn-backend: cannot read handshake: {error}");
        EXIT_USAGE
    })?;
    let Some(frame) = frame else {
        return Ok(false);
    };
    match protocol::decode_client_line(&frame) {
        Ok(ClientMessage::Hello(hello)) if hello.protocol_version == PROTOCOL_VERSION => Ok(true),
        Ok(ClientMessage::Hello(hello)) => {
            eprintln!(
                "norn-backend: unsupported protocol version {}; this build speaks {PROTOCOL_VERSION}",
                hello.protocol_version
            );
            Err(EXIT_PROTOCOL_MISMATCH)
        }
        Ok(_) => {
            eprintln!("norn-backend: expected a hello handshake before any request");
            Err(EXIT_PROTOCOL_MISMATCH)
        }
        Err(error) => {
            eprintln!("norn-backend: invalid handshake: {}", error.message);
            Err(EXIT_PROTOCOL_MISMATCH)
        }
    }
}

fn read_loop(reader: &mut impl BufRead, backend: &Backend) {
    loop {
        let frame = match read_frame(reader) {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(error) => {
                eprintln!("norn-backend: input error: {error}");
                break;
            }
        };
        match protocol::decode_client_line(&frame) {
            Ok(ClientMessage::Request(request)) => {
                if dispatch(request, backend) {
                    break;
                }
            }
            Ok(ClientMessage::Hello(_)) => {
                eprintln!("norn-backend: ignoring a repeated handshake");
            }
            Err(error) => {
                // A malformed frame has no correlation id, so it cannot be
                // answered; report it and keep serving.
                eprintln!("norn-backend: dropped malformed frame: {}", error.message);
            }
        }
        if backend.shutdown.load(Ordering::SeqCst) {
            break;
        }
    }
}

/// Returns true when the backend should shut down.
fn dispatch(request: Request, backend: &Backend) -> bool {
    let id = request.id().to_string();
    let method = request.method();
    match request {
        Request::LocalReviewStart { params, .. } => {
            if backend
                .active_operations
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    (n < MAX_ACTIVE_OPERATIONS).then_some(n + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Conflict,
                        "Too many active review operations.",
                    ),
                );
                return false;
            }
            let operation_id = format!(
                "op-{}",
                backend.next_operation.fetch_add(1, Ordering::SeqCst)
            );
            backend.start_local_review(&id, &operation_id, params);
            false
        }
        Request::LocalReviewHistory { .. }
        | Request::LocalReviewFindings { .. }
        | Request::LocalReviewOutput { .. }
        | Request::LocalReviewThreads { .. } => {
            let active = backend.active_local.clone();
            if active
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    (n < 2).then_some(n + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Conflict,
                        "Two local evidence requests are already running.",
                    ),
                );
                return false;
            }
            let counter = ActiveCounter(active);
            let manager = backend.local_snapshots.clone();
            let shutdown = backend.shutdown.clone();
            let control = backend.control.clone();
            thread::spawn(move || {
                let result = match request {
                    Request::LocalReviewHistory { params, .. } => {
                        crate::services::local_reviews::history(&manager, params).map(|v| json!(v))
                    }
                    Request::LocalReviewFindings { params, .. } => {
                        crate::services::local_reviews::findings(&manager, params).map(|v| json!(v))
                    }
                    Request::LocalReviewOutput { params, .. } => {
                        crate::services::local_reviews::output(&manager, params).map(|v| json!(v))
                    }
                    Request::LocalReviewThreads { params, .. } => {
                        crate::services::local_reviews::threads(&manager, params).map(|v| json!(v))
                    }
                    _ => unreachable!(),
                };
                if !shutdown.load(Ordering::Acquire) && !manager.stopped() {
                    let response = match result {
                        Ok(value) => Response {
                            id,
                            ok: true,
                            result: Some(value),
                            error: None,
                        },
                        Err(message) => Response {
                            id,
                            ok: false,
                            result: None,
                            error: Some(ProtocolError::new(
                                ProtocolErrorCode::TargetStale,
                                message,
                            )),
                        },
                    };
                    let _ = control.send(ServerMessage::Response(response));
                }
                counter.decrement();
            });
            false
        }
        Request::LocalSnapshotCancel { params, .. } => {
            match backend.local_snapshots.cancel(params.clone()) {
                Ok(canceled) => {
                    backend.respond(
                        &id,
                        json!(crate::services::local_snapshot::CancelResult {
                            request: params,
                            canceled
                        }),
                    );
                }
                Err(message) => {
                    backend.fail(
                        &id,
                        ProtocolError::new(ProtocolErrorCode::Conflict, message),
                    );
                }
            }
            false
        }
        Request::LocalOptions { .. }
        | Request::LocalSnapshot { .. }
        | Request::LocalSnapshotInfo { .. }
        | Request::LocalSnapshotFiles { .. }
        | Request::LocalSnapshotDiff { .. }
        | Request::LocalSnapshotRecheck { .. } => {
            let active = backend.active_local.clone();
            if active
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    (n < 2).then_some(n + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Conflict,
                        "Two local evidence requests are already running.",
                    ),
                );
                return false;
            }
            let manager = backend.local_snapshots.clone();
            let shutdown = backend.shutdown.clone();
            let control = backend.control.clone();
            let reservation = match &request {
                Request::LocalSnapshot { params, .. } => {
                    Some(crate::services::local_snapshot::CancelParams {
                        source: params.comparison.source.clone(),
                        capture_id: params.capture_id.clone(),
                    })
                }
                Request::LocalSnapshotRecheck { params, .. } => {
                    Some(crate::services::local_snapshot::CancelParams {
                        source: params.snapshot.source.clone(),
                        capture_id: params.capture_id.clone(),
                    })
                }
                _ => None,
            };
            let ticket = match reservation
                .map(|params| manager.reserve(params))
                .transpose()
            {
                Ok(ticket) => ticket,
                Err(message) => {
                    active.fetch_sub(1, Ordering::SeqCst);
                    backend.fail(
                        &id,
                        ProtocolError::new(ProtocolErrorCode::Conflict, message),
                    );
                    return false;
                }
            };
            thread::spawn(move || {
                let result = match request {
                    Request::LocalOptions { params, .. } => {
                        manager.options(params).map(|v| json!(v))
                    }
                    Request::LocalSnapshot { params, .. } => manager
                        .capture_reserved(params, ticket.expect("capture reserved before dispatch"))
                        .map(|v| json!(v)),
                    Request::LocalSnapshotInfo { params, .. } => {
                        manager.info(params).map(|v| json!(v))
                    }
                    Request::LocalSnapshotFiles { params, .. } => {
                        manager.files(params).map(|v| json!(v))
                    }
                    Request::LocalSnapshotDiff { params, .. } => {
                        manager.diff(params).map(|v| json!(v))
                    }
                    Request::LocalSnapshotRecheck { params, .. } => manager
                        .recheck_reserved(params, ticket.expect("recheck reserved before dispatch"))
                        .map(|v| json!(v)),
                    _ => unreachable!(),
                };
                if !shutdown.load(Ordering::Acquire) && !manager.stopped() {
                    let response = match result {
                        Ok(value) => Response {
                            id,
                            ok: true,
                            result: Some(value),
                            error: None,
                        },
                        Err(message) => Response {
                            id,
                            ok: false,
                            result: None,
                            error: Some(ProtocolError::new(
                                if message.contains("stale") {
                                    ProtocolErrorCode::TargetStale
                                } else {
                                    ProtocolErrorCode::Conflict
                                },
                                message,
                            )),
                        },
                    };
                    let _ = control.send(ServerMessage::Response(response));
                }
                active.fetch_sub(1, Ordering::SeqCst);
            });
            false
        }
        Request::PublicationCancel { params, .. } => {
            match backend.publication.cancel(params) {
                Ok(canceled) => {
                    backend.respond(&id, json!({ "canceled": canceled }));
                }
                Err(message) => {
                    backend.fail(
                        &id,
                        ProtocolError::new(ProtocolErrorCode::Conflict, message),
                    );
                }
            }
            false
        }
        Request::ProviderApprovalPreview { .. }
        | Request::ProviderApprovalConfirm { .. }
        | Request::ProviderApprovalStatus { .. }
        | Request::ProviderApprovalReconcile { .. } => {
            let active = backend.active_approval.clone();
            if active
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    (n < 1).then_some(n + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Conflict,
                        "An approval request is in progress. Inspect its result before retrying.",
                    ),
                );
                return false;
            }
            let reads = backend.active_provider_reads.clone();
            if reads
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    (n < 2).then_some(n + 1)
                })
                .is_err()
            {
                active.fetch_sub(1, Ordering::SeqCst);
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Conflict,
                        "Two provider reads are in progress; wait before inspecting approval.",
                    ),
                );
                return false;
            }
            let manager = backend.provider_approval.clone();
            let control = backend.control.clone();
            thread::spawn(move || {
                let result = match request {
                    Request::ProviderApprovalPreview { params, .. } => {
                        manager.preview(params).map(|v| json!(v))
                    }
                    Request::ProviderApprovalConfirm { params, .. } => {
                        manager.confirm(params).map(|v| json!(v))
                    }
                    Request::ProviderApprovalStatus { params, .. } => {
                        manager.status(params, false).map(|v| json!(v))
                    }
                    Request::ProviderApprovalReconcile { params, .. } => {
                        manager.status(params, true).map(|v| json!(v))
                    }
                    _ => unreachable!(),
                };
                reads.fetch_sub(1, Ordering::SeqCst);
                active.fetch_sub(1, Ordering::SeqCst);
                let response = match result {
                    Ok(value) => Response {
                        id,
                        ok: true,
                        result: Some(value),
                        error: None,
                    },
                    Err(message) => Response {
                        id,
                        ok: false,
                        result: None,
                        error: Some(ProtocolError::new(ProtocolErrorCode::Conflict, message)),
                    },
                };
                let _ = control.send(ServerMessage::Response(response));
            });
            false
        }
        Request::PublicationPrepare { .. }
        | Request::PublicationApply { .. }
        | Request::PublicationStatus { .. }
        | Request::PublicationReconcile { .. }
        | Request::CommentsList { .. } => {
            let writes = matches!(
                request,
                Request::PublicationApply { .. } | Request::PublicationReconcile { .. }
            );
            let active = if writes {
                backend.active_publication.clone()
            } else {
                backend.active_publication_reads.clone()
            };
            let bound = if writes { 1 } else { 2 };
            if active
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    (n < bound).then_some(n + 1)
                })
                .is_err()
            {
                backend.fail(&id, ProtocolError::new(ProtocolErrorCode::Conflict, "Another publication request is in progress; inspect its result before retrying."));
                return false;
            }
            let manager = backend.publication.clone();
            let control = backend.control.clone();
            thread::spawn(move || {
                let result = match request {
                    Request::PublicationPrepare { params, .. } => {
                        manager.prepare(params).map(|value| json!(value))
                    }
                    Request::PublicationApply { params, .. } => {
                        manager.apply(params).map(|value| json!(value))
                    }
                    Request::PublicationStatus { params, .. } => {
                        manager.status(params).map(|value| json!(value))
                    }
                    Request::PublicationReconcile { params, .. } => {
                        manager.reconcile(params).map(|value| json!(value))
                    }
                    Request::CommentsList { params, .. } => {
                        manager.comments(params).map(|value| json!(value))
                    }
                    _ => unreachable!(),
                };
                active.fetch_sub(1, Ordering::SeqCst);
                let response = match result {
                    Ok(value) => Response {
                        id,
                        ok: true,
                        result: Some(value),
                        error: None,
                    },
                    Err(message) => Response {
                        id,
                        ok: false,
                        result: None,
                        error: Some(ProtocolError::new(ProtocolErrorCode::Conflict, message)),
                    },
                };
                let _ = control.send(ServerMessage::Response(response));
            });
            false
        }
        Request::ProviderSnapshot { .. }
        | Request::ProviderSnapshotFiles { .. }
        | Request::ProviderSnapshotDiff { .. }
        | Request::ProviderPrs { .. }
        | Request::ProviderPrDetail { .. }
        | Request::ProviderUser { .. } => {
            let snapshots = backend.provider_snapshots.clone();
            let active = backend.active_provider_reads.clone();
            if active
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    (n < 2).then_some(n + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Conflict,
                        "Two provider reads are already in progress; wait before refreshing.",
                    ),
                );
                return false;
            }
            let control = backend.control.clone();
            let shutdown = backend.shutdown.clone();
            thread::spawn(move || {
                use crate::services::bitbucket::workspace_prs::Api;
                let result = if shutdown.load(Ordering::SeqCst) {
                    Err("Provider read canceled during shutdown.".into())
                } else {
                    match request {
                        Request::ProviderSnapshot { params, .. } => {
                            Api::from_stored(params.repository.clone())
                                .and_then(|api| api.capture(params))
                                .and_then(|capture| snapshots.insert(capture))
                                .map(|value| json!(value))
                        }
                        Request::ProviderSnapshotFiles { params, .. } => {
                            snapshots.files(params).map(|value| json!(value))
                        }
                        Request::ProviderSnapshotDiff { params, .. } => {
                            snapshots.diff(params).map(|value| json!(value))
                        }
                        Request::ProviderPrs { params, .. } => {
                            Api::from_stored(params.repository.clone())
                                .and_then(|api| api.list(params))
                                .map(|value| json!(value))
                        }
                        Request::ProviderPrDetail { params, .. } => {
                            Api::from_stored(params.repository.clone())
                                .and_then(|api| api.detail(params))
                                .map(|value| json!(value))
                        }
                        Request::ProviderUser { params, .. } => Api::from_stored(params.repository)
                            .and_then(|api| api.user())
                            .map(|value| json!(value)),
                        _ => unreachable!(),
                    }
                };
                active.fetch_sub(1, Ordering::SeqCst);
                let response = match result {
                    Ok(value) => Response {
                        id,
                        ok: true,
                        result: Some(value),
                        error: None,
                    },
                    Err(message) => Response {
                        id,
                        ok: false,
                        result: None,
                        error: Some(ProtocolError::new(ProtocolErrorCode::Conflict, message)),
                    },
                };
                let _ = control.send(ServerMessage::Response(response));
            });
            false
        }
        Request::WorkspaceRead { .. }
        | Request::WorkspaceScopes { .. }
        | Request::WorkspaceMutate { .. }
        | Request::DraftPrepare { .. }
        | Request::DraftStage { .. } => {
            if backend
                .active_workspace
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    (current < 4).then_some(current + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Conflict,
                        "Too many user-data requests; retry after the current request.",
                    ),
                );
                return false;
            }
            let control = backend.control.clone();
            let active = backend.active_workspace.clone();
            thread::spawn(move || {
                let result = match request {
                    Request::WorkspaceScopes { params, .. } => {
                        crate::services::workspace_data::scope_read(params)
                            .map(|value| json!(value))
                            .map_err(|message| {
                                ProtocolError::new(ProtocolErrorCode::Conflict, message)
                            })
                    }
                    Request::WorkspaceRead { params, .. } => {
                        crate::services::workspace_data::record_read(params)
                            .map(|value| json!(value))
                            .map_err(|message| {
                                ProtocolError::new(ProtocolErrorCode::Conflict, message)
                            })
                    }
                    Request::WorkspaceMutate { params, .. } => {
                        crate::services::workspace_data::record_mutate(params)
                            .map(|value| json!(value))
                            .map_err(|message| {
                                ProtocolError::new(ProtocolErrorCode::Conflict, message)
                            })
                    }
                    Request::DraftPrepare { params, .. } => {
                        crate::services::draft_staging::prepare(params).map(|value| json!(value))
                    }
                    Request::DraftStage { params, .. } => {
                        crate::services::draft_staging::stage(params).map(|value| json!(value))
                    }
                    _ => unreachable!(),
                };
                let response = match result {
                    Ok(value) => Response {
                        id,
                        ok: true,
                        result: Some(value),
                        error: None,
                    },
                    Err(error) => Response {
                        id,
                        ok: false,
                        result: None,
                        error: Some(error),
                    },
                };
                active.fetch_sub(1, Ordering::SeqCst);
                let _ = control.send(ServerMessage::Response(response));
            });
            false
        }
        Request::DataPreview { .. }
        | Request::DataImport { .. }
        | Request::DataExport { .. }
        | Request::SettingsGet { .. }
        | Request::WorkspaceReadiness { .. }
        | Request::SettingsSave { .. }
        | Request::AuthStatus { .. }
        | Request::AuthSet { .. }
        | Request::AuthClear { .. } => {
            if backend
                .active_settings
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    (current < 1).then_some(current + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Conflict,
                        "Another settings request is in progress.",
                    ),
                );
                return false;
            }
            let control = backend.control.clone();
            let active = backend.active_settings.clone();
            thread::spawn(move || {
                let result = match request {
                    Request::DataPreview { params, .. } => {
                        crate::services::workspace_data::preview(
                            std::path::Path::new(&params.path),
                            params.provider,
                        )
                        .map(|value| json!(value))
                    }
                    Request::DataImport { params, .. } => crate::services::workspace_data::import(
                        std::path::Path::new(&params.path),
                        params.provider,
                        &params.input_version,
                        &params.destination_version,
                    )
                    .map(|value| json!(value)),
                    Request::DataExport { params, .. } => {
                        crate::services::workspace_data::export(std::path::Path::new(&params.path))
                            .map(|value| json!({"exported":value}))
                    }
                    Request::SettingsSave { params, .. } => {
                        crate::services::settings::save(&params.expected_version, params.settings)
                            .map(|value| json!(value))
                    }
                    Request::AuthStatus { .. } => {
                        Ok(json!({"credentials":crate::services::workspace_auth::statuses()}))
                    }
                    Request::WorkspaceReadiness { .. } => {
                        crate::services::workspace_readiness::snapshot().map(|value| json!(value))
                    }
                    Request::AuthSet { params, .. } => {
                        let provider = params.provider;
                        crate::services::workspace_auth::set(params)
                            .map(|()| json!({"saved":true,"provider":provider}))
                    }
                    Request::AuthClear { params, .. } => {
                        crate::services::workspace_auth::clear(params.provider)
                            .map(|()| json!({"cleared":true,"provider":params.provider}))
                    }
                    _ => crate::services::settings::snapshot().map(|value| json!(value)),
                };
                let response = match result {
                    Ok(value) => Response {
                        id,
                        ok: true,
                        result: Some(json!(value)),
                        error: None,
                    },
                    Err(message) => Response {
                        id,
                        ok: false,
                        result: None,
                        error: Some(ProtocolError::new(ProtocolErrorCode::Conflict, message)),
                    },
                };
                active.fetch_sub(1, Ordering::SeqCst);
                let _ = control.send(ServerMessage::Response(response));
            });
            false
        }
        Request::RepositoryStatus { .. } => {
            backend.respond(&id, json!({ "repos": repositories() }));
            false
        }
        Request::DiffFile { params, .. } => {
            if backend
                .active_diffs
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    (current < MAX_CONCURRENT_DIFFS).then_some(current + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Internal,
                        "too many concurrent diff requests",
                    ),
                );
                return false;
            }
            let control = backend.control.clone();
            let counter = ActiveCounter(backend.active_diffs.clone());
            let snapshots = backend.provider_snapshots.clone();
            thread::spawn(move || {
                diff_file_response(&control, &id, params, &snapshots);
                counter.decrement();
            });
            false
        }
        Request::ReviewStart { params, .. } => {
            if matches!(&params.target, TargetIdentity::Provider { pr_id: Some(id), .. } if crate::local_review::is_local_review_id(u64::from(*id)))
            {
                backend.fail(&id, ProtocolError::new(ProtocolErrorCode::InvalidParams, "Saved local data is read-only here; select a working-tree or branch target to start a local review."));
                return false;
            }
            if backend
                .active_operations
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    (current < MAX_ACTIVE_OPERATIONS).then_some(current + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Internal,
                        "too many active review operations",
                    ),
                );
                return false;
            }
            let operation_id = format!(
                "op-{}",
                backend.next_operation.fetch_add(1, Ordering::SeqCst)
            );
            backend.start_review(&id, &operation_id, params);
            false
        }
        Request::ReviewFixState { params, .. } => {
            let result = fix_state_response(&params.target);
            match result {
                Ok(result) => {
                    backend.respond(&id, result);
                }
                Err(error) => {
                    backend.fail(&id, error);
                }
            }
            false
        }
        Request::ReviewCommitPreview { params, .. } => {
            backend.preview_mutation(
                id,
                params.target,
                ApprovedAction::Commit,
                Some(params.message),
            );
            false
        }
        Request::ReviewSyncPreview { params, .. }
        | Request::ReviewConflictsPreview { params, .. }
        | Request::ReviewPushPreview { params, .. }
        | Request::ReviewFixPreview { params, .. } => {
            let action = match method {
                "review.sync.preview" => ApprovedAction::Sync,
                "review.conflicts.preview" => ApprovedAction::Conflicts,
                "review.push.preview" => ApprovedAction::Push,
                _ => ApprovedAction::Fix,
            };
            backend.preview_mutation(id, params.target, action, None);
            false
        }
        Request::ReviewCommit { params, .. }
        | Request::ReviewPush { params, .. }
        | Request::ReviewSync { params, .. }
        | Request::ReviewConflicts { params, .. }
        | Request::ReviewFix { params, .. } => {
            if backend
                .active_operations
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    (current < MAX_ACTIVE_OPERATIONS).then_some(current + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(ProtocolErrorCode::Conflict, "too many operations"),
                );
                return false;
            }
            let action = match method {
                "review.commit" => ApprovedAction::Commit,
                "review.push" => ApprovedAction::Push,
                "review.sync" => ApprovedAction::Sync,
                "review.conflicts" => ApprovedAction::Conflicts,
                _ => ApprovedAction::Fix,
            };
            backend.start_fix(&id, params, action);
            false
        }
        Request::ReviewReply { params, .. } => {
            if backend
                .active_operations
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    (current < MAX_ACTIVE_OPERATIONS).then_some(current + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Internal,
                        "too many active review operations",
                    ),
                );
                return false;
            }
            let operation_id = format!(
                "op-{}",
                backend.next_operation.fetch_add(1, Ordering::SeqCst)
            );
            backend.start_reply(&id, &operation_id, params.target, params.message);
            false
        }
        Request::OperationStatus { params, .. } => {
            match operation_snapshot(backend, params.operation_id.as_deref()) {
                Some(result) => {
                    backend.respond(&id, result);
                }
                None => {
                    backend.fail(
                        &id,
                        ProtocolError::new(ProtocolErrorCode::NotFound, "unknown operation"),
                    );
                }
            }
            false
        }
        Request::OperationCancel { params, .. } => {
            match cancel_operation(backend, params.operation_id.as_deref()) {
                Some(result) => {
                    backend.respond(&id, result);
                }
                None => {
                    backend.fail(
                        &id,
                        ProtocolError::new(ProtocolErrorCode::NotFound, "unknown operation"),
                    );
                }
            }
            false
        }
        Request::ReviewFiles { params, .. } => {
            if backend
                .active_files
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    (current < MAX_CONCURRENT_FILES).then_some(current + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Internal,
                        "too many concurrent file requests",
                    ),
                );
                return false;
            }
            let control = backend.control.clone();
            let counter = ActiveCounter(backend.active_files.clone());
            let snapshots = backend.provider_snapshots.clone();
            thread::spawn(move || {
                review_files_response(&control, &id, params.target, &snapshots);
                counter.decrement();
            });
            false
        }
        Request::ReviewFindings { params, .. } => {
            match findings_response(&params.target) {
                Ok(value) => backend.respond(&id, value),
                Err(error) => backend.fail(&id, error),
            };
            false
        }
        Request::ReviewTargets { .. } => {
            backend.respond(&id, review_targets_response());
            false
        }
        Request::ReviewProfiles { params, .. } => {
            let active = backend.active_provider_reads.clone();
            if active
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                    (count < 2).then_some(count + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Conflict,
                        "Too many concurrent profile requests.",
                    ),
                );
                return false;
            }
            let counter = ActiveCounter(active);
            let control = backend.control.clone();
            let shutdown = backend.shutdown.clone();
            thread::spawn(move || {
                let result = if shutdown.load(Ordering::SeqCst) {
                    Err(ProtocolError::new(
                        ProtocolErrorCode::Cancelled,
                        "Profile request cancelled.",
                    ))
                } else {
                    review_profiles_response(&params.target)
                };
                if !shutdown.load(Ordering::SeqCst) {
                    let (value, error) = match result {
                        Ok(value) => (Some(value), None),
                        Err(error) => (None, Some(error)),
                    };
                    let _ = control.send(ServerMessage::Response(Response {
                        id,
                        ok: error.is_none(),
                        result: value,
                        error,
                    }));
                }
                counter.decrement();
            });
            false
        }
        Request::ReviewHistory { params, .. } => {
            match review_history_response(&params.target) {
                Ok(value) => backend.respond(&id, value),
                Err(error) => backend.fail(&id, error),
            };
            false
        }
        Request::ReviewThreads { params, .. } => {
            match review_threads_response(&params.target) {
                Ok(value) => backend.respond(&id, value),
                Err(error) => backend.fail(&id, error),
            };
            false
        }
        Request::ReviewOutput { params, .. } => {
            match review_output_response(&params.target) {
                Ok(value) => backend.respond(&id, value),
                Err(error) => backend.fail(&id, error),
            };
            false
        }
        Request::FilePreview { .. } | Request::ProviderSnapshotImage { .. } => {
            if backend
                .active_previews
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    (current < MAX_CONCURRENT_PREVIEWS).then_some(current + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Internal,
                        "too many concurrent preview requests",
                    ),
                );
                return false;
            }
            let control = backend.control.clone();
            let counter = ActiveCounter(backend.active_previews.clone());
            let snapshots = backend.provider_snapshots.clone();
            let shutdown = backend.shutdown.clone();
            thread::spawn(move || {
                match request {
                    Request::FilePreview { params, .. } => {
                        file_preview_response(&control, &id, params, &snapshots)
                    }
                    Request::ProviderSnapshotImage { params, .. } => {
                        let result = if shutdown.load(Ordering::SeqCst) {
                            Err("Image preview canceled during shutdown.".to_string())
                        } else {
                            snapshots.image(params).map(|value| json!(value))
                        };
                        let response = match result {
                            Ok(value) => Response {
                                id,
                                ok: true,
                                result: Some(value),
                                error: None,
                            },
                            Err(message) => Response {
                                id,
                                ok: false,
                                result: None,
                                error: Some(ProtocolError::new(
                                    ProtocolErrorCode::Conflict,
                                    message,
                                )),
                            },
                        };
                        let _ = control.send(ServerMessage::Response(response));
                    }
                    _ => unreachable!(),
                }
                counter.decrement();
            });
            false
        }
        Request::BrowserOpen { .. }
        | Request::ProviderSnapshotBrowser { .. }
        | Request::ProviderBrowser { .. } => {
            if backend
                .active_browser
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    (current < MAX_CONCURRENT_BROWSER_OPENS).then_some(current + 1)
                })
                .is_err()
            {
                backend.fail(
                    &id,
                    ProtocolError::new(
                        ProtocolErrorCode::Internal,
                        "a browser open request is already in progress",
                    ),
                );
                return false;
            }
            let control = backend.control.clone();
            let browser = Arc::clone(&backend.browser);
            let counter = ActiveCounter(backend.active_browser.clone());
            let snapshots = backend.provider_snapshots.clone();
            let shutdown = backend.shutdown.clone();
            let provider_reads = backend.active_provider_reads.clone();
            thread::spawn(move || {
                match request {
                    Request::BrowserOpen { params, .. } => {
                        browser_open_response(&control, &browser, &id, params.target, &snapshots)
                    }
                    Request::ProviderBrowser { params, .. } => {
                        let result = provider_browser_open(params, &provider_reads, &shutdown);
                        let response = match result {
                            Ok(value) => Response {
                                id,
                                ok: true,
                                result: Some(value),
                                error: None,
                            },
                            Err(message) => Response {
                                id,
                                ok: false,
                                result: None,
                                error: Some(ProtocolError::new(
                                    ProtocolErrorCode::Conflict,
                                    message,
                                )),
                            },
                        };
                        let _ = control.send(ServerMessage::Response(response));
                    }
                    Request::ProviderSnapshotBrowser { params, .. } => {
                        let result = if shutdown.load(Ordering::SeqCst) {
                            Err("Browser preview canceled during shutdown.".into())
                        } else {
                            snapshot_browser_open(&browser, &snapshots, params)
                        };
                        let response = match result {
                            Ok(value) => Response {
                                id,
                                ok: true,
                                result: Some(value),
                                error: None,
                            },
                            Err(message) => Response {
                                id,
                                ok: false,
                                result: None,
                                error: Some(ProtocolError::new(
                                    ProtocolErrorCode::Conflict,
                                    message,
                                )),
                            },
                        };
                        let _ = control.send(ServerMessage::Response(response));
                    }
                    _ => unreachable!(),
                }
                counter.decrement();
            });
            false
        }
        Request::Shutdown { .. } => {
            backend.respond(&id, json!({}));
            backend.shutdown.store(true, Ordering::SeqCst);
            true
        }
    }
}

#[derive(Clone)]
struct ActiveCounter(Arc<AtomicUsize>);

impl ActiveCounter {
    fn decrement(&self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn repositories() -> Vec<serde_json::Value> {
    config::load()
        .repos
        .into_iter()
        .map(|repo| {
            json!({
                "provider": match repo.provider {
                    config::ReviewProvider::Github => "github",
                    config::ReviewProvider::Bitbucket => "bitbucket",
                },
                "workspace": repo.workspace,
                "repo": repo.repo,
                "localPath": repo.local_path,
            })
        })
        .collect()
}

/// Resolve actual saved runs from jobs without guessing their provider or treating
/// a job ID as a run ID. Imported scopes are exposed separately by workspace.scopes.
fn review_targets_response() -> serde_json::Value {
    let mut targets = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if let Ok(jobs) = crate::review_storage::list_recent_review_jobs(50) {
        for job in jobs {
            if job.pr_id == 0 {
                continue;
            }
            let Some(thread_id) = job.thread_id.as_deref() else {
                continue;
            };
            for provider in [
                crate::services::review::ReviewProvider::Github,
                crate::services::review::ReviewProvider::Bitbucket,
            ] {
                let Ok(Some(store)) =
                    crate::services::review_scope::load(&crate::services::review_scope::Scope {
                        provider,
                        workspace: job.workspace.clone(),
                        repo: job.repo.clone(),
                        pr_id: job.pr_id,
                    })
                else {
                    continue;
                };
                let Some(thread) = store.threads.iter().find(|thread| thread.id == thread_id)
                else {
                    continue;
                };
                let Some(run) = store.review_runs.iter().rev().find(|run| {
                    run.thread_id.as_deref() == Some(thread_id)
                        && run.workspace == job.workspace
                        && run.repo == job.repo
                        && run.pr_id == job.pr_id
                        && !run.id.is_empty()
                        && run.id.len() <= 256
                }) else {
                    continue;
                };
                let provider = match run.provider {
                    crate::services::review::ReviewProvider::Github => "github",
                    crate::services::review::ReviewProvider::Bitbucket => "bitbucket",
                };
                if !seen.insert((
                    provider.to_string(),
                    run.workspace.clone(),
                    run.repo.clone(),
                    run.pr_id,
                    run.id.clone(),
                )) {
                    continue;
                }
                targets.push(json!({
                    "provider": provider, "workspace": run.workspace, "repo": run.repo,
                    "prId": run.pr_id, "runId": run.id,
                    "title": thread.title.chars().take(256).collect::<String>(),
                    "status": run.status,
                }));
            }
        }
    }
    json!({ "targets": targets })
}

fn provider_review_scope(
    target: &TargetIdentity,
) -> Result<crate::services::review_scope::Scope, ProtocolError> {
    let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        pr_id: Some(pr_id),
        ..
    } = target
    else {
        return Err(fix_error("Select a saved pull-request review."));
    };
    Ok(crate::services::review_scope::Scope {
        provider: match provider {
            ProviderKind::Github => crate::services::review::ReviewProvider::Github,
            ProviderKind::Bitbucket => crate::services::review::ReviewProvider::Bitbucket,
        },
        workspace: workspace.clone(),
        repo: repo.clone(),
        pr_id: *pr_id,
    })
}

fn load_provider_review_store(
    target: &TargetIdentity,
) -> Result<Option<crate::services::review::AiReviewStoreData>, String> {
    let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        pr_id: Some(pr_id),
        ..
    } = target
    else {
        return Ok(None);
    };
    crate::services::review_scope::load(&crate::services::review_scope::Scope {
        provider: match provider {
            ProviderKind::Github => crate::services::review::ReviewProvider::Github,
            ProviderKind::Bitbucket => crate::services::review::ReviewProvider::Bitbucket,
        },
        workspace: workspace.clone(),
        repo: repo.clone(),
        pr_id: *pr_id,
    })
}

/// Filter shared legacy storage by the complete saved run identity before
/// exposing findings or conversations. This does not rewrite legacy storage.
fn scoped_review_store(
    target: &TargetIdentity,
    mut store: crate::services::review::AiReviewStoreData,
) -> Result<crate::services::review::AiReviewStoreData, ProtocolError> {
    let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        pr_id: Some(pr_id),
        run_id,
    } = target
    else {
        return Ok(crate::services::review::AiReviewStoreData::default());
    };
    let expected = match provider {
        ProviderKind::Github => crate::services::review::ReviewProvider::Github,
        ProviderKind::Bitbucket => crate::services::review::ReviewProvider::Bitbucket,
    };
    store.review_runs.retain(|run| {
        run.provider == expected
            && run.workspace == *workspace
            && run.repo == *repo
            && run.pr_id == *pr_id
    });
    if run_id
        .as_deref()
        .is_some_and(|id| !store.review_runs.iter().any(|run| run.id == id))
    {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "Selected saved review does not match this target.",
        ));
    }
    let threads: std::collections::HashSet<_> = store
        .review_runs
        .iter()
        .filter_map(|run| run.thread_id.as_deref())
        .collect();
    store
        .threads
        .retain(|thread| threads.contains(thread.id.as_str()));
    if store
        .active_thread_id
        .as_deref()
        .is_some_and(|id| !threads.contains(id))
    {
        store.active_thread_id = None;
    }
    Ok(store)
}

fn review_output_response(target: &TargetIdentity) -> Result<serde_json::Value, ProtocolError> {
    let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        pr_id: Some(pr_id),
        run_id,
    } = target
    else {
        return Err(ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "Select a stored pull-request review to copy its output.",
        ));
    };
    let store = load_provider_review_store(target).map_err(|_| {
        ProtocolError::new(
            ProtocolErrorCode::Internal,
            "Stored review output is unavailable.",
        )
    })?;
    let run = select_review_run(store, run_id.as_deref()).ok_or_else(|| {
        let mismatch = run_id.as_deref().is_some_and(|id| {
            crate::services::review_scope::foreign_run_exists(
                &crate::services::review_scope::Scope {
                    provider: match provider {
                        ProviderKind::Github => crate::services::review::ReviewProvider::Github,
                        ProviderKind::Bitbucket => {
                            crate::services::review::ReviewProvider::Bitbucket
                        }
                    },
                    workspace: workspace.clone(),
                    repo: repo.clone(),
                    pr_id: *pr_id,
                },
                id,
            )
            .unwrap_or(false)
        });
        ProtocolError::new(
            if mismatch {
                ProtocolErrorCode::TargetStale
            } else {
                ProtocolErrorCode::NotFound
            },
            "Selected stored review was not found.",
        )
    })?;
    if run.provider
        != match provider {
            ProviderKind::Github => crate::services::review::ReviewProvider::Github,
            ProviderKind::Bitbucket => crate::services::review::ReviewProvider::Bitbucket,
        }
        || run.workspace != *workspace
        || run.repo != *repo
        || run.pr_id != *pr_id
    {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "Stored review does not match the selected target.",
        ));
    }
    let text = run
        .summary_markdown
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| {
            ProtocolError::new(
                ProtocolErrorCode::NotFound,
                "Selected review has no stored output.",
            )
        })?;
    if text.len() > 64 * 1024 {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidParams,
            "Review output exceeds the 64 KiB terminal clipboard limit; nothing was copied.",
        ));
    }
    Ok(json!({ "target": target, "runId": run.id, "text": text }))
}

fn review_threads_response(target: &TargetIdentity) -> Result<serde_json::Value, ProtocolError> {
    let TargetIdentity::Provider { pr_id: Some(_), .. } = target else {
        return Ok(json!({ "threads": [] }));
    };
    let store = load_provider_review_store(target)
        .map_err(|error| ProtocolError::new(ProtocolErrorCode::Internal, error))?
        .unwrap_or_default();
    let store = scoped_review_store(target, store)?;
    let selected_thread = match target {
        TargetIdentity::Provider {
            run_id: Some(run_id),
            ..
        } => store
            .review_runs
            .iter()
            .find(|run| &run.id == run_id)
            .and_then(|run| run.thread_id.as_deref()),
        _ => store.active_thread_id.as_deref(),
    };
    Ok(thread_summaries(&store, selected_thread))
}

fn thread_summaries(
    store: &crate::services::review::AiReviewStoreData,
    selected_thread: Option<&str>,
) -> serde_json::Value {
    // Preview data stays well below the 1 MiB frame bound even with escaped
    // Unicode. Session resume identifiers and credentials never leave Rust.
    let threads = store
        .threads
        .iter()
        .rev()
        .take(12)
        .map(|thread| {
            let latest_run = store
                .review_runs
                .iter()
                .rev()
                .find(|run| run.thread_id.as_deref() == Some(thread.id.as_str()));
            let messages = thread
                .messages
                .iter()
                .rev()
                .take(8)
                .rev()
                .map(|message| {
                    json!({
                        "id": message.id,
                        "role": message.role,
                        "content": message.content.chars().take(1000).collect::<String>(),
                        "truncated": message.content.chars().count() > 1000,
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "id": thread.id,
                "title": thread.title.chars().take(128).collect::<String>(),
                "latestRunId": latest_run.map(|run| &run.id),
            "selected": selected_thread == Some(thread.id.as_str()),
                "messages": messages,
                "truncated": thread.messages.len() > 8,
            })
        })
        .collect::<Vec<_>>();
    json!({ "threads": threads, "truncated": store.threads.len() > 12 })
}

fn provider_matches(kind: ProviderKind) -> config::ReviewProvider {
    match kind {
        ProviderKind::Github => config::ReviewProvider::Github,
        ProviderKind::Bitbucket => config::ReviewProvider::Bitbucket,
    }
}

/// Recent stored review runs for a target, most recent first (bounded).
fn review_history_response(target: &TargetIdentity) -> Result<serde_json::Value, ProtocolError> {
    let runs = match target {
        TargetIdentity::Provider { pr_id: Some(_), .. } => {
            let store = load_provider_review_store(target).map_err(|error| {
                ProtocolError::new(
                    ProtocolErrorCode::Internal,
                    format!("review store: {error}"),
                )
            })?;
            let mut runs = scoped_review_store(target, store.unwrap_or_default())?.review_runs;
            runs.reverse();
            runs.into_iter()
                .take(20)
                .map(|run| {
                    json!({
                        "runId": run.id,
                        "createdAt": run.created_at,
                        "status": serde_json::to_value(run.status).unwrap_or_else(|_| json!("unknown")),
                        "turnKind": serde_json::to_value(run.turn_kind).unwrap_or_else(|_| json!("initial")),
                        "findingsCount": run.findings.len(),
                    })
                })
                .collect::<Vec<_>>()
        }
        _ => Vec::new(),
    };
    Ok(json!({ "runs": runs }))
}

/// Return one image chunk pinned to a changed-file side and content hash.
fn file_preview_response(
    control: &Sender<ServerMessage>,
    id: &str,
    params: protocol::FilePreviewParams,
    snapshots: &crate::services::provider_snapshot::Manager,
) {
    let result = image_preview_result(&params, snapshots);
    let response = match result {
        Ok(value) => Response {
            id: id.into(),
            ok: true,
            result: Some(value),
            error: None,
        },
        Err(error) => Response {
            id: id.into(),
            ok: false,
            result: None,
            error: Some(error),
        },
    };
    let _ = control.send(ServerMessage::Response(response));
}
fn image_preview_result(
    params: &protocol::FilePreviewParams,
    snapshots: &crate::services::provider_snapshot::Manager,
) -> Result<serde_json::Value, ProtocolError> {
    use sha2::{Digest, Sha256};
    let side = params.side.as_deref().unwrap_or("new");
    if let Some(info) = saved_provider_snapshot(&params.target, snapshots)? {
        let request = crate::services::provider_snapshot::ImageParams {
            snapshot: info.snapshot,
            path: params.path.clone(),
            side: if side == "old" {
                crate::services::provider_snapshot::ImageSide::Old
            } else {
                crate::services::provider_snapshot::ImageSide::New
            },
            offset: params.offset.unwrap_or(0) as usize,
            max_bytes: params.max_bytes.unwrap_or(192 * 1024) as usize,
            content_sha256: params.content_sha256.clone(),
        };
        let image = snapshots
            .image(request)
            .map_err(|message| ProtocolError::new(ProtocolErrorCode::TargetStale, message))?;
        return Ok(
            json!({ "path": params.path, "side": side, "available": image.available,
            "mimeType": image.mime_type, "size": image.size, "dataBase64": image.data_base64,
            "offset": params.offset.unwrap_or(0), "nextOffset": image.next_offset, "contentSha256": image.content_sha256,
            "width": image.width, "height": image.height }),
        );
    }
    let repo_path = resolve_repo_path(&params.target).ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "No local repository for image comparison.",
        )
    })?;
    let range = reviewed_range(&params.target)?;
    if target_pr_id(&params.target).is_some() && range.is_none() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "No stored reviewed range for image comparison.",
        ));
    }
    let files = changed_file_entries(
        &repo_path,
        range
            .as_ref()
            .map(|(base, head)| (base.as_str(), head.as_str())),
    )?;
    let file = files
        .iter()
        .find(|file| file["path"].as_str() == Some(&params.path))
        .ok_or_else(|| {
            ProtocolError::new(
                ProtocolErrorCode::NotFound,
                "Image path is outside this target's changed files.",
            )
        })?;
    let status = file["status"].as_str().unwrap_or("");
    if (side == "old" && matches!(status, "added" | "untracked"))
        || (side == "new" && status == "deleted")
    {
        return Ok(
            json!({"path":params.path,"side":side,"available":false,"mimeType":"","size":0,"dataBase64":"","offset":0,"nextOffset":null,"contentSha256":null,"width":null,"height":null}),
        );
    }
    let source_path = if side == "old" {
        file["oldPath"].as_str().unwrap_or(&params.path)
    } else {
        &params.path
    };
    let mime = image_mime_for_path(source_path).ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "Unsupported image preview format; use the browser viewer.",
        )
    })?;
    if mime == "image/svg+xml" {
        return Err(ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "SVG terminal previews are unavailable; use the browser viewer.",
        ));
    }
    let revision = range
        .as_ref()
        .map(|(base, head)| {
            if side == "old" {
                base.as_str()
            } else {
                head.as_str()
            }
        })
        .or_else(|| if side == "old" { Some("HEAD") } else { None });
    let bytes = if let Some(revision) = revision {
        let spec = format!("{revision}:{source_path}");
        git_bytes(&repo_path, &["show", &spec], MAX_PREVIEW_BYTES + 1).ok_or_else(|| {
            ProtocolError::new(
                ProtocolErrorCode::NotFound,
                "Reviewed image revision is unavailable in the local checkout.",
            )
        })?
    } else {
        let absolute = safe_canonical_path(&repo_path, source_path).ok_or_else(|| {
            ProtocolError::new(
                ProtocolErrorCode::NotFound,
                "Image path is unavailable or outside the checkout.",
            )
        })?;
        if std::fs::symlink_metadata(repo_path.join(source_path))
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(ProtocolError::new(
                ProtocolErrorCode::NotFound,
                "Symlink image previews are unavailable; use the browser viewer.",
            ));
        }
        let file = std::fs::File::open(absolute).map_err(|_| {
            ProtocolError::new(ProtocolErrorCode::NotFound, "Cannot open image preview.")
        })?;
        let mut bytes = Vec::new();
        file.take(MAX_PREVIEW_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                ProtocolError::new(ProtocolErrorCode::Internal, "Cannot read image preview.")
            })?;
        bytes
    };
    if bytes.len() as u64 > MAX_PREVIEW_BYTES {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "Image preview exceeds the 2 MiB limit; use the browser viewer.",
        ));
    }
    let hash = hex::encode(Sha256::digest(&bytes));
    if params
        .content_sha256
        .as_ref()
        .is_some_and(|expected| !expected.eq_ignore_ascii_case(&hash))
    {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "Image changed during loading; select the file again.",
        ));
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| ProtocolError::new(ProtocolErrorCode::NotFound, "Invalid image header."))?;
    let expected = match mime {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/gif" => image::ImageFormat::Gif,
        "image/webp" => image::ImageFormat::WebP,
        "image/bmp" => image::ImageFormat::Bmp,
        _ => {
            return Err(ProtocolError::new(
                ProtocolErrorCode::NotFound,
                "Unsupported terminal image format.",
            ))
        }
    };
    if reader.format() != Some(expected) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "Image content does not match its file format; use the browser viewer.",
        ));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(40_000_000 * 4 + 16 * 1024 * 1024);
    reader.limits(limits);
    let (width, height) = reader.into_dimensions().map_err(|_| {
        ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "Invalid or unsupported image data; use the browser viewer.",
        )
    })?;
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || u64::from(width) * u64::from(height) > 40_000_000
    {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "Image dimensions exceed terminal preview limits; use the browser viewer.",
        ));
    }
    let offset = params.offset.unwrap_or(0) as usize;
    if offset > bytes.len() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidRequest,
            "Image chunk offset exceeds its content.",
        ));
    }
    if params.max_bytes.is_none() && bytes.len() > 192 * 1024 {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidRequest,
            "This image requires bounded chunk requests.",
        ));
    }
    let end = (offset + params.max_bytes.unwrap_or(192 * 1024) as usize).min(bytes.len());
    Ok(
        json!({"path":params.path,"side":side,"available":true,"mimeType":mime,"size":bytes.len(),"dataBase64":STANDARD.encode(&bytes[offset..end]),"offset":offset,"nextOffset":if end<bytes.len(){Some(end)}else{None},"contentSha256":hash,"width":width,"height":height}),
    )
}

fn image_mime_for_path(path: &str) -> Option<&'static str> {
    let normalized = path.to_ascii_lowercase();
    let extension = normalized.rsplit('.').next()?;
    match extension {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn browser_open_response(
    control: &Sender<ServerMessage>,
    browser: &Arc<Mutex<Option<WebDiffServer>>>,
    id: &str,
    target: TargetIdentity,
    snapshots: &Arc<crate::services::provider_snapshot::Manager>,
) {
    let respond = |value: serde_json::Value| {
        let _ = control.send(ServerMessage::Response(Response {
            id: id.to_string(),
            ok: true,
            result: Some(value),
            error: None,
        }));
    };
    let fail = |error: ProtocolError| {
        let _ = control.send(ServerMessage::Response(Response {
            id: id.to_string(),
            ok: false,
            result: None,
            error: Some(error),
        }));
    };
    let snapshot = match saved_provider_snapshot(&target, snapshots) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            fail(error);
            return;
        }
    };
    let result = match snapshot {
        Some(info) => snapshot_browser_state(
            snapshots,
            &crate::services::provider_snapshot::BrowserParams {
                snapshot: info.snapshot,
            },
        )
        .map_err(inline_start_error),
        None => browser_diff_state(&target),
    };
    let state = match result {
        Ok(state) => state,
        Err(error) => {
            fail(error);
            return;
        }
    };
    let url = match browser_server_url(browser, state) {
        Ok(url) => url,
        Err(message) => {
            fail(ProtocolError::new(ProtocolErrorCode::Internal, message));
            return;
        }
    };
    if std::env::var("NORN_BROWSER_OPEN")
        .map(|value| value != "0")
        .unwrap_or(true)
    {
        let _ = open_browser_url(&url);
    }
    respond(json!({ "url": url }));
}

fn browser_server_url(
    browser: &Arc<Mutex<Option<WebDiffServer>>>,
    state: WebDiffState,
) -> Result<String, String> {
    let mut guard = browser
        .lock()
        .map_err(|_| "Browser server lock is unavailable.".to_string())?;
    match guard.as_mut() {
        Some(server) => {
            server.update_pr(state);
            Ok(server.url())
        }
        None => {
            let server = WebDiffServer::start(Arc::new(RwLock::new(state)))?;
            let url = server.url();
            *guard = Some(server);
            Ok(url)
        }
    }
}

fn snapshot_browser_state(
    snapshots: &Arc<crate::services::provider_snapshot::Manager>,
    params: &crate::services::provider_snapshot::BrowserParams,
) -> Result<WebDiffState, String> {
    params.validate()?;
    let data = snapshots.get(&params.snapshot)?;
    let files = snapshots.inventory(&params.snapshot)?;
    let metadata: Vec<_> = files.iter().map(|file| json!(file)).collect();
    Ok(WebDiffState {
        provider: Some(params.snapshot.repository.provider),
        workspace: params.snapshot.repository.workspace.clone(),
        repo: params.snapshot.repository.repo.clone(),
        pr_id: params.snapshot.pr_id,
        pr_title: data.info.title.clone(),
        source_branch: data.info.source_branch.clone(),
        target_branch: data.info.destination_branch.clone(),
        base_sha: Some(data.info.base_sha.clone()),
        diff: Some(data.patch.clone()),
        diffstat: Some(diffstat_from_files(&metadata)),
        snapshot_source: Some(
            crate::services::provider_snapshot::BrowserSnapshotSource::new(
                params.snapshot.clone(),
                snapshots.clone(),
            ),
        ),
        ..WebDiffState::default()
    })
}

fn provider_browser_open(
    params: crate::services::bitbucket::workspace_prs::DetailParams,
    provider_reads: &AtomicUsize,
    shutdown: &AtomicBool,
) -> Result<serde_json::Value, String> {
    use crate::services::bitbucket::workspace_prs::{self, Api};
    params.validate()?;
    if shutdown.load(Ordering::Acquire) {
        return Err("Provider browser canceled during shutdown.".into());
    }
    let resolved = if workspace_prs::needs_browser_alias(&params.repository) {
        provider_reads
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < 2).then_some(n + 1)
            })
            .map_err(|_| {
                "Two provider reads are already in progress; wait before opening this PR."
                    .to_string()
            })?;
        let result = Api::from_stored(params.repository.clone())
            .and_then(|api| api.browser_repository(&params));
        provider_reads.fetch_sub(1, Ordering::SeqCst);
        result?
    } else {
        params.repository.clone()
    };
    let url = workspace_prs::browser_url(&params, &resolved)?;
    if shutdown.load(Ordering::Acquire) {
        return Err("Provider browser canceled during shutdown.".into());
    }
    let launch_requested = std::env::var("NORN_BROWSER_OPEN")
        .map(|value| value != "0")
        .unwrap_or(true);
    let launch_error = if launch_requested {
        open_browser_url(&url).err()
    } else {
        None
    };
    Ok(
        json!({"request":params,"repository":resolved,"url":url,"launchRequested":launch_requested,"launchError":launch_error}),
    )
}

fn snapshot_browser_open(
    browser: &Arc<Mutex<Option<WebDiffServer>>>,
    snapshots: &Arc<crate::services::provider_snapshot::Manager>,
    params: crate::services::provider_snapshot::BrowserParams,
) -> Result<serde_json::Value, String> {
    let state = snapshot_browser_state(snapshots, &params)?;
    let url = browser_server_url(browser, state)?;
    let launch_requested = std::env::var("NORN_BROWSER_OPEN")
        .map(|value| value != "0")
        .unwrap_or(true);
    let launch_error = if launch_requested {
        open_browser_url(&url).err()
    } else {
        None
    };
    Ok(
        json!({ "request": params, "url": url, "launchRequested": launch_requested, "launchError": launch_error }),
    )
}

/// Build the browser-viewer state for a target from the local checkout, reusing
/// the same reviewed revisions and diff computation as the review slice. No
/// provider credentials are included.
fn browser_diff_state(target: &TargetIdentity) -> Result<WebDiffState, ProtocolError> {
    let repo_path = resolve_repo_path(target).ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "no local repository for this target",
        )
    })?;
    if !is_git_repo(&repo_path) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            format!("`{}` is not a git repository", repo_path.display()),
        ));
    }
    let mut state = WebDiffState::default();

    if let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        pr_id: Some(pr_id),
        run_id,
    } = target
    {
        let Some((base, head)) = reviewed_range(target)? else {
            return Err(ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "no stored reviewed range for this pull request",
            ));
        };
        if !revisions_available(&repo_path, &base, &head) {
            return Err(ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "reviewed revisions are not available in the local checkout",
            ));
        }
        let diff = git(
            &repo_path,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "-M",
                "--src-prefix=a/",
                "--dst-prefix=b/",
                base.as_str(),
                head.as_str(),
            ],
        )
        .ok_or_else(|| {
            ProtocolError::new(
                ProtocolErrorCode::Internal,
                "failed to generate the reviewed diff",
            )
        })?;
        let files = changed_file_entries(&repo_path, Some((base.as_str(), head.as_str())))?;
        let run = load_provider_review_store(target)
            .map_err(|error| {
                ProtocolError::new(
                    ProtocolErrorCode::Internal,
                    format!("review store: {error}"),
                )
            })
            .ok()
            .and_then(|store| select_review_run(store, run_id.as_deref()));
        state.provider = Some(provider_matches(*provider));
        state.target_kind = WebDiffTargetKind::PullRequest;
        state.workspace = workspace.clone();
        state.repo = repo.clone();
        state.pr_id = *pr_id;
        state.pr_title = format!("Pull request #{pr_id}");
        if let Some(run) = run {
            state.source_branch = run.source_branch;
            state.target_branch = run.destination_branch;
        }
        state.base_sha = Some(base);
        state.diff = Some(diff);
        state.diffstat = Some(diffstat_from_files(&files));
        return Ok(state);
    }

    // Working-tree target (local, or a configured repository without a PR).
    let files = changed_file_entries(&repo_path, None)?;
    let diff = working_tree_patch(&repo_path, &files)?;
    state.target_kind = WebDiffTargetKind::Local;
    state.pr_title = "Local changes".to_string();
    state.source_branch = git(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    state.target_branch = "HEAD".to_string();
    state.base_sha = git(&repo_path, &["rev-parse", "HEAD"]).map(|value| value.trim().to_string());
    state.diff = Some(diff);
    state.diffstat = Some(diffstat_from_files(&files));
    match target {
        TargetIdentity::Provider {
            workspace, repo, ..
        } => {
            state.provider = None;
            state.workspace = workspace.clone();
            state.repo = repo.clone();
        }
        TargetIdentity::Local {
            local_snapshot_sha256,
        } => {
            state.local_snapshot_sha256 = Some(local_snapshot_sha256.clone());
        }
    }
    Ok(state)
}

/// Concatenate per-file working-tree patches, including untracked files and
/// repositories before their first commit, mirroring the terminal diff view.
fn working_tree_patch(
    repo_path: &Path,
    files: &[serde_json::Value],
) -> Result<String, ProtocolError> {
    let mut patch = String::new();
    for file in files {
        let Some(path) = file["path"].as_str() else {
            continue;
        };
        let old_path = file["oldPath"].as_str();
        match file_diff(repo_path, path, None, old_path) {
            Some(diff) => {
                patch.push_str(&diff);
                if !diff.ends_with('\n') {
                    patch.push('\n');
                }
            }
            None => {
                // Binary or zero-line changes legitimately have no textual patch.
                let additions = file["additions"].as_u64().unwrap_or(0);
                let deletions = file["deletions"].as_u64().unwrap_or(0);
                if additions == 0 && deletions == 0 {
                    continue;
                }
                return Err(ProtocolError::new(
                    ProtocolErrorCode::Internal,
                    format!("failed to generate the working-tree diff for `{path}`"),
                ));
            }
        }
    }
    Ok(patch)
}

fn diffstat_from_files(
    files: &[serde_json::Value],
) -> Vec<crate::services::bitbucket::DiffstatEntry> {
    files
        .iter()
        .map(|file| {
            let path = file["path"].as_str().unwrap_or_default().to_string();
            let old = file["oldPath"].as_str().map(str::to_string);
            let status = match file["status"].as_str().unwrap_or("modified") {
                "added" | "untracked" => "added",
                "deleted" => "removed",
                "renamed" => "renamed",
                "copied" => "copied",
                _ => "modified",
            };
            let (old_path, new_path) = match status {
                "removed" => (old.or_else(|| Some(path.clone())), None),
                "renamed" | "copied" => (old, Some(path.clone())),
                _ => (None, Some(path.clone())),
            };
            crate::services::bitbucket::DiffstatEntry {
                status: status.to_string(),
                lines_added: file["additions"].as_u64().unwrap_or(0) as u32,
                lines_removed: file["deletions"].as_u64().unwrap_or(0) as u32,
                old_path,
                new_path,
            }
        })
        .collect()
}

/// Resolve the local checkout for the requested target. Provider targets require
/// a configured matching repository; local targets use the backend working
/// directory (overridable for deployment and tests).
fn resolve_repo_path(target: &TargetIdentity) -> Option<PathBuf> {
    match target {
        TargetIdentity::Provider {
            provider,
            workspace,
            repo,
            ..
        } => {
            let wanted = provider_matches(*provider);
            config::load().repos.into_iter().find_map(|entry| {
                if entry.provider == wanted && entry.workspace == *workspace && entry.repo == *repo
                {
                    entry
                        .local_path
                        .filter(|path| !path.is_empty())
                        .and_then(|path| PathBuf::from(path).canonicalize().ok())
                } else {
                    None
                }
            })
        }
        TargetIdentity::Local { .. } => std::env::var_os("NORN_BACKEND_REPO_PATH")
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok()),
    }
}

fn is_git_repo(repo_path: &Path) -> bool {
    git(repo_path, &["rev-parse", "--is-inside-work-tree"])
        .is_some_and(|output| output.trim() == "true")
}

fn diff_file_response(
    control: &Sender<ServerMessage>,
    id: &str,
    params: protocol::DiffFileParams,
    snapshots: &crate::services::provider_snapshot::Manager,
) {
    let respond = |value: serde_json::Value| {
        let _ = control.send(ServerMessage::Response(Response {
            id: id.to_string(),
            ok: true,
            result: Some(value),
            error: None,
        }));
    };
    let fail = |error: ProtocolError| {
        let _ = control.send(ServerMessage::Response(Response {
            id: id.to_string(),
            ok: false,
            result: None,
            error: Some(error),
        }));
    };

    match saved_provider_snapshot(&params.target, snapshots) {
        Ok(Some(info)) => {
            let result = snapshots.get(&info.snapshot).and_then(|data| {
                scope_to_file(&data.patch, &params.path, params.old_path.as_deref()).ok_or_else(
                    || "The path is outside this saved review's captured files.".to_string(),
                )
            });
            match result {
                Ok(diff) => {
                    let (diff, truncated) = truncate_diff(diff);
                    respond(
                        json!({ "target": params.target, "path": params.path, "diff": diff, "truncated": truncated }),
                    );
                }
                Err(message) => fail(ProtocolError::new(ProtocolErrorCode::TargetStale, message)),
            }
            return;
        }
        Err(error) => {
            fail(error);
            return;
        }
        Ok(None) => {}
    }
    let Some(repo_path) = resolve_repo_path(&params.target) else {
        fail(ProtocolError::new(
            ProtocolErrorCode::NotFound,
            format!("no local repository for `{}`", params.path),
        ));
        return;
    };
    if !is_git_repo(&repo_path) {
        fail(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            format!("`{}` is not a git repository", repo_path.display()),
        ));
        return;
    }
    let range = match reviewed_range(&params.target) {
        Ok(range) => range,
        Err(error) => {
            fail(error);
            return;
        }
    };
    if target_pr_id(&params.target).is_some() {
        match range.as_ref() {
            Some((base, head)) => {
                if !revisions_available(&repo_path, base, head) {
                    fail(ProtocolError::new(
                        ProtocolErrorCode::TargetStale,
                        "reviewed revisions are not available in the local checkout",
                    ));
                    return;
                }
            }
            None => {
                fail(ProtocolError::new(
                    ProtocolErrorCode::TargetStale,
                    "no stored reviewed range for this pull request",
                ));
                return;
            }
        }
    }
    let range_ref = range
        .as_ref()
        .map(|(base, head)| (base.as_str(), head.as_str()));
    let (diff, truncated) = match file_diff(
        &repo_path,
        &params.path,
        range_ref,
        params.old_path.as_deref(),
    ) {
        Some(diff) => truncate_diff(diff),
        None => {
            fail(ProtocolError::new(
                ProtocolErrorCode::NotFound,
                format!("no diff for `{}`", params.path),
            ));
            return;
        }
    };
    respond(json!({
        "target": params.target,
        "path": params.path,
        "diff": diff,
        "truncated": truncated,
    }));
}

fn truncate_diff(diff: String) -> (String, bool) {
    if diff.len() <= MAX_DIFF_BYTES {
        return (diff, false);
    }
    let mut boundary = MAX_DIFF_BYTES;
    while boundary > 0 && !diff.is_char_boundary(boundary) {
        boundary -= 1;
    }
    (diff[..boundary].to_string(), true)
}

/// Keep only the `diff --git` section whose destination is `path`. Passing a
/// rename source can make Git emit several sections (for example when the old
/// path was recreated); anchor line numbers must come from the selected file.
/// Falls back to the whole diff when no section matches.
fn scope_to_file(diff: &str, path: &str, old_source: Option<&str>) -> Option<String> {
    let destination = git_maybe_quote(&format!("b/{path}"));
    let mut expected = vec![format!(
        "diff --git {} {}",
        git_maybe_quote(&format!("a/{path}")),
        destination
    )];
    if let Some(old) = old_source {
        if old != path {
            expected.push(format!(
                "diff --git {} {}",
                git_maybe_quote(&format!("a/{old}")),
                destination
            ));
        }
    }
    let mut result = String::new();
    let mut current = String::new();
    let mut current_match = false;
    let mut matched = false;
    for line in diff.split_inclusive('\n') {
        if line.starts_with("diff --git ") {
            if !current.is_empty() {
                if current_match {
                    result.push_str(&current);
                    matched = true;
                }
                current.clear();
            }
            let without_newline = line.strip_suffix('\n').unwrap_or(line);
            let header = without_newline
                .strip_suffix('\r')
                .unwrap_or(without_newline);
            current_match = expected.iter().any(|candidate| candidate == header);
        }
        current.push_str(line);
    }
    if !current.is_empty() && current_match {
        result.push_str(&current);
        matched = true;
    }
    matched.then_some(result)
}

/// Quote a full prefixed path only when Git would (control characters, quotes or
/// backslashes); with `core.quotePath=false` other characters stay literal.
fn git_maybe_quote(path: &str) -> String {
    let needs_quote = path
        .chars()
        .any(|character| (character as u32) < 0x20 || character as u32 == 0x7f)
        || path.contains('"')
        || path.contains('\\');
    if needs_quote {
        git_quoted_path(path)
    } else {
        path.to_string()
    }
}

/// Git's C-style quoting for a full prefixed path (for example `b/name\t`).
fn git_quoted_path(path: &str) -> String {
    let mut quoted = String::from("\"");
    for character in path.chars() {
        match character {
            '\u{7}' => quoted.push_str("\\a"),
            '\u{8}' => quoted.push_str("\\b"),
            '\t' => quoted.push_str("\\t"),
            '\n' => quoted.push_str("\\n"),
            '\u{b}' => quoted.push_str("\\v"),
            '\u{c}' => quoted.push_str("\\f"),
            '\r' => quoted.push_str("\\r"),
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            other if (other as u32) < 0x20 || other as u32 == 0x7f => {
                quoted.push_str(&format!("\\{:03o}", other as u32));
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

fn file_diff(
    repo_path: &Path,
    path: &str,
    range: Option<(&str, &str)>,
    old_path: Option<&str>,
) -> Option<String> {
    if !is_safe_relative_path(path) {
        return None;
    }
    // Include the pre-rename source path supplied from `review.files` so Git can
    // detect the rename; filtering by the destination alone would report an
    // addition and lose old-side lines.
    let old_source = old_path
        .filter(|old| !old.is_empty() && *old != path && is_safe_relative_path(old))
        .map(str::to_string);
    let build = |spec: &[&str]| {
        let mut args: Vec<&str> = spec.to_vec();
        args.push("--");
        args.push(path);
        if let Some(old) = old_source.as_deref() {
            if old != path {
                args.push(old);
            }
        }
        git(repo_path, &args)
    };
    if let Some((base, head)) = range {
        let diff = build(&[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "-M",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            base,
            head,
        ])?;
        if diff.trim().is_empty() {
            return None;
        }
        return scope_to_file(&diff, path, old_source.as_deref());
    }
    // HEAD covers committed, staged, and unstaged tracked changes (including
    // deletions). Before the first commit, compare the working tree against the
    // empty tree so a staged addition followed by an unstaged edit shows as one
    // net change, matching `review.files`.
    let head_exists = git(repo_path, &["rev-parse", "--verify", "--quiet", "HEAD"]).is_some();
    let empty_tree = if head_exists {
        None
    } else {
        empty_tree_oid(repo_path)
    };
    let mut specs: Vec<Vec<&str>> = Vec::new();
    if head_exists {
        specs.push(vec![
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "-M",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "HEAD",
        ]);
    } else if let Some(oid) = empty_tree.as_deref() {
        specs.push(vec![
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "-M",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            oid,
        ]);
    }
    specs.push(vec![
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "-M",
        "--src-prefix=a/",
        "--dst-prefix=b/",
    ]);
    specs.push(vec![
        "diff",
        "--cached",
        "--no-ext-diff",
        "--no-textconv",
        "-M",
        "--src-prefix=a/",
        "--dst-prefix=b/",
    ]);
    for spec in &specs {
        if let Some(diff) = build(spec) {
            if !diff.trim().is_empty() {
                if let Some(scoped) = scope_to_file(&diff, path, old_source.as_deref()) {
                    return Some(scoped);
                }
            }
        }
    }
    // No diff: only untracked files become a synthetic addition. Tracked files
    // (including an unchanged file or a deletion already covered above) are not
    // additions.
    if git(repo_path, &["ls-files", "--error-unmatch", "--", path]).is_some() {
        return None;
    }
    let untracked = git(
        repo_path,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            path,
        ],
    )
    .is_some_and(|output| output.split('\0').any(|entry| entry == path));
    if !untracked {
        return None;
    }
    synthetic_new_file_diff(repo_path, path)
}

/// Reject absolute paths and parent traversal regardless of whether the file
/// currently exists (a tracked deletion has no file on disk).
fn is_safe_relative_path(path: &str) -> bool {
    let relative = Path::new(path);
    !relative.is_absolute()
        && !relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

/// Canonicalize a repository-relative path for reading, rejecting symlinks and
/// escapes so synthetic reads stay inside the checkout.
fn safe_canonical_path(repo_path: &Path, path: &str) -> Option<PathBuf> {
    if !is_safe_relative_path(path) {
        return None;
    }
    let joined = repo_path.join(path);
    let metadata = std::fs::symlink_metadata(&joined).ok()?;
    if metadata.file_type().is_symlink() {
        return None;
    }
    let canonical_repo = std::fs::canonicalize(repo_path).ok()?;
    let canonical = std::fs::canonicalize(&joined).ok()?;
    canonical.starts_with(&canonical_repo).then_some(canonical)
}

fn synthetic_new_file_diff(repo_path: &Path, path: &str) -> Option<String> {
    let absolute = safe_canonical_path(repo_path, path)?;
    let metadata = std::fs::symlink_metadata(&absolute).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_SYNTHETIC_FILE_BYTES {
        return None;
    }
    let contents = std::fs::read_to_string(&absolute).ok()?;
    if contents.contains('\0') {
        return None;
    }
    let escaped_path = path.replace('\\', "/");
    let line_count = contents.lines().count();
    let mut patch = format!(
        "diff --git a/{escaped_path} b/{escaped_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{escaped_path}\n@@ -0,0 +1,{line_count} @@\n"
    );
    for line in contents.lines() {
        patch.push('+');
        patch.push_str(line);
        patch.push('\n');
    }
    if !contents.is_empty() && !contents.ends_with('\n') {
        patch.push_str("\\ No newline at end of file\n");
    }
    Some(patch)
}

fn review_files_response(
    control: &Sender<ServerMessage>,
    id: &str,
    target: TargetIdentity,
    snapshots: &crate::services::provider_snapshot::Manager,
) {
    let respond = |value: serde_json::Value| {
        let _ = control.send(ServerMessage::Response(Response {
            id: id.to_string(),
            ok: true,
            result: Some(value),
            error: None,
        }));
    };
    let fail = |error: ProtocolError| {
        let _ = control.send(ServerMessage::Response(Response {
            id: id.to_string(),
            ok: false,
            result: None,
            error: Some(error),
        }));
    };
    match saved_provider_snapshot(&target, snapshots) {
        Ok(Some(info)) => {
            // Native inventory is paged through the existing snapshot contract;
            // never serialize a potentially multi-megabyte array into one frame.
            respond(json!({ "target": target, "files": [], "providerSnapshot": info }));
            return;
        }
        Err(error) => {
            fail(error);
            return;
        }
        Ok(None) => {}
    }
    let Some(repo_path) = resolve_repo_path(&target) else {
        fail(ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "no local repository for this target",
        ));
        return;
    };
    if !is_git_repo(&repo_path) {
        fail(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            format!("`{}` is not a git repository", repo_path.display()),
        ));
        return;
    }
    let range = match reviewed_range(&target) {
        Ok(range) => range,
        Err(error) => {
            fail(error);
            return;
        }
    };
    if target_pr_id(&target).is_some() {
        match range.as_ref() {
            Some((base, head)) => {
                if !revisions_available(&repo_path, base, head) {
                    fail(ProtocolError::new(
                        ProtocolErrorCode::TargetStale,
                        "reviewed revisions are not available in the local checkout",
                    ));
                    return;
                }
            }
            None => {
                fail(ProtocolError::new(
                    ProtocolErrorCode::TargetStale,
                    "no stored reviewed range for this pull request",
                ));
                return;
            }
        }
    }
    let range_ref = range
        .as_ref()
        .map(|(base, head)| (base.as_str(), head.as_str()));
    match changed_file_entries(&repo_path, range_ref) {
        Ok(files) => respond(json!({ "target": target, "files": files })),
        Err(error) => fail(error),
    }
}

fn saved_provider_snapshot(
    target: &TargetIdentity,
    snapshots: &crate::services::provider_snapshot::Manager,
) -> Result<Option<crate::services::provider_snapshot::SnapshotInfo>, ProtocolError> {
    if !matches!(target, TargetIdentity::Provider { pr_id: Some(_), .. }) {
        return Ok(None);
    }
    let scope = provider_review_scope(target)?;
    let data = load_provider_review_store(target).map_err(inline_start_error)?;
    let TargetIdentity::Provider { run_id, .. } = target else {
        unreachable!()
    };
    let Some(run) = select_review_run(data, run_id.as_deref()) else {
        return Ok(None);
    };
    let Some(id) = run.provider_evidence_id.as_deref() else {
        return Ok(None);
    };
    crate::services::provider_snapshot::durable::load_run(&scope, &run)
        .map_err(|message| ProtocolError::new(ProtocolErrorCode::TargetStale, message))?;
    crate::services::provider_snapshot::durable::restore(snapshots, &scope, id)
        .map(Some)
        .map_err(|message| ProtocolError::new(ProtocolErrorCode::TargetStale, message))
}

/// The reviewed base/head revisions for a stored PR target, if the stored review
/// run recorded them. Returns `Ok(None)` for working-tree targets or targets
/// without a recorded range.
fn reviewed_range(target: &TargetIdentity) -> Result<Option<(String, String)>, ProtocolError> {
    let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        pr_id: Some(pr_id),
        run_id,
        ..
    } = target
    else {
        return Ok(None);
    };
    let store = load_provider_review_store(target).map_err(|e| {
        ProtocolError::new(ProtocolErrorCode::Internal, format!("review store: {e}"))
    })?;
    let Some(run) = select_review_run(store, run_id.as_deref()) else {
        return Ok(None);
    };
    if run.provider
        != match provider {
            ProviderKind::Github => crate::services::review::ReviewProvider::Github,
            ProviderKind::Bitbucket => crate::services::review::ReviewProvider::Bitbucket,
        }
        || run.workspace != *workspace
        || run.repo != *repo
        || run.pr_id != *pr_id
    {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "Stored review does not match this provider and repository target.",
        ));
    }
    match (
        run.reviewed_diff_base_sha.or(run.reviewed_base_sha),
        run.reviewed_head_sha,
    ) {
        (Some(base), Some(head)) => Ok(Some((base, head))),
        _ => Ok(None),
    }
}

/// Select the review run a target is pinned to: the requested `runId` when
/// present, otherwise the most recent stored run.
fn select_review_run(
    store: Option<crate::services::review::AiReviewStoreData>,
    run_id: Option<&str>,
) -> Option<crate::services::review::ReviewRun> {
    let store = store?;
    match run_id {
        Some(id) => store.review_runs.into_iter().find(|run| run.id == id),
        None => store.review_runs.into_iter().next_back(),
    }
}

fn revisions_available(repo_path: &Path, base: &str, head: &str) -> bool {
    let verify = |revision: &str| {
        let spec = format!("{revision}^{{commit}}");
        git(
            repo_path,
            &["rev-parse", "--verify", "--quiet", spec.as_str()],
        )
        .is_some()
    };
    verify(base) && verify(head)
}

fn target_pr_id(target: &TargetIdentity) -> Option<u32> {
    match target {
        TargetIdentity::Provider {
            pr_id: Some(pr_id), ..
        } => Some(*pr_id),
        _ => None,
    }
}

/// Best-effort changed-file list with status and line counts, covering staged,
/// unstaged, renamed, deleted and untracked files. All paths are read with
/// NUL-delimited Git output so renames and unusual filenames survive intact.
///
/// When `range` is set the list is the PR diff between two revisions and
/// untracked files are excluded; otherwise it is the checkout's working-tree
/// diff against HEAD (or the index before the first commit).
fn changed_file_entries(
    repo_path: &Path,
    range: Option<(&str, &str)>,
) -> Result<Vec<serde_json::Value>, ProtocolError> {
    use std::collections::BTreeMap;

    let mut entries: BTreeMap<String, (String, u64, u64, Option<String>)> = BTreeMap::new();

    // Compare against HEAD when it exists. Before the first commit there is no
    // HEAD, so compare the working tree against Git's empty tree to get the net
    // staged + unstaged change for each path.
    let head_exists =
        range.is_none() && git(repo_path, &["rev-parse", "--verify", "--quiet", "HEAD"]).is_some();
    let empty_tree = if range.is_none() && !head_exists {
        empty_tree_oid(repo_path)
    } else {
        None
    };
    let (specs, include_untracked): (Vec<Vec<&str>>, bool) = match range {
        Some((base, head)) => (vec![vec![base, head]], false),
        None => {
            if head_exists {
                (vec![vec!["HEAD"]], true)
            } else if let Some(oid) = empty_tree.as_deref() {
                (vec![vec![oid]], true)
            } else {
                (vec![vec!["--cached"]], true)
            }
        }
    };

    for diff_spec in &specs {
        let mut name_status_args = vec!["diff", "-z", "-M", "--name-status"];
        name_status_args.extend_from_slice(diff_spec);
        let output = git(repo_path, &name_status_args).ok_or_else(|| {
            ProtocolError::new(ProtocolErrorCode::Internal, "git diff --name-status failed")
        })?;
        {
            let tokens: Vec<&str> = output.split('\0').collect();
            let mut index = 0;
            while index < tokens.len() {
                let code = tokens[index];
                if code.is_empty() {
                    index += 1;
                    continue;
                }
                let kind = code.chars().next().unwrap_or('M');
                if kind == 'R' || kind == 'C' {
                    if index + 2 >= tokens.len() {
                        break;
                    }
                    let old_path = tokens[index + 1].to_string();
                    let path = tokens[index + 2].to_string();
                    entries
                        .entry(path)
                        .or_insert(("renamed".to_string(), 0, 0, Some(old_path)));
                    index += 3;
                } else {
                    if index + 1 >= tokens.len() {
                        break;
                    }
                    let status = match kind {
                        'A' => "added",
                        'D' => "deleted",
                        _ => "modified",
                    };
                    entries.entry(tokens[index + 1].to_string()).or_insert((
                        status.to_string(),
                        0,
                        0,
                        None,
                    ));
                    index += 2;
                }
            }
        }

        let mut numstat_args = vec!["diff", "-z", "-M", "--numstat"];
        numstat_args.extend_from_slice(diff_spec);
        let output = git(repo_path, &numstat_args).ok_or_else(|| {
            ProtocolError::new(ProtocolErrorCode::Internal, "git diff --numstat failed")
        })?;
        {
            let tokens: Vec<&str> = output.split('\0').collect();
            let mut index = 0;
            while index < tokens.len() {
                let token = tokens[index];
                if token.is_empty() {
                    index += 1;
                    continue;
                }
                let parts: Vec<&str> = token.splitn(3, '\t').collect();
                if parts.len() < 2 {
                    index += 1;
                    continue;
                }
                let additions = parts[0].parse::<u64>().unwrap_or(0);
                let deletions = parts[1].parse::<u64>().unwrap_or(0);
                // A rename records counts, an empty path, then the old and new paths
                // as separate NUL-delimited fields; key it by the destination.
                let (path, extra) = if parts.len() >= 3 && !parts[2].is_empty() {
                    (parts[2].to_string(), 0)
                } else if index + 2 < tokens.len() {
                    (tokens[index + 2].to_string(), 2)
                } else {
                    break;
                };
                entries
                    .entry(path)
                    .and_modify(|entry| {
                        entry.1 = additions;
                        entry.2 = deletions;
                    })
                    .or_insert(("modified".to_string(), additions, deletions, None));
                index += 1 + extra;
            }
        }
    }

    if include_untracked {
        let output = git(
            repo_path,
            &["ls-files", "-z", "--others", "--exclude-standard"],
        )
        .ok_or_else(|| ProtocolError::new(ProtocolErrorCode::Internal, "git ls-files failed"))?;
        for path in output.split('\0').filter(|entry| !entry.is_empty()) {
            let (additions, deletions) = untracked_stats(repo_path, path);
            entries.entry(path.to_string()).or_insert((
                "untracked".to_string(),
                additions,
                deletions,
                None,
            ));
        }
    }

    Ok(entries
        .into_iter()
        .map(|(path, (status, additions, deletions, old_path))| {
            json!({
                "path": path,
                "status": status,
                "additions": additions,
                "deletions": deletions,
                "oldPath": old_path,
            })
        })
        .collect())
}

/// The object id of Git's empty tree for this repository's hash algorithm.
fn empty_tree_oid(repo_path: &Path) -> Option<String> {
    git(repo_path, &["hash-object", "-t", "tree", "--stdin"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Additions for an untracked text file, mirroring `synthetic_new_file_diff`:/// bounded, inside the checkout, no symlinks; binary or unreadable files count
/// as zero.
fn untracked_stats(repo_path: &Path, path: &str) -> (u64, u64) {
    let Some(absolute) = safe_canonical_path(repo_path, path) else {
        return (0, 0);
    };
    let Ok(metadata) = std::fs::symlink_metadata(&absolute) else {
        return (0, 0);
    };
    if !metadata.is_file() || metadata.len() > MAX_SYNTHETIC_FILE_BYTES {
        return (0, 0);
    }
    let Ok(contents) = std::fs::read_to_string(&absolute) else {
        return (0, 0);
    };
    if contents.contains('\0') {
        return (0, 0);
    }
    (contents.lines().count() as u64, 0)
}

fn findings_response(target: &TargetIdentity) -> Result<serde_json::Value, ProtocolError> {
    let findings = match target {
        TargetIdentity::Provider {
            pr_id: Some(_),
            run_id,
            ..
        } => {
            let store = load_provider_review_store(target).map_err(|error| {
                ProtocolError::new(
                    ProtocolErrorCode::Internal,
                    format!("review store: {error}"),
                )
            })?;
            let store = scoped_review_store(target, store.unwrap_or_default())?;
            select_review_run(Some(store), run_id.as_deref())
                .map(|run| run.findings.into_iter().map(finding_json).collect())
                .unwrap_or_default()
        }
        _ => Vec::new(),
    };
    Ok(json!({ "target": target, "findings": findings }))
}

fn finding_json(finding: crate::services::review::ReviewFinding) -> serde_json::Value {
    let severity = serde_json::to_value(finding.severity).unwrap_or_else(|_| json!("info"));
    let anchor = finding.anchor.as_ref().map(|anchor| {
        let side = serde_json::to_value(anchor.side).unwrap_or_else(|_| json!("new"));
        json!({
            "path": anchor.path,
            "startLine": anchor.start_line,
            "endLine": anchor.end_line,
            "side": side,
        })
    });
    json!({
        "id": finding.id,
        "title": finding.title,
        "severity": severity,
        "summary": finding.summary,
        "anchor": anchor,
    })
}

fn changed_files(repo_path: &Path) -> Vec<String> {
    let mut files = git_names(repo_path, &["diff", "--name-only", "-z", "HEAD"]);
    if files.is_empty() {
        files = git_names(repo_path, &["diff", "--name-only", "-z"]);
    }
    files.extend(git_names(
        repo_path,
        &["diff", "--cached", "--name-only", "-z"],
    ));
    files.extend(git_names(
        repo_path,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    ));
    files.sort();
    files.dedup();
    files
}

fn git_names(repo_path: &Path, args: &[&str]) -> Vec<String> {
    git(repo_path, args)
        .map(|output| {
            output
                .split('\0')
                .filter(|entry| !entry.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn git(repo_path: &Path, args: &[&str]) -> Option<String> {
    git_bytes(repo_path, args, (MAX_DIFF_BYTES + 4096) as u64)
        .map(|buffer| String::from_utf8_lossy(&buffer).to_string())
}

/// Run Git and return raw bytes (binary-safe), bounded by `limit`. Used for
/// image previews where the output is not UTF-8.
fn git_bytes(repo_path: &Path, args: &[&str], limit: u64) -> Option<Vec<u8>> {
    let mut child = spawn_git(repo_path, args)?;
    let pid = child.id();
    let stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut buffer = Vec::new();
        let mut stdout = Read::take(stdout, limit);
        let _ = Read::read_to_end(&mut stdout, &mut buffer);
        buffer
    });
    let deadline = Instant::now() + GIT_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break None,
        }
    };
    let buffer = reader.join().unwrap_or_default();
    forget_git_child(pid);
    status.filter(|status| status.success()).map(|_| buffer)
}

fn spawn_git(repo_path: &Path, args: &[&str]) -> Option<Child> {
    if shutting_down().load(Ordering::SeqCst) {
        return None;
    }
    let child = Command::new("git")
        .arg("-c")
        .arg("color.ui=false")
        .arg("-c")
        .arg("core.pager=cat")
        .arg("-c")
        // Keep a leading space on blank context lines so unified-diff parsers
        // can advance old/new line counters correctly.
        .arg("diff.suppressBlankEmpty=false")
        .arg("-c")
        // Keep non-ASCII paths literal so diff section headers match the path we
        // pass in, instead of Git's octal escapes.
        .arg("core.quotePath=false")
        .arg("-c")
        .arg("diff.outputIndicatorNew=+")
        .arg("-c")
        .arg("diff.outputIndicatorOld=-")
        .arg("-c")
        .arg("diff.outputIndicatorContext= ")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        // Treat every pathspec literally so a filename containing wildcard
        // characters cannot match additional files.
        .env("GIT_LITERAL_PATHSPECS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let Ok(mut children) = git_children().lock() {
        children.insert(child.id());
    }
    Some(child)
}

/// Process-wide store for inline review sessions started by the backend.
fn review_run_store() -> &'static AiReviewRunStore {
    static STORE: OnceLock<AiReviewRunStore> = OnceLock::new();
    STORE.get_or_init(AiReviewRunStore::default)
}

fn git_children() -> &'static Mutex<HashSet<u32>> {
    static CHILDREN: OnceLock<Mutex<HashSet<u32>>> = OnceLock::new();
    CHILDREN.get_or_init(|| Mutex::new(HashSet::new()))
}

fn forget_git_child(pid: u32) {
    if let Ok(mut children) = git_children().lock() {
        children.remove(&pid);
    }
}

/// Terminate Git children this backend started. Workers exit when their child is
/// reaped; unrelated processes are never touched.
fn kill_git_children() {
    let Ok(children) = git_children().lock() else {
        return;
    };
    for pid in children.iter() {
        #[cfg(unix)]
        unsafe {
            libc::kill(*pid as i32, libc::SIGTERM);
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
        }
    }
}

fn prune_terminal_operations(operations: &mut HashMap<String, Operation>) {
    if operations.len() <= 64 {
        return;
    }
    let terminal: Vec<String> = operations
        .iter()
        .filter(|(_, operation)| {
            matches!(
                operation.state,
                OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled
            )
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in terminal {
        operations.remove(&id);
    }
}

impl Backend {
    fn start_reply(
        &self,
        request_id: &str,
        operation_id: &str,
        target: TargetIdentity,
        message: String,
    ) {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut operations = self.operations.lock().expect("operations lock");
            prune_terminal_operations(&mut operations);
            operations.insert(
                operation_id.to_string(),
                Operation {
                    cancel: cancel.clone(),
                    state: OperationState::Accepted,
                    sequence: 0,
                    error: None,
                },
            );
        }
        self.respond(
            request_id,
            json!({
                "target": target,
                "operationId": operation_id,
                "state": "accepted",
            }),
        );
        let writer = self.events.clone();
        let operations = self.operations.clone();
        let counter = ActiveCounter(self.active_operations.clone());
        let operation_id = operation_id.to_string();
        thread::spawn(move || {
            run_reply_operation(
                &writer,
                &operations,
                &operation_id,
                &target,
                &cancel,
                message,
            );
            counter.decrement();
        });
    }

    fn preview_mutation(
        &self,
        id: String,
        target: TargetIdentity,
        action: ApprovedAction,
        message: Option<String>,
    ) {
        if self
            .active_previews
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                (current < MAX_CONCURRENT_PREVIEWS).then_some(current + 1)
            })
            .is_err()
        {
            self.fail(
                &id,
                ProtocolError::new(ProtocolErrorCode::Conflict, "too many previews"),
            );
            return;
        }
        let control = self.control.clone();
        let approvals = self.fix_approvals.clone();
        let counter = ActiveCounter(self.active_previews.clone());
        thread::spawn(move || {
            let result = mutation_preview(&target, action, message, &approvals);
            let _ = control.send(ServerMessage::Response(Response {
                id,
                ok: result.is_ok(),
                result: result.as_ref().ok().cloned(),
                error: result.err(),
            }));
            counter.decrement();
        });
    }

    fn start_fix(
        &self,
        request_id: &str,
        params: protocol::ReviewFixParams,
        action: ApprovedAction,
    ) {
        let operation_id = format!("op-{}", self.next_operation.fetch_add(1, Ordering::SeqCst));
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut operations = self.operations.lock().expect("operations lock");
            prune_terminal_operations(&mut operations);
            operations.insert(
                operation_id.clone(),
                Operation {
                    cancel: cancel.clone(),
                    state: OperationState::Accepted,
                    sequence: 0,
                    error: None,
                },
            );
        }
        self.respond(
            request_id,
            json!({"target": params.target, "operationId": operation_id, "state": "accepted"}),
        );
        let writer = self.events.clone();
        let operations = self.operations.clone();
        let approvals = self.fix_approvals.clone();
        let counter = ActiveCounter(self.active_operations.clone());
        thread::spawn(move || {
            run_fix_operation(
                &writer,
                &operations,
                &operation_id,
                &params,
                cancel,
                &approvals,
                action,
            );
            counter.decrement();
        });
    }

    fn start_local_review(
        &self,
        request_id: &str,
        operation_id: &str,
        params: crate::services::local_reviews::StartParams,
    ) {
        let mut nonce = [0; 32];
        if getrandom::getrandom(&mut nonce).is_err() {
            self.active_operations.fetch_sub(1, Ordering::SeqCst);
            self.fail(
                request_id,
                inline_start_error("Cannot register a local review capture.".into()),
            );
            return;
        }
        let capture_id = hex::encode(nonce);
        let ticket =
            match self
                .local_snapshots
                .reserve(crate::services::local_snapshot::CancelParams {
                    source: params.snapshot.source.clone(),
                    capture_id: capture_id.clone(),
                }) {
                Ok(ticket) => ticket,
                Err(message) => {
                    self.active_operations.fetch_sub(1, Ordering::SeqCst);
                    self.fail(
                        request_id,
                        ProtocolError::new(ProtocolErrorCode::Conflict, message),
                    );
                    return;
                }
            };
        let target = TargetIdentity::Local {
            local_snapshot_sha256: params.snapshot.snapshot_id.clone(),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        if let Ok(mut operations) = self.operations.lock() {
            prune_terminal_operations(&mut operations);
            operations.insert(
                operation_id.into(),
                Operation {
                    cancel: cancel.clone(),
                    state: OperationState::Accepted,
                    sequence: 0,
                    error: None,
                },
            );
        } else {
            self.active_operations.fetch_sub(1, Ordering::SeqCst);
            self.fail(
                request_id,
                inline_start_error("Cannot register a local review operation.".into()),
            );
            return;
        }
        self.respond(
            request_id,
            json!({"request":params,"target":target,"operationId":operation_id,"state":"accepted"}),
        );
        let writer = self.events.clone();
        let operations = self.operations.clone();
        let manager = self.local_snapshots.clone();
        let reads = self.active_local.clone();
        let shutdown = self.shutdown.clone();
        let counter = ActiveCounter(self.active_operations.clone());
        let operation_id = operation_id.to_string();
        thread::spawn(move || {
            emit_event(
                &writer,
                &operations,
                &operation_id,
                &target,
                1,
                OperationEvent::State {
                    state: OperationState::Running,
                    error: None,
                },
                OperationState::Running,
                None,
            );
            let result = start_captured_local_session(
                &params, &manager, &reads, &shutdown, &cancel, capture_id, ticket,
            );
            if cancel.load(Ordering::Acquire)
                || shutdown.load(Ordering::Acquire)
                || manager.stopped()
            {
                if let Ok(session) = result {
                    let _ = (session.cancel)();
                }
                finish_cancelled(&writer, &operations, &operation_id, &target, 2);
            } else {
                match result {
                    Ok(session) => monitor_inline_session(
                        &writer,
                        &operations,
                        &operation_id,
                        &target,
                        &cancel,
                        session,
                    ),
                    Err(error) => {
                        emit_failed(&writer, &operations, &operation_id, &target, 2, error)
                    }
                }
            }
            counter.decrement();
        });
    }

    fn start_review(
        &self,
        request_id: &str,
        operation_id: &str,
        params: protocol::ReviewStartParams,
    ) {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut operations = self.operations.lock().expect("operations lock");
            prune_terminal_operations(&mut operations);
            operations.insert(
                operation_id.to_string(),
                Operation {
                    cancel: cancel.clone(),
                    state: OperationState::Accepted,
                    sequence: 0,
                    error: None,
                },
            );
        }
        self.respond(
            request_id,
            json!({
                "target": params.target,
                "operationId": operation_id,
                "state": "accepted",
            }),
        );

        let writer = self.events.clone();
        let operations = self.operations.clone();
        let counter = ActiveCounter(self.active_operations.clone());
        let operation_id = operation_id.to_string();
        let delay = self.operation_delay();
        let runtime = ReviewRuntime {
            engine: review_engine(),
            snapshots: self.provider_snapshots.clone(),
            provider_reads: self.active_provider_reads.clone(),
            shutdown: self.shutdown.clone(),
        };
        thread::spawn(move || {
            run_review_operation(
                runtime,
                &writer,
                &operations,
                &operation_id,
                &params,
                &cancel,
                delay,
            );
            counter.decrement();
        });
    }
}

fn set_operation(
    operations: &Operations,
    operation_id: &str,
    state: OperationState,
    sequence: u64,
    error: Option<ProtocolError>,
) {
    if let Ok(mut operations) = operations.lock() {
        if let Some(operation) = operations.get_mut(operation_id) {
            operation.state = state;
            operation.sequence = sequence;
            operation.error = error;
        }
    }
}

/// Decide the terminal state under the operations lock so a concurrent cancel
/// cannot be lost. Returns true when the operation succeeded.
fn finalize_operation(operations: &Operations, operation_id: &str, cancel: &AtomicBool) -> bool {
    if let Ok(mut operations) = operations.lock() {
        if let Some(operation) = operations.get_mut(operation_id) {
            if cancel.load(Ordering::SeqCst) {
                operation.state = OperationState::Cancelled;
                operation.error = None;
                return false;
            }
            operation.state = OperationState::Succeeded;
            operation.error = None;
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn emit_event(
    writer: &SyncSender<ServerMessage>,
    operations: &Operations,
    operation_id: &str,
    target: &TargetIdentity,
    sequence: u64,
    event: OperationEvent,
    state: OperationState,
    error: Option<ProtocolError>,
) {
    let _ = writer.send(ServerMessage::Event(OperationEventFrame {
        operation_id: operation_id.to_string(),
        sequence,
        target: target.clone(),
        event,
    }));
    set_operation(operations, operation_id, state, sequence, error);
}

fn finish_cancelled(
    writer: &SyncSender<ServerMessage>,
    operations: &Operations,
    operation_id: &str,
    target: &TargetIdentity,
    sequence: u64,
) {
    emit_event(
        writer,
        operations,
        operation_id,
        target,
        sequence,
        OperationEvent::State {
            state: OperationState::Cancelled,
            error: None,
        },
        OperationState::Cancelled,
        None,
    );
}

fn run_review_operation(
    runtime: ReviewRuntime,
    writer: &SyncSender<ServerMessage>,
    operations: &Operations,
    operation_id: &str,
    params: &protocol::ReviewStartParams,
    cancel: &AtomicBool,
    delay: Duration,
) {
    let target = &params.target;
    match runtime.engine {
        ReviewEngine::Simulated => {
            run_simulated_review_operation(writer, operations, operation_id, target, cancel, delay);
        }
        ReviewEngine::Fake => {
            run_fake_review_operation(writer, operations, operation_id, target, cancel)
        }
        ReviewEngine::Real => {
            run_real_review_operation(writer, operations, operation_id, params, cancel, &runtime)
        }
    }
}

/// Deterministic review engine for tests: a running state, one progress event
/// and a terminal succeeded state, with no external processes.
fn run_fake_review_operation(
    writer: &SyncSender<ServerMessage>,
    operations: &Operations,
    operation_id: &str,
    target: &TargetIdentity,
    cancel: &AtomicBool,
) {
    set_operation(operations, operation_id, OperationState::Running, 0, None);
    emit_event(
        writer,
        operations,
        operation_id,
        target,
        1,
        OperationEvent::State {
            state: OperationState::Running,
            error: None,
        },
        OperationState::Running,
        None,
    );
    emit_event(
        writer,
        operations,
        operation_id,
        target,
        2,
        OperationEvent::Progress {
            message: "fake review analysed".to_string(),
            completed: Some(1),
            total: Some(1),
        },
        OperationState::Running,
        None,
    );
    if cancel.load(Ordering::SeqCst) {
        finish_cancelled(writer, operations, operation_id, target, 3);
        return;
    }
    emit_event(
        writer,
        operations,
        operation_id,
        target,
        3,
        OperationEvent::State {
            state: OperationState::Succeeded,
            error: None,
        },
        OperationState::Succeeded,
        None,
    );
}

/// Run the inline review session for the target, streaming its logs as progress
/// events and honouring cancellation by terminating the agent process.
fn run_real_review_operation(
    writer: &SyncSender<ServerMessage>,
    operations: &Operations,
    operation_id: &str,
    params: &protocol::ReviewStartParams,
    cancel: &AtomicBool,
    runtime: &ReviewRuntime,
) {
    let target = &params.target;
    set_operation(operations, operation_id, OperationState::Running, 0, None);
    emit_event(
        writer,
        operations,
        operation_id,
        target,
        1,
        OperationEvent::State {
            state: OperationState::Running,
            error: None,
        },
        OperationState::Running,
        None,
    );
    if cancel.load(Ordering::SeqCst) {
        finish_cancelled(writer, operations, operation_id, target, 2);
        return;
    }
    let started = if params.snapshot.is_some() {
        start_snapshot_session(params, runtime, cancel)
    } else {
        start_inline_session(target, params.profile.as_deref())
    };
    match started {
        Ok(session) => {
            monitor_inline_session(writer, operations, operation_id, target, cancel, session)
        }
        Err(error) => emit_failed(writer, operations, operation_id, target, 2, error),
    }
}

/// Stream a started inline session until it finishes, fails, is cancelled, or
/// times out. Session reads/cancellation are bound to the started thread id.
fn monitor_inline_session(
    writer: &SyncSender<ServerMessage>,
    operations: &Operations,
    operation_id: &str,
    target: &TargetIdentity,
    cancel: &AtomicBool,
    session: InlineSession,
) {
    let mut seen_logs = 0usize;
    let deadline = Instant::now() + Duration::from_secs(ai_review_timeout_secs());
    let mut sequence = 2u64;
    loop {
        if let Some(state) = (session.read)() {
            if state.thread_id != session.thread_id {
                emit_failed(
                    writer,
                    operations,
                    operation_id,
                    target,
                    sequence + 1,
                    ProtocolError::new(
                        ProtocolErrorCode::Internal,
                        "the review session was replaced by a newer run",
                    ),
                );
                return;
            }
            for log in state.logs.iter().skip(seen_logs) {
                emit_event(
                    writer,
                    operations,
                    operation_id,
                    target,
                    sequence,
                    OperationEvent::Progress {
                        message: log.clone(),
                        completed: None,
                        total: None,
                    },
                    OperationState::Running,
                    None,
                );
                sequence += 1;
            }
            seen_logs = state.logs.len();
            match state.status {
                AiReviewRunStatus::Succeeded => {
                    emit_event(
                        writer,
                        operations,
                        operation_id,
                        target,
                        sequence,
                        OperationEvent::State {
                            state: OperationState::Succeeded,
                            error: None,
                        },
                        OperationState::Succeeded,
                        None,
                    );
                    return;
                }
                AiReviewRunStatus::Failed => {
                    emit_failed(
                        writer,
                        operations,
                        operation_id,
                        target,
                        sequence,
                        ProtocolError::new(
                            ProtocolErrorCode::Internal,
                            state.error.unwrap_or_else(|| "review failed".to_string()),
                        ),
                    );
                    return;
                }
                AiReviewRunStatus::Cancelled => {
                    finish_cancelled(writer, operations, operation_id, target, sequence);
                    return;
                }
                AiReviewRunStatus::Idle | AiReviewRunStatus::Running => {}
            }
        }
        if cancel.load(Ordering::SeqCst) {
            let next = sequence + 1;
            match (session.cancel)() {
                Ok(()) => finish_cancelled(writer, operations, operation_id, target, next),
                Err(error) => emit_failed(
                    writer,
                    operations,
                    operation_id,
                    target,
                    next,
                    ProtocolError::new(ProtocolErrorCode::Internal, error),
                ),
            }
            return;
        }
        if Instant::now() >= deadline {
            let _ = (session.cancel)();
            emit_failed(
                writer,
                operations,
                operation_id,
                target,
                sequence + 1,
                ProtocolError::new(ProtocolErrorCode::Internal, "review timed out"),
            );
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn run_reply_operation(
    writer: &SyncSender<ServerMessage>,
    operations: &Operations,
    operation_id: &str,
    target: &TargetIdentity,
    cancel: &AtomicBool,
    message: String,
) {
    set_operation(operations, operation_id, OperationState::Running, 0, None);
    emit_event(
        writer,
        operations,
        operation_id,
        target,
        1,
        OperationEvent::State {
            state: OperationState::Running,
            error: None,
        },
        OperationState::Running,
        None,
    );
    if cancel.load(Ordering::SeqCst) {
        finish_cancelled(writer, operations, operation_id, target, 2);
        return;
    }
    match start_inline_reply_session(target, &message) {
        Ok(session) => {
            monitor_inline_session(writer, operations, operation_id, target, cancel, session)
        }
        Err(error) => emit_failed(writer, operations, operation_id, target, 2, error),
    }
}

fn start_inline_reply_session(
    target: &TargetIdentity,
    message: &str,
) -> Result<InlineSession, ProtocolError> {
    let store = review_run_store().clone();
    let TargetIdentity::Provider {
        pr_id: Some(_),
        run_id,
        ..
    } = target
    else {
        return Err(ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "replies are available for pull-request reviews",
        ));
    };
    let store_data = load_provider_review_store(target).map_err(inline_start_error)?;
    let run = select_review_run(store_data.clone(), run_id.as_deref());
    let thread_id = run
        .as_ref()
        .and_then(|run| run.thread_id.clone())
        .ok_or_else(|| {
            ProtocolError::new(
                ProtocolErrorCode::NotFound,
                "the selected review has no thread to reply to",
            )
        })?;
    // Resume the provider recorded on the thread (falling back to a Claude
    // resume session id, then the configured provider).
    let thread = store_data
        .as_ref()
        .and_then(|data| data.threads.iter().find(|thread| thread.id == thread_id))
        .ok_or_else(|| {
            ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "The selected review has no unambiguous saved conversation.",
            )
        })?;
    let run = run.expect("selected run with thread checked");
    let scope = provider_review_scope(target)?;
    let (repo_path, title, diff) = provider_run_input(target, &scope, &run)?;
    let source_branch = run.source_branch.clone();
    let destination_branch = run.destination_branch.clone();
    let prompt =
        provider_execution_prompt(target, repo_path.as_deref(), run.review_profile.as_deref())?;
    let base_payload = build_review_payload(
        &prompt,
        &title,
        "pr",
        "This target contains the provider pull-request diff.",
        &source_branch,
        &destination_branch,
        &diff,
    );
    let ai_provider = thread
        .ai_provider
        .or_else(|| {
            thread
                .claude_session_id
                .as_ref()
                .map(|_| config::AiProvider::Claude)
        })
        .unwrap_or_else(|| config::load().ai_provider);
    let read_scope = scope.clone();
    let cancel_scope = scope.clone();
    let selected_run_id = run.id.clone();
    let started = crate::services::review::reply_provider_inline_review_native(
        &store,
        crate::services::review::ProviderInlineReviewRequest {
            context: crate::services::review::ProviderInlineContext {
                scope,
                repo_path,
                provider_evidence_id: run.provider_evidence_id,
                reviewed_diff_base_sha: run.reviewed_diff_base_sha,
            },
            title,
            source_branch,
            destination_branch,
            reviewed_base_sha: run.reviewed_base_sha,
            reviewed_head_sha: run.reviewed_head_sha,
            payload: base_payload,
            ai_provider,
            review_profile: run.review_profile,
        },
        &selected_run_id,
        thread_id,
        message.to_string(),
    )
    .map_err(inline_start_error)?;
    let read_store = store.clone();
    Ok(InlineSession {
        thread_id: started.thread_id,
        read: Box::new(move || {
            crate::services::review::get_provider_ai_review_run_state_native(
                &read_store,
                &read_scope,
            )
        }),
        cancel: Box::new(move || {
            crate::services::review::cancel_provider_inline_review_native(&store, &cancel_scope)
                .map(|_| ())
        }),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum ApprovedAction {
    Sync,
    Conflicts,
    Fix,
    Commit,
    Push,
}

#[derive(Clone, Debug)]
struct FixApproval {
    action: ApprovedAction,
    message: Option<String>,
    files: Vec<String>,
    target: TargetIdentity,
    repo_path: PathBuf,
    snapshot: String,
    payload: String,
    source: String,
    destination: String,
    thread_id: String,
    expires: Instant,
}

fn fix_error(message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::TargetStale, message)
}

fn fix_context(target: &TargetIdentity) -> Result<FixApproval, ProtocolError> {
    let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        pr_id: Some(pr_id),
        run_id: Some(run_id),
    } = target
    else {
        return Err(fix_error(
            "Select a saved pull-request review before requesting a fix.",
        ));
    };
    let repo_path = resolve_repo_path(target)
        .and_then(|path| path.canonicalize().ok())
        .ok_or_else(|| fix_error("The configured local clone is unavailable."))?;
    let store = load_provider_review_store(target)
        .map_err(fix_error)?
        .ok_or_else(|| fix_error("The saved review is unavailable."))?;
    let run = store
        .review_runs
        .iter()
        .find(|run| run.id == *run_id)
        .ok_or_else(|| fix_error("The selected review no longer exists."))?;
    let expected_provider = match provider {
        ProviderKind::Github => crate::services::review::ReviewProvider::Github,
        ProviderKind::Bitbucket => crate::services::review::ReviewProvider::Bitbucket,
    };
    if run.provider != expected_provider
        || run.workspace != *workspace
        || run.repo != *repo
        || run.pr_id != *pr_id
    {
        return Err(fix_error("The saved review belongs to a different target."));
    }
    let thread = store
        .threads
        .iter()
        .find(|thread| Some(&thread.id) == run.thread_id.as_ref())
        .ok_or_else(|| fix_error("The selected review has no saved conversation."))?;
    if thread.messages.is_empty() {
        return Err(fix_error("The selected conversation is empty."));
    }
    for branch in [&run.source_branch, &run.destination_branch] {
        if branch.starts_with('-')
            || git(&repo_path, &["check-ref-format", "--branch", branch]).is_none()
        {
            return Err(fix_error("The saved review has an invalid branch."));
        }
    }
    let snapshot =
        crate::services::review_fix::checkout_fingerprint(&repo_path).map_err(fix_error)?;
    let (base, head) = reviewed_range(target)?
        .ok_or_else(|| fix_error("The reviewed revisions are unavailable."))?;
    let diff = git_bytes(
        &repo_path,
        &["diff", "--no-ext-diff", "--no-textconv", &base, &head],
        MAX_DIFF_BYTES as u64 + 1,
    )
    .filter(|bytes| bytes.len() <= MAX_DIFF_BYTES)
    .ok_or_else(|| fix_error("The reviewed diff is unavailable or too large."))?;
    let payload = crate::services::review_prompt::build_fix_payload(
        *pr_id,
        &run.source_branch,
        &run.destination_branch,
        thread,
        &String::from_utf8_lossy(&diff),
    );
    if payload.len() > MAX_FRAME_BYTES {
        return Err(fix_error("The conversation is too large for a fix."));
    }
    Ok(FixApproval {
        action: ApprovedAction::Fix,
        message: None,
        files: Vec::new(),
        target: target.clone(),
        repo_path,
        snapshot,
        payload,
        source: run.source_branch.clone(),
        destination: run.destination_branch.clone(),
        thread_id: thread.id.clone(),
        expires: Instant::now() + Duration::from_secs(300),
    })
}

fn mutation_preview(
    target: &TargetIdentity,
    action: ApprovedAction,
    message: Option<String>,
    approvals: &Mutex<HashMap<String, FixApproval>>,
) -> Result<serde_json::Value, ProtocolError> {
    let mut approval = fix_context(target)?;
    approval.action = action;
    approval.message = message;
    let current_branch = git(&approval.repo_path, &["branch", "--show-current"])
        .ok_or_else(|| fix_error("Cannot inspect the current branch."))?;
    let mut files = Vec::new();
    let mut commit_sha = None;
    let effects = match action {
        ApprovedAction::Sync | ApprovedAction::Conflicts => {
            crate::services::review::validate_branch_merge_checkout(&approval.repo_path, &approval.source, &approval.destination, action == ApprovedAction::Conflicts).map_err(fix_error)?;
            let mut effects = vec![format!("Fetch origin and fast-forward {} before merging origin/{} without committing.", approval.source, approval.destination),
                "Requires a clean source checkout for a new merge; no local changes are stashed or discarded.".to_string(),
                "Conflicts or interrupted changes remain in the clone for inspection.".to_string(),
                "No automatic commit, push or review publication. Approve these separately.".to_string()];
            if action == ApprovedAction::Conflicts {
                effects.push("Run Claude with file-edit permissions to resolve conflicts; an existing merge must match the saved destination.".to_string());
            }
            effects
        }
        ApprovedAction::Fix => vec!["Stash and restore existing local changes if needed.".to_string(),
            "Check out the review source branch.".to_string(), "Fetch from origin and fast-forward the source branch.".to_string(),
            "Run Claude with file-edit permissions using the saved conversation.".to_string(),
            "Leave edits for inspection; committing, pushing and publication require separate actions.".to_string()],
        ApprovedAction::Commit | ApprovedAction::Push => {
            let state = get_ai_review_fix_state_native(fix_store(), &provider_review_scope(target)?, Some(&approval.thread_id))
                .ok_or_else(|| fix_error("No fix session exists for the selected conversation."))?;
            if state.status != AiReviewFixStatus::Succeeded || current_branch.trim() != approval.source ||
                state.repo_path.as_deref().and_then(|path| Path::new(path).canonicalize().ok()).as_ref() != Some(&approval.repo_path) {
                return Err(fix_error("The fix session or current branch changed."));
            }
            if action == ApprovedAction::Commit {
                if state.phase != AiReviewFixPhase::ReadyToCommit { return Err(fix_error("Complete a fix before committing.")); }
                let message = approval.message.as_ref().ok_or_else(|| fix_error("A commit message is required."))?;
                if message.trim().is_empty() || message.chars().count() > 4096 { return Err(fix_error("A commit message between 1 and 4096 characters is required.")); }
                validate_fix_commit_paths(&approval.repo_path, &state.files_touched).map_err(fix_error)?;
                crate::services::review::validate_merge_commit_files(&approval.repo_path, &state.files_touched).map_err(fix_error)?;
                files = state.files_touched;
                approval.files = files.clone();
                vec![format!("Stage and commit only these files: {}", files.join(", ")),
                    format!("Commit message: {message}"), "Unrelated staged files remain staged and are excluded from this commit.".to_string(),
                    "Git commit hooks may run. This action does not push or publish review comments.".to_string()]
            } else {
                if state.phase != AiReviewFixPhase::ReadyToPush || state.commit_sha.is_none() ||
                    git(&approval.repo_path, &["rev-parse", "HEAD"]).map(|head| head.trim().to_string()) != state.commit_sha {
                    return Err(fix_error("Create a commit and keep its source branch and HEAD unchanged before pushing."));
                }
                commit_sha = state.commit_sha;
                vec![format!("Push commit {} from {} to origin/{}.", commit_sha.as_deref().unwrap_or(""), approval.source, approval.source),
                    "Use an explicit branch refspec without force push.".to_string(),
                    "Git pre-push hooks may run. No review comments are published.".to_string(),
                    "If interrupted, inspect the remote before trying again; a failed response does not prove nothing was pushed.".to_string()]
            }
        }
    };
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random)
        .map_err(|_| fix_error("Could not create an approval token."))?;
    let token = hex::encode(random);
    let dirty = git(&approval.repo_path, &["status", "--porcelain=v1"])
        .ok_or_else(|| fix_error("Could not inspect local changes."))?;
    let result = json!({"target":target,"action":action,"approvalToken":token,"expiresInSeconds":300,
        "repoPath":approval.repo_path,"currentBranch":current_branch.trim(),"sourceBranch":approval.source,
        "destinationBranch":approval.destination,"dirtyFiles":dirty.lines().take(50).map(|line| line.chars().take(512).collect::<String>()).collect::<Vec<_>>(),
        "dirtyFileCount":dirty.lines().count(),"dirtyFilesTruncated":dirty.lines().count() > 50 || dirty.lines().any(|line| line.chars().count() > 512),
        "effects":effects,"filesToCommit":files,"commitMessage":approval.message,"commitSha":commit_sha,"snapshotSha256":approval.snapshot});
    let mut approvals = approvals
        .lock()
        .map_err(|_| fix_error("Approval storage is unavailable."))?;
    approvals.retain(|_, value| value.expires > Instant::now());
    if approvals.len() >= 32 {
        return Err(fix_error(
            "Too many outstanding approvals; wait for them to expire.",
        ));
    }
    approvals.insert(token, approval);
    Ok(result)
}

fn consume_fix_approval(
    params: &protocol::ReviewFixParams,
    approvals: &Mutex<HashMap<String, FixApproval>>,
    action: ApprovedAction,
) -> Result<FixApproval, ProtocolError> {
    let approval = approvals
        .lock()
        .map_err(|_| fix_error("Approval storage is unavailable."))?
        .remove(&params.approval_token)
        .ok_or_else(|| fix_error("Approval is missing or already used. Request a new preview."))?;
    if approval.action != action {
        return Err(fix_error(
            "Approval belongs to a different action. Request a new preview.",
        ));
    }
    if approval.target != params.target || approval.expires <= Instant::now() {
        return Err(fix_error(
            "Approval expired or belongs to a different review. Request a new preview.",
        ));
    }
    let current = fix_context(&params.target)?;
    if current.repo_path != approval.repo_path
        || current.snapshot != approval.snapshot
        || current.payload != approval.payload
        || current.source != approval.source
        || current.destination != approval.destination
        || current.thread_id != approval.thread_id
    {
        return Err(fix_error(
            "The checkout or conversation changed after preview. Request a new preview.",
        ));
    }
    Ok(approval)
}

fn fix_state_response(target: &TargetIdentity) -> Result<serde_json::Value, ProtocolError> {
    let TargetIdentity::Provider {
        pr_id: Some(_),
        run_id: Some(run_id),
        ..
    } = target
    else {
        return Err(fix_error("Select a saved pull-request review."));
    };
    let data = load_provider_review_store(target).map_err(fix_error)?;
    let thread_id = data
        .as_ref()
        .and_then(|data| data.review_runs.iter().find(|run| run.id == *run_id))
        .and_then(|run| run.thread_id.as_deref());
    let scope = provider_review_scope(target)?;
    let state = thread_id
        .and_then(|thread_id| get_ai_review_fix_state_native(fix_store(), &scope, Some(thread_id)));
    Ok(json!({ "target": target, "state": state.map(|state| json!({
        "status": state.status, "phase": state.phase, "commitSha": state.commit_sha,
        "summary": state.summary.map(|s| s.chars().take(4096).collect::<String>()),
        "suggestedCommitMessage": state.suggested_commit_message.map(|s| s.chars().take(4096).collect::<String>()),
        "filesTouched": state.files_touched.iter().take(100).map(|s| s.chars().take(1024).collect::<String>()).collect::<Vec<_>>(),
        "tests": state.tests.iter().take(50).map(|s| s.chars().take(1024).collect::<String>()).collect::<Vec<_>>()
    })) }))
}

fn fix_store() -> &'static AiReviewFixStore {
    static STORE: OnceLock<AiReviewFixStore> = OnceLock::new();
    STORE.get_or_init(AiReviewFixStore::default)
}

fn run_fix_operation(
    writer: &SyncSender<ServerMessage>,
    operations: &Operations,
    operation_id: &str,
    params: &protocol::ReviewFixParams,
    cancel: Arc<AtomicBool>,
    approvals: &Mutex<HashMap<String, FixApproval>>,
    action: ApprovedAction,
) {
    let target = &params.target;
    let start = || -> Result<FixApproval, ProtocolError> {
        let approval = consume_fix_approval(params, approvals, action)?;
        if cancel.load(Ordering::Acquire) {
            return Err(ProtocolError::new(
                ProtocolErrorCode::Cancelled,
                "Fix cancelled before execution.",
            ));
        }
        Ok(approval)
    };
    let approval = match start() {
        Ok(approval) => approval,
        Err(error) => {
            emit_failed(writer, operations, operation_id, target, 1, error);
            return;
        }
    };
    let TargetIdentity::Provider {
        workspace,
        repo,
        pr_id: Some(pr_id),
        ..
    } = target
    else {
        return;
    };
    let scope = provider_review_scope(target).expect("provider target checked");
    let started = match approval.action {
        ApprovedAction::Sync | ApprovedAction::Conflicts => {
            crate::services::review::start_branch_merge_native(
                fix_store(),
                workspace,
                repo,
                *pr_id,
                &approval.thread_id,
                approval.repo_path,
                approval.source,
                approval.destination,
                approval.snapshot,
                cancel.clone(),
                approval.action == ApprovedAction::Conflicts,
                Some(&scope),
            )
        }
        ApprovedAction::Fix => start_ai_review_fix_native(
            fix_store(),
            workspace.clone(),
            repo.clone(),
            *pr_id,
            Some(approval.thread_id.clone()),
            approval.repo_path,
            approval.payload,
            approval.source,
            approval.destination,
            cancel.clone(),
            Some(approval.snapshot),
            Some(&scope),
        ),
        ApprovedAction::Commit => start_ai_review_commit_native(
            fix_store(),
            workspace,
            repo,
            *pr_id,
            &approval.thread_id,
            approval.message.unwrap_or_default(),
            approval.repo_path,
            approval.source,
            approval.snapshot,
            cancel.clone(),
            approval.files,
            Some(&scope),
        ),
        ApprovedAction::Push => start_ai_review_push_native(
            fix_store(),
            workspace,
            repo,
            *pr_id,
            &approval.thread_id,
            approval.repo_path,
            approval.source,
            approval.snapshot,
            cancel.clone(),
            Some(&scope),
        ),
    };
    if let Err(error) = started {
        emit_failed(
            writer,
            operations,
            operation_id,
            target,
            1,
            inline_start_error(error),
        );
        return;
    }
    let mut sequence = 1;
    let mut previous_logs = Vec::new();
    loop {
        if let Some(state) =
            get_ai_review_fix_state_native(fix_store(), &scope, Some(&approval.thread_id))
        {
            if state.logs != previous_logs {
                // A bounded ring can drop its prefix; emit only the new suffix.
                let overlap = (0..=previous_logs.len().min(state.logs.len()))
                    .rev()
                    .find(|count| {
                        previous_logs[previous_logs.len() - count..] == state.logs[..*count]
                    })
                    .unwrap_or(0);
                for line in state.logs.iter().skip(overlap) {
                    emit_event(
                        writer,
                        operations,
                        operation_id,
                        target,
                        sequence,
                        OperationEvent::Progress {
                            message: line.chars().take(4096).collect(),
                            completed: None,
                            total: None,
                        },
                        OperationState::Running,
                        None,
                    );
                    sequence += 1;
                }
                previous_logs = state.logs.clone();
            }
            if state.status == AiReviewFixStatus::Succeeded
                || state.status == AiReviewFixStatus::Failed
                || state.status == AiReviewFixStatus::Cancelled
            {
                if cancel.load(Ordering::Acquire) {
                    finish_cancelled(writer, operations, operation_id, target, sequence);
                } else if state.status == AiReviewFixStatus::Failed {
                    emit_failed(
                        writer,
                        operations,
                        operation_id,
                        target,
                        sequence,
                        ProtocolError::new(
                            ProtocolErrorCode::Internal,
                            state
                                .error
                                .unwrap_or_else(|| "Fix failed.".to_string())
                                .chars()
                                .take(4096)
                                .collect::<String>(),
                        ),
                    );
                } else {
                    emit_event(
                        writer,
                        operations,
                        operation_id,
                        target,
                        sequence,
                        OperationEvent::State {
                            state: OperationState::Succeeded,
                            error: None,
                        },
                        OperationState::Succeeded,
                        None,
                    );
                }
                return;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}

struct InlineSession {
    thread_id: Option<String>,
    read: Box<dyn Fn() -> Option<crate::services::review::AiReviewRunState> + Send>,
    cancel: Box<dyn Fn() -> Result<(), String> + Send>,
}

#[allow(clippy::too_many_arguments)]
// This permit covers fallible/cancelable local start work. Release on every
// return, including stale rechecks and EOF, rather than leaking the read budget.
struct LocalReadPermit(Arc<AtomicUsize>);
impl Drop for LocalReadPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn start_captured_local_session(
    params: &crate::services::local_reviews::StartParams,
    manager: &Arc<crate::services::local_snapshot::Manager>,
    reads: &Arc<AtomicUsize>,
    shutdown: &Arc<AtomicBool>,
    cancel: &Arc<AtomicBool>,
    capture_id: String,
    ticket: crate::services::local_snapshot::Ticket,
) -> Result<InlineSession, ProtocolError> {
    params.validate().map_err(inline_start_error)?;
    let canceled =
        || cancel.load(Ordering::Acquire) || shutdown.load(Ordering::Acquire) || manager.stopped();
    if canceled() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::Cancelled,
            "Local review canceled before execution.",
        ));
    }
    reads
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
            (n < 2).then_some(n + 1)
        })
        .map_err(|_| {
            ProtocolError::new(
                ProtocolErrorCode::Conflict,
                "Two local evidence requests are already running.",
            )
        })?;
    let read_permit = LocalReadPermit(reads.clone());
    let native_cancel = ticket.cancellation();
    let bridge_done = Arc::new(AtomicBool::new(false));
    let bridge = thread::spawn({
        let done = bridge_done.clone();
        let cancel = cancel.clone();
        let shutdown = shutdown.clone();
        move || {
            while !done.load(Ordering::Acquire) {
                if cancel.load(Ordering::Acquire) || shutdown.load(Ordering::Acquire) {
                    native_cancel.cancel();
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    });
    let checked = manager.recheck_reserved(
        crate::services::local_snapshot::RecheckParams {
            snapshot: params.snapshot.clone(),
            capture_id,
        },
        ticket,
    );
    bridge_done.store(true, Ordering::Release);
    let _ = bridge.join();
    if canceled() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::Cancelled,
            "Local review canceled before execution.",
        ));
    }
    checked.map_err(|message| ProtocolError::new(ProtocolErrorCode::TargetStale, message))?;
    let (data, existing) = crate::services::local_reviews::store(manager, &params.snapshot)
        .map_err(|message| ProtocolError::new(ProtocolErrorCode::TargetStale, message))?;
    if existing.review_runs.len() >= 1000 || existing.threads.len() >= 1000 {
        return Err(inline_start_error(
            "The saved local review has reached its run limit.".into(),
        ));
    }
    drop(read_permit);
    let info = &data.info;
    let repo = &info.snapshot.repository;
    let policy_target = TargetIdentity::Provider {
        provider: match repo.provider {
            config::ReviewProvider::Github => ProviderKind::Github,
            config::ReviewProvider::Bitbucket => ProviderKind::Bitbucket,
        },
        workspace: repo.workspace.clone(),
        repo: repo.repo.clone(),
        pr_id: None,
        run_id: None,
    };
    let (prompt, selected_profile) =
        resolve_inline_prompt_with_profile(&policy_target, data.root(), params.profile.as_deref())?;
    let mut layered = String::new();
    for (kind, patch) in data.layers() {
        let label = match kind {
            crate::local_review::LocalReviewDiffLayerKind::Committed => "Committed",
            crate::local_review::LocalReviewDiffLayerKind::Staged => "Staged",
            crate::local_review::LocalReviewDiffLayerKind::Unstaged => "Unstaged",
        };
        layered.push_str(&format!("\n## {label} layer\n"));
        layered.push_str(patch);
    }
    let payload = build_review_payload(
        &prompt,
        &format!("Local review · {}", info.branch),
        if info.comparison.mode == crate::services::local_snapshot::Mode::Branch { "branch" } else { "working-tree" },
        "Each labeled layer is independent captured evidence. The same path may have incompatible hunks in different layers; name its layer in findings. Untracked contents and unselected layers are excluded.",
        &info.branch,
        info.comparison.base_ref.as_deref().unwrap_or("working tree"),
        &layered,
    );
    if canceled() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::Cancelled,
            "Local review canceled before execution.",
        ));
    }
    let store = review_run_store().clone();
    let workspace = repo.workspace.clone();
    let repository = repo.repo.clone();
    let snapshot = info.snapshot.snapshot_id.clone();
    let started = crate::services::review::start_captured_local_inline_review_native(
        store.clone(),
        &data,
        payload,
        selected_profile,
        cancel.clone(),
    )
    .map_err(inline_start_error)?;
    let read_store = store.clone();
    let read_workspace = workspace.clone();
    let read_repo = repository.clone();
    let read_snapshot = snapshot.clone();
    Ok(InlineSession {
        thread_id: started.thread_id,
        read: Box::new(move || {
            get_local_ai_review_run_state_native(
                &read_store,
                &read_workspace,
                &read_repo,
                &read_snapshot,
            )
            .ok()
            .flatten()
        }),
        cancel: Box::new(move || {
            cancel_local_inline_review_native(&store, &workspace, &repository, &snapshot)
                .map(|_| ())
        }),
    })
}

fn ai_review_timeout_secs() -> u64 {
    std::env::var("NORN_AI_PROVIDER_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300)
        .clamp(30, 3600)
}

fn emit_failed(
    writer: &SyncSender<ServerMessage>,
    operations: &Operations,
    operation_id: &str,
    target: &TargetIdentity,
    sequence: u64,
    error: ProtocolError,
) {
    emit_event(
        writer,
        operations,
        operation_id,
        target,
        sequence,
        OperationEvent::State {
            state: OperationState::Failed,
            error: Some(error.clone()),
        },
        OperationState::Failed,
        Some(error),
    );
}

fn load_inline_review_policy(
    _target: &TargetIdentity,
    repo_path: &Path,
    profile: Option<&str>,
) -> Result<crate::repo_config::RepoReviewConfigLoadResult, ProtocolError> {
    crate::services::bitbucket::validate_repo_review_config_native(repo_path, profile).map_err(
        |_| {
            ProtocolError::new(
                ProtocolErrorCode::InvalidParams,
                "This repository's review policy is unavailable or invalid.",
            )
        },
    )
}

fn profile_remote_matches(
    remote: &str,
    provider: config::ReviewProvider,
    workspace: &str,
    repo: &str,
) -> bool {
    crate::local_repo::parse_git_remote(remote).is_ok_and(|(actual, owner, name)| {
        actual == provider
            && owner.eq_ignore_ascii_case(workspace)
            && name.eq_ignore_ascii_case(repo)
    })
}

fn review_profiles_response(target: &TargetIdentity) -> Result<serde_json::Value, ProtocolError> {
    use crate::services::review_profiles::{Catalog, Params, Source};
    Params {
        target: target.clone(),
    }
    .validate()?;
    if let TargetIdentity::Provider {
        run_id: Some(run_id),
        ..
    } = target
    {
        let store = load_provider_review_store(target).map_err(|_| {
            ProtocolError::new(
                ProtocolErrorCode::Conflict,
                "Cannot validate the saved review owner.",
            )
        })?;
        let owned =
            store.and_then(|store| store.review_runs.into_iter().find(|run| run.id == *run_id));
        if owned.is_none() {
            return Err(ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "The selected saved review is unavailable.",
            ));
        }
    }
    let path = if let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        ..
    } = target
    {
        let entries: Vec<_> = config::load()
            .repos
            .into_iter()
            .filter(|entry| {
                entry.provider == provider_matches(*provider)
                    && entry.workspace == *workspace
                    && entry.repo == *repo
            })
            .collect();
        if entries.len() > 1 {
            return Err(ProtocolError::new(
                ProtocolErrorCode::Conflict,
                "The selected repository has ambiguous checkout configuration.",
            ));
        }
        entries
            .first()
            .and_then(|entry| entry.local_path.as_deref())
            .filter(|path| !path.trim().is_empty())
            .map(|path| {
                PathBuf::from(path).canonicalize().map_err(|_| {
                    ProtocolError::new(
                        ProtocolErrorCode::TargetStale,
                        "The configured checkout is unavailable.",
                    )
                })
            })
            .transpose()?
    } else {
        resolve_repo_path(target)
    };
    let Some(path) = path else {
        if matches!(target, TargetIdentity::Local { .. })
            || crate::organization_policy::organization_policy_is_configured()
        {
            return Err(ProtocolError::new(
                ProtocolErrorCode::InvalidParams,
                "Review profiles require a configured local repository.",
            ));
        }
        return Ok(serde_json::json!(Catalog {
            target: target.clone(),
            profiles: vec![],
            default_profile: None,
            source: Source::Base,
            notice: Some(
                "No local checkout is configured. The native base profile is available.".into()
            )
        }));
    };
    let canonical = path.canonicalize().map_err(|_| {
        ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "The configured repository is unavailable.",
        )
    })?;
    let top = git(&canonical, &["rev-parse", "--show-toplevel"])
        .and_then(|value| PathBuf::from(value.trim()).canonicalize().ok());
    if top.as_ref() != Some(&canonical) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "The configured path is not the selected Git repository root.",
        ));
    }
    if let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        ..
    } = target
    {
        let remote = git(&canonical, &["remote", "get-url", "origin"]);
        if !remote.is_some_and(|remote| {
            profile_remote_matches(remote.trim(), provider_matches(*provider), workspace, repo)
        }) {
            return Err(ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "The configured checkout does not match the selected provider repository.",
            ));
        }
    }
    if let TargetIdentity::Local {
        local_snapshot_sha256,
    } = target
    {
        let owners: Vec<_> = config::load()
            .repos
            .into_iter()
            .filter(|entry| {
                entry
                    .local_path
                    .as_deref()
                    .and_then(|value| PathBuf::from(value).canonicalize().ok())
                    .as_ref()
                    == Some(&canonical)
            })
            .collect();
        if owners.len() != 1 {
            return Err(ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "Select one explicitly configured local repository before choosing profiles.",
            ));
        }
        let owner = &owners[0];
        if !git(&canonical, &["remote", "get-url", "origin"]).is_some_and(|remote| {
            profile_remote_matches(remote.trim(), owner.provider, &owner.workspace, &owner.repo)
        }) {
            return Err(ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "The configured checkout does not match the selected local repository.",
            ));
        }
        let snapshot = crate::local_review::local_review_snapshot_for_configured_path(
            owner.provider,
            &owner.workspace,
            &owner.repo,
            &canonical,
            crate::local_review::LocalReviewCancellation::new(),
        )
        .map_err(|_| {
            ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "The local review snapshot is unavailable.",
            )
        })?;
        if snapshot.snapshot_sha256 != *local_snapshot_sha256 {
            return Err(ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "The local review snapshot changed. Refresh it before choosing profiles.",
            ));
        }
    }
    serde_json::to_value(Catalog::from_policy(
        target.clone(),
        load_inline_review_policy(target, &canonical, None)?,
    )?)
    .map_err(|_| {
        ProtocolError::new(
            ProtocolErrorCode::Internal,
            "Cannot encode the review profile catalog.",
        )
    })
}

fn resolve_inline_prompt_with_profile(
    target: &TargetIdentity,
    repo_path: &std::path::Path,
    profile: Option<&str>,
) -> Result<(String, Option<String>), ProtocolError> {
    let loaded = load_inline_review_policy(target, repo_path, profile)?;
    if !loaded.errors.is_empty()
        && (matches!(target, TargetIdentity::Provider { .. }) || profile.is_some())
    {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidParams,
            "This repository's review policy is invalid; correct it before starting.",
        ));
    }
    if profile.is_some_and(|requested| loaded.selected_profile.as_deref() != Some(requested)) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidParams,
            "The selected review profile is unavailable.",
        ));
    }
    let canonical_repo = std::fs::canonicalize(repo_path).ok();
    let owner = match target {
        TargetIdentity::Provider {
            provider,
            workspace,
            repo,
            ..
        } => Some((provider_matches(*provider), workspace.clone(), repo.clone())),
        TargetIdentity::Local { .. } => config::load()
            .repos
            .into_iter()
            .find(|entry| {
                entry
                    .local_path
                    .as_deref()
                    .and_then(|path| std::fs::canonicalize(path).ok())
                    .is_some_and(|path| Some(path) == canonical_repo)
            })
            .map(|entry| (entry.provider, entry.workspace, entry.repo)),
    };
    if let Some((provider, workspace, repo)) = owner {
        let user = crate::services::workspace_data::prompt_for_scope(provider, &workspace, &repo)
            .map_err(|message| ProtocolError::new(ProtocolErrorCode::Conflict, message))?;
        if let Some(prompt) = user.filter(|prompt| !prompt.trim().is_empty()) {
            return prompt_with_references(target, prompt.trim().to_string())
                .map(|prompt| (prompt, loaded.selected_profile.clone()));
        }
    }
    if !loaded.errors.is_empty() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidParams,
            "This repository's review policy is invalid; correct it before starting.",
        ));
    }
    prompt_with_references(target, resolve_review_prompt(loaded.config.as_ref()))
        .map(|prompt| (prompt, loaded.selected_profile))
}

fn prompt_with_references(
    target: &TargetIdentity,
    prompt: String,
) -> Result<String, ProtocolError> {
    let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        pr_id: Some(pr_id),
        ..
    } = target
    else {
        return Ok(prompt);
    };
    let references = crate::services::workspace_data::references_for_scope(
        provider_matches(*provider),
        workspace,
        repo,
        *pr_id,
    )
    .map_err(|message| ProtocolError::new(ProtocolErrorCode::Conflict, message))?;
    crate::services::review_prompt::append_reference_context(prompt, &references)
        .map_err(|message| ProtocolError::new(ProtocolErrorCode::InvalidParams, message))
}

fn provider_execution_prompt(
    target: &TargetIdentity,
    repo_path: Option<&std::path::Path>,
    profile: Option<&str>,
) -> Result<String, ProtocolError> {
    if let Some(path) = repo_path {
        return resolve_inline_prompt_with_profile(target, path, profile).map(|value| value.0);
    }
    if profile.is_some() || crate::organization_policy::organization_policy_is_configured() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidParams,
            "This review profile or organization policy requires a configured local repository.",
        ));
    }
    let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        ..
    } = target
    else {
        return Err(fix_error("Select a provider review."));
    };
    let user = crate::services::workspace_data::prompt_for_scope(
        provider_matches(*provider),
        workspace,
        repo,
    )
    .map_err(|message| ProtocolError::new(ProtocolErrorCode::Conflict, message))?;
    prompt_with_references(
        target,
        user.filter(|prompt| !prompt.trim().is_empty())
            .unwrap_or_else(|| resolve_review_prompt(None)),
    )
}

fn provider_run_input(
    target: &TargetIdentity,
    scope: &crate::services::review_scope::Scope,
    run: &crate::services::review::ReviewRun,
) -> Result<(Option<PathBuf>, String, String), ProtocolError> {
    let repo_path = resolve_repo_path(target);
    if run.provider_evidence_id.is_some() {
        let capture = crate::services::provider_snapshot::durable::load_run(scope, run)
            .map_err(inline_start_error)?;
        return Ok((repo_path, capture.title, capture.patch));
    }
    let path = repo_path.as_deref().ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "This older saved review requires its configured local repository.",
        )
    })?;
    let base = run
        .reviewed_diff_base_sha
        .as_deref()
        .or(run.reviewed_base_sha.as_deref());
    let (Some(base), Some(head)) = (base, run.reviewed_head_sha.as_deref()) else {
        return Err(fix_error("No pinned reviewed range is available."));
    };
    if !revisions_available(path, base, head) {
        return Err(fix_error(
            "Reviewed revisions are not available in the configured clone.",
        ));
    }
    let diff = git(
        path,
        &["diff", "--no-ext-diff", "--no-textconv", "-M", base, head],
    )
    .ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::Internal,
            "Cannot generate the pinned review diff.",
        )
    })?;
    Ok((repo_path, format!("Pull request #{}", scope.pr_id), diff))
}

fn start_snapshot_session(
    params: &protocol::ReviewStartParams,
    runtime: &ReviewRuntime,
    cancel: &AtomicBool,
) -> Result<InlineSession, ProtocolError> {
    start_snapshot_session_with_recheck(params, runtime, cancel, |data| {
        crate::services::bitbucket::workspace_prs::Api::from_stored(
            data.info.request.repository.clone(),
        )?
        .recheck_snapshot(data)
    })
}

fn start_snapshot_session_with_recheck(
    params: &protocol::ReviewStartParams,
    runtime: &ReviewRuntime,
    cancel: &AtomicBool,
    recheck: impl FnOnce(&crate::services::provider_snapshot::SnapshotData) -> Result<(), String>,
) -> Result<InlineSession, ProtocolError> {
    let target = &params.target;
    let snapshot = params
        .snapshot
        .as_ref()
        .ok_or_else(|| fix_error("Select a provider preview."))?;
    let scope = provider_review_scope(target)?;
    if !matches!(target, TargetIdentity::Provider { run_id: None, .. })
        || snapshot.repository.provider
            != provider_matches(match scope.provider {
                crate::services::review::ReviewProvider::Github => ProviderKind::Github,
                crate::services::review::ReviewProvider::Bitbucket => ProviderKind::Bitbucket,
            })
        || snapshot.repository.workspace != scope.workspace
        || snapshot.repository.repo != scope.repo
        || snapshot.pr_id != scope.pr_id
    {
        return Err(fix_error(
            "The provider preview belongs to a different target.",
        ));
    }
    // Cache ownership and expiry are proved before even constructing a credentialed API.
    let data = runtime
        .snapshots
        .get(snapshot)
        .map_err(|message| ProtocolError::new(ProtocolErrorCode::TargetStale, message))?;
    let cancelled = || cancel.load(Ordering::Acquire) || runtime.shutdown.load(Ordering::Acquire);
    if cancelled() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::Cancelled,
            "Review canceled before execution.",
        ));
    }
    runtime
        .provider_reads
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
            (n < 2).then_some(n + 1)
        })
        .map_err(|_| {
            ProtocolError::new(
                ProtocolErrorCode::Conflict,
                "Two provider reads are already in progress; wait before starting this review.",
            )
        })?;
    let checked = recheck(&data);
    runtime.provider_reads.fetch_sub(1, Ordering::SeqCst);
    checked.map_err(|message| ProtocolError::new(ProtocolErrorCode::TargetStale, message))?;
    if cancelled() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::Cancelled,
            "Review canceled before execution.",
        ));
    }
    let repo_path = resolve_repo_path(target);
    let prompt =
        provider_execution_prompt(target, repo_path.as_deref(), params.profile.as_deref())?;
    let payload = build_review_payload(
        &prompt,
        &data.info.title,
        "pr",
        "This target contains the exact captured provider pull-request diff.",
        &data.info.source_branch,
        &data.info.destination_branch,
        &data.patch,
    );
    let evidence_id = crate::services::provider_snapshot::durable::save(&scope, &data)
        .map_err(inline_start_error)?;
    if cancelled() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::Cancelled,
            "Review canceled before execution.",
        ));
    }
    let store = review_run_store().clone();
    let read_scope = scope.clone();
    let cancel_scope = scope.clone();
    let started = crate::services::review::start_provider_inline_review_native(
        store.clone(),
        crate::services::review::ProviderInlineReviewRequest {
            context: crate::services::review::ProviderInlineContext {
                scope,
                repo_path,
                provider_evidence_id: Some(evidence_id),
                reviewed_diff_base_sha: Some(data.info.base_sha.clone()),
            },
            title: data.info.title.clone(),
            source_branch: data.info.source_branch.clone(),
            destination_branch: data.info.destination_branch.clone(),
            reviewed_base_sha: Some(data.info.destination_sha.clone()),
            reviewed_head_sha: Some(data.info.head_sha.clone()),
            payload,
            ai_provider: config::load().ai_provider,
            review_profile: params.profile.clone(),
        },
    )
    .map_err(inline_start_error)?;
    let read_store = store.clone();
    Ok(InlineSession {
        thread_id: started.thread_id,
        read: Box::new(move || {
            crate::services::review::get_provider_ai_review_run_state_native(
                &read_store,
                &read_scope,
            )
        }),
        cancel: Box::new(move || {
            crate::services::review::cancel_provider_inline_review_native(&store, &cancel_scope)
                .map(|_| ())
        }),
    })
}

fn start_inline_session(
    target: &TargetIdentity,
    profile: Option<&str>,
) -> Result<InlineSession, ProtocolError> {
    let store = review_run_store().clone();
    if matches!(target, TargetIdentity::Provider { pr_id: Some(_), .. }) {
        let scope = provider_review_scope(target)?;
        let run_id = match target {
            TargetIdentity::Provider { run_id, .. } => run_id.as_deref(),
            _ => None,
        };
        if let Some(saved) = select_review_run(
            load_provider_review_store(target).map_err(inline_start_error)?,
            run_id,
        ) {
            if saved.provider_evidence_id.is_some() {
                let (repo_path, title, diff) = provider_run_input(target, &scope, &saved)?;
                let prompt = provider_execution_prompt(target, repo_path.as_deref(), profile)?;
                let payload = build_review_payload(
                    &prompt,
                    &title,
                    "pr",
                    "This target contains the exact captured provider pull-request diff.",
                    &saved.source_branch,
                    &saved.destination_branch,
                    &diff,
                );
                let read_scope = scope.clone();
                let cancel_scope = scope.clone();
                let started = crate::services::review::start_provider_inline_review_native(
                    store.clone(),
                    crate::services::review::ProviderInlineReviewRequest {
                        context: crate::services::review::ProviderInlineContext {
                            scope,
                            repo_path,
                            provider_evidence_id: saved.provider_evidence_id,
                            reviewed_diff_base_sha: saved.reviewed_diff_base_sha,
                        },
                        title,
                        source_branch: saved.source_branch,
                        destination_branch: saved.destination_branch,
                        reviewed_base_sha: saved.reviewed_base_sha,
                        reviewed_head_sha: saved.reviewed_head_sha,
                        payload,
                        ai_provider: config::load().ai_provider,
                        review_profile: profile.map(ToOwned::to_owned),
                    },
                )
                .map_err(inline_start_error)?;
                let read_store = store.clone();
                return Ok(InlineSession {
                    thread_id: started.thread_id,
                    read: Box::new(move || {
                        crate::services::review::get_provider_ai_review_run_state_native(
                            &read_store,
                            &read_scope,
                        )
                    }),
                    cancel: Box::new(move || {
                        crate::services::review::cancel_provider_inline_review_native(
                            &store,
                            &cancel_scope,
                        )
                        .map(|_| ())
                    }),
                });
            }
        }
    }
    let repo_path = resolve_repo_path(target).ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "no reviewable local repository for this target",
        )
    })?;
    if !is_git_repo(&repo_path) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "no reviewable local repository for this target",
        ));
    }
    let (prompt, selected_profile) =
        resolve_inline_prompt_with_profile(target, &repo_path, profile)?;
    let ai_provider = config::load().ai_provider;

    match target {
        TargetIdentity::Provider {
            provider,
            workspace,
            repo,
            pr_id: Some(pr_id),
            run_id,
            ..
        } => {
            let scope = crate::services::review_scope::Scope {
                provider: match provider {
                    ProviderKind::Github => crate::services::review::ReviewProvider::Github,
                    ProviderKind::Bitbucket => crate::services::review::ReviewProvider::Bitbucket,
                },
                workspace: workspace.clone(),
                repo: repo.clone(),
                pr_id: *pr_id,
            };
            let data = load_provider_review_store(target).map_err(inline_start_error)?;
            let saved = select_review_run(data, run_id.as_deref()).ok_or_else(|| {
                ProtocolError::new(
                    ProtocolErrorCode::TargetStale,
                    "The selected saved review is unavailable.",
                )
            })?;
            let base = saved
                .reviewed_diff_base_sha
                .as_ref()
                .or(saved.reviewed_base_sha.as_ref());
            let (Some(base), Some(head)) = (base, saved.reviewed_head_sha.as_ref()) else {
                return Err(ProtocolError::new(
                    ProtocolErrorCode::TargetStale,
                    "No pinned reviewed range is available.",
                ));
            };
            if !revisions_available(&repo_path, base, head) {
                return Err(ProtocolError::new(
                    ProtocolErrorCode::TargetStale,
                    "Reviewed revisions are not available in the configured clone.",
                ));
            }
            let diff = git(
                &repo_path,
                &["diff", "--no-ext-diff", "--no-textconv", "-M", base, head],
            )
            .ok_or_else(|| {
                ProtocolError::new(
                    ProtocolErrorCode::Internal,
                    "Cannot generate the pinned review diff.",
                )
            })?;
            let payload = build_review_payload(
                &prompt,
                &format!("Pull request #{pr_id}"),
                "pr",
                "This target contains the provider pull-request diff.",
                &saved.source_branch,
                &saved.destination_branch,
                &diff,
            );
            let read_scope = scope.clone();
            let cancel_scope = scope.clone();
            let started = crate::services::review::start_provider_inline_review_native(
                store.clone(),
                crate::services::review::ProviderInlineReviewRequest {
                    context: crate::services::review::ProviderInlineContext {
                        scope,
                        repo_path: Some(repo_path.clone()),
                        provider_evidence_id: saved.provider_evidence_id,
                        reviewed_diff_base_sha: saved.reviewed_diff_base_sha,
                    },
                    title: format!("Pull request #{pr_id}"),
                    source_branch: saved.source_branch,
                    destination_branch: saved.destination_branch,
                    reviewed_base_sha: saved.reviewed_base_sha,
                    reviewed_head_sha: saved.reviewed_head_sha,
                    payload,
                    ai_provider,
                    review_profile: profile.map(ToOwned::to_owned),
                },
            )
            .map_err(inline_start_error)?;
            let read_store = store.clone();
            Ok(InlineSession {
                thread_id: started.thread_id,
                read: Box::new(move || {
                    crate::services::review::get_provider_ai_review_run_state_native(
                        &read_store,
                        &read_scope,
                    )
                }),
                cancel: Box::new(move || {
                    crate::services::review::cancel_provider_inline_review_native(
                        &store,
                        &cancel_scope,
                    )
                    .map(|_| ())
                }),
            })
        }
        TargetIdentity::Local {
            local_snapshot_sha256,
        } => {
            let files = changed_file_entries(&repo_path, None)?;
            let diff = working_tree_patch(&repo_path, &files)?;
            let source_branch = git(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])
                .map(|value| value.trim().to_string())
                .unwrap_or_else(|| "HEAD".to_string());
            let payload = build_review_payload(
                &prompt,
                "Local changes",
                "working-tree",
                "This target can contain staged, unstaged, and untracked files. Do not describe a file as committed unless the supplied evidence proves that it is committed.",
                &source_branch,
                "HEAD",
                &diff,
            );
            let (workspace, repo) = local_review_identity(&repo_path);
            let snapshot = local_snapshot_sha256.clone();
            let started = start_local_inline_review_native(
                store.clone(),
                workspace.clone(),
                repo.clone(),
                0,
                snapshot.clone(),
                "Local changes".to_string(),
                source_branch,
                "HEAD".to_string(),
                None,
                payload,
                None,
                Some("Review".to_string()),
                true,
                ai_provider,
                None,
                None,
                None,
                None,
                None,
                None,
                selected_profile,
            )
            .map_err(inline_start_error)?;
            let thread_id = started.thread_id.clone();
            let read_store = store.clone();
            let read_workspace = workspace.clone();
            let read_repo = repo.clone();
            let read_snapshot = snapshot.clone();
            Ok(InlineSession {
                thread_id,
                read: Box::new(move || {
                    get_local_ai_review_run_state_native(
                        &read_store,
                        &read_workspace,
                        &read_repo,
                        &read_snapshot,
                    )
                    .ok()
                    .flatten()
                }),
                cancel: Box::new(move || {
                    cancel_local_inline_review_native(&store, &workspace, &repo, &snapshot)
                        .map(|_| ())
                }),
            })
        }
        TargetIdentity::Provider { pr_id: None, .. } => Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "select a pull request to start an inline review",
        )),
    }
}

fn inline_start_error(error: String) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::Internal, error)
}

/// Best-effort workspace/repo identity for a local checkout: a configured
/// repository whose local path matches, otherwise a placeholder.
fn local_review_identity(repo_path: &Path) -> (String, String) {
    let canonical = std::fs::canonicalize(repo_path).unwrap_or_else(|_| repo_path.to_path_buf());
    for repo in config::load().repos {
        if let Some(local_path) = repo.local_path {
            let candidate =
                std::fs::canonicalize(&local_path).unwrap_or_else(|_| PathBuf::from(&local_path));
            if candidate == canonical {
                return (repo.workspace, repo.repo);
            }
        }
    }
    ("local".to_string(), "local".to_string())
}

fn run_simulated_review_operation(
    writer: &SyncSender<ServerMessage>,
    operations: &Operations,
    operation_id: &str,
    target: &TargetIdentity,
    cancel: &AtomicBool,
    delay: Duration,
) {
    set_operation(operations, operation_id, OperationState::Running, 0, None);
    emit_event(
        writer,
        operations,
        operation_id,
        target,
        1,
        OperationEvent::State {
            state: OperationState::Running,
            error: None,
        },
        OperationState::Running,
        None,
    );

    let repo_path = resolve_repo_path(target);
    if repo_path.as_deref().is_none_or(|path| !is_git_repo(path)) {
        let error = ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "no reviewable local repository for this target",
        );
        emit_event(
            writer,
            operations,
            operation_id,
            target,
            2,
            OperationEvent::State {
                state: OperationState::Failed,
                error: Some(error.clone()),
            },
            OperationState::Failed,
            Some(error),
        );
        return;
    }

    let files = repo_path.as_deref().map(changed_files).unwrap_or_default();
    let mut sequence = 1;
    let mut processed = 0_u64;

    for path in &files {
        if cancel.load(Ordering::SeqCst) {
            finish_cancelled(writer, operations, operation_id, target, sequence + 1);
            return;
        }
        thread::sleep(delay);
        processed += 1;
        sequence += 1;
        emit_event(
            writer,
            operations,
            operation_id,
            target,
            sequence,
            OperationEvent::Progress {
                message: format!("collected {path}"),
                completed: Some(processed),
                total: Some(files.len() as u64),
            },
            OperationState::Running,
            None,
        );
    }

    // A short cancellable window even for an empty change set, then an atomic
    // final decision so a concurrent cancel is never lost to a fast success.
    if files.is_empty() {
        thread::sleep(delay);
    }
    sequence += 1;
    if finalize_operation(operations, operation_id, cancel) {
        emit_event(
            writer,
            operations,
            operation_id,
            target,
            sequence,
            OperationEvent::State {
                state: OperationState::Succeeded,
                error: None,
            },
            OperationState::Succeeded,
            None,
        );
    } else {
        finish_cancelled(writer, operations, operation_id, target, sequence);
    }
}

fn operation_snapshot(backend: &Backend, operation_id: Option<&str>) -> Option<serde_json::Value> {
    let operation_id = operation_id?;
    let operations = backend.operations.lock().ok()?;
    let operation = operations.get(operation_id)?;
    Some(json!({
        "operationId": operation_id,
        "state": operation.state,
        "sequence": operation.sequence,
        "error": operation.error,
    }))
}

fn cancel_operation(backend: &Backend, operation_id: Option<&str>) -> Option<serde_json::Value> {
    let operation_id = operation_id?;
    let mut operations = backend.operations.lock().ok()?;
    let operation = operations.get_mut(operation_id)?;
    operation.cancel.store(true, Ordering::SeqCst);
    Some(json!({
        "operationId": operation_id,
        "state": operation.state,
        "sequence": operation.sequence,
        "error": operation.error,
    }))
}

fn writer_loop(
    control: Receiver<ServerMessage>,
    events: Receiver<ServerMessage>,
    shutdown: Arc<AtomicBool>,
) {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    loop {
        // Control responses have priority and never block their senders.
        let mut drained = false;
        while let Ok(message) = control.try_recv() {
            write_frame(&mut stdout, message);
            drained = true;
        }
        match events.recv_timeout(Duration::from_millis(20)) {
            Ok(message) => write_frame(&mut stdout, message),
            Err(RecvTimeoutError::Timeout) => {
                if shutdown.load(Ordering::SeqCst) && !drained {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    while let Ok(message) = control.try_recv() {
        write_frame(&mut stdout, message);
    }
    while let Ok(message) = events.try_recv() {
        write_frame(&mut stdout, message);
    }
}

fn write_frame(stdout: &mut impl Write, message: ServerMessage) {
    match protocol::encode_line(&message) {
        Ok(frame) => {
            if stdout.write_all(frame.as_bytes()).is_ok() {
                let _ = stdout.flush();
            }
        }
        Err(error) => {
            eprintln!("norn-backend: failed to encode a frame: {}", error.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_catalog_remote_owner_is_exact_not_a_substring() {
        for (provider, host) in [
            (config::ReviewProvider::Github, "github.com"),
            (config::ReviewProvider::Bitbucket, "bitbucket.org"),
        ] {
            assert!(profile_remote_matches(
                &format!("https://{host}/owned/same.git"),
                provider,
                "owned",
                "same"
            ));
            assert!(profile_remote_matches(
                &format!("git@{host}:Owned/Same.git"),
                provider,
                "owned",
                "same"
            ));
            for remote in [
                format!("https://{host}/owned/same-other.git"),
                format!("https://{host}.example.invalid/owned/same.git"),
                format!("https://example.invalid/{host}/owned/same.git"),
                format!("https://{host}/other/owned/same.git"),
                format!("https://{host}/owned/same/nested.git"),
            ] {
                assert!(!profile_remote_matches(&remote, provider, "owned", "same"));
            }
        }
    }

    #[test]
    fn native_profile_catalog_proves_local_snapshot_and_exact_provider_checkout() {
        struct Restore(Vec<(&'static str, Option<std::ffi::OsString>)>);
        impl Drop for Restore {
            fn drop(&mut self) {
                for (key, value) in &self.0 {
                    if let Some(value) = value {
                        std::env::set_var(key, value);
                    } else {
                        std::env::remove_var(key);
                    }
                }
            }
        }
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        let mut restore = Restore(Vec::new());
        for (key, value) in [
            ("NORN_CONFIG_DIR", config_dir.path()),
            ("NORN_BACKEND_REPO_PATH", root.path()),
        ] {
            restore.0.push((key, std::env::var_os(key)));
            std::env::set_var(key, value);
        }
        for args in [
            vec!["init"],
            vec!["config", "user.name", "Owned"],
            vec!["config", "user.email", "owned@example.invalid"],
            vec![
                "remote",
                "add",
                "origin",
                "https://github.com/owned/same.git",
            ],
        ] {
            assert!(Command::new("git")
                .arg("-C")
                .arg(root.path())
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        }
        std::fs::write(root.path().join("tracked.txt"), "before\n").unwrap();
        for args in [
            vec!["add", "tracked.txt"],
            vec!["-c", "commit.gpgsign=false", "commit", "-m", "owned"],
        ] {
            assert!(Command::new("git")
                .arg("-C")
                .arg(root.path())
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        }
        std::fs::write(
            root.path().join(".norn.yaml"),
            "version: 0.1\nprofiles:\n  default: {}\n  security: {}\n",
        )
        .unwrap();
        let settings = serde_json::json!({"repos":[{"provider":"github","workspace":"owned","repo":"same","localPath":root.path()}],"defaultDiffView":"unified","theme":"dark"});
        std::fs::write(
            config_dir.path().join("settings.json"),
            settings.to_string(),
        )
        .unwrap();
        let snapshot = crate::local_review::local_review_snapshot_for_configured_path(
            config::ReviewProvider::Github,
            "owned",
            "same",
            root.path(),
            crate::local_review::LocalReviewCancellation::new(),
        )
        .unwrap();
        let target = TargetIdentity::Local {
            local_snapshot_sha256: snapshot.snapshot_sha256,
        };
        let catalog = review_profiles_response(&target).unwrap();
        assert_eq!(
            catalog["profiles"],
            serde_json::json!(["default", "security"])
        );
        assert_eq!(catalog["defaultProfile"], "default");
        std::fs::write(root.path().join("tracked.txt"), "after\n").unwrap();
        assert_eq!(
            review_profiles_response(&target).unwrap_err().code,
            ProtocolErrorCode::TargetStale
        );
        let github = TargetIdentity::Provider {
            provider: ProviderKind::Github,
            workspace: "owned".into(),
            repo: "same".into(),
            pr_id: Some(7),
            run_id: None,
        };
        assert_eq!(
            review_profiles_response(&github).unwrap()["source"],
            "policy"
        );
        let bitbucket = TargetIdentity::Provider {
            provider: ProviderKind::Bitbucket,
            workspace: "owned".into(),
            repo: "same".into(),
            pr_id: Some(7),
            run_id: None,
        };
        assert_eq!(
            review_profiles_response(&bitbucket).unwrap()["source"],
            "base"
        );
        let mut broken = settings.clone();
        broken["repos"][0]["localPath"] = serde_json::json!(root.path().join("missing"));
        std::fs::write(config_dir.path().join("settings.json"), broken.to_string()).unwrap();
        assert_eq!(
            review_profiles_response(&github).unwrap_err().code,
            ProtocolErrorCode::TargetStale
        );
        let mut ambiguous = settings.clone();
        ambiguous["repos"]
            .as_array_mut()
            .unwrap()
            .push(settings["repos"][0].clone());
        std::fs::write(
            config_dir.path().join("settings.json"),
            ambiguous.to_string(),
        )
        .unwrap();
        assert_eq!(
            review_profiles_response(&github).unwrap_err().code,
            ProtocolErrorCode::Conflict
        );
    }

    #[cfg(unix)]
    #[test]
    fn exact_preview_starts_real_native_pipeline_without_clone_and_persists_evidence() {
        use crate::services::provider_snapshot::{tests::capture, Manager};
        use std::os::unix::fs::PermissionsExt;
        struct RestoreEnvironment(Vec<(&'static str, Option<std::ffi::OsString>)>);
        impl Drop for RestoreEnvironment {
            fn drop(&mut self) {
                for (key, previous) in &self.0 {
                    if let Some(value) = previous {
                        std::env::set_var(key, value);
                    } else {
                        std::env::remove_var(key);
                    }
                }
            }
        }
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = tempfile::tempdir().unwrap();
        let agent = owned.path().join("owned-agent");
        let input = owned.path().join("agent-input");
        std::fs::write(&agent, format!("#!/bin/sh\ncat > '{}'\nprintf '%s\\n' '{{\"result\":\"Exact initial capture complete.\"}}'\n", input.display())).unwrap();
        std::fs::set_permissions(&agent, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(
            owned.path().join("settings.json"),
            r#"{"repos":[],"aiProvider":"claude","defaultDiffView":"unified","theme":"dark"}"#,
        )
        .unwrap();
        let mut restore = RestoreEnvironment(Vec::new());
        for (key, path) in [
            ("NORN_CONFIG_DIR", owned.path()),
            ("NORN_REVIEW_DATA_DIR", owned.path()),
            ("NORN_REVIEW_AGENT_BIN", agent.as_path()),
        ] {
            restore.0.push((key, std::env::var_os(key)));
            std::env::set_var(key, path);
        }
        for provider in [
            config::ReviewProvider::Github,
            config::ReviewProvider::Bitbucket,
        ] {
            let snapshots = Arc::new(Manager::default());
            let captured = capture(provider);
            let patch = captured.patch.clone();
            let info = snapshots.insert(captured).unwrap();
            let runtime = ReviewRuntime {
                engine: ReviewEngine::Real,
                snapshots,
                provider_reads: Arc::new(AtomicUsize::new(0)),
                shutdown: Arc::new(AtomicBool::new(false)),
            };
            let target = TargetIdentity::Provider {
                provider: match provider {
                    config::ReviewProvider::Github => ProviderKind::Github,
                    config::ReviewProvider::Bitbucket => ProviderKind::Bitbucket,
                },
                workspace: info.request.repository.workspace.clone(),
                repo: info.request.repository.repo.clone(),
                pr_id: Some(info.request.pr_id),
                run_id: None,
            };
            let params = protocol::ReviewStartParams {
                target: target.clone(),
                profile: None,
                snapshot: Some(info.snapshot.clone()),
            };
            let session = start_snapshot_session_with_recheck(
                &params,
                &runtime,
                &AtomicBool::new(false),
                |_| Ok(()),
            )
            .unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let state = (session.read)().unwrap();
                if state.status != crate::services::review::AiReviewRunStatus::Running {
                    assert_eq!(
                        state.status,
                        crate::services::review::AiReviewRunStatus::Succeeded,
                        "{state:?}"
                    );
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "owned fake agent did not terminate"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(std::fs::read_to_string(&input).unwrap().contains(&patch));
            let scope = provider_review_scope(&target).unwrap();
            let saved = crate::services::review_scope::load_owned(&scope)
                .unwrap()
                .unwrap();
            let run = saved.review_runs.last().unwrap();
            assert_ne!(run.id, info.snapshot.snapshot_id);
            assert_eq!(
                run.reviewed_base_sha.as_deref(),
                Some(info.destination_sha.as_str())
            );
            assert_eq!(
                run.reviewed_diff_base_sha.as_deref(),
                Some(info.base_sha.as_str())
            );
            assert_eq!(
                crate::services::provider_snapshot::durable::load_run(&scope, run)
                    .unwrap()
                    .patch,
                patch
            );
        }
    }

    #[test]
    fn snapshot_start_fences_cache_ownership_drift_cancel_and_shutdown_before_agent() {
        use crate::services::provider_snapshot::{tests::capture, Manager};
        for provider in [
            config::ReviewProvider::Github,
            config::ReviewProvider::Bitbucket,
        ] {
            let snapshots = Arc::new(Manager::default());
            let info = snapshots.insert(capture(provider)).unwrap();
            let runtime = ReviewRuntime {
                engine: ReviewEngine::Real,
                snapshots,
                provider_reads: Arc::new(AtomicUsize::new(0)),
                shutdown: Arc::new(AtomicBool::new(false)),
            };
            let params = protocol::ReviewStartParams {
                target: TargetIdentity::Provider {
                    provider: match provider {
                        config::ReviewProvider::Github => ProviderKind::Github,
                        config::ReviewProvider::Bitbucket => ProviderKind::Bitbucket,
                    },
                    workspace: info.request.repository.workspace.clone(),
                    repo: info.request.repository.repo.clone(),
                    pr_id: Some(info.request.pr_id),
                    run_id: None,
                },
                profile: None,
                snapshot: Some(info.snapshot.clone()),
            };
            let cancel = AtomicBool::new(false);
            let mut wrong = params.clone();
            wrong.snapshot.as_mut().unwrap().repository.repo = "foreign".into();
            assert!(
                start_snapshot_session_with_recheck(&wrong, &runtime, &cancel, |_| panic!(
                    "foreign capture reached credentials"
                ))
                .is_err()
            );
            wrong = params.clone();
            wrong.snapshot.as_mut().unwrap().snapshot_id = "0".repeat(64);
            assert!(
                start_snapshot_session_with_recheck(&wrong, &runtime, &cancel, |_| panic!(
                    "missing capture reached credentials"
                ))
                .is_err()
            );
            cancel.store(true, Ordering::Release);
            assert!(
                start_snapshot_session_with_recheck(&params, &runtime, &cancel, |_| panic!(
                    "canceled capture reached credentials"
                ))
                .is_err()
            );
            cancel.store(false, Ordering::Release);
            assert!(
                start_snapshot_session_with_recheck(&params, &runtime, &cancel, |_| Err(
                    "Owned PR drift".into()
                ))
                .is_err()
            );
            assert_eq!(runtime.provider_reads.load(Ordering::Acquire), 0);
            assert!(
                start_snapshot_session_with_recheck(&params, &runtime, &cancel, |_| {
                    runtime.shutdown.store(true, Ordering::Release);
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(runtime.provider_reads.load(Ordering::Acquire), 0);
            runtime.snapshots.stop();
            assert!(
                start_snapshot_session_with_recheck(&params, &runtime, &cancel, |_| panic!(
                    "stopped capture reached credentials"
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn native_snapshot_browser_preserves_exact_evidence_without_local_checkout_or_run() {
        use crate::config::ReviewProvider;
        use crate::services::{
            bitbucket::workspace_prs::{DetailParams, RepositoryIdentity},
            provider_snapshot::{BrowserParams, Capture, Manager},
        };
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            let manager = Arc::new(Manager::default());
            let patch = "diff --git a/old.png b/new.png\nsimilarity index 100%\nrename from old.png\nrename to new.png\n";
            let info = manager
                .insert(Capture {
                    request: DetailParams {
                        repository: RepositoryIdentity {
                            provider,
                            workspace: "Owned".into(),
                            repo: "Repo".into(),
                        },
                        pr_id: 7,
                    },
                    title: "Owned preview".into(),
                    source_branch: "feature".into(),
                    destination_branch: "main".into(),
                    head_sha: "a".repeat(40),
                    destination_sha: "b".repeat(40),
                    base_sha: "c".repeat(40),
                    patch: patch.into(),
                    expected_count: 1,
                    expected_files: None,
                    source_repository: None,
                })
                .unwrap();
            let state = snapshot_browser_state(
                &manager,
                &BrowserParams {
                    snapshot: info.snapshot.clone(),
                },
            )
            .unwrap();
            assert_eq!(state.provider, Some(provider));
            assert_eq!(state.pr_id, 7);
            assert_eq!(state.diff.as_deref(), Some(patch));
            assert_eq!(state.base_sha.as_deref(), Some("c".repeat(40).as_str()));
            let files = state.diffstat.unwrap();
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].old_path.as_deref(), Some("old.png"));
            assert_eq!(files[0].new_path.as_deref(), Some("new.png"));
            assert!(state.snapshot_source.is_some());
            assert!(!state.population_failed);
            assert!(snapshot_browser_state(
                &Arc::new(Manager::default()),
                &BrowserParams {
                    snapshot: info.snapshot
                }
            )
            .is_err());
        }
    }

    #[test]
    fn fix_approvals_expire_and_cannot_cross_targets_or_be_replayed() {
        let target = TargetIdentity::Local {
            local_snapshot_sha256: "a".repeat(64),
        };
        let approval = FixApproval {
            action: ApprovedAction::Fix,
            message: None,
            files: Vec::new(),
            target: target.clone(),
            repo_path: PathBuf::from("unused"),
            snapshot: "snapshot".to_string(),
            payload: "payload".to_string(),
            source: "source".to_string(),
            destination: "main".to_string(),
            thread_id: "thread".to_string(),
            expires: Instant::now() - Duration::from_secs(1),
        };
        let approvals = Mutex::new(HashMap::from([("a".repeat(64), approval.clone())]));
        let params = protocol::ReviewFixParams {
            target: target.clone(),
            approval_token: "a".repeat(64),
        };
        assert!(
            consume_fix_approval(&params, &approvals, ApprovedAction::Fix)
                .unwrap_err()
                .message
                .contains("expired")
        );
        assert!(
            consume_fix_approval(&params, &approvals, ApprovedAction::Fix)
                .unwrap_err()
                .message
                .contains("already used")
        );
        let mut approval = approval;
        approval.expires = Instant::now() + Duration::from_secs(300);
        approvals.lock().unwrap().insert("a".repeat(64), approval);
        let other = protocol::ReviewFixParams {
            target: TargetIdentity::Local {
                local_snapshot_sha256: "b".repeat(64),
            },
            ..params
        };
        assert!(
            consume_fix_approval(&other, &approvals, ApprovedAction::Fix)
                .unwrap_err()
                .message
                .contains("different review")
        );
        assert!(approvals.lock().unwrap().is_empty());
    }

    #[test]
    fn conversation_preview_is_bounded_and_excludes_resume_secrets() {
        use crate::services::review::{
            AiReviewMessage, AiReviewMessageRole, AiReviewStoreData, AiReviewThread,
        };
        let store = AiReviewStoreData {
            threads: vec![AiReviewThread {
                id: "thread-1".into(),
                title: "Saved conversation".into(),
                created_at: "1".into(),
                updated_at: "2".into(),
                claude_session_id: Some("private-resume-id".into()),
                ai_provider: None,
                messages: (0..10)
                    .map(|i| AiReviewMessage {
                        id: format!("message-{i}"),
                        role: AiReviewMessageRole::Assistant,
                        content: "✓".repeat(1500),
                        created_at: "1".into(),
                    })
                    .collect(),
            }],
            ..Default::default()
        };
        let result = thread_summaries(&store, Some("thread-1"));
        assert_eq!(result["threads"][0]["selected"], true);
        assert_eq!(
            result["threads"][0]["messages"].as_array().unwrap().len(),
            8
        );
        assert_eq!(result["threads"][0]["messages"][0]["id"], "message-2");
        assert_eq!(
            result["threads"][0]["messages"][0]["content"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            1000
        );
        assert_eq!(result["threads"][0]["messages"][0]["truncated"], true);
        assert_eq!(result["threads"][0]["truncated"], true);
        assert!(!result.to_string().contains("private-resume-id"));
    }

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp repo");
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {args:?} failed");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "t"]);
        dir
    }

    #[test]
    fn changed_file_entries_uses_the_requested_range() {
        let dir = init_repo();
        std::fs::write(dir.path().join("a.txt"), "one\n").expect("write");
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {args:?} failed");
        };
        git(&["add", "-A"]);
        git(&["commit", "-qm", "one"]);
        let base = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("rev-parse")
                .stdout,
        )
        .expect("utf8")
        .trim()
        .to_string();
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").expect("edit");
        std::fs::write(dir.path().join("b.txt"), "new\n").expect("add");
        git(&["add", "-A"]);
        git(&["commit", "-qm", "two"]);
        let head = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("rev-parse")
                .stdout,
        )
        .expect("utf8")
        .trim()
        .to_string();

        // Clean working tree: the working-tree view is empty.
        assert!(changed_file_entries(dir.path(), None)
            .expect("entries")
            .is_empty());

        // The requested range reports the PR diff instead.
        let entries = changed_file_entries(dir.path(), Some((base.as_str(), head.as_str())))
            .expect("entries");
        let paths: Vec<&str> = entries.iter().filter_map(|e| e["path"].as_str()).collect();
        assert!(paths.contains(&"a.txt"), "paths: {paths:?}");
        assert!(paths.contains(&"b.txt"), "paths: {paths:?}");
        let a = entries
            .iter()
            .find(|e| e["path"] == "a.txt")
            .expect("a.txt entry");
        assert_eq!(a["status"], "modified");
        assert!(a["additions"].as_u64().unwrap_or(0) >= 1);
    }

    #[test]
    fn file_diff_includes_the_rename_source() {
        let dir = init_repo();
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\nthree\n").expect("write");
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {args:?} failed");
        };
        git(&["add", "-A"]);
        git(&["commit", "-qm", "one"]);
        git(&["mv", "a.txt", "b.txt"]);
        std::fs::write(dir.path().join("b.txt"), "one\ntwo\nTHREE\n").expect("edit");

        let Some(diff) = file_diff(dir.path(), "b.txt", None, Some("a.txt")) else {
            panic!("expected a diff for the renamed file");
        };
        assert!(
            diff.contains("rename from a.txt"),
            "diff must keep rename context: {diff}"
        );
    }

    #[test]
    fn changed_file_entries_detects_renames_with_detection_disabled() {
        let dir = init_repo();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {args:?} failed");
        };
        git(&["config", "diff.renames", "false"]);
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\nthree\n").expect("write");
        git(&["add", "-A"]);
        git(&["commit", "-qm", "one"]);
        git(&["mv", "a.txt", "b.txt"]);
        std::fs::write(dir.path().join("b.txt"), "one\ntwo\nFOUR\n").expect("edit");

        let entries = changed_file_entries(dir.path(), None).expect("entries");
        let paths: Vec<&str> = entries.iter().filter_map(|e| e["path"].as_str()).collect();
        assert_eq!(paths, vec!["b.txt"], "paths: {paths:?}");
        assert_eq!(entries[0]["status"], "renamed");
        assert_eq!(entries[0]["oldPath"], "a.txt");
    }

    #[test]
    fn changed_file_entries_covers_unstaged_changes_before_first_commit() {
        let dir = init_repo();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {args:?} failed");
        };
        std::fs::write(dir.path().join("first.txt"), "one\n").expect("write");
        git(&["add", "-A"]);
        std::fs::write(dir.path().join("first.txt"), "one\ntwo\n").expect("edit");

        let entries = changed_file_entries(dir.path(), None).expect("entries");
        let entry = entries
            .iter()
            .find(|e| e["path"] == "first.txt")
            .expect("first.txt entry");
        assert_eq!(entry["status"], "added");
        assert_eq!(entry["additions"], 2);
    }

    #[test]
    fn file_diff_treats_the_path_literally() {
        let dir = init_repo();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {args:?} failed");
        };
        std::fs::write(dir.path().join("star*.txt"), "one\n").expect("write");
        std::fs::write(dir.path().join("other.txt"), "x\n").expect("write");
        git(&["add", "-A"]);
        git(&["commit", "-qm", "one"]);
        std::fs::write(dir.path().join("star*.txt"), "one\ntwo\n").expect("edit");

        let diff = file_diff(dir.path(), "star*.txt", None, None).expect("diff");
        assert!(diff.contains("star*.txt"), "diff: {diff}");
        assert!(!diff.contains("other.txt"), "diff: {diff}");
    }

    #[test]
    fn file_diff_preserves_blank_context_lines() {
        let dir = init_repo();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {args:?} failed");
        };
        std::fs::write(dir.path().join("f.txt"), "a\n\nb\n").expect("write");
        git(&["add", "-A"]);
        git(&["commit", "-qm", "one"]);
        git(&["config", "diff.suppressBlankEmpty", "true"]);
        std::fs::write(dir.path().join("f.txt"), "A\n\nb\n").expect("edit");

        let diff = file_diff(dir.path(), "f.txt", None, None).expect("diff");
        assert!(
            diff.contains("\n \n"),
            "blank context line must keep its leading space: {diff}"
        );
    }

    #[test]
    fn working_tree_patch_includes_staged_and_untracked_files() {
        let dir = init_repo();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {args:?} failed");
        };
        std::fs::write(dir.path().join("staged.txt"), "one\n").expect("write");
        git(&["add", "-A"]);
        std::fs::write(dir.path().join("untracked.txt"), "two\n").expect("write");

        let files = changed_file_entries(dir.path(), None).expect("files");
        let patch = working_tree_patch(dir.path(), &files).expect("patch");
        assert!(patch.contains("staged.txt"), "patch: {patch}");
        assert!(patch.contains("untracked.txt"), "patch: {patch}");
    }

    #[test]
    fn scope_to_file_keeps_only_the_destination_section() {
        let diff = "diff --git a/old b/old\n--- a/old\n+++ b/old\n@@ -1 +1 @@\n-x\n+y\ndiff --git a/old b/new\n--- a/old\n+++ b/new\n@@ -1 +1 @@\n-a\n+b\n";
        let scoped = scope_to_file(diff, "new", Some("old")).expect("scoped");
        assert!(scoped.contains("b/new"), "scoped: {scoped}");
        assert!(!scoped.contains("b/old"), "scoped: {scoped}");
    }

    #[test]
    fn scope_to_file_rejects_a_suffix_collision() {
        let diff = "diff --git a/foo b/x b/x\n--- a/foo b/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/foo b/x b/foo b/x\n--- a/foo b/x\n+++ b/foo b/x\n@@ -1 +1 @@\n-c\n+d\n";
        let scoped = scope_to_file(diff, "x", Some("foo b/x")).expect("scoped");
        assert!(scoped.contains("+++ b/x"), "scoped: {scoped}");
        assert!(!scoped.contains("+++ b/foo b/x"), "scoped: {scoped}");
    }

    #[test]
    fn file_diff_matches_quoted_and_prefixed_headers() {
        let dir = init_repo();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {args:?} failed");
        };
        git(&["config", "diff.noprefix", "true"]);
        let name = "ta\tb.txt";
        std::fs::write(dir.path().join(name), "one\n").expect("write");
        git(&["add", "-A"]);
        git(&["commit", "-qm", "one"]);
        std::fs::write(dir.path().join(name), "one\ntwo\n").expect("edit");

        let diff = file_diff(dir.path(), name, None, None).expect("diff");
        assert!(diff.contains("two"), "diff: {diff}");
        assert!(!diff.contains("new file mode"), "synthetic: {diff}");
    }

    #[test]
    fn select_review_run_prefers_the_requested_run_id() {
        use crate::services::review::{AiReviewStoreData, ReviewRun};
        let make = |id: &str| -> ReviewRun {
            serde_json::from_value(serde_json::json!({
                "id": id,
                "schemaVersion": "1",
                "provider": "github",
                "workspace": "acme",
                "repo": "payments",
                "prId": 42,
                "sourceBranch": "feat",
                "destinationBranch": "main",
                "reviewedBaseSha": "base",
                "reviewedHeadSha": "head",
                "status": "succeeded",
                "turnKind": "initial",
                "createdAt": "2024-01-01T00:00:00Z",
                "diffFingerprint": "fp",
                "findings": []
            }))
            .expect("deserialize run")
        };
        let store = AiReviewStoreData {
            review_runs: vec![make("run-1"), make("run-2")],
            ..Default::default()
        };
        assert_eq!(
            select_review_run(Some(store.clone()), Some("run-1")).map(|run| run.id),
            Some("run-1".to_string())
        );
        assert_eq!(
            select_review_run(Some(store), None).map(|run| run.id),
            Some("run-2".to_string())
        );
    }
}
