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
use std::io::{BufRead, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;

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
const GIT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CONCURRENT_DIFFS: usize = 4;
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
    active_operations: Arc<AtomicUsize>,
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

fn provider_matches(kind: ProviderKind) -> config::ReviewProvider {
    match kind {
        ProviderKind::Github => config::ReviewProvider::Github,
        ProviderKind::Bitbucket => config::ReviewProvider::Bitbucket,
    }
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
    let (diff, truncated) = match file_diff(&repo_path, &params.path) {
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

fn file_diff(repo_path: &Path, path: &str) -> Option<String> {
    if !is_safe_relative_path(path) {
        return None;
    }
    // HEAD covers committed, staged, and unstaged tracked changes (including
    // deletions); the cached and unstaged diffs cover unborn-HEAD repositories.
    for args in [
        ["diff", "--no-ext-diff", "HEAD", "--", path].as_slice(),
        ["diff", "--no-ext-diff", "--", path].as_slice(),
        ["diff", "--cached", "--no-ext-diff", "--", path].as_slice(),
    ] {
        if let Some(diff) = git(repo_path, args) {
            if !diff.trim().is_empty() {
                return Some(diff);
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
    let mut child = spawn_git(repo_path, args)?;
    let pid = child.id();
    let stdout = child.stdout.take()?;
    let limit = (MAX_DIFF_BYTES + 4096) as u64;
    let reader = thread::spawn(move || {
        let mut buffer = Vec::new();
        let mut stdout = std::io::Read::take(stdout, limit);
        let _ = std::io::Read::read_to_end(&mut stdout, &mut buffer);
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
    status
        .filter(|status| status.success())
        .map(|_| String::from_utf8_lossy(&buffer).to_string())
}

fn spawn_git(repo_path: &Path, args: &[&str]) -> Option<Child> {
    if shutting_down().load(Ordering::SeqCst) {
        return None;
    }
    let child = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
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
