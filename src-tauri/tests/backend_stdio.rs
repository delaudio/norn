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
    use base64::Engine as _;
    let png=base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==").unwrap();
    std::fs::write(repo.path().join("pic.png"), &png).expect("write");

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
    assert_eq!(response["result"]["size"], png.len());
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

#[cfg(unix)]
struct FixFixture {
    pending: std::cell::RefCell<Vec<Value>>,
    repo: tempfile::TempDir,
    _origin: tempfile::TempDir,
    _data: tempfile::TempDir,
    _config: tempfile::TempDir,
    _agent: tempfile::TempDir,
    backend: Backend,
    target: Value,
}

#[cfg(unix)]
impl FixFixture {
    fn new(agent_script: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let repo = temp_repo();
        git(repo.path(), &["checkout", "-b", "feature"]);
        std::fs::write(repo.path().join("tracked.txt"), "base\n").unwrap();
        git(repo.path(), &["add", "."]);
        git(repo.path(), &["commit", "-m", "base"]);
        let head = Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        let head = String::from_utf8(head.stdout).unwrap().trim().to_string();
        let origin = tempfile::tempdir().unwrap();
        git(origin.path(), &["init", "--bare"]);
        git(
            repo.path(),
            &["remote", "add", "origin", origin.path().to_str().unwrap()],
        );
        git(repo.path(), &["push", "-u", "origin", "feature"]);
        let config = tempfile::tempdir().unwrap();
        let settings = json!({"repos":[{"provider":"github","workspace":"fixture","repo":"demo","localPath":repo.path().canonicalize().unwrap()}],
            "defaultDiffView":"unified","theme":"dark"});
        std::fs::write(config.path().join("settings.json"), settings.to_string()).unwrap();
        let data = tempfile::tempdir().unwrap();
        let agent = tempfile::tempdir().unwrap();
        let binary = agent.path().join("fake-agent");
        std::fs::write(&binary, agent_script).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut backend = Backend::spawn_env(
            repo.path(),
            &[
                ("NORN_CONFIG_DIR", config.path().to_str().unwrap()),
                ("NORN_REVIEW_DATA_DIR", data.path().to_str().unwrap()),
                ("NORN_REVIEW_AGENT_BIN", binary.to_str().unwrap()),
            ],
        );
        backend.handshake();
        backend.send(
            json!({"type":"request","id":"init-store","method":"review.targets","params":{}}),
        );
        backend.wait_for(|v| v["id"] == "init-store", Duration::from_secs(10));
        let store = json!({"activeThreadId":"thread-fix", "threads":[{"id":"thread-fix","title":"Fix review","createdAt":"1","updatedAt":"1",
            "claudeSessionId":"private-resume", "aiProvider":"claude", "messages":[{"id":"message","role":"assistant","content":"Fix tracked.txt. Latest position.","createdAt":"1"}]}],
            "reviewRuns":[{"id":"run-fix","schemaVersion":"1","provider":"github","workspace":"fixture","repo":"demo","prId":9,
                "sourceBranch":"feature","destinationBranch":"feature","reviewedBaseSha":head,"reviewedHeadSha":head,"status":"succeeded","turnKind":"initial",
                "createdAt":"1","finishedAt":"2","diffFingerprint":"fixture","threadId":"thread-fix","summaryMarkdown":"Fix tracked.txt.","findings":[]}]});
        let connection = rusqlite::Connection::open(data.path().join("norn.sqlite3")).unwrap();
        connection.execute("INSERT INTO ai_review_stores (review_key,workspace,repo,pr_id,store_json,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,'1','1')",
            rusqlite::params!["fixture_demo_9", "fixture", "demo", 9, store.to_string()]).unwrap();
        let target = json!({"kind":"provider","provider":"github","workspace":"fixture","repo":"demo","prId":9,"runId":"run-fix"});
        Self {
            pending: std::cell::RefCell::new(Vec::new()),
            repo,
            _origin: origin,
            _data: data,
            _config: config,
            _agent: agent,
            backend,
            target,
        }
    }

    fn preview(&mut self) -> Value {
        self.backend.send(json!({"type":"request","id":"preview","method":"review.fix.preview","params":{"target":self.target}}));
        let result = self
            .backend
            .wait_for(|v| v["id"] == "preview", Duration::from_secs(15));
        assert_eq!(result["ok"], true, "{result}");
        result["result"].clone()
    }

    fn mutation_preview(&mut self, method: &str, message: Option<&str>) -> Value {
        let mut params = json!({"target":self.target});
        if let Some(message) = message {
            params["message"] = json!(message);
        }
        self.backend.send(
            json!({"type":"request","id":"mutation-preview","method":method,"params":params}),
        );
        let result = self.backend.wait_for(
            |value| value["id"] == "mutation-preview",
            Duration::from_secs(15),
        );
        assert_eq!(result["ok"], true, "{result}");
        result["result"].clone()
    }

    fn execute(&mut self, token: &str) -> String {
        self.execute_action("review.fix", token)
    }

    fn execute_action(&mut self, method: &str, token: &str) -> String {
        self.backend.send(json!({"type":"request","id":"fix","method":method,"params":{"target":self.target,"approvalToken":token}}));
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let result = loop {
            let value = self
                .backend
                .next(deadline.saturating_duration_since(std::time::Instant::now()))
                .expect("fix response");
            if value["id"] == "fix" {
                break value;
            }
            self.pending.borrow_mut().push(value);
        };
        assert_eq!(result["ok"], true, "{result}");
        result["result"]["operationId"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn terminal(&self, operation: &str) -> Value {
        let pending_index = self.pending.borrow().iter().position(|v| {
            v["operationId"] == operation
                && ["succeeded", "failed", "cancelled"]
                    .contains(&v["event"]["state"].as_str().unwrap_or(""))
        });
        if let Some(index) = pending_index {
            return self.pending.borrow_mut().remove(index);
        }
        self.backend.wait_for(
            |v| {
                v["operationId"] == operation
                    && ["succeeded", "failed", "cancelled"]
                        .contains(&v["event"]["state"].as_str().unwrap_or(""))
            },
            Duration::from_secs(20),
        )
    }
}

#[cfg(unix)]
impl Drop for FixFixture {
    fn drop(&mut self) {
        self.backend
            .send(json!({"type":"request","id":"stop","method":"shutdown","params":{}}));
        let _ = self.backend.child.wait();
    }
}

#[cfg(unix)]
#[test]
fn fix_approval_rejects_changed_content_and_replay_without_starting_agent() {
    let mut fixture = FixFixture::new("#!/bin/sh\nprintf invoked > agent-ran\nexit 1\n");
    let preview = fixture.preview();
    assert!(preview["effects"].as_array().unwrap().len() >= 5);
    assert!(!preview.to_string().contains("private-resume"));
    // A newly untracked file must invalidate approval, even if HEAD is unchanged.
    std::fs::write(fixture.repo.path().join("new.txt"), "local edit\n").unwrap();
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    let terminal = fixture.terminal(&operation);
    assert_eq!(terminal["event"]["state"], "failed");
    assert_eq!(terminal["event"]["error"]["code"], "targetStale");
    assert!(!fixture.repo.path().join("agent-ran").exists());
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "failed");
}

#[cfg(unix)]
#[test]
fn approved_fix_detects_edits_to_already_dirty_files_and_exposes_result() {
    let mut fixture = FixFixture::new(
        r#"#!/bin/sh
printf 'fixed\n' > tracked.txt
printf '%s\n' '{"structured_output":{"status":"success","summary":"Fixed tracked content","commitMessage":"fix: content","tests":["fake check"],"filesTouched":["tracked.txt"]},"session_id":"private-agent-session"}'
"#,
    );
    std::fs::write(fixture.repo.path().join("tracked.txt"), "existing dirty\n").unwrap();
    let preview = fixture.preview();
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    assert_eq!(
        std::fs::read_to_string(fixture.repo.path().join("tracked.txt")).unwrap(),
        "fixed\n"
    );
    fixture.backend.send(json!({"type":"request","id":"result","method":"review.fix.state","params":{"target":fixture.target}}));
    let result = fixture
        .backend
        .wait_for(|v| v["id"] == "result", Duration::from_secs(10));
    assert_eq!(result["result"]["state"]["phase"], "readyToCommit");
    assert_eq!(result["result"]["state"]["filesTouched"][0], "tracked.txt");
    assert!(!result.to_string().contains("private-agent-session"));
    let history = Command::new("git")
        .arg("-C")
        .arg(fixture.repo.path())
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(history.stdout).unwrap().trim(), "1");
}

#[cfg(unix)]
#[test]
fn cancel_fix_reaps_agent_and_preserves_partial_edits() {
    let mut fixture = FixFixture::new("#!/bin/sh\nprintf 'partial\\n' > tracked.txt\nprintf started > agent-started\nsleep 60 &\nwait\n");
    let preview = fixture.preview();
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !fixture.repo.path().join("agent-started").exists() {
        assert!(std::time::Instant::now() < deadline, "agent did not start");
        thread::sleep(Duration::from_millis(20));
    }
    fixture.backend.send(json!({"type":"request","id":"cancel-fix","method":"operation.cancel","params":{"operationId":operation}}));
    fixture
        .backend
        .wait_for(|v| v["id"] == "cancel-fix", Duration::from_secs(5));
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "cancelled");
    assert_eq!(
        std::fs::read_to_string(fixture.repo.path().join("tracked.txt")).unwrap(),
        "partial\n"
    );
}

#[cfg(unix)]
#[test]
fn failed_fix_preserves_partial_edits_without_committing() {
    let mut fixture =
        FixFixture::new("#!/bin/sh\nprintf 'partial failure\\n' > tracked.txt\nexit 7\n");
    let preview = fixture.preview();
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    let terminal = fixture.terminal(&operation);
    assert_eq!(terminal["event"]["state"], "failed");
    assert!(terminal["event"]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("partial changes are preserved"));
    assert_eq!(
        std::fs::read_to_string(fixture.repo.path().join("tracked.txt")).unwrap(),
        "partial failure\n"
    );
    let history = Command::new("git")
        .arg("-C")
        .arg(fixture.repo.path())
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(history.stdout).unwrap().trim(), "1");
}

#[cfg(unix)]
#[test]
fn fix_approval_rejects_changed_binary_content_and_staged_identity() {
    let mut fixture = FixFixture::new("#!/bin/sh\nprintf invoked > agent-ran\nexit 1\n");
    std::fs::write(fixture.repo.path().join("tracked.txt"), b"\0binary-one").unwrap();
    let preview = fixture.preview();
    std::fs::write(fixture.repo.path().join("tracked.txt"), b"\0binary-two").unwrap();
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    assert_eq!(
        fixture.terminal(&operation)["event"]["error"]["code"],
        "targetStale"
    );
    let preview = fixture.preview();
    git(fixture.repo.path(), &["add", "tracked.txt"]);
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    assert_eq!(
        fixture.terminal(&operation)["event"]["error"]["code"],
        "targetStale"
    );
    assert!(!fixture.repo.path().join("agent-ran").exists());
}

#[cfg(unix)]
const SUCCESSFUL_FIX_AGENT: &str = r#"#!/bin/sh
printf 'fixed\n' > tracked.txt
printf '%s\n' '{"structured_output":{"status":"success","summary":"Fixed tracked content","commitMessage":"fix: content","tests":["fake check"],"filesTouched":["tracked.txt"]}}'
"#;

#[cfg(unix)]
#[test]
fn approved_commit_excludes_unrelated_staged_files_and_pushes_exact_source_ref() {
    let mut fixture = FixFixture::new(SUCCESSFUL_FIX_AGENT);
    let preview = fixture.preview();
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    std::fs::write(
        fixture.repo.path().join("unrelated.txt"),
        "private local work\n",
    )
    .unwrap();
    git(fixture.repo.path(), &["add", "unrelated.txt"]);
    let message = "fix: Unicode ✓ \"$HOME\" `literal`";
    let preview = fixture.mutation_preview("review.commit.preview", Some(message));
    assert_eq!(preview["action"], "commit");
    assert_eq!(preview["filesToCommit"], json!(["tracked.txt"]));
    assert_eq!(preview["commitMessage"], message);
    let operation =
        fixture.execute_action("review.commit", preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    let committed = Command::new("git")
        .arg("-C")
        .arg(fixture.repo.path())
        .args(["show", "--format=", "--name-only", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(committed.stdout).unwrap().trim(),
        "tracked.txt"
    );
    let staged = Command::new("git")
        .arg("-C")
        .arg(fixture.repo.path())
        .args(["diff", "--cached", "--name-only"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(staged.stdout).unwrap().trim(),
        "unrelated.txt"
    );
    let actual_message = Command::new("git")
        .arg("-C")
        .arg(fixture.repo.path())
        .args(["log", "-1", "--format=%B"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(actual_message.stdout).unwrap().trim(),
        message
    );
    let preview = fixture.mutation_preview("review.push.preview", None);
    assert_eq!(preview["action"], "push");
    let commit = preview["commitSha"].as_str().unwrap().to_string();
    let operation =
        fixture.execute_action("review.push", preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    let remote = Command::new("git")
        .arg("-C")
        .arg(fixture._origin.path())
        .args(["rev-parse", "refs/heads/feature"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(remote.stdout).unwrap().trim(), commit);
    fixture.backend.send(json!({"type":"request","id":"final-state","method":"review.fix.state","params":{"target":fixture.target}}));
    let final_state = fixture
        .backend
        .wait_for(|v| v["id"] == "final-state", Duration::from_secs(5));
    assert_eq!(final_state["result"]["state"]["phase"], "completed");
    assert_eq!(final_state["result"]["state"]["commitSha"], commit);
}

#[cfg(unix)]
#[test]
fn commit_approval_cannot_be_used_for_push_or_replayed_or_after_content_changes() {
    let mut fixture = FixFixture::new(SUCCESSFUL_FIX_AGENT);
    let preview = fixture.preview();
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    let preview = fixture.mutation_preview("review.commit.preview", Some("fix: content"));
    let operation =
        fixture.execute_action("review.push", preview["approvalToken"].as_str().unwrap());
    assert!(fixture.terminal(&operation)["event"]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("different action"));
    let operation =
        fixture.execute_action("review.commit", preview["approvalToken"].as_str().unwrap());
    assert!(fixture.terminal(&operation)["event"]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("already used"));
    let preview = fixture.mutation_preview("review.commit.preview", Some("fix: content"));
    std::fs::write(
        fixture.repo.path().join("tracked.txt"),
        "changed after preview\n",
    )
    .unwrap();
    let operation =
        fixture.execute_action("review.commit", preview["approvalToken"].as_str().unwrap());
    assert_eq!(
        fixture.terminal(&operation)["event"]["error"]["code"],
        "targetStale"
    );
    let count = Command::new("git")
        .arg("-C")
        .arg(fixture.repo.path())
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(count.stdout).unwrap().trim(), "1");
}

#[cfg(unix)]
#[test]
fn commit_validates_paths_and_preserves_staged_work_that_predates_the_fix() {
    let mut fixture = FixFixture::new(SUCCESSFUL_FIX_AGENT);
    std::fs::write(
        fixture.repo.path().join("unrelated.txt"),
        "existing staged work\n",
    )
    .unwrap();
    git(fixture.repo.path(), &["add", "unrelated.txt"]);
    let preview = fixture.preview();
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    let staged = Command::new("git")
        .arg("-C")
        .arg(fixture.repo.path())
        .args(["diff", "--cached", "--name-only"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(staged.stdout).unwrap().trim(),
        "unrelated.txt"
    );
    let preview = fixture.mutation_preview("review.commit.preview", Some("fix: content"));
    let operation =
        fixture.execute_action("review.commit", preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    let staged = Command::new("git")
        .arg("-C")
        .arg(fixture.repo.path())
        .args(["diff", "--cached", "--name-only"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(staged.stdout).unwrap().trim(),
        "unrelated.txt"
    );
    let mut unsafe_fixture = FixFixture::new(
        r#"#!/bin/sh
printf 'fixed\n' > tracked.txt
printf '%s\n' '{"structured_output":{"status":"success","summary":"Fixed content","commitMessage":"fix: content","tests":[],"filesTouched":["../outside.txt"]}}'
"#,
    );
    let preview = unsafe_fixture.preview();
    let operation = unsafe_fixture.execute(preview["approvalToken"].as_str().unwrap());
    assert_eq!(
        unsafe_fixture.terminal(&operation)["event"]["state"],
        "succeeded"
    );
    unsafe_fixture.backend.send(json!({"type":"request","id":"unsafe-path","method":"review.commit.preview","params":{"target":unsafe_fixture.target,"message":"fix: content"}}));
    let result = unsafe_fixture
        .backend
        .wait_for(|v| v["id"] == "unsafe-path", Duration::from_secs(10));
    assert_eq!(result["ok"], false);
    assert!(result["error"]["message"]
        .as_str()
        .unwrap()
        .contains("approved clone"));
}

#[cfg(unix)]
#[test]
fn approved_commit_can_be_cancelled_while_hooks_run_without_auto_retry() {
    use std::os::unix::fs::PermissionsExt;
    let mut fixture = FixFixture::new(SUCCESSFUL_FIX_AGENT);
    let preview = fixture.preview();
    let operation = fixture.execute(preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    let hook = fixture.repo.path().join(".git/hooks/pre-commit");
    std::fs::write(
        &hook,
        "#!/bin/sh\nprintf started > .git/commit-hook-started\nsleep 60 &\nwait\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let preview = fixture.mutation_preview("review.commit.preview", Some("fix: content"));
    let operation =
        fixture.execute_action("review.commit", preview["approvalToken"].as_str().unwrap());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !fixture
        .repo
        .path()
        .join(".git/commit-hook-started")
        .exists()
    {
        assert!(
            std::time::Instant::now() < deadline,
            "commit hook did not start"
        );
        thread::sleep(Duration::from_millis(20));
    }
    fixture.backend.send(json!({"type":"request","id":"cancel-commit","method":"operation.cancel","params":{"operationId":operation}}));
    fixture.backend.wait_for(
        |value| value["id"] == "cancel-commit",
        Duration::from_secs(5),
    );
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "cancelled");
    let count = Command::new("git")
        .arg("-C")
        .arg(fixture.repo.path())
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(count.stdout).unwrap().trim(), "1");
    let operation =
        fixture.execute_action("review.commit", preview["approvalToken"].as_str().unwrap());
    assert!(fixture.terminal(&operation)["event"]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("already used"));
}

#[cfg(unix)]
impl FixFixture {
    fn prepare_destination(&self, conflicting: bool) {
        git(self.repo.path(), &["checkout", "-b", "destination"]);
        let filename = if conflicting {
            "tracked.txt"
        } else {
            "destination.txt"
        };
        std::fs::write(self.repo.path().join(filename), "destination change\n").unwrap();
        git(self.repo.path(), &["add", filename]);
        git(self.repo.path(), &["commit", "-m", "destination change"]);
        git(self.repo.path(), &["push", "origin", "destination"]);
        git(self.repo.path(), &["checkout", "feature"]);
        if conflicting {
            std::fs::write(self.repo.path().join("tracked.txt"), "source change\n").unwrap();
            git(self.repo.path(), &["add", "tracked.txt"]);
            git(self.repo.path(), &["commit", "-m", "source change"]);
            git(self.repo.path(), &["push", "origin", "feature"]);
        }
        let connection =
            rusqlite::Connection::open(self._data.path().join("norn.sqlite3")).unwrap();
        let saved: String = connection
            .query_row(
                "SELECT store_json FROM ai_review_stores WHERE review_key='fixture_demo_9'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut saved: Value = serde_json::from_str(&saved).unwrap();
        saved["reviewRuns"][0]["destinationBranch"] = json!("destination");
        connection
            .execute(
                "UPDATE ai_review_stores SET store_json=?1 WHERE review_key='fixture_demo_9'",
                [saved.to_string()],
            )
            .unwrap();
    }
}

#[cfg(unix)]
#[test]
fn approved_sync_leaves_merge_for_separate_commit_and_push() {
    let mut fixture = FixFixture::new("#!/bin/sh\nexit 99\n");
    fixture.prepare_destination(false);
    let remote_before = Command::new("git")
        .arg("-C")
        .arg(fixture._origin.path())
        .args(["rev-parse", "feature"])
        .output()
        .unwrap()
        .stdout;
    let preview = fixture.mutation_preview("review.sync.preview", None);
    assert_eq!(preview["action"], "sync");
    let operation =
        fixture.execute_action("review.sync", preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    assert!(fixture.repo.path().join("destination.txt").exists());
    assert!(fixture.repo.path().join(".git/MERGE_HEAD").exists());
    let preview =
        fixture.mutation_preview("review.commit.preview", Some("merge: reviewed destination"));
    assert_eq!(preview["filesToCommit"], json!(["destination.txt"]));
    let operation =
        fixture.execute_action("review.commit", preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    let parents = Command::new("git")
        .arg("-C")
        .arg(fixture.repo.path())
        .args(["rev-list", "--parents", "-n", "1", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(parents.stdout)
            .unwrap()
            .split_whitespace()
            .count(),
        3
    );
    let remote = Command::new("git")
        .arg("-C")
        .arg(fixture._origin.path())
        .args(["rev-parse", "feature"])
        .output()
        .unwrap()
        .stdout;
    assert_eq!(remote, remote_before, "sync and commit must not publish");
    let preview = fixture.mutation_preview("review.push.preview", None);
    let operation =
        fixture.execute_action("review.push", preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
}

#[cfg(unix)]
#[test]
fn sync_conflicts_are_preserved_then_resolved_with_a_separate_approval() {
    let mut fixture = FixFixture::new(
        r#"#!/bin/sh
printf 'resolved\n' > tracked.txt
git add -- tracked.txt
printf '%s\n' '{"structured_output":{"status":"success","summary":"Resolved merge","commitMessage":"merge: resolve","tests":[],"filesTouched":["tracked.txt"]}}'
"#,
    );
    fixture.prepare_destination(true);
    let preview = fixture.mutation_preview("review.sync.preview", None);
    let operation =
        fixture.execute_action("review.sync", preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "failed");
    assert!(fixture.repo.path().join(".git/MERGE_HEAD").exists());
    let preview = fixture.mutation_preview("review.conflicts.preview", None);
    let operation = fixture.execute_action(
        "review.conflicts",
        preview["approvalToken"].as_str().unwrap(),
    );
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
    assert_eq!(
        std::fs::read_to_string(fixture.repo.path().join("tracked.txt")).unwrap(),
        "resolved\n"
    );
    let preview = fixture.mutation_preview("review.commit.preview", Some("merge: resolve"));
    let operation =
        fixture.execute_action("review.commit", preview["approvalToken"].as_str().unwrap());
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "succeeded");
}

#[cfg(unix)]
#[test]
fn branch_sync_refuses_dirty_checkout_and_stale_or_wrong_action_approval() {
    let mut fixture = FixFixture::new("#!/bin/sh\nexit 99\n");
    fixture.prepare_destination(false);
    let preview = fixture.mutation_preview("review.sync.preview", None);
    let operation = fixture.execute_action(
        "review.conflicts",
        preview["approvalToken"].as_str().unwrap(),
    );
    assert_eq!(
        fixture.terminal(&operation)["event"]["error"]["code"],
        "targetStale"
    );
    let preview = fixture.mutation_preview("review.sync.preview", None);
    std::fs::write(
        fixture.repo.path().join("local.txt"),
        "preserve local work\n",
    )
    .unwrap();
    let operation =
        fixture.execute_action("review.sync", preview["approvalToken"].as_str().unwrap());
    assert_eq!(
        fixture.terminal(&operation)["event"]["error"]["code"],
        "targetStale"
    );
    fixture.backend.send(json!({"type":"request","id":"dirty-sync","method":"review.sync.preview","params":{"target":fixture.target}}));
    let refused = fixture
        .backend
        .wait_for(|v| v["id"] == "dirty-sync", Duration::from_secs(10));
    assert_eq!(refused["ok"], false);
    assert_eq!(
        std::fs::read_to_string(fixture.repo.path().join("local.txt")).unwrap(),
        "preserve local work\n"
    );
    assert!(!fixture.repo.path().join(".git/MERGE_HEAD").exists());
}

#[cfg(unix)]
#[test]
fn unresolved_agent_result_and_unrelated_merge_edits_are_refused() {
    let mut fixture = FixFixture::new(
        r#"#!/bin/sh
printf '%s\n' '{"structured_output":{"status":"success","summary":"Claims success","commitMessage":"merge","tests":[],"filesTouched":[]}}'
"#,
    );
    fixture.prepare_destination(true);
    let preview = fixture.mutation_preview("review.conflicts.preview", None);
    let operation = fixture.execute_action(
        "review.conflicts",
        preview["approvalToken"].as_str().unwrap(),
    );
    assert_eq!(fixture.terminal(&operation)["event"]["state"], "failed");
    assert!(fixture.repo.path().join(".git/MERGE_HEAD").exists());
    let mut clean = FixFixture::new("#!/bin/sh\nexit 99\n");
    clean.prepare_destination(false);
    let preview = clean.mutation_preview("review.sync.preview", None);
    let operation = clean.execute_action("review.sync", preview["approvalToken"].as_str().unwrap());
    assert_eq!(clean.terminal(&operation)["event"]["state"], "succeeded");
    std::fs::write(clean.repo.path().join("unrelated.txt"), "private\n").unwrap();
    git(clean.repo.path(), &["add", "unrelated.txt"]);
    clean.backend.send(json!({"type":"request","id":"unrelated-merge","method":"review.commit.preview","params":{"target":clean.target,"message":"merge"}}));
    let refused = clean
        .backend
        .wait_for(|v| v["id"] == "unrelated-merge", Duration::from_secs(10));
    assert_eq!(refused["ok"], false, "{refused}");
    assert!(clean.repo.path().join(".git/MERGE_HEAD").exists());
}

#[test]
fn settings_stdio_roundtrip_uses_isolated_config_and_rejects_stale_versions() {
    let repo = temp_repo();
    let config = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let mut backend = Backend::spawn_env(
        repo.path(),
        &[
            ("NORN_CONFIG_DIR", config.path().to_str().unwrap()),
            ("NORN_REVIEW_DATA_DIR", data.path().to_str().unwrap()),
        ],
    );
    backend.handshake();
    backend.send(json!({"type":"request","id":"settings","method":"settings.get","params":{}}));
    let before = backend.wait_for(|v| v["id"] == "settings", Duration::from_secs(10));
    assert_eq!(before["ok"], true, "{before}");
    let mut settings = before["result"]["settings"].clone();
    settings["codexModel"] = json!("custom/org:reasoning ✓");
    settings["repos"] = json!([{"provider":"github","workspace":"fixture","repo":"demo","localPath":repo.path().to_str().unwrap()}]);
    let params = json!({"expectedVersion":before["result"]["version"],"settings":settings});
    backend.send(json!({"type":"request","id":"save","method":"settings.save","params":params}));
    let saved = backend.wait_for(|v| v["id"] == "save", Duration::from_secs(10));
    assert_eq!(saved["ok"], true, "{saved}");
    assert_eq!(
        saved["result"]["settings"]["codexModel"],
        "custom/org:reasoning ✓"
    );
    backend.send(json!({"type":"request","id":"stale","method":"settings.save","params":params}));
    let stale = backend.wait_for(|v| v["id"] == "stale", Duration::from_secs(10));
    assert_eq!(stale["ok"], false);
    assert_eq!(stale["error"]["code"], "conflict");
    let persisted: Value =
        serde_json::from_slice(&std::fs::read(config.path().join("settings.json")).unwrap())
            .unwrap();
    assert_eq!(persisted["codexModel"], "custom/org:reasoning ✓");
    assert!(persisted.get("token").is_none());
    backend.send(json!({"type":"request","id":"redacted","method":"auth.set","params":{"provider":"fixture_sensitive_value","token":"fixture_sensitive_value"}}));
    backend.send(json!({"type":"request","id":"stop","method":"shutdown","params":{}}));
    backend.child.wait().unwrap();
    let mut diagnostics = String::new();
    std::io::Read::read_to_string(&mut backend.child.stderr.take().unwrap(), &mut diagnostics)
        .unwrap();
    assert!(diagnostics.contains("malformed protocol frame"));
    assert!(!diagnostics.contains("fixture_sensitive_value"));
}

#[test]
fn user_data_preview_import_and_repeat_are_bound_to_isolated_storage() {
    let repo = temp_repo();
    let data = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let source = data.path().join("desktop-export.json");
    std::fs::write(&source,serde_json::to_vec(&json!({"schemaVersion":1,"source":"desktopLocalStorage","entries":[{"key":"norn.reviewPrompt.v1.owner/repo","value":"custom review instructions"},{"key":"lachesi.viewedFiles.owner/repo#7","value":"[\"a.rs\"]"}]})).unwrap()).unwrap();
    let source_original = std::fs::read(&source).unwrap();
    let mut backend = Backend::spawn_env(
        repo.path(),
        &[
            ("NORN_DATA_DIR", data.path().to_str().unwrap()),
            ("NORN_CONFIG_DIR", config.path().to_str().unwrap()),
        ],
    );
    backend.handshake();
    backend.send(json!({"type":"request","id":"preview","method":"data.preview","params":{"path":source,"provider":"github"}}));
    let response = backend.wait_for(|v| v["id"] == "preview", Duration::from_secs(5));
    assert_eq!(response["ok"], true);
    let preview = &response["result"];
    assert_eq!(preview["additionsCount"], 2);
    assert!(!data.path().join("workspace-data-v1.json").exists());
    assert!(!response.to_string().contains("custom review instructions"));
    backend.send(json!({"type":"request","id":"import","method":"data.import","params":{"path":source,"provider":"github","inputVersion":preview["inputVersion"],"destinationVersion":preview["destinationVersion"],"confirm":true}}));
    let result = backend.wait_for(|v| v["id"] == "import", Duration::from_secs(5));
    assert_eq!(result["ok"], true);
    assert_eq!(result["result"]["imported"], 2);
    let destination = std::fs::read(data.path().join("workspace-data-v1.json")).unwrap();
    backend.send(json!({"type":"request","id":"stale","method":"data.import","params":{"path":source,"provider":"github","inputVersion":preview["inputVersion"],"destinationVersion":preview["destinationVersion"],"confirm":true}}));
    let stale = backend.wait_for(|v| v["id"] == "stale", Duration::from_secs(5));
    assert_eq!(stale["ok"], false);
    assert_eq!(
        std::fs::read(data.path().join("workspace-data-v1.json")).unwrap(),
        destination
    );
    assert_eq!(std::fs::read(&source).unwrap(), source_original);
    backend.send(json!({"type":"request","id":"stop","method":"shutdown","params":{}}));
    let _ = backend.wait_for(|v| v["id"] == "stop", Duration::from_secs(5));
    assert!(backend.child.wait().unwrap().success());
}

fn bitmap_bytes(color: [u8; 3]) -> Vec<u8> {
    let image =
        image::DynamicImage::ImageRgb8(image::ImageBuffer::from_pixel(512, 512, image::Rgb(color)));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Bmp).unwrap();
    bytes.into_inner()
}
fn preview_chunk(
    backend: &mut Backend,
    path: &str,
    side: &str,
    offset: usize,
    hash: Option<&str>,
) -> Value {
    let id = format!("image-{side}-{offset}");
    let mut params = json!({"target":{"kind":"local","localSnapshotSha256":LOCAL_SHA},"path":path,"side":side,"offset":offset,"maxBytes":196608});
    if let Some(hash) = hash {
        params["contentSha256"] = json!(hash);
    }
    backend.send(json!({"type":"request","id":id,"method":"file.preview","params":params}));
    backend.wait_for(|v| v["id"] == id, Duration::from_secs(5))
}
#[test]
fn image_comparison_chunks_are_bounded_and_fenced_against_content_drift() {
    use base64::Engine as _;
    let repo = temp_repo();
    let before = bitmap_bytes([255, 0, 0]);
    let after = bitmap_bytes([0, 0, 255]);
    std::fs::write(repo.path().join("pic.bmp"), &before).unwrap();
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "base image"]);
    std::fs::write(repo.path().join("pic.bmp"), &after).unwrap();
    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    for (side, expected) in [("old", &before), ("new", &after)] {
        let mut offset = 0;
        let mut hash = None;
        let mut collected = Vec::new();
        loop {
            let response = preview_chunk(&mut backend, "pic.bmp", side, offset, hash.as_deref());
            assert_eq!(response["ok"], true, "{response}");
            assert!(serde_json::to_vec(&response).unwrap().len() < 1_048_576);
            let chunk = &response["result"];
            assert_eq!(chunk["width"], 512);
            assert_eq!(chunk["height"], 512);
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(chunk["dataBase64"].as_str().unwrap())
                .unwrap();
            assert!(bytes.len() <= 196608);
            collected.extend(bytes);
            hash = Some(chunk["contentSha256"].as_str().unwrap().to_owned());
            if chunk["nextOffset"].is_null() {
                break;
            }
            offset = chunk["nextOffset"].as_u64().unwrap() as usize;
        }
        assert_eq!(&collected, expected);
    }
    let first = preview_chunk(&mut backend, "pic.bmp", "new", 0, None);
    let hash = first["result"]["contentSha256"].as_str().unwrap();
    let mut changed = after.clone();
    changed.push(0);
    std::fs::write(repo.path().join("pic.bmp"), changed).unwrap();
    let stale = preview_chunk(&mut backend, "pic.bmp", "new", 196608, Some(hash));
    assert_eq!(stale["ok"], false);
    assert_eq!(stale["error"]["code"], "targetStale");
    assert!(stale["result"].is_null());
    std::fs::write(repo.path().join("not-changed.bmp"), &before).unwrap();
    git(repo.path(), &["add", "not-changed.bmp"]);
    git(repo.path(), &["commit", "-qm", "unrelated image"]);
    let unrelated = preview_chunk(&mut backend, "not-changed.bmp", "new", 0, None);
    assert_eq!(unrelated["ok"], false);
}
#[test]
fn image_comparison_preserves_missing_sides_and_rename_sources() {
    let repo = temp_repo();
    let bytes = bitmap_bytes([20, 40, 60]);
    std::fs::write(repo.path().join("before.bmp"), &bytes).unwrap();
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "base"]);
    git(repo.path(), &["mv", "before.bmp", "after.bmp"]);
    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    let old = preview_chunk(&mut backend, "after.bmp", "old", 0, None);
    let new = preview_chunk(&mut backend, "after.bmp", "new", 0, None);
    assert_eq!(old["ok"], true, "{old}");
    assert_eq!(new["ok"], true, "{new}");
    assert_eq!(
        old["result"]["contentSha256"],
        new["result"]["contentSha256"]
    );
    git(repo.path(), &["commit", "-qm", "rename"]);
    std::fs::remove_file(repo.path().join("after.bmp")).unwrap();
    let deleted = preview_chunk(&mut backend, "after.bmp", "new", 0, None);
    assert_eq!(deleted["ok"], true);
    assert_eq!(deleted["result"]["available"], false);
    let before = preview_chunk(&mut backend, "after.bmp", "old", 0, None);
    assert_eq!(before["ok"], true);
    assert_eq!(before["result"]["available"], true);
    std::fs::write(repo.path().join("new.bmp"), &bytes).unwrap();
    let absent = preview_chunk(&mut backend, "new.bmp", "old", 0, None);
    assert_eq!(absent["ok"], true);
    assert_eq!(absent["result"]["available"], false);
    let present = preview_chunk(&mut backend, "new.bmp", "new", 0, None);
    assert_eq!(present["ok"], true);
    assert_eq!(present["result"]["available"], true);
}
#[test]
fn image_comparison_refuses_malformed_oversized_and_symlink_content() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("invalid.png"), b"not an image").unwrap();
    let mut oversized = bitmap_bytes([0, 0, 0]);
    oversized.resize(2 * 1024 * 1024 + 1, 0);
    std::fs::write(repo.path().join("huge.bmp"), oversized).unwrap();
    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    assert_eq!(
        preview_chunk(&mut backend, "invalid.png", "new", 0, None)["ok"],
        false
    );
    assert_eq!(
        preview_chunk(&mut backend, "huge.bmp", "new", 0, None)["ok"],
        false
    );
    let wide = image::DynamicImage::ImageRgb8(image::ImageBuffer::from_pixel(
        8193,
        1,
        image::Rgb([0, 0, 0]),
    ));
    let mut encoded = std::io::Cursor::new(Vec::new());
    wide.write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    std::fs::write(repo.path().join("wide.png"), encoded.into_inner()).unwrap();
    assert_eq!(
        preview_chunk(&mut backend, "wide.png", "new", 0, None)["ok"],
        false
    );
    #[cfg(unix)]
    {
        std::fs::write(repo.path().join("target.bmp"), bitmap_bytes([0, 0, 0])).unwrap();
        std::os::unix::fs::symlink("target.bmp", repo.path().join("link.bmp")).unwrap();
        let response = preview_chunk(&mut backend, "link.bmp", "new", 0, None);
        assert_eq!(response["ok"], false);
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("outside the checkout"));
    }
}
