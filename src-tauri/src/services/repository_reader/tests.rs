use super::find::FindSource;
use super::*;
use std::process::Command;
fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "owned git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn repo() -> tempfile::TempDir {
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    git(root.path(), &["init"]);
    git(root.path(), &["config", "user.name", "Owned Author"]);
    git(
        root.path(),
        &["config", "user.email", "owned@example.invalid"],
    );
    root
}
fn commit(root: &Path, message: &str) {
    git(root, &["add", "--all"]);
    git(
        root,
        &["-c", "commit.gpgsign=false", "commit", "-m", message],
    );
}
#[test]
fn inventory_and_diff_retain_status_and_literal_paths() {
    let root = repo();
    let root = root.path();
    for path in [
        "unchanged.txt",
        "deleted.txt",
        "rename old.txt",
        "modified.txt",
        "--option ü.txt",
    ] {
        std::fs::write(root.join(path), format!("owned distinct content {path}\n")).unwrap();
    }
    commit(root, "owned base");
    std::fs::remove_file(root.join("deleted.txt")).unwrap();
    git(root, &["mv", "--", "rename old.txt", "rename ü new.txt"]);
    std::fs::write(root.join("modified.txt"), "changed\n").unwrap();
    std::fs::write(root.join("untracked.txt"), "untracked\n").unwrap();
    std::fs::write(root.join("added.txt"), "added\n").unwrap();
    git(root, &["add", "added.txt"]);
    std::fs::write(root.join("--option ü.txt"), "literal option change\n").unwrap();
    let files = inventory::collect(root, &control()).unwrap();
    let find = |path: &str| files.files.iter().find(|file| file.path == path).unwrap();
    assert_eq!(
        find("unchanged.txt").status,
        RepositoryFileStatus::Unchanged
    );
    assert_eq!(find("deleted.txt").status, RepositoryFileStatus::Deleted);
    assert_eq!(
        find("rename ü new.txt").status,
        RepositoryFileStatus::Renamed
    );
    assert_eq!(
        find("rename ü new.txt").old_path.as_deref(),
        Some("rename old.txt")
    );
    assert_eq!(find("modified.txt").status, RepositoryFileStatus::Modified);
    assert_eq!(find("added.txt").status, RepositoryFileStatus::Added);
    assert_eq!(
        find("untracked.txt").status,
        RepositoryFileStatus::Untracked
    );
    assert!(diff::read_at(root, "--option ü.txt", &control())
        .unwrap()
        .0
        .contains("+literal option change"));
    let untracked = diff::read_at(root, "untracked.txt", &control()).unwrap().0;
    assert!(untracked.contains("+untracked"));
    assert!(!untracked.contains(&root.to_string_lossy().to_string()));
    assert!(diff::read_at(root, "deleted.txt", &control())
        .unwrap()
        .0
        .contains("deleted file mode"));
    assert!(diff::read_at(root, ".git/config", &control()).is_err());
    let old = files.version;
    git(root, &["add", "modified.txt"]);
    assert_ne!(old, inventory::collect(root, &control()).unwrap().version);
}
#[test]
fn unborn_and_binary_size_and_utf8_states_are_explicit() {
    let root = repo();
    let root = root.path();
    std::fs::write(root.join("text.txt"), "é🙂\n").unwrap();
    assert_eq!(
        content::read_at(root, "text.txt", &control())
            .unwrap()
            .state,
        ContentState::Text
    );
    assert!(diff::read_at(root, "text.txt", &control())
        .unwrap()
        .0
        .contains("+é🙂"));
    assert!(git::head(root, &control()).unwrap().is_none());
    for (path, bytes) in [
        ("binary", vec![0, 1]),
        ("bad-utf8", vec![255]),
        ("large", vec![b'x'; MAX_FILE_BYTES + 1]),
    ] {
        std::fs::write(root.join(path), bytes).unwrap();
        let document = content::read_at(root, path, &control()).unwrap();
        assert_eq!(
            document.state,
            if path == "large" {
                ContentState::TooLarge
            } else {
                ContentState::Binary
            }
        );
        assert!(document.text.is_empty());
    }
    assert_eq!(
        content::read_at(root, "missing", &control()).unwrap().state,
        ContentState::Missing
    );
}
#[test]
fn blame_retains_author_and_complete_message_even_with_legacy_marker() {
    let root = repo();
    let root = root.path();
    std::fs::write(root.join("owned.txt"), "first\nsecond\nthird\n").unwrap();
    let message = format!(
        "Owned title\n\nbody {} literal\nlast paragraph",
        legacy::GIT_SHOW_MESSAGE_MARKER
    );
    commit(root, &message);
    let lines = blame::read_lines(root, "owned.txt", 1, 3, &control()).unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].line, 2);
    assert_eq!(lines[0].author.as_deref(), Some("Owned Author"));
    assert_eq!(
        lines[0].author_email.as_deref(),
        Some("owned@example.invalid")
    );
    assert!(lines[0].author_time.is_some());
    assert_eq!(lines[0].message.as_deref(), Some(message.as_str()));
    assert!(lines[0].sha.len() >= 40);
}
#[test]
fn find_maps_unicode_expansion_and_utf16_and_nonoverlapping_pages() {
    let text = "🙂 İTest\nTEST test test";
    let (total, page) = find::matches(text, "test", 1, 2, &control()).unwrap();
    assert_eq!(total, 4);
    assert_eq!(page.len(), 2);
    assert_eq!(
        page[0],
        find::Match {
            start_byte: 12,
            end_byte: 16,
            start_utf16: 9,
            end_utf16: 13,
            line: 2,
            column_utf16: 0
        }
    );
    let (total, page) = find::matches("aaaaa", "aa", 0, 32, &control()).unwrap();
    assert_eq!(total, 2);
    assert_eq!(page[1].start_utf16, 2);
    let (_, page) = find::matches("İ", "i", 0, 32, &control()).unwrap();
    assert_eq!(page[0].end_utf16, 1);
    assert!(find::matches("a", "a", 2, 32, &control()).is_err());
}
#[test]
fn cancelled_reads_do_not_start_git_or_read_bytes() {
    let root = repo();
    std::fs::write(root.path().join("text"), "owned").unwrap();
    let cancel = LocalReviewCancellation::new();
    cancel.cancel();
    let control = GitRunControl::new(Duration::from_secs(20), cancel, "Owned cancellation");
    assert!(inventory::collect(root.path(), &control).is_err());
    assert!(content::read_at(root.path(), "text", &control).is_err());
}

struct RestoreConfig(Option<std::ffi::OsString>);
impl Drop for RestoreConfig {
    fn drop(&mut self) {
        if let Some(value) = &self.0 {
            std::env::set_var("NORN_CONFIG_DIR", value);
        } else {
            std::env::remove_var("NORN_CONFIG_DIR");
        }
    }
}
#[test]
fn qualified_manager_pages_pin_versions_provider_and_clone_and_shutdown() {
    let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
        .lock()
        .unwrap();
    let config = tempfile::tempdir().unwrap();
    let first = repo();
    let second = repo();
    let moved = repo();
    let _restore = RestoreConfig(std::env::var_os("NORN_CONFIG_DIR"));
    std::env::set_var("NORN_CONFIG_DIR", config.path());
    for (root, provider) in [
        (first.path(), "github"),
        (second.path(), "bitbucket"),
        (moved.path(), "github"),
    ] {
        git(
            root,
            &[
                "remote",
                "add",
                "origin",
                &format!(
                    "https://{}.org/owned/same.git",
                    if provider == "github" {
                        "github"
                    } else {
                        "bitbucket"
                    }
                ),
            ],
        );
        if provider == "github" {
            git(
                root,
                &[
                    "remote",
                    "set-url",
                    "origin",
                    "https://github.com/owned/same.git",
                ],
            );
        }
        std::fs::write(root.join("text.txt"), format!("{provider} é🙂\nowned\n")).unwrap();
        std::fs::write(root.join("blame.txt"), "owned\n".repeat(10_000)).unwrap();
        commit(root, "Owned metadata\n\nComplete paragraph");
        std::fs::write(root.join("new.txt"), "new\n").unwrap();
    }
    let settings = |first: &Path| serde_json::json!({"repos":[{"provider":"github","workspace":"owned","repo":"same","localPath":first},{"provider":"bitbucket","workspace":"owned","repo":"same","localPath":second.path()}],"defaultDiffView":"unified","theme":"dark"});
    let write_settings = |value: serde_json::Value| {
        std::fs::write(config.path().join("settings.json"), value.to_string()).unwrap()
    };
    write_settings(settings(first.path()));
    let manager = Manager::new();
    let request = |provider| InfoParams {
        repository: RepositoryIdentity {
            provider,
            workspace: "owned".into(),
            repo: "same".into(),
        },
    };
    let info = manager
        .info(request(crate::config::ReviewProvider::Github))
        .unwrap();
    let other = manager
        .info(request(crate::config::ReviewProvider::Bitbucket))
        .unwrap();
    assert_ne!(info.context.root_id, other.context.root_id);
    let files_params = FilesParams {
        context: info.context.clone(),
        offset: 0,
        limit: 1,
        query: String::new(),
        changed_only: false,
        version: None,
    };
    let page = manager.files(files_params.clone()).unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.files.len(), 1);
    assert_eq!(page.next_offset, Some(1));
    let mut next = files_params.clone();
    next.offset = 1;
    next.version = Some(page.version);
    assert_eq!(
        manager.files(next.clone()).unwrap().files[0].path,
        "new.txt"
    );
    std::fs::write(first.path().join("another.txt"), "owned").unwrap();
    assert!(manager.files(next).is_err());
    let content_params = ContentParams {
        context: info.context.clone(),
        path: "text.txt".into(),
        offset: 0,
        max_bytes: 4,
        content_sha256: None,
    };
    let first_page = manager.content(content_params.clone()).unwrap();
    let sha = first_page.content_sha256.unwrap();
    let mut next = content_params.clone();
    next.offset = first_page.next_offset.unwrap();
    next.content_sha256 = Some(sha.clone());
    assert!(!manager.content(next.clone()).unwrap().text.is_empty());
    std::fs::write(first.path().join("text.txt"), "replacement\n").unwrap();
    assert!(manager.content(next).is_err());
    assert!(manager
        .find(FindParams {
            context: info.context.clone(),
            path: "text.txt".into(),
            source: FindSource::Content,
            query: "owned".into(),
            offset: 0,
            limit: 32,
            content_sha256: sha
        })
        .is_err());
    let content = manager
        .content(ContentParams {
            path: "blame.txt".into(),
            ..content_params
        })
        .unwrap();
    let blame = manager
        .blame(BlameParams {
            context: info.context.clone(),
            path: "blame.txt".into(),
            offset: 5000,
            limit: 32,
            content_sha256: content.content_sha256.unwrap(),
        })
        .unwrap();
    assert!(blame.available);
    assert_eq!(blame.total_lines, 10_000);
    assert_eq!(blame.lines.len(), 32);
    assert_eq!(blame.lines[0].line, 5001);
    assert_eq!(blame.next_offset, Some(5032));
    assert_eq!(
        blame.lines[0].message.as_deref(),
        Some("Owned metadata\n\nComplete paragraph")
    );
    write_settings(settings(moved.path()));
    assert!(manager.files(files_params).is_err());
    let mut duplicate = settings(moved.path());
    let extra = duplicate["repos"][0].clone();
    duplicate["repos"].as_array_mut().unwrap().push(extra);
    write_settings(duplicate);
    assert!(manager
        .info(request(crate::config::ReviewProvider::Github))
        .is_err());
    write_settings(settings(moved.path()));
    manager.stop();
    assert!(manager
        .info(request(crate::config::ReviewProvider::Github))
        .is_err());
}

#[test]
fn inventory_preserves_merge_conflicts() {
    let root = repo();
    let root = root.path();
    std::fs::write(root.join("conflict.txt"), "base\n").unwrap();
    commit(root, "base");
    git(root, &["checkout", "-b", "left"]);
    std::fs::write(root.join("conflict.txt"), "left\n").unwrap();
    commit(root, "left");
    git(root, &["checkout", "-b", "right", "HEAD~1"]);
    std::fs::write(root.join("conflict.txt"), "right\n").unwrap();
    commit(root, "right");
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "commit.gpgsign=false", "merge", "left"])
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let files = inventory::collect(root, &control()).unwrap();
    assert_eq!(
        files
            .files
            .iter()
            .find(|file| file.path == "conflict.txt")
            .unwrap()
            .status,
        RepositoryFileStatus::Conflicted
    );
}

#[test]
fn oversized_diff_is_rejected_instead_of_returning_a_partial_patch() {
    let root = repo();
    let root = root.path();
    std::fs::write(root.join("large.txt"), "old owned line\n".repeat(400_000)).unwrap();
    commit(root, "owned large base");
    std::fs::write(root.join("large.txt"), "new owned line\n".repeat(400_000)).unwrap();
    assert!(diff::read_at(root, "large.txt", &control()).is_err());
}

#[test]
fn excessive_inventory_is_rejected_before_any_page_is_exposed() {
    use std::io::Write;
    let root = repo();
    let root = root.path();
    std::fs::write(root.join("seed.txt"), "owned\n").unwrap();
    commit(root, "owned seed");
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD:seed.txt"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let oid = String::from_utf8(output.stdout).unwrap();
    let entries: String = (0..=MAX_FILES)
        .map(|index| format!("100644 {}\tbounded-{index:05}.txt\n", oid.trim()))
        .collect();
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(entries.as_bytes())
        .unwrap();
    assert!(child.wait_with_output().unwrap().status.success());
    assert!(inventory::collect(root, &control())
        .err()
        .unwrap()
        .contains("inventory exceeds"));
}
#[cfg(unix)]
#[test]
fn symlinks_cannot_export_outside_bytes_and_internal_links_remain_readable() {
    let root = repo();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), "owned outside bytes").unwrap();
    std::fs::write(root.path().join("text"), "owned inside bytes").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret"), root.path().join("escape")).unwrap();
    std::os::unix::fs::symlink("text", root.path().join("internal")).unwrap();
    assert!(content::read_at(root.path(), "escape", &control()).is_err());
    assert!(diff::read_at(root.path(), "escape", &control()).is_err());
    assert_eq!(
        content::read_at(root.path(), "internal", &control())
            .unwrap()
            .text,
        "owned inside bytes"
    );
    std::fs::write(root.path().join("a:literal"), "colon").unwrap();
    assert_eq!(
        content::read_at(root.path(), "a:literal", &control())
            .unwrap()
            .text,
        "colon"
    );
    let hostile = "legitimate\n\u{1b}[31m.txt";
    std::fs::write(root.path().join(hostile), "owned control filename\n").unwrap();
    assert!(inventory::collect(root.path(), &control())
        .unwrap()
        .files
        .iter()
        .any(|file| file.path == hostile));
    assert_eq!(
        content::read_at(root.path(), hostile, &control())
            .unwrap()
            .text,
        "owned control filename\n"
    );
    assert!(diff::read_at(root.path(), hostile, &control())
        .unwrap()
        .0
        .contains("+owned control filename"));
}
