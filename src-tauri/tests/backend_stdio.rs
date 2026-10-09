//! Real subprocess integration tests for the persistent Rust stdio backend.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

const LOCAL_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct Backend {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
}

impl Backend {
    fn spawn(cwd: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_norn-backend"))
            .current_dir(cwd)
            .env("NORN_BACKEND_OPERATION_DELAY_MS", "200")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn backend");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (tx, rx) = channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self { child, stdin, rx }
    }

    fn send(&mut self, value: Value) {
        self.send_line(&serde_json::to_string(&value).expect("encode"));
    }

    fn send_line(&mut self, line: &str) {
        self.stdin
            .write_all(format!("{line}\n").as_bytes())
            .expect("write frame");
        self.stdin.flush().expect("flush");
    }

    fn next(&self, timeout: Duration) -> Option<Value> {
        let line = self.rx.recv_timeout(timeout).ok()?;
        serde_json::from_str(&line).ok()
    }

    fn wait_for(&self, predicate: impl Fn(&Value) -> bool, timeout: Duration) -> Value {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let value = self
                .next(remaining)
                .unwrap_or_else(|| panic!("timed out waiting for a frame"));
            if predicate(&value) {
                return value;
            }
        }
    }

    fn handshake(&mut self) {
        self.send(json!({
            "type": "hello",
            "protocolVersion": 1,
            "client": { "name": "test", "version": "0.0.0" }
        }));
        let ready = self.wait_for(|v| v["type"] == "ready", Duration::from_secs(5));
        assert_eq!(ready["protocolVersion"], 1);
    }
}

fn temp_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp repo");
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "t@t"]);
    git(dir.path(), &["config", "user.name", "t"]);
    dir
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("run git");
    assert!(output.status.success(), "git {args:?} failed");
}

#[test]
fn handshake_lists_repositories_and_shuts_down() {
    let repo = temp_repo();
    let mut backend = Backend::spawn(repo.path());
    backend.handshake();

    backend.send(json!({
        "type": "request",
        "id": "req-1",
        "method": "repository.status",
        "params": {}
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-1",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true);
    assert!(response["result"]["repos"].is_array());

    backend.send(json!({
        "type": "request",
        "id": "req-2",
        "method": "shutdown",
        "params": {}
    }));
    let shutdown = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-2",
        Duration::from_secs(5),
    );
    assert_eq!(shutdown["ok"], true);
    let status = backend.child.wait().expect("wait backend");
    assert!(status.success(), "backend should exit cleanly");
}

#[test]
fn diff_file_returns_the_working_tree_diff() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("a.txt"), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    std::fs::write(repo.path().join("a.txt"), "two\n").expect("write");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-diff",
        "method": "diff.file",
        "params": {
            "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA },
            "path": "a.txt"
        }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-diff",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    let diff = response["result"]["diff"].as_str().expect("diff string");
    assert!(diff.contains("-one"), "diff: {diff}");
    assert!(diff.contains("+two"), "diff: {diff}");
}

#[test]
fn review_operation_can_be_cancelled() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("a.txt"), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    std::fs::write(repo.path().join("a.txt"), "two\n").expect("write");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-start",
        "method": "review.start",
        "params": { "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA } }
    }));
    let started = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-start",
        Duration::from_secs(5),
    );
    let operation_id = started["result"]["operationId"]
        .as_str()
        .expect("operation id")
        .to_string();

    backend.send(json!({
        "type": "request",
        "id": "req-cancel",
        "method": "operation.cancel",
        "params": { "operationId": operation_id }
    }));
    let cancelled = backend.wait_for(
        |v| {
            v["type"] == "event"
                && v["operationId"] == operation_id.as_str()
                && v["event"]["state"] == "cancelled"
        },
        Duration::from_secs(5),
    );
    assert_eq!(cancelled["event"]["kind"], "state");
}

#[test]
fn diff_file_includes_untracked_new_files() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("a.txt"), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    std::fs::write(repo.path().join("new.txt"), "hello\nworld\n").expect("write untracked");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-untracked",
        "method": "diff.file",
        "params": {
            "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA },
            "path": "new.txt"
        }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-untracked",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    let diff = response["result"]["diff"].as_str().expect("diff string");
    assert!(diff.contains("new file mode"), "diff: {diff}");
    assert!(diff.contains("+hello"), "diff: {diff}");
}

#[test]
fn diff_file_reports_a_deleted_tracked_file() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("gone.txt"), "here\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    std::fs::remove_file(repo.path().join("gone.txt")).expect("delete");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-deleted",
        "method": "diff.file",
        "params": {
            "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA },
            "path": "gone.txt"
        }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-deleted",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    let diff = response["result"]["diff"].as_str().expect("diff string");
    assert!(diff.contains("deleted file mode"), "diff: {diff}");
    assert!(diff.contains("-here"), "diff: {diff}");
}

#[test]
fn diff_file_reports_staged_addition_without_head() {
    let dir = tempfile::tempdir().expect("temp repo");
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "t@t"]);
    git(dir.path(), &["config", "user.name", "t"]);
    std::fs::write(dir.path().join("added.txt"), "fresh\n").expect("write");
    git(dir.path(), &["add", "added.txt"]);

    let mut backend = Backend::spawn(dir.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-staged",
        "method": "diff.file",
        "params": {
            "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA },
            "path": "added.txt"
        }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-staged",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    let diff = response["result"]["diff"].as_str().expect("diff string");
    assert!(diff.contains("+fresh"), "diff: {diff}");
}

#[test]
fn review_files_lists_changed_files() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("a.txt"), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    std::fs::write(repo.path().join("a.txt"), "two\n").expect("modify");
    std::fs::write(repo.path().join("new.txt"), "fresh\n").expect("untracked");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-files",
        "method": "review.files",
        "params": { "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA } }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-files",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    let files = response["result"]["files"].as_array().expect("files array");
    let paths: Vec<&str> = files.iter().filter_map(|f| f["path"].as_str()).collect();
    assert!(paths.contains(&"a.txt"), "paths: {paths:?}");
    assert!(paths.contains(&"new.txt"), "paths: {paths:?}");
    let untracked = files
        .iter()
        .find(|f| f["path"] == "new.txt")
        .expect("untracked entry");
    assert_eq!(untracked["additions"], 1);
}

#[test]
fn review_files_reports_renames_without_bogus_entries() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("old.txt"), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    git(repo.path(), &["mv", "old.txt", "renamed.txt"]);
    std::fs::write(repo.path().join("renamed.txt"), "one\ntwo\n").expect("edit");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-rename",
        "method": "review.files",
        "params": { "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA } }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-rename",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    let files = response["result"]["files"].as_array().expect("files array");
    let paths: Vec<&str> = files.iter().filter_map(|f| f["path"].as_str()).collect();
    assert_eq!(paths, vec!["renamed.txt"], "paths: {paths:?}");
    let entry = &files[0];
    assert_eq!(entry["status"], "renamed");
    assert_eq!(entry["oldPath"], "old.txt");
    assert!(entry["additions"].as_u64().unwrap_or(0) >= 1);
}

#[test]
fn review_files_lists_staged_files_before_the_first_commit() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("first.txt"), "hello\n").expect("write");
    git(repo.path(), &["add", "-A"]);

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-unborn",
        "method": "review.files",
        "params": { "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA } }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-unborn",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    let files = response["result"]["files"].as_array().expect("files array");
    let paths: Vec<&str> = files.iter().filter_map(|f| f["path"].as_str()).collect();
    assert_eq!(paths, vec!["first.txt"], "paths: {paths:?}");
    assert_eq!(files[0]["status"], "added");
}

#[test]
fn review_files_preserves_filenames_with_tabs() {
    let repo = temp_repo();
    let name = "weird\tname.txt";
    std::fs::write(repo.path().join(name), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    std::fs::write(repo.path().join(name), "one\ntwo\n").expect("edit");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-tab",
        "method": "review.files",
        "params": { "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA } }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-tab",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    let files = response["result"]["files"].as_array().expect("files array");
    let paths: Vec<&str> = files.iter().filter_map(|f| f["path"].as_str()).collect();
    assert_eq!(paths, vec![name], "paths: {paths:?}");
    assert!(files[0]["additions"].as_u64().unwrap_or(0) >= 1);
}

#[test]
fn review_targets_returns_a_target_list() {
    let repo = temp_repo();
    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-targets",
        "method": "review.targets",
        "params": {}
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-targets",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    assert!(response["result"]["targets"].is_array());
}

#[test]
fn diff_file_before_first_commit_includes_staged_content() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("first.txt"), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    std::fs::write(repo.path().join("first.txt"), "one\ntwo\n").expect("edit");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-unborn-diff",
        "method": "diff.file",
        "params": {
            "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA },
            "path": "first.txt"
        }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-unborn-diff",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    let diff = response["result"]["diff"].as_str().expect("diff string");
    assert!(diff.contains("+one"), "diff: {diff}");
    assert!(diff.contains("+two"), "diff: {diff}");
}

#[test]
fn malformed_frames_do_not_stop_the_backend() {
    let repo = temp_repo();
    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send_line("{ this is not json }");
    backend.send(json!({
        "type": "request",
        "id": "req-after-malformed",
        "method": "repository.status",
        "params": {}
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-after-malformed",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true);
}

#[test]
fn incompatible_handshake_fails_without_protocol_output() {
    let repo = temp_repo();
    let mut backend = Backend::spawn(repo.path());
    backend.send(json!({
        "type": "hello",
        "protocolVersion": 2,
        "client": { "name": "test", "version": "0.0.0" }
    }));
    let status = backend.child.wait().expect("wait backend");
    assert_eq!(status.code(), Some(3));
    assert!(backend.next(Duration::from_millis(200)).is_none());
}
