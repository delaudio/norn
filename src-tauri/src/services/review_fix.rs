//! Read-only content identity for explicit filesystem mutation approval.
use crate::local_review::{run_git_bounded_with_control, GitRunControl, LocalReviewCancellation};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Component, Path};
use std::time::Duration;

pub(crate) fn checkout_fingerprint(path: &Path) -> Result<String, String> {
    let snapshot = crate::local_review::local_review_snapshot_for_approval_path(
        crate::config::ReviewProvider::Github,
        "",
        "",
        path,
    )?;
    if snapshot.head_sha.is_none()
        || snapshot.warnings.iter().any(|warning| {
            warning.to_lowercase().contains("truncat")
                || warning.to_lowercase().contains("too large")
        })
    {
        return Err("A complete checkout snapshot is required for fix approval.".to_string());
    }
    let control = GitRunControl::new(
        Duration::from_secs(10),
        LocalReviewCancellation::new(),
        "Fix preview",
    );
    let output = run_git_bounded_with_control(
        path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        None,
        1024 * 1024,
        "changed file list",
        &control,
    )?;
    if !output.status.success() {
        return Err("Cannot inspect changed files.".to_string());
    }
    let index = run_git_bounded_with_control(
        path,
        &["ls-files", "--stage", "-z"],
        None,
        4 * 1024 * 1024,
        "index identity",
        &control,
    )?;
    if !index.status.success() {
        return Err("Cannot inspect the Git index.".to_string());
    }
    let remote = run_git_bounded_with_control(
        path,
        &["remote", "get-url", "origin"],
        None,
        16 * 1024,
        "origin configuration",
        &control,
    )?;
    if !remote.status.success() {
        return Err("The fix workflow requires an origin remote.".to_string());
    }
    let push_remote = run_git_bounded_with_control(
        path,
        &["remote", "get-url", "--push", "--all", "origin"],
        None,
        16 * 1024,
        "push destination",
        &control,
    )?;
    if !push_remote.status.success()
        || push_remote
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .count()
            != 1
    {
        return Err("Fix publication requires one origin push destination.".to_string());
    }
    let mut hasher = Sha256::new();
    hasher.update(snapshot.snapshot_sha256.as_bytes());
    // Pending operation identities must be part of approval, including a merge
    // of a destination that happens to leave the same working tree contents.
    for reference in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "REBASE_HEAD",
    ] {
        let state = run_git_bounded_with_control(
            path,
            &["rev-parse", "--verify", reference],
            None,
            4096,
            "Git operation identity",
            &control,
        )?;
        hasher.update(reference.as_bytes());
        hasher.update(state.stdout);
    }
    let refs = run_git_bounded_with_control(
        path,
        &["show-ref"],
        None,
        4 * 1024 * 1024,
        "branch identities",
        &control,
    )?;
    if !refs.status.success() {
        return Err("Cannot inspect branch identities.".to_string());
    }
    hasher.update(refs.stdout);
    hasher.update(index.stdout);
    hasher.update((remote.stdout.len() as u64).to_be_bytes());
    hasher.update(remote.stdout);
    hasher.update((push_remote.stdout.len() as u64).to_be_bytes());
    hasher.update(push_remote.stdout);
    let mut total_bytes = 0usize;
    let mut entries = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty());
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            return Err("Invalid changed file record.".to_string());
        }
        hasher.update((entry.len() as u64).to_be_bytes());
        hasher.update(entry);
        if entry[..2]
            .iter()
            .any(|status| *status == b'R' || *status == b'C')
        {
            let old_path = entries
                .next()
                .ok_or_else(|| "Incomplete rename record.".to_string())?;
            hasher.update((old_path.len() as u64).to_be_bytes());
            hasher.update(old_path);
        }
        let raw_path = &entry[3..];
        let filename = std::str::from_utf8(raw_path)
            .map_err(|_| "Non-UTF8 paths cannot be approved.".to_string())?;
        let relative = Path::new(filename);
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err("Unsafe changed file path.".to_string());
        }
        let file_path = path.join(relative);
        let metadata = match fs::symlink_metadata(&file_path) {
            Ok(metadata) => metadata,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && entry[..2].contains(&b'D') =>
            {
                hasher.update(b"deleted");
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        if !metadata.is_file()
            || file_path
                .canonicalize()
                .map_err(|error| error.to_string())?
                .strip_prefix(path)
                .is_err()
        {
            return Err("Changed links and special files cannot be approved.".to_string());
        }
        let mut contents = Vec::new();
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        options
            .open(file_path)
            .map_err(|error| error.to_string())?
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut contents)
            .map_err(|error| error.to_string())?;
        total_bytes += contents.len();
        if contents.len() > 2 * 1024 * 1024 || total_bytes > 16 * 1024 * 1024 {
            return Err("Changed content exceeds the fix preview budget.".to_string());
        }
        hasher.update((contents.len() as u64).to_be_bytes());
        hasher.update(contents);
    }
    Ok(hex::encode(hasher.finalize()))
}
