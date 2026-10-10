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

/// Every operation the backend tracks while it is running.
struct Operation {
    cancel: Arc<AtomicBool>,
    state: OperationState,
    sequence: u64,
    error: Option<ProtocolError>,
}

type Operations = Arc<Mutex<HashMap<String, Operation>>>;

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
    browser: Arc<Mutex<Option<WebDiffServer>>>,
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
        browser: Arc::new(Mutex::new(None)),
        active_operations: Arc::new(AtomicUsize::new(0)),
    };

    backend.send_control(ServerMessage::Ready(Ready {
        protocol_version: PROTOCOL_VERSION,
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: vec![
            "repository".to_string(),
            "diff".to_string(),
            "review".to_string(),
        ],
    }));

    read_loop(&mut reader, &backend);

    if let Ok(operations) = backend.operations.lock() {
        for operation in operations.values() {
            operation.cancel.store(true, Ordering::SeqCst);
        }
    }
    // Stop spawning new children, terminate the ones this backend owns, and let
    // the writer flush and exit.
    shutting_down().store(true, Ordering::SeqCst);
    kill_git_children();
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
    match request {
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
            thread::spawn(move || {
                diff_file_response(&control, &id, params);
                counter.decrement();
            });
            false
        }
        Request::ReviewStart { params, .. } => {
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
            backend.start_review(&id, &operation_id, params.target);
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
            thread::spawn(move || {
                review_files_response(&control, &id, params.target);
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
        Request::FilePreview { params, .. } => {
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
            thread::spawn(move || {
                file_preview_response(&control, &id, params);
                counter.decrement();
            });
            false
        }
        Request::BrowserOpen { params, .. } => {
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
            thread::spawn(move || {
                browser_open_response(&control, &browser, &id, params.target);
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

/// Real, offline review targets: recent review jobs whose workspace/repo match a
/// configured repository, so the shell can select a stored PR review (with its
/// `prId`) and reach its persisted findings.
fn review_targets_response() -> serde_json::Value {
    let configured = config::load().repos;
    let mut targets = Vec::new();
    if let Ok(jobs) = crate::review_storage::list_recent_review_jobs(50) {
        for job in jobs {
            if job.pr_id == 0 {
                continue;
            }
            let Some(repo) = configured
                .iter()
                .find(|entry| entry.workspace == job.workspace && entry.repo == job.repo)
            else {
                continue;
            };
            let provider = match repo.provider {
                config::ReviewProvider::Github => "github",
                config::ReviewProvider::Bitbucket => "bitbucket",
            };
            targets.push(json!({
                "provider": provider,
                "workspace": job.workspace,
                "repo": job.repo,
                "prId": job.pr_id,
                "runId": job.id,
                "title": job.pr_title,
                "status": serde_json::to_value(job.status).unwrap_or_else(|_| json!("unknown")),
            }));
        }
    }
    json!({ "targets": targets })
}

fn provider_matches(kind: ProviderKind) -> config::ReviewProvider {
    match kind {
        ProviderKind::Github => config::ReviewProvider::Github,
        ProviderKind::Bitbucket => config::ReviewProvider::Bitbucket,
    }
}

/// Start (or update) the authenticated browser diff session for a target and
/// return its URL. The OS browser is opened unless `NORN_BROWSER_OPEN=0`.
/// Return the bounded new-side image bytes for the selected file, or an error
/// so the shell can show an explicit fallback instead of an empty panel.
fn file_preview_response(
    control: &Sender<ServerMessage>,
    id: &str,
    params: protocol::FilePreviewParams,
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
    let Some(mime) = image_mime_for_path(&params.path) else {
        fail(ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "unsupported image preview type",
        ));
        return;
    };
    let Some(repo_path) = resolve_repo_path(&params.target) else {
        fail(ProtocolError::new(
            ProtocolErrorCode::NotFound,
            "no local repository for this target",
        ));
        return;
    };
    let bytes = match preview_bytes(&params.target, &repo_path, &params.path) {
        Ok(bytes) => bytes,
        Err(error) => {
            fail(error);
            return;
        }
    };
    respond(json!({
        "path": params.path,
        "mimeType": mime,
        "size": bytes.len(),
        "dataBase64": STANDARD.encode(&bytes),
    }));
}

/// The bounded new-side bytes for `path`, resolved from the target revision
/// (reviewed head for stored PR targets; the checkout otherwise).
fn preview_bytes(
    target: &TargetIdentity,
    repo_path: &Path,
    path: &str,
) -> Result<Vec<u8>, ProtocolError> {
    if let TargetIdentity::Provider { pr_id: Some(_), .. } = target {
        let Some((_base, head)) = reviewed_range(target)? else {
            return Err(ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "no stored reviewed range for this pull request",
            ));
        };
        if !revisions_available(repo_path, &head, &head) {
            return Err(ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "reviewed revisions are not available in the local checkout",
            ));
        }
        let spec = format!("{head}:{path}");
        let bytes = git_bytes(repo_path, &["show", spec.as_str()], MAX_PREVIEW_BYTES + 1)
            .ok_or_else(|| {
                ProtocolError::new(
                    ProtocolErrorCode::NotFound,
                    format!("no preview for `{path}`"),
                )
            })?;
        if bytes.len() as u64 > MAX_PREVIEW_BYTES {
            return Err(ProtocolError::new(
                ProtocolErrorCode::TargetStale,
                "image preview is unavailable or too large",
            ));
        }
        return Ok(bytes);
    }

    let absolute = safe_canonical_path(repo_path, path).ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::NotFound,
            format!("no preview for `{path}`"),
        )
    })?;
    let file = std::fs::File::open(&absolute).map_err(|_| {
        ProtocolError::new(
            ProtocolErrorCode::NotFound,
            format!("no preview for `{path}`"),
        )
    })?;
    let mut buffer = Vec::new();
    let mut reader = Read::take(file, MAX_PREVIEW_BYTES + 1);
    reader.read_to_end(&mut buffer).map_err(|_| {
        ProtocolError::new(
            ProtocolErrorCode::Internal,
            format!("failed to read the preview for `{path}`"),
        )
    })?;
    if buffer.len() as u64 > MAX_PREVIEW_BYTES {
        return Err(ProtocolError::new(
            ProtocolErrorCode::TargetStale,
            "image preview is unavailable or too large",
        ));
    }
    Ok(buffer)
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
    let state = match browser_diff_state(&target) {
        Ok(state) => state,
        Err(error) => {
            fail(error);
            return;
        }
    };
    let mut guard = match browser.lock() {
        Ok(guard) => guard,
        Err(_) => {
            fail(ProtocolError::new(
                ProtocolErrorCode::Internal,
                "browser server lock poisoned",
            ));
            return;
        }
    };
    let url = match guard.as_mut() {
        Some(server) => {
            server.update_pr(state);
            server.url()
        }
        None => {
            let shared = Arc::new(RwLock::new(state));
            match WebDiffServer::start(shared) {
                Ok(server) => {
                    let url = server.url();
                    *guard = Some(server);
                    url
                }
                Err(error) => {
                    fail(ProtocolError::new(ProtocolErrorCode::Internal, error));
                    return;
                }
            }
        }
    };
    drop(guard);
    if std::env::var("NORN_BROWSER_OPEN")
        .map(|value| value != "0")
        .unwrap_or(true)
    {
        let _ = open_browser_url(&url);
    }
    respond(json!({ "url": url }));
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
        let run = crate::services::review::load_ai_review_store_native(workspace, repo, *pr_id)
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
                _ => "modified",
            };
            let (old_path, new_path) = match status {
                "removed" => (old.or_else(|| Some(path.clone())), None),
                "renamed" => (old, Some(path.clone())),
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
                        .map(PathBuf::from)
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

fn diff_file_response(control: &Sender<ServerMessage>, id: &str, params: protocol::DiffFileParams) {
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

fn review_files_response(control: &Sender<ServerMessage>, id: &str, target: TargetIdentity) {
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

/// The reviewed base/head revisions for a stored PR target, if the stored review
/// run recorded them. Returns `Ok(None)` for working-tree targets or targets
/// without a recorded range.
fn reviewed_range(target: &TargetIdentity) -> Result<Option<(String, String)>, ProtocolError> {
    let TargetIdentity::Provider {
        workspace,
        repo,
        pr_id: Some(pr_id),
        run_id,
        ..
    } = target
    else {
        return Ok(None);
    };
    let store = crate::services::review::load_ai_review_store_native(workspace, repo, *pr_id)
        .map_err(|e| {
            ProtocolError::new(ProtocolErrorCode::Internal, format!("review store: {e}"))
        })?;
    let Some(run) = select_review_run(store, run_id.as_deref()) else {
        return Ok(None);
    };
    match (run.reviewed_base_sha, run.reviewed_head_sha) {
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
            workspace,
            repo,
            pr_id: Some(pr_id),
            run_id,
            ..
        } => {
            let store =
                crate::services::review::load_ai_review_store_native(workspace, repo, *pr_id)
                    .map_err(|error| {
                        ProtocolError::new(
                            ProtocolErrorCode::Internal,
                            format!("review store: {error}"),
                        )
                    })?;
            select_review_run(store, run_id.as_deref())
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
    fn start_review(&self, request_id: &str, operation_id: &str, target: TargetIdentity) {
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
        let delay = self.operation_delay();
        thread::spawn(move || {
            run_review_operation(&writer, &operations, &operation_id, &target, &cancel, delay);
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
