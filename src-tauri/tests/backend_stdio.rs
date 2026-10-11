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
    _isolated: tempfile::TempDir,
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Backend {
    fn spawn(cwd: &Path) -> Self {
        Self::spawn_env(cwd, &[])
    }

    fn spawn_env(cwd: &Path, extra_env: &[(&str, &str)]) -> Self {
        let isolated = tempfile::tempdir().expect("isolated backend storage");
        let mut command = Command::new(env!("CARGO_BIN_EXE_norn-backend"));
        command
            .current_dir(cwd)
            .env("NORN_DATA_DIR", isolated.path().join("workspace"))
            .env("NORN_CONFIG_DIR", isolated.path().join("config"))
            .env("NORN_REVIEW_DATA_DIR", isolated.path().join("reviews"))
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
        Self {
            _isolated: isolated,
            child,
            stdin,
            rx,
        }
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
#[test]
fn review_output_is_exact_bounded_and_pinned_to_provider_and_run() {
    let mut fixture = FixFixture::new("#!/bin/sh\nexit 99\n");
    let request = |fixture: &mut FixFixture, target: Value| {
        fixture.backend.send(json!({"type":"request","id":"copy-output","method":"review.output","params":{"target":target}}));
        fixture.backend.wait_for(
            |value| value["id"] == "copy-output",
            Duration::from_secs(10),
        )
    };
    let target = fixture.target.clone();
    let result = request(&mut fixture, target.clone());
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["result"]["target"], target);
    assert_eq!(result["result"]["runId"], "run-fix");
    assert_eq!(result["result"]["text"], "Fix tracked.txt.");
    assert!(!result.to_string().contains("private-resume"));
    let mut wrong = target.clone();
    wrong["provider"] = json!("bitbucket");
    assert_eq!(request(&mut fixture, wrong)["error"]["code"], "targetStale");
    let mut missing = target.clone();
    missing["runId"] = json!("does-not-exist");
    assert_eq!(request(&mut fixture, missing)["error"]["code"], "notFound");
    let connection = rusqlite::Connection::open(fixture._data.path().join("norn.sqlite3")).unwrap();
    let saved: String = connection
        .query_row(
            "SELECT store_json FROM ai_review_stores WHERE review_key='fixture_demo_9'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut saved: Value = serde_json::from_str(&saved).unwrap();
    let exact = "## Complete Unicode output\n世界 👩‍💻\n";
    saved["reviewRuns"][0]["summaryMarkdown"] = json!(exact);
    connection
        .execute(
            "UPDATE ai_review_stores SET store_json=?1 WHERE review_key='fixture_demo_9'",
            [saved.to_string()],
        )
        .unwrap();
    assert_eq!(
        request(&mut fixture, target.clone())["result"]["text"],
        exact
    );
    saved["reviewRuns"][0]["summaryMarkdown"] = json!("界".repeat(30_000));
    connection
        .execute(
            "UPDATE ai_review_stores SET store_json=?1 WHERE review_key='fixture_demo_9'",
            [saved.to_string()],
        )
        .unwrap();
    let oversized = request(&mut fixture, target);
    assert_eq!(oversized["error"]["code"], "invalidParams", "{oversized}");
    assert!(oversized.get("result").is_none());
}

#[cfg(unix)]
#[test]
fn finding_drafts_bind_previews_persist_across_restart_and_preserve_edited_duplicates() {
    let mut fixture = FixFixture::new("#!/bin/sh\nprintf unexpected > \"$0.executed\"\nexit 99\n");
    fixture.backend = Backend::spawn_env(
        fixture.repo.path(),
        &[
            ("NORN_CONFIG_DIR", fixture._config.path().to_str().unwrap()),
            (
                "NORN_REVIEW_DATA_DIR",
                fixture._data.path().to_str().unwrap(),
            ),
            ("NORN_DATA_DIR", fixture._data.path().to_str().unwrap()),
        ],
    );
    fixture.backend.handshake();
    let connection = rusqlite::Connection::open(fixture._data.path().join("norn.sqlite3")).unwrap();
    let saved: String = connection
        .query_row(
            "SELECT store_json FROM ai_review_stores WHERE review_key='fixture_demo_9'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut saved: Value = serde_json::from_str(&saved).unwrap();
    saved["reviewRuns"][0]["findings"] = json!([{"id":"f1","fingerprint":"fp1","title":"Validate input","severity":"high","confidence":"high","category":"bug","status":"new",
        "summary":"Saved finding 世界\r\nSecond line.","rationale":"Caller may send null.","ruleId":null,"source":"llm","anchor":{"path":"tracked.txt","startLine":1,"endLine":1,"side":"new"},"suggestedFix":"Return early.","evidenceIds":[],"publication":null}]);
    let save = |value: &Value| {
        connection
            .execute(
                "UPDATE ai_review_stores SET store_json=?1 WHERE review_key='fixture_demo_9'",
                [value.to_string()],
            )
            .unwrap();
    };
    save(&saved);
    let request = |backend: &mut Backend, id: &str, method: &str, params: Value| {
        backend.send(json!({"type":"request","id":id,"method":method,"params":params}));
        backend.wait_for(|value| value["id"] == id, Duration::from_secs(5))
    };
    let key = "drafts.github:fixture/demo/9";
    let params = json!({"target":fixture.target,"findingId":"f1","publicationMode":"inline"});
    let preview = request(
        &mut fixture.backend,
        "preview",
        "draft.prepare",
        params.clone(),
    );
    assert_eq!(preview["ok"], true, "{preview}");
    let prepared = &preview["result"];
    assert_eq!(prepared["draft"]["to"], 1);
    assert_eq!(prepared["draft"]["from"], Value::Null);
    assert_eq!(
        prepared["draft"]["findingRef"],
        json!({"reviewRunId":"run-fix","findingId":"f1","findingFingerprint":"fp1"})
    );
    assert!(prepared["draft"]["raw"]
        .as_str()
        .unwrap()
        .contains("世界\r\nSecond line."));
    let before = request(
        &mut fixture.backend,
        "before",
        "workspace.read",
        json!({"key":key}),
    );
    assert_eq!(before["result"]["available"], false, "{before}");
    let mut stage = params.clone();
    stage["expectedVersion"] = prepared["recordVersion"].clone();
    stage["previewHash"] = json!("0".repeat(64));
    let refused = request(
        &mut fixture.backend,
        "bad-hash",
        "draft.stage",
        stage.clone(),
    );
    assert_eq!(refused["error"]["code"], "conflict", "{refused}");
    stage["previewHash"] = prepared["previewHash"].clone();
    let original_summary = saved["reviewRuns"][0]["findings"][0]["summary"].clone();
    saved["reviewRuns"][0]["findings"][0]["summary"] = json!("Changed after preview");
    save(&saved);
    let changed_review = request(
        &mut fixture.backend,
        "changed-review",
        "draft.stage",
        stage.clone(),
    );
    assert_eq!(changed_review["error"]["code"], "conflict");
    saved["reviewRuns"][0]["findings"][0]["summary"] = original_summary;
    save(&saved);
    let staged = request(&mut fixture.backend, "stage", "draft.stage", stage.clone());
    assert_eq!(staged["ok"], true, "{staged}");
    assert_eq!(staged["result"]["changed"], true);
    let duplicate_stale = request(
        &mut fixture.backend,
        "duplicate-stale",
        "draft.stage",
        stage,
    );
    assert_eq!(duplicate_stale["error"]["code"], "conflict");
    fixture.backend = Backend::spawn_env(
        fixture.repo.path(),
        &[
            ("NORN_CONFIG_DIR", fixture._config.path().to_str().unwrap()),
            (
                "NORN_REVIEW_DATA_DIR",
                fixture._data.path().to_str().unwrap(),
            ),
            ("NORN_DATA_DIR", fixture._data.path().to_str().unwrap()),
        ],
    );
    fixture.backend.handshake();
    let restored = request(
        &mut fixture.backend,
        "restored",
        "workspace.read",
        json!({"key":key}),
    );
    let mut edited = restored["result"]["payload"]["value"][0].clone();
    assert_eq!(edited["localId"], staged["result"]["localId"]);
    assert_eq!(edited["raw"], prepared["draft"]["raw"]);
    edited["raw"] = json!("Reviewer edit 世界");
    edited["publicationMode"] = json!("general");
    let updated = request(
        &mut fixture.backend,
        "edit",
        "workspace.mutate",
        json!({"key":key,"expectedVersion":restored["result"]["recordVersion"],"mutation":{"kind":"putDraft","draft":edited}}),
    );
    assert_eq!(updated["ok"], true, "{updated}");
    let again = request(
        &mut fixture.backend,
        "again",
        "draft.prepare",
        params.clone(),
    );
    assert_eq!(again["result"]["alreadyStaged"], true);
    assert_eq!(again["result"]["draft"], edited);
    let mut duplicate = params.clone();
    duplicate["expectedVersion"] = again["result"]["recordVersion"].clone();
    duplicate["previewHash"] = again["result"]["previewHash"].clone();
    let acknowledged = request(&mut fixture.backend, "duplicate", "draft.stage", duplicate);
    assert_eq!(acknowledged["ok"], true, "{acknowledged}");
    assert_eq!(acknowledged["result"]["changed"], false);
    let after = request(
        &mut fixture.backend,
        "after",
        "workspace.read",
        json!({"key":key}),
    );
    assert_eq!(after["result"]["payload"]["value"], json!([edited]));
    let mut wrong = params.clone();
    wrong["target"]["provider"] = json!("bitbucket");
    assert_eq!(
        request(
            &mut fixture.backend,
            "wrong-provider",
            "draft.prepare",
            wrong
        )["error"]["code"],
        "targetStale"
    );
    let mut missing = params.clone();
    missing["target"]["runId"] = json!("missing");
    assert_eq!(
        request(
            &mut fixture.backend,
            "missing-run",
            "draft.prepare",
            missing
        )["error"]["code"],
        "notFound"
    );
    saved["reviewRuns"][0]["findings"][0]["anchor"] = Value::Null;
    save(&saved);
    assert_eq!(
        request(
            &mut fixture.backend,
            "unanchored",
            "draft.prepare",
            params.clone()
        )["error"]["code"],
        "invalidParams"
    );
    saved["reviewRuns"][0]["findings"][0]["status"] = json!("published");
    save(&saved);
    assert_eq!(
        request(&mut fixture.backend, "published", "draft.prepare", params)["error"]["code"],
        "conflict"
    );
    assert!(!fixture._agent.path().join("fake-agent.executed").exists());
}

#[cfg(unix)]
#[test]
fn workspace_readiness_never_executes_found_tools_or_reads_credentials() {
    use std::os::unix::fs::PermissionsExt;
    let repo = tempfile::tempdir().unwrap();
    let tools = tempfile::tempdir().unwrap();
    let marker = tools.path().join("executed");
    let executable = tools.path().join("opencode");
    std::fs::write(
        &executable,
        format!("#!/bin/sh\nprintf executed > '{}'\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut backend = Backend::spawn_env(repo.path(), &[("PATH", tools.path().to_str().unwrap())]);
    backend.handshake();
    backend.send(
        json!({"type":"request","id":"ready-status","method":"workspace.readiness","params":{}}),
    );
    let response = backend.wait_for(
        |value| value["id"] == "ready-status",
        Duration::from_secs(10),
    );
    assert_eq!(response["ok"], true, "{response}");
    let result = &response["result"];
    assert_eq!(result["configuredRepos"], 0);
    assert!(result["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "opencode" && tool["available"] == true));
    assert!(result.get("credentials").is_none());
    assert!(result.get("token").is_none());
    assert!(!marker.exists());
    assert!(!response.to_string().contains("executed"));
    backend.send(json!({"type":"request","id":"done","method":"shutdown","params":{}}));
    backend.wait_for(|value| value["id"] == "done", Duration::from_secs(10));
    assert!(backend.child.wait().unwrap().success());
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

#[test]
fn canonical_records_stdio_preserve_scopes_and_reject_stale_mutations() {
    let repo = temp_repo();
    let mut backend = Backend::spawn(repo.path());
    backend.handshake();
    let key = "prompt.github:owner/repo";
    backend
        .send(json!({"type":"request","id":"read","method":"workspace.read","params":{"key":key}}));
    let before = backend.wait_for(|v| v["id"] == "read", Duration::from_secs(5));
    assert_eq!(before["ok"], true, "{before}");
    assert_eq!(before["result"]["available"], false);
    let version = before["result"]["recordVersion"].as_str().unwrap();
    backend.send(json!({"type":"request","id":"save","method":"workspace.mutate","params":{"key":key,"expectedVersion":version,"mutation":{"kind":"setPrompt","value":"Saved ✓"}}}));
    let saved = backend.wait_for(|v| v["id"] == "save", Duration::from_secs(5));
    assert_eq!(saved["ok"], true, "{saved}");
    backend.send(json!({"type":"request","id":"stale","method":"workspace.mutate","params":{"key":key,"expectedVersion":version,"mutation":{"kind":"setPrompt","value":"Overwritten"}}}));
    assert_eq!(
        backend.wait_for(|v| v["id"] == "stale", Duration::from_secs(5))["ok"],
        false
    );
    backend.send(
        json!({"type":"request","id":"after","method":"workspace.read","params":{"key":key}}),
    );
    assert_eq!(
        backend.wait_for(|v| v["id"] == "after", Duration::from_secs(5))["result"]["payload"]
            ["value"],
        "Saved ✓"
    );
    backend.send(json!({"type":"request","id":"other","method":"workspace.read","params":{"key":"prompt.bitbucket:owner/repo"}}));
    assert_eq!(
        backend.wait_for(|v| v["id"] == "other", Duration::from_secs(5))["result"]["available"],
        false
    );
}

#[test]
fn staged_drafts_survive_backend_restart_preserve_finding_metadata_and_fence_cas() {
    let repo = temp_repo();
    let data = tempfile::tempdir().unwrap();
    let data_path = data.path().to_str().unwrap();
    let key = "drafts.github:owner/repo/7";
    let request = |backend: &mut Backend, id: &str, method: &str, params: Value| {
        backend.send(json!({"type":"request","id":id,"method":method,"params":params}));
        backend.wait_for(|v| v["id"] == id, Duration::from_secs(5))
    };
    let mut backend = Backend::spawn_env(repo.path(), &[("NORN_DATA_DIR", data_path)]);
    backend.handshake();
    let before = request(
        &mut backend,
        "initial",
        "workspace.read",
        json!({"key":key}),
    );
    assert_eq!(before["ok"], true, "{before}");
    let imported = json!({"localId":"imported", "prId":7,"path":"src/test.ts","to":12,"from":null,
        "raw":"Imported comment 世界\nSecond line.","parentId":"19","createdAt":1,
        "source":"aiFinding","publicationMode":"inline","findingRef":{"reviewRunId":"run-1","findingId":"finding-1","findingFingerprint":"fingerprint-1"},
        "reviewBaseSha":"a".repeat(40),"reviewHeadSha":"b".repeat(40)});
    let saved = request(
        &mut backend,
        "seed",
        "workspace.mutate",
        json!({"key":key,"expectedVersion":before["result"]["recordVersion"],"mutation":{"kind":"putDraft","draft":imported}}),
    );
    assert_eq!(saved["ok"], true, "{saved}");
    drop(backend);
    let mut backend = Backend::spawn_env(repo.path(), &[("NORN_DATA_DIR", data_path)]);
    backend.handshake();
    let restored = request(
        &mut backend,
        "restored",
        "workspace.read",
        json!({"key":key}),
    );
    assert_eq!(
        restored["result"]["payload"]["value"],
        json!([imported]),
        "{restored}"
    );
    let mut edited = imported.clone();
    edited["raw"] = json!("Explicit reviewer edit 世界");
    let changed = request(
        &mut backend,
        "edit",
        "workspace.mutate",
        json!({"key":key,"expectedVersion":restored["result"]["recordVersion"],"mutation":{"kind":"putDraft","draft":edited}}),
    );
    assert_eq!(changed["ok"], true, "{changed}");
    let stale = request(
        &mut backend,
        "stale",
        "workspace.mutate",
        json!({"key":key,"expectedVersion":restored["result"]["recordVersion"],"mutation":{"kind":"putDraft","draft":imported}}),
    );
    assert_eq!(stale["ok"], false, "{stale}");
    let after = request(&mut backend, "after", "workspace.read", json!({"key":key}));
    assert_eq!(after["result"]["payload"]["value"], json!([edited]));
    let other = request(
        &mut backend,
        "other",
        "workspace.read",
        json!({"key":"drafts.bitbucket:owner/repo/7"}),
    );
    assert_eq!(other["result"]["available"], false);
    let mut wrong_pr = edited.clone();
    wrong_pr["prId"] = json!(8);
    let rejected = request(
        &mut backend,
        "wrong-pr",
        "workspace.mutate",
        json!({"key":key,"expectedVersion":after["result"]["recordVersion"],"mutation":{"kind":"putDraft","draft":wrong_pr}}),
    );
    assert_eq!(rejected["ok"], false);
    let removed = request(
        &mut backend,
        "remove",
        "workspace.mutate",
        json!({"key":key,"expectedVersion":after["result"]["recordVersion"],"mutation":{"kind":"removeDraft","localId":"imported"}}),
    );
    assert_eq!(removed["ok"], true, "{removed}");
    drop(backend);
    let mut backend = Backend::spawn_env(repo.path(), &[("NORN_DATA_DIR", data_path)]);
    backend.handshake();
    let empty = request(&mut backend, "empty", "workspace.read", json!({"key":key}));
    assert_eq!(empty["result"]["payload"]["value"], json!([]));
}

#[cfg(unix)]
#[test]
fn inline_prompt_uses_selected_policy_and_canonical_override_precedence() {
    use std::os::unix::fs::PermissionsExt;
    for (policy, user, expected, forbidden) in [
        (
            "version: 0.1\nreview:\n  prompt:\n    replace: SELECTED_REPLACE\n",
            None,
            "SELECTED_REPLACE",
            "## Repository review policy",
        ),
        (
            "version: 0.1\nreview:\n  prompt:\n    extend: SELECTED_EXTEND\n",
            None,
            "SELECTED_EXTEND",
            "OTHER_PROVIDER",
        ),
        (
            "version: 0.1\nreview:\n  prompt:\n    replace: SELECTED_REPLACE\n",
            Some("USER_OVERRIDE"),
            "USER_OVERRIDE",
            "SELECTED_REPLACE",
        ),
        (
            "version: 0.1\nreview:\n  prompt:\n    replace: SELECTED_REPLACE\n",
            Some(""),
            "SELECTED_REPLACE",
            "OTHER_PROVIDER",
        ),
        (
            "invalid: [\n",
            Some("USER_OVERRIDE"),
            "USER_OVERRIDE",
            "SELECTED_REPLACE",
        ),
    ] {
        let repo = temp_repo();
        std::fs::write(repo.path().join("a.txt"), "before\n").unwrap();
        git(repo.path(), &["add", "a.txt"]);
        git(repo.path(), &["commit", "-qm", "init"]);
        std::fs::write(repo.path().join("a.txt"), "after\n").unwrap();
        std::fs::write(repo.path().join(".norn.yaml"), policy).unwrap();
        git(repo.path(), &["add", ".norn.yaml"]);
        git(repo.path(), &["commit", "-qm", "synthetic policy"]);
        let storage = tempfile::tempdir().unwrap();
        let settings = json!({"repos":[{"provider":"github","workspace":"fixture","repo":"demo","localPath":repo.path().canonicalize().unwrap()}],"defaultDiffView":"unified","theme":"dark"});
        std::fs::write(storage.path().join("settings.json"), settings.to_string()).unwrap();
        let mut records = vec![
            json!({"key":"prompt.bitbucket:fixture/demo","payload":{"kind":"prompt","value":"OTHER_PROVIDER"}}),
        ];
        if let Some(user) = user {
            records.push(json!({"key":"prompt.github:fixture/demo","payload":{"kind":"prompt","value":user}}));
        }
        std::fs::write(
            storage.path().join("workspace-data-v1.json"),
            json!({"schemaVersion":1,"records":records}).to_string(),
        )
        .unwrap();
        let binary = storage.path().join("fake-agent");
        let capture = storage.path().join("captured-prompt");
        std::fs::write(&binary,"#!/bin/sh\ncat > \"$NORN_FAKE_PROMPT_CAPTURE\"\nprintf '%s\\n' '{\"result\":\"No review findings.\",\"session_id\":\"prompt-fixture\"}'\n").unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut backend = Backend::spawn_env(
            repo.path(),
            &[
                ("NORN_REVIEW_ENGINE", "real"),
                ("NORN_CONFIG_DIR", storage.path().to_str().unwrap()),
                ("NORN_DATA_DIR", storage.path().to_str().unwrap()),
                ("NORN_REVIEW_AGENT_BIN", binary.to_str().unwrap()),
                ("NORN_FAKE_PROMPT_CAPTURE", capture.to_str().unwrap()),
            ],
        );
        backend.handshake();
        backend.send(json!({"type":"request","id":"start","method":"review.start","params":{"target":{"kind":"local","localSnapshotSha256":LOCAL_SHA}}}));
        let started = backend.wait_for(|v| v["id"] == "start", Duration::from_secs(5));
        assert_eq!(started["ok"], true, "{started}");
        let operation = started["result"]["operationId"].as_str().unwrap();
        let terminal = backend.wait_for(
            |v| {
                v["operationId"] == operation
                    && v["event"]["kind"] == "state"
                    && ["succeeded", "failed"].contains(&v["event"]["state"].as_str().unwrap_or(""))
            },
            Duration::from_secs(15),
        );
        assert_eq!(terminal["event"]["state"], "succeeded", "{terminal}");
        let payload = std::fs::read_to_string(capture).unwrap();
        assert!(
            payload.contains(expected),
            "Expected synthetic marker {expected} absent: {payload}"
        );
        assert!(
            !payload.contains(forbidden),
            "Unrelated prompt marker leaked"
        );
    }
}

#[cfg(unix)]
#[test]
fn reply_receives_saved_prompt_and_invalid_policy_stops_before_agent() {
    for override_prompt in [None, Some("REPLY_USER_OVERRIDE")] {
        let mut fixture = FixFixture::new("#!/bin/sh\ncat > captured-reply-payload\nprintf '%s\\n' '{\"result\":\"Reply fixture completed.\",\"session_id\":\"reply-session\"}'\n");
        let capture = fixture._agent.path().join("captured-reply-payload");
        std::fs::write(fixture._agent.path().join("fake-agent"), format!("#!/bin/sh\ncat > '{}'\nprintf '%s\\n' '{{\"result\":\"Reply fixture completed.\",\"session_id\":\"reply-session\"}}'\n", capture.display())).unwrap();
        std::fs::write(
            fixture.repo.path().join(".norn.yaml"),
            "version: 0.1\nreview:\n  prompt:\n    replace: REPLY_POLICY\n",
        )
        .unwrap();
        // Fallback/fresh-thread replies resend policy; Claude resume sends only
        // the new message and retains its earlier conversation, matching desktop.
        let connection =
            rusqlite::Connection::open(fixture._data.path().join("norn.sqlite3")).unwrap();
        let stored: String = connection
            .query_row(
                "SELECT store_json FROM ai_review_stores WHERE review_key='fixture_demo_9'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut stored: Value = serde_json::from_str(&stored).unwrap();
        stored["threads"][0]["claudeSessionId"] = Value::Null;
        connection
            .execute(
                "UPDATE ai_review_stores SET store_json=?1 WHERE review_key='fixture_demo_9'",
                [stored.to_string()],
            )
            .unwrap();
        let root = fixture.backend._isolated.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let selected_reference = json!({"id":"selected-ref","type":"note","source":"manual","body":"REPLY_REFERENCE_CONTEXT 世界 ````", "createdAt":1,"updatedAt":2});
        let wrong_reference = json!({"id":"other-ref","type":"note","source":"manual","body":"WRONG_PROVIDER_REFERENCE", "createdAt":1,"updatedAt":2});
        let mut records = vec![
            json!({"key":"references.github:fixture/demo/9","payload":{"kind":"references","value":[selected_reference]}}),
            json!({"key":"references.bitbucket:fixture/demo/9","payload":{"kind":"references","value":[wrong_reference]}}),
        ];
        if let Some(prompt) = override_prompt {
            records.push(json!({"key":"prompt.github:fixture/demo","payload":{"kind":"prompt","value":prompt}}));
        }
        std::fs::write(
            root.join("workspace-data-v1.json"),
            json!({"schemaVersion":1,"records":records}).to_string(),
        )
        .unwrap();
        fixture.backend.send(json!({"type":"request","id":"reply-prompt","method":"review.reply","params":{"target":fixture.target,"message":"Please explain this finding."}}));
        let started = fixture
            .backend
            .wait_for(|v| v["id"] == "reply-prompt", Duration::from_secs(5));
        assert_eq!(started["ok"], true, "{started}");
        let operation = started["result"]["operationId"].as_str().unwrap();
        assert_eq!(fixture.terminal(operation)["event"]["state"], "succeeded");
        let payload = std::fs::read_to_string(capture).unwrap();
        assert!(payload.contains(override_prompt.unwrap_or("REPLY_POLICY")));
        assert!(payload.contains("REPLY_REFERENCE_CONTEXT 世界"));
        assert!(payload.contains("`````json"));
        assert!(!payload.contains("WRONG_PROVIDER_REFERENCE"));
        if override_prompt.is_some() {
            assert!(!payload.contains("REPLY_POLICY"));
        }
    }
    let mut invalid = FixFixture::new("#!/bin/sh\nprintf up > unexpected-agent\nexit 99\n");
    let unexpected = invalid._agent.path().join("unexpected-agent");
    std::fs::write(
        invalid._agent.path().join("fake-agent"),
        format!(
            "#!/bin/sh\nprintf up > '{}'\nexit 99\n",
            unexpected.display()
        ),
    )
    .unwrap();
    std::fs::write(invalid.repo.path().join(".norn.yaml"), "invalid: [\n").unwrap();
    invalid.backend.send(json!({"type":"request","id":"invalid-policy","method":"review.reply","params":{"target":invalid.target,"message":"Reply"}}));
    let started = invalid
        .backend
        .wait_for(|v| v["id"] == "invalid-policy", Duration::from_secs(5));
    assert_eq!(started["ok"], true, "{started}");
    let operation = started["result"]["operationId"].as_str().unwrap();
    let terminal = invalid.terminal(operation);
    assert_eq!(terminal["event"]["state"], "failed");
    assert_eq!(terminal["event"]["error"]["code"], "invalidParams");
    assert!(!unexpected.exists());
}

#[cfg(unix)]
#[test]
fn new_inline_review_consumes_scoped_references_and_refuses_oversize_before_agent() {
    for oversized in [false, true] {
        let fixture = FixFixture::new("#!/bin/sh\ncat > /dev/null\nexit 99\n");
        let capture = fixture._agent.path().join("reference-payload");
        let binary = fixture._agent.path().join("fake-agent");
        std::fs::write(&binary, format!("#!/bin/sh\ncat > '{}'\nprintf '%s\\n' '{{\"result\":\"Reference fixture complete.\",\"session_id\":\"reference-session\"}}'\n", capture.display())).unwrap();
        let mut backend = Backend::spawn_env(
            fixture.repo.path(),
            &[
                ("NORN_REVIEW_ENGINE", "real"),
                ("NORN_CONFIG_DIR", fixture._config.path().to_str().unwrap()),
                (
                    "NORN_REVIEW_DATA_DIR",
                    fixture._data.path().to_str().unwrap(),
                ),
                ("NORN_REVIEW_AGENT_BIN", binary.to_str().unwrap()),
            ],
        );
        backend.handshake();
        let root = backend._isolated.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let body = if oversized {
            "x".repeat(65536)
        } else {
            "START_REFERENCE_CONTEXT 世界 ````".into()
        };
        let references = (0..if oversized {5} else {1}).map(|index| json!({"id":format!("ref-{index}"),"type":"note","source":"manual","body":body,"createdAt":1,"updatedAt":2})).collect::<Vec<_>>();
        std::fs::write(root.join("workspace-data-v1.json"), json!({"schemaVersion":1,"records":[
            {"key":"references.github:fixture/demo/9","payload":{"kind":"references","value":references}},
            {"key":"references.bitbucket:fixture/demo/9","payload":{"kind":"references","value":[{"id":"wrong","type":"note","source":"manual","body":"WRONG_SCOPE_REFERENCE","createdAt":1,"updatedAt":2}]}},
            {"key":"references.github:fixture/demo/10","payload":{"kind":"references","value":[{"id":"other-pr","type":"note","source":"manual","body":"OTHER_PR_REFERENCE","createdAt":1,"updatedAt":2}]}}
        ]}).to_string()).unwrap();
        backend.send(json!({"type":"request","id":"reference-start","method":"review.start","params":{"target":fixture.target}}));
        let started = backend.wait_for(|v| v["id"] == "reference-start", Duration::from_secs(5));
        assert_eq!(started["ok"], true, "{started}");
        let operation = started["result"]["operationId"].as_str().unwrap();
        let terminal = backend.wait_for(
            |v| {
                v["operationId"] == operation
                    && v["event"]["kind"] == "state"
                    && ["succeeded", "failed"].contains(&v["event"]["state"].as_str().unwrap_or(""))
            },
            Duration::from_secs(15),
        );
        if oversized {
            assert_eq!(terminal["event"]["state"], "failed", "{terminal}");
            assert_eq!(terminal["event"]["error"]["code"], "invalidParams");
            assert!(terminal["event"]["error"]["message"]
                .as_str()
                .unwrap()
                .contains("256 KiB"));
            assert!(!capture.exists());
        } else {
            assert_eq!(terminal["event"]["state"], "succeeded", "{terminal}");
            let payload = std::fs::read_to_string(&capture).unwrap();
            assert!(payload.contains("START_REFERENCE_CONTEXT 世界"));
            assert!(payload.contains("`````json"));
            assert!(
                !payload.contains("WRONG_SCOPE_REFERENCE")
                    && !payload.contains("OTHER_PR_REFERENCE")
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn resumed_reply_keeps_prior_conversation_instead_of_resending_new_reference_context() {
    let mut fixture = FixFixture::new("#!/bin/sh\ncat > /dev/null\nexit 99\n");
    let capture = fixture._agent.path().join("resumed-reference-payload");
    std::fs::write(fixture._agent.path().join("fake-agent"), format!("#!/bin/sh\ncat > '{}'\nprintf '%s\\n' '{{\"result\":\"Resumed fixture complete.\",\"session_id\":\"session-fix\"}}'\n", capture.display())).unwrap();
    let root = fixture.backend._isolated.path().join("workspace");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("workspace-data-v1.json"), json!({"schemaVersion":1,"records":[{"key":"references.github:fixture/demo/9","payload":{"kind":"references","value":[{"id":"new-context","type":"note","source":"manual","body":"NEW_REFERENCE_CONTEXT_NOT_RESENT","createdAt":1,"updatedAt":2}]}}]}).to_string()).unwrap();
    fixture.backend.send(json!({"type":"request","id":"resumed-ref-reply","method":"review.reply","params":{"target":fixture.target,"message":"Please clarify the prior finding."}}));
    let started = fixture
        .backend
        .wait_for(|v| v["id"] == "resumed-ref-reply", Duration::from_secs(5));
    assert_eq!(started["ok"], true, "{started}");
    let operation = started["result"]["operationId"].as_str().unwrap();
    assert_eq!(fixture.terminal(operation)["event"]["state"], "succeeded");
    let payload = std::fs::read_to_string(capture).unwrap();
    assert!(payload.contains("Please clarify the prior finding."));
    assert!(!payload.contains("NEW_REFERENCE_CONTEXT_NOT_RESENT"));
}

#[test]
fn publication_control_and_invalid_approvals_never_access_provider_credentials() {
    let repository = temp_repo();
    let mut backend = Backend::spawn(repository.path());
    backend.handshake();
    let scope = json!({"provider":"github","workspace":"synthetic","repo":"review","prId":7});
    for (id, method, params, success) in [
        (
            "publication-status",
            "publication.status",
            json!({"scope":scope,"after":null,"limit":20}),
            true,
        ),
        (
            "publication-empty",
            "publication.prepare",
            json!({"scope":scope,"localIds":[]}),
            false,
        ),
        (
            "publication-fake",
            "publication.apply",
            json!({"scope":scope,"approvalId":"a".repeat(64),"localId":"draft"}),
            false,
        ),
        (
            "publication-cancel",
            "publication.cancel",
            json!({"scope":scope,"approvalId":"a".repeat(64)}),
            true,
        ),
        (
            "publication-missing",
            "publication.reconcile",
            json!({"scope":scope,"localId":"missing"}),
            false,
        ),
        (
            "comments-unbounded",
            "comments.list",
            json!({"scope":scope,"offset":0,"limit":101,"version":null}),
            false,
        ),
    ] {
        backend.send(json!({"type":"request","id":id,"method":method,"params":params}));
        let response = backend.wait_for(|frame| frame["id"] == id, Duration::from_secs(5));
        assert_eq!(response["ok"], success, "{id}");
        if id == "publication-status" {
            assert_eq!(response["result"]["entries"], json!([]));
        }
    }
    backend.send(
        json!({"type":"request","id":"still-responsive","method":"operation.status","params":{}}),
    );
    assert_eq!(
        backend.wait_for(
            |frame| frame["id"] == "still-responsive",
            Duration::from_secs(5)
        )["ok"],
        false
    );
}

#[test]
fn canonical_scope_inventory_is_offline_paged_and_reachable_after_restart() {
    let repo = temp_repo();
    let data = tempfile::tempdir().unwrap();
    let path = data.path().join("workspace-data-v1.json");
    let original = json!({"schemaVersion":1,"records":[
        {"key":"viewed.github:imported/repo/7","payload":{"kind":"viewed","value":["owned-private-path.rs"]}},
        {"key":"viewed.bitbucket:imported/repo/7","payload":{"kind":"viewed","value":["owned-bitbucket.rs"]}}
    ]});
    std::fs::write(&path, original.to_string()).unwrap();
    let mut backend = Backend::spawn_env(
        repo.path(),
        &[("NORN_DATA_DIR", data.path().to_str().unwrap())],
    );
    backend.handshake();
    backend.send(
        json!({"type":"request","id":"scopes","method":"workspace.scopes","params":{"limit":1}}),
    );
    let first = backend.wait_for(|value| value["id"] == "scopes", Duration::from_secs(5));
    assert_eq!(first["ok"], true, "{first}");
    assert_eq!(first["result"]["totalScopes"], 2);
    assert_eq!(first["result"]["scopes"][0]["provider"], "bitbucket");
    assert!(!first.to_string().contains("owned-private-path"));
    let version = first["result"]["inventoryVersion"].clone();
    std::fs::write(&path, json!({"schemaVersion":1,"records":[{"key":"viewed.github:imported/repo/7","payload":{"kind":"viewed","value":["after-edit.rs"]}}]}).to_string()).unwrap();
    backend.send(json!({"type":"request","id":"second","method":"workspace.scopes","params":{"offset":1,"limit":1,"inventoryVersion":version}}));
    let second = backend.wait_for(|value| value["id"] == "second", Duration::from_secs(5));
    assert_eq!(second["ok"], true, "{second}");
    assert_eq!(second["result"]["totalScopes"], 2);
    assert_eq!(second["result"]["scopes"][0]["provider"], "github");
    backend.send(json!({"type":"request","id":"local-refuse","method":"review.start","params":{"target":{"kind":"provider","provider":"github","workspace":"imported","repo":"repo","prId":2147483649u32}}}));
    let refused = backend.wait_for(
        |value| value["id"] == "local-refuse",
        Duration::from_secs(5),
    );
    assert_eq!(refused["ok"], false, "{refused}");
    assert_eq!(refused["error"]["code"], "invalidParams");
    drop(backend);
    let mut backend = Backend::spawn_env(
        repo.path(),
        &[("NORN_DATA_DIR", data.path().to_str().unwrap())],
    );
    backend.handshake();
    backend.send(json!({"type":"request","id":"expired","method":"workspace.scopes","params":{"offset":1,"inventoryVersion":version}}));
    let expired = backend.wait_for(|value| value["id"] == "expired", Duration::from_secs(5));
    assert_eq!(expired["ok"], false, "{expired}");
    backend.send(json!({"type":"request","id":"refresh","method":"workspace.scopes","params":{}}));
    let refreshed = backend.wait_for(|value| value["id"] == "refresh", Duration::from_secs(5));
    assert_eq!(refreshed["result"]["totalScopes"], 1);
    backend.send(json!({"type":"request","id":"content","method":"workspace.read","params":{"key":"viewed.github:imported/repo/7"}}));
    let content = backend.wait_for(|value| value["id"] == "content", Duration::from_secs(5));
    assert_eq!(content["result"]["payload"]["value"][0], "after-edit.rs");
}

#[test]
#[cfg(unix)]
fn saved_targets_resolve_run_and_provider_without_configuration_guessing() {
    let mut fixture = FixFixture::new("#!/bin/sh\nexit 99\n");
    fixture
        .backend
        .send(json!({"type":"request","id":"targets-real","method":"review.targets","params":{}}));
    let targets = fixture.backend.wait_for(
        |value| value["id"] == "targets-real",
        Duration::from_secs(5),
    );
    assert_eq!(targets["result"]["targets"][0]["runId"], "run-fix");
    assert_eq!(targets["result"]["targets"][0]["provider"], "github");
    let connection = rusqlite::Connection::open(fixture._data.path().join("norn.sqlite3")).unwrap();
    let stored: String = connection
        .query_row(
            "SELECT store_json FROM ai_review_stores WHERE review_key='fixture_demo_9'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut stored: Value = serde_json::from_str(&stored).unwrap();
    stored["reviewRuns"][0]["provider"] = json!("bitbucket");
    connection
        .execute(
            "UPDATE ai_review_stores SET store_json=?1 WHERE review_key='fixture_demo_9'",
            [stored.to_string()],
        )
        .unwrap();
    fixture.backend.send(
        json!({"type":"request","id":"targets-provider","method":"review.targets","params":{}}),
    );
    let targets = fixture.backend.wait_for(
        |value| value["id"] == "targets-provider",
        Duration::from_secs(5),
    );
    assert_eq!(targets["result"]["targets"][0]["provider"], "bitbucket");
    assert_eq!(targets["result"]["targets"][0]["runId"], "run-fix");
    for (index, method) in ["review.history", "review.findings", "review.threads"]
        .into_iter()
        .enumerate()
    {
        let id = format!("wrong-provider-{index}");
        fixture.backend.send(
            json!({"type":"request","id":id,"method":method,"params":{"target":fixture.target}}),
        );
        let response = fixture
            .backend
            .wait_for(|value| value["id"] == id, Duration::from_secs(5));
        assert_eq!(response["ok"], false, "{response}");
        assert_eq!(response["error"]["code"], "targetStale");
    }
}
