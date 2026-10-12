use super::*;
use crate::{
    config::ReviewProvider,
    local_review::{GitRunControl, LocalReviewCancellation},
};
use std::{path::Path, process::Command, time::Duration};
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
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    git(root.path(), &["init", "-b", "main"]);
    git(root.path(), &["config", "user.name", "Owned"]);
    git(
        root.path(),
        &["config", "user.email", "owned@example.invalid"],
    );
    git(
        root.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/owned/operations.git",
        ],
    );
    root
}
fn context() -> Context {
    Context {
        repository: RepositoryIdentity {
            provider: ReviewProvider::Github,
            workspace: "owned".into(),
            repo: "operations".into(),
        },
        root_id: "a".repeat(64),
    }
}
fn control() -> GitRunControl {
    GitRunControl::new(
        Duration::from_secs(20),
        LocalReviewCancellation::new(),
        "Owned operation fixture",
    )
}
fn commit(root: &Path) {
    git(root, &["add", "--all"]);
    git(
        root,
        &["-c", "commit.gpgsign=false", "commit", "-m", "owned"],
    );
}
#[test]
fn action_requires_full_ref_kind_and_no_untyped_authority() {
    for reference in [
        "main",
        "--detach",
        "refs/heads/../outside",
        "refs/heads/x@{1}",
        "refs/heads/a\nb",
        "refs/remotes/origin/main",
    ] {
        assert!(Action::Checkout {
            reference: reference.into(),
            branch_kind: BranchKind::Local
        }
        .validate()
        .is_err());
    }
    assert!(Action::Checkout {
        reference: "refs/heads/feature ü".into(),
        branch_kind: BranchKind::Local
    }
    .validate()
    .is_err());
    assert!(Action::Checkout {
        reference: "refs/heads/feature-ü".into(),
        branch_kind: BranchKind::Local
    }
    .validate()
    .is_ok());
    assert!(serde_json::from_value::<PreviewParams>(serde_json::json!({"context":context(),"action":{"kind":"fetch"},"remoteUrl":"https://outside.example"})).is_err());
    assert!(ApplyParams {
        context: context(),
        action: Action::Fetch {},
        token: "a".repeat(64),
        confirm: false
    }
    .validate()
    .is_err());
}
#[test]
fn snapshot_tracks_unborn_dirty_raw_bytes_config_and_branch_identity() {
    let root = fixture();
    let ctx = context();
    let unborn = state::snapshot(&ctx, root.path(), &control()).unwrap();
    assert_eq!(unborn.checkout.state, CheckoutState::Unborn);
    assert!(!unborn.checkout.dirty);
    std::fs::write(root.path().join("--owned ü.txt"), "one\n").unwrap();
    commit(root.path());
    git(root.path(), &["branch", "feature"]);
    let clean = state::snapshot(&ctx, root.path(), &control()).unwrap();
    assert_eq!(clean.checkout.branch.as_deref(), Some("main"));
    assert_eq!(clean.branches.len(), 2);
    std::fs::write(root.path().join("--owned ü.txt"), "two\n").unwrap();
    let dirty = state::snapshot(&ctx, root.path(), &control()).unwrap();
    assert!(dirty.checkout.dirty);
    assert_ne!(dirty.checkout.version, clean.checkout.version);
    std::fs::write(root.path().join("--owned ü.txt"), "same-size-change\n").unwrap();
    let changed = state::snapshot(&ctx, root.path(), &control()).unwrap();
    assert_ne!(changed.checkout.version, dirty.checkout.version);
    git(root.path(), &["config", "branch.main.remote", "origin"]);
    let configured = state::snapshot(&ctx, root.path(), &control()).unwrap();
    assert_ne!(changed.checkout.version, configured.checkout.version);
    let foreign = Context {
        repository: RepositoryIdentity {
            provider: ReviewProvider::Bitbucket,
            ..ctx.repository.clone()
        },
        ..ctx
    };
    assert!(state::snapshot(&foreign, root.path(), &control()).is_err());
}
#[test]
fn preview_configuration_is_hashed_and_does_not_execute_a_filter() {
    let root = fixture();
    std::fs::write(root.path().join("owned.txt"), "base\n").unwrap();
    commit(root.path());
    let witness = root.path().join("must-not-exist");
    // If invoked, the owned helper visibly writes a witness. Preview must disable it.
    let command = format!("echo owned > '{}'", witness.display());
    git(root.path(), &["config", "filter.owned.clean", &command]);
    std::fs::write(
        root.path().join(".gitattributes"),
        "owned.txt filter=owned\n",
    )
    .unwrap();
    std::fs::write(root.path().join("owned.txt"), "changed\n").unwrap();
    let snapshot = state::snapshot(&context(), root.path(), &control()).unwrap();
    assert!(snapshot.checkout.filtered);
    assert!(snapshot.checkout.dirty);
    assert!(!witness.exists());
    assert!(!serde_json::to_string(&snapshot.checkout)
        .unwrap()
        .contains("must-not-exist"));
}

#[test]
fn configured_upstream_survives_missing_tracking_ref_and_fetch_changes_only_remote_proof() {
    let root = fixture();
    std::fs::write(root.path().join("owned.txt"), "base\n").unwrap();
    commit(root.path());
    git(root.path(), &["config", "branch.main.remote", "origin"]);
    git(
        root.path(),
        &["config", "branch.main.merge", "refs/heads/main"],
    );
    let before = state::snapshot(&context(), root.path(), &control()).unwrap();
    assert_eq!(
        before.checkout.upstream.as_deref(),
        Some("refs/remotes/origin/main")
    );
    let head = before.checkout.head_sha.as_ref().unwrap();
    git(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", head],
    );
    let after = state::snapshot(&context(), root.path(), &control()).unwrap();
    assert_eq!(before.working_version, after.working_version);
    assert_ne!(before.checkout.version, after.checkout.version);
    git(root.path(), &["branch", "local-change"]);
    let local = state::snapshot(&context(), root.path(), &control()).unwrap();
    assert_ne!(after.working_version, local.working_version);
}

#[test]
fn stash_saves_and_cleans_hostile_untracked_names_with_normal_git_pathspec_semantics() {
    let root = fixture();
    std::fs::write(root.path().join("owned.txt"), "base\n").unwrap();
    commit(root.path());
    let name = if cfg!(windows) {
        "--owned ü & $(owned);.txt"
    } else {
        "--owned ü & $(owned);\n\u{1b}.txt"
    };
    std::fs::write(root.path().join(name), "keep untracked\n").unwrap();
    std::fs::write(root.path().join("owned.txt"), "keep modified\n").unwrap();
    let output = git::run(
        root.path(),
        &["stash", "push", "--include-untracked", "-m", "owned stash"],
        256 * 1024,
        None,
        false,
        &control(),
    )
    .unwrap();
    assert!(output.status.success());
    assert!(!root.path().join(name).exists());
    let after = state::snapshot(&context(), root.path(), &control()).unwrap();
    assert!(!after.checkout.dirty);
    assert_eq!(
        std::fs::read_to_string(root.path().join("owned.txt")).unwrap(),
        "base\n"
    );
    let saved = git::checked(
        root.path(),
        &["show", &format!("refs/stash^3:{name}")],
        4096,
        Some(&after.configuration),
        &control(),
    )
    .unwrap();
    assert_eq!(saved, b"keep untracked\n");
}
