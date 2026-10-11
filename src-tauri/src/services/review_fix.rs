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
        &["ls-files", "--others", "--exclude-standard", "-z"],
        None,
        1024 * 1024,
        "untracked file list",
        &control,
    )?;
    if !output.status.success() {
        return Err("Cannot inspect untracked files.".to_string());
    }
    let mut hasher = Sha256::new();
    hasher.update(snapshot.snapshot_sha256.as_bytes());
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
    hasher.update((remote.stdout.len() as u64).to_be_bytes());
    hasher.update(remote.stdout);
    let mut total_bytes = 0usize;
    for raw_path in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let filename = std::str::from_utf8(raw_path)
            .map_err(|_| "Non-UTF8 paths cannot be approved.".to_string())?;
        let relative = Path::new(filename);
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err("Unsafe untracked file path.".to_string());
        }
        let file_path = path.join(relative);
        let metadata = fs::symlink_metadata(&file_path).map_err(|error| error.to_string())?;
        if !metadata.is_file()
            || file_path
                .canonicalize()
                .map_err(|error| error.to_string())?
                .strip_prefix(path)
                .is_err()
        {
            return Err("Untracked links and special files cannot be approved.".to_string());
        }
        let mut contents = Vec::new();
        fs::File::open(file_path)
            .map_err(|error| error.to_string())?
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut contents)
            .map_err(|error| error.to_string())?;
        total_bytes += contents.len();
        if contents.len() > 2 * 1024 * 1024 || total_bytes > 16 * 1024 * 1024 {
            return Err("Untracked content exceeds the fix preview budget.".to_string());
        }
        hasher.update((raw_path.len() as u64).to_be_bytes());
        hasher.update(raw_path);
        hasher.update((contents.len() as u64).to_be_bytes());
        hasher.update(contents);
    }
    Ok(hex::encode(hasher.finalize()))
}
