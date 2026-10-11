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
        Self::spawn_env(cwd, &[])
    }

    fn spawn_env(cwd: &Path, extra_env: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_norn-backend"));
        command
            .current_dir(cwd)
            .env("NORN_BACKEND_OPERATION_DELAY_MS", "200")
            .env("NORN_BROWSER_OPEN", "0")
            // Pin the deterministic engine; individual tests override via extra_env.
            .env("NORN_REVIEW_ENGINE", "simulated");
        for (key, value) in extra_env {
            command.env(key, value);
        }
        let mut child = command
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
fn browser_open_returns_an_authenticated_session_url() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("a.txt"), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    std::fs::write(repo.path().join("a.txt"), "two\n").expect("edit");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-browser",
        "method": "browser.open",
        "params": { "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA } }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-browser",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    let url = response["result"]["url"].as_str().expect("url string");
    assert!(url.starts_with("http://127.0.0.1:"), "url: {url}");
    let token = url
        .split("/session/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .expect("session token");
    assert_eq!(token.len(), 64, "token: {token}");
    assert!(
        token.bytes().all(|b| b.is_ascii_hexdigit()),
        "token: {token}"
    );
}

#[test]
fn file_preview_returns_bounded_image_bytes() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("pic.png"), [0x89u8, 0x50, 0x4e, 0x47]).expect("write");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-preview",
        "method": "file.preview",
        "params": {
            "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA },
            "path": "pic.png"
        }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-preview",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], true, "response: {response}");
    assert_eq!(response["result"]["mimeType"], "image/png");
    assert_eq!(response["result"]["size"], 4);
    assert!(!response["result"]["dataBase64"]
        .as_str()
        .unwrap_or("")
        .is_empty());
}

#[test]
fn file_preview_rejects_unsupported_types() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("notes.txt"), "hello\n").expect("write");

    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-preview-bad",
        "method": "file.preview",
        "params": {
            "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA },
            "path": "notes.txt"
        }
    }));
    let response = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-preview-bad",
        Duration::from_secs(5),
    );
    assert_eq!(response["ok"], false, "response: {response}");
}

#[test]
fn fake_review_engine_streams_deterministic_events() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("a.txt"), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    std::fs::write(repo.path().join("a.txt"), "two\n").expect("edit");

    let mut backend = Backend::spawn_env(repo.path(), &[("NORN_REVIEW_ENGINE", "fake")]);
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

    let progress = backend.wait_for(
        |v| {
            v["type"] == "event"
                && v["operationId"] == operation_id.as_str()
                && v["event"]["message"] == "fake review analysed"
        },
        Duration::from_secs(5),
    );
    assert_eq!(progress["event"]["kind"], "progress");

    let succeeded = backend.wait_for(
        |v| {
            v["type"] == "event"
                && v["operationId"] == operation_id.as_str()
                && v["event"]["state"] == "succeeded"
        },
        Duration::from_secs(5),
    );
    assert_eq!(succeeded["event"]["kind"], "state");
}

#[test]
fn inline_review_streams_and_succeeds_with_a_fake_agent() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("a.txt"), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    std::fs::write(repo.path().join("a.txt"), "two\n").expect("edit");

    let data_dir = tempfile::tempdir().expect("data dir");
    let agent_dir = tempfile::tempdir().expect("agent dir");
    let agent_path = agent_dir.path().join("fake-claude");
    let script = r#"#!/bin/sh
printf '%s\n' '{"result":"No review findings were produced.","session_id":"fake-session"}'
"#;
    std::fs::write(&agent_path, script).expect("write agent");
    let mut perms = std::fs::metadata(&agent_path)
        .expect("metadata")
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&agent_path, perms).expect("chmod");

    let mut backend = Backend::spawn_env(
        repo.path(),
        &[
            ("NORN_REVIEW_ENGINE", "real"),
            (
                "NORN_REVIEW_AGENT_BIN",
                agent_path.to_str().expect("agent path"),
            ),
            (
                "NORN_REVIEW_DATA_DIR",
                data_dir.path().to_str().expect("data dir"),
            ),
        ],
    );
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-inline",
        "method": "review.start",
        "params": { "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA } }
    }));
    let started = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-inline",
        Duration::from_secs(5),
    );
    let operation_id = started["result"]["operationId"]
        .as_str()
        .expect("operation id")
        .to_string();

    backend.wait_for(
        |v| {
            v["type"] == "event"
                && v["operationId"] == operation_id.as_str()
                && v["event"]["kind"] == "progress"
        },
        Duration::from_secs(15),
    );
    let terminal = backend.wait_for(
        |v| {
            v["type"] == "event"
                && v["operationId"] == operation_id.as_str()
                && v["event"]["state"] == "succeeded"
        },
        Duration::from_secs(15),
    );
    assert_eq!(terminal["event"]["state"], "succeeded");
}

#[test]
fn inline_review_can_be_cancelled() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("a.txt"), "one\n").expect("write");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "init"]);
    std::fs::write(repo.path().join("a.txt"), "two\n").expect("edit");

    let data_dir = tempfile::tempdir().expect("data dir");
    let agent_dir = tempfile::tempdir().expect("agent dir");
    let started_marker = agent_dir.path().join("started");
    let done_marker = agent_dir.path().join("done");
    let agent_path = agent_dir.path().join("slow-claude");
    let script = r#"#!/bin/sh
if [ -n "$NORN_FAKE_STARTED" ]; then printf 'up' > "$NORN_FAKE_STARTED"; fi
sleep 10
if [ -n "$NORN_FAKE_DONE" ]; then printf 'done' > "$NORN_FAKE_DONE"; fi
printf '%s\n' '{"result":"late","session_id":"fake-session"}'
"#;
    std::fs::write(&agent_path, script).expect("write agent");
    let mut perms = std::fs::metadata(&agent_path)
        .expect("metadata")
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&agent_path, perms).expect("chmod");

    let mut backend = Backend::spawn_env(
        repo.path(),
        &[
            ("NORN_REVIEW_ENGINE", "real"),
            (
                "NORN_REVIEW_AGENT_BIN",
                agent_path.to_str().expect("agent path"),
            ),
            (
                "NORN_REVIEW_DATA_DIR",
                data_dir.path().to_str().expect("data dir"),
            ),
            (
                "NORN_FAKE_STARTED",
                started_marker.to_str().expect("started marker"),
            ),
            ("NORN_FAKE_DONE", done_marker.to_str().expect("done marker")),
        ],
    );
    backend.handshake();
    backend.send(json!({
        "type": "request",
        "id": "req-inline-cancel",
        "method": "review.start",
        "params": { "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA } }
    }));
    let started = backend.wait_for(
        |v| v["type"] == "response" && v["id"] == "req-inline-cancel",
        Duration::from_secs(5),
    );
    let operation_id = started["result"]["operationId"]
        .as_str()
        .expect("operation id")
        .to_string();

    let marker_deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !started_marker.exists() {
        assert!(
            std::time::Instant::now() < marker_deadline,
            "fake agent never started"
        );
        thread::sleep(Duration::from_millis(50));
    }

    backend.send(json!({
        "type": "request",
        "id": "req-cancel-inline",
        "method": "operation.cancel",
        "params": { "operationId": operation_id }
    }));
    backend.wait_for(
        |v| {
            v["type"] == "event"
                && v["operationId"] == operation_id.as_str()
                && v["event"]["state"] == "cancelled"
        },
        Duration::from_secs(15),
    );

    // Wait beyond the agent's own 10s completion delay to prove it was
    // terminated rather than left running.
    thread::sleep(Duration::from_millis(11_000));
    assert!(
        !done_marker.exists(),
        "the cancelled agent process still completed"
    );
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

#[test]
fn thread_request_is_typed_and_local_targets_have_no_pr_threads() {
    let repo = temp_repo();
    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    backend.send(json!({
        "type": "request", "id": "threads", "method": "review.threads",
        "params": { "target": { "kind": "local", "localSnapshotSha256": LOCAL_SHA } }
    }));
    let response = backend.wait_for(|v| v["id"] == "threads", Duration::from_secs(5));
    assert_eq!(response["ok"], true);
    assert_eq!(response["result"]["threads"], json!([]));
    backend
        .send(json!({ "type": "request", "id": "shutdown", "method": "shutdown", "params": {} }));
    assert!(backend.child.wait().expect("backend exits").success());
}
