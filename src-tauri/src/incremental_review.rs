//! Read-only incremental review scope construction between immutable commits.

use std::fmt;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde::{Deserialize, Serialize};

const MAX_INCREMENTAL_PATCH_BYTES: usize = 2 * 1024 * 1024;
const MAX_INCREMENTAL_METADATA_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IncrementalChangedFileKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncrementalChangedFile {
    pub path: String,
    pub previous_path: Option<String>,
    pub kind: IncrementalChangedFileKind,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncrementalReviewScope {
    /// Full Git commit object ID: 40 hex characters for SHA-1 or 64 for SHA-256.
    pub previous_head_sha: String,
    /// Full Git commit object ID: 40 hex characters for SHA-1 or 64 for SHA-256.
    pub current_head_sha: String,
    pub files: Vec<IncrementalChangedFile>,
    pub patch: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IncrementalReviewSetupErrorCode {
    RepositoryUnavailable,
    InvalidCommitSha,
    CommitUnavailable,
    NonUtf8Patch,
    NonUtf8Path,
    PatchTooLarge,
    MetadataTooLarge,
    DiffFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IncrementalCommitRole {
    PreviousHead,
    CurrentHead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncrementalReviewSetupError {
    pub code: IncrementalReviewSetupErrorCode,
    pub commit_role: Option<IncrementalCommitRole>,
    pub message: String,
}

impl fmt::Display for IncrementalReviewSetupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for IncrementalReviewSetupError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GitPath(Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawChangedFile {
    path: GitPath,
    previous_path: Option<GitPath>,
    kind: IncrementalChangedFileKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiffStats {
    additions: Option<u64>,
    deletions: Option<u64>,
    binary: bool,
}

pub fn build_incremental_review_scope(
    repo_path: impl AsRef<Path>,
    previous_head_sha: &str,
    current_head_sha: &str,
) -> Result<IncrementalReviewScope, IncrementalReviewSetupError> {
    let repo_path = repo_path.as_ref();
    validate_repository(repo_path)?;
    validate_commit(
        repo_path,
        previous_head_sha,
        IncrementalCommitRole::PreviousHead,
    )?;
    validate_commit(
        repo_path,
        current_head_sha,
        IncrementalCommitRole::CurrentHead,
    )?;

    let name_status = git_diff(
        repo_path,
        previous_head_sha,
        current_head_sha,
        &["--name-status", "-z"],
    )?;
    let changed_files = parse_name_status(&name_status)?;
    let numstat = git_diff(
        repo_path,
        previous_head_sha,
        current_head_sha,
        &["--numstat", "-z"],
    )?;
    let stats = parse_numstat(&numstat)?;
    let patch = git_diff_patch(repo_path, previous_head_sha, current_head_sha)?;

    let files = materialize_changed_files(changed_files, stats)?;

    let patch = String::from_utf8(patch).map_err(|_| {
        setup_error(
            IncrementalReviewSetupErrorCode::NonUtf8Patch,
            None,
            "Incremental patch contains non-UTF-8 text.",
        )
    })?;

    Ok(IncrementalReviewScope {
        previous_head_sha: previous_head_sha.to_string(),
        current_head_sha: current_head_sha.to_string(),
        files,
        patch,
    })
}

fn materialize_changed_files(
    changed_files: Vec<RawChangedFile>,
    stats: Vec<(GitPath, DiffStats)>,
) -> Result<Vec<IncrementalChangedFile>, IncrementalReviewSetupError> {
    if stats.len() != changed_files.len() {
        return Err(diff_error());
    }
    changed_files
        .into_iter()
        .zip(stats)
        .map(
            |(file, (stats_path, stats))| -> Result<
                IncrementalChangedFile,
                IncrementalReviewSetupError,
            > {
                if file.path != stats_path {
                    return Err(diff_error());
                }
                let path = display_git_path(&file.path)?;
                let previous_path = file
                    .previous_path
                    .as_ref()
                    .map(display_git_path)
                    .transpose()?;
                Ok(IncrementalChangedFile {
                    path,
                    previous_path,
                    kind: file.kind,
                    additions: stats.additions,
                    deletions: stats.deletions,
                    binary: stats.binary,
                })
            },
        )
        .collect()
}

fn validate_repository(repo_path: &Path) -> Result<(), IncrementalReviewSetupError> {
    let output = git(repo_path, &["rev-parse", "--is-inside-work-tree"]).map_err(|_| {
        setup_error(
            IncrementalReviewSetupErrorCode::RepositoryUnavailable,
            None,
            "Repository is unavailable.",
        )
    })?;
    if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true" {
        Ok(())
    } else {
        Err(setup_error(
            IncrementalReviewSetupErrorCode::RepositoryUnavailable,
            None,
            "Repository is unavailable.",
        ))
    }
}

fn validate_commit(
    repo_path: &Path,
    sha: &str,
    role: IncrementalCommitRole,
) -> Result<(), IncrementalReviewSetupError> {
    if !is_full_commit_object_id(sha) {
        return Err(setup_error(
            IncrementalReviewSetupErrorCode::InvalidCommitSha,
            Some(role),
            commit_error_message(
                role,
                "is not a full Git commit object ID (40-character SHA-1 or 64-character SHA-256)",
            ),
        ));
    }
    let object = format!("{sha}^{{commit}}");
    let output = git(repo_path, &["cat-file", "-e", &object]).map_err(|_| {
        setup_error(
            IncrementalReviewSetupErrorCode::CommitUnavailable,
            Some(role),
            commit_error_message(role, "is unavailable"),
        )
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(setup_error(
            IncrementalReviewSetupErrorCode::CommitUnavailable,
            Some(role),
            commit_error_message(role, "is unavailable"),
        ))
    }
}

fn is_full_commit_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn commit_error_message(role: IncrementalCommitRole, suffix: &str) -> String {
    let field = match role {
        IncrementalCommitRole::PreviousHead => "`previousHeadSha`",
        IncrementalCommitRole::CurrentHead => "`currentHeadSha`",
    };
    format!("{field} {suffix}.")
}

fn git_diff(
    repo_path: &Path,
    previous_head_sha: &str,
    current_head_sha: &str,
    options: &[&str],
) -> Result<Vec<u8>, IncrementalReviewSetupError> {
    let mut args = vec!["diff", "--no-ext-diff", "--no-textconv", "--find-renames"];
    args.extend_from_slice(options);
    args.extend([previous_head_sha, current_head_sha, "--"]);
    git_bounded_stdout(
        repo_path,
        &args,
        MAX_INCREMENTAL_METADATA_BYTES,
        IncrementalReviewSetupErrorCode::MetadataTooLarge,
        "Incremental diff metadata exceeds the supported size limit.",
    )
}

fn git_diff_patch(
    repo_path: &Path,
    previous_head_sha: &str,
    current_head_sha: &str,
) -> Result<Vec<u8>, IncrementalReviewSetupError> {
    git_bounded_stdout(
        repo_path,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames",
            "--patch",
            "--full-index",
            previous_head_sha,
            current_head_sha,
            "--",
        ],
        MAX_INCREMENTAL_PATCH_BYTES,
        IncrementalReviewSetupErrorCode::PatchTooLarge,
        "Incremental patch exceeds the supported size limit.",
    )
}

fn git_bounded_stdout(
    repo_path: &Path,
    args: &[&str],
    max_bytes: usize,
    size_error_code: IncrementalReviewSetupErrorCode,
    size_error_message: &'static str,
) -> Result<Vec<u8>, IncrementalReviewSetupError> {
    let mut child = git_command(repo_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| diff_error())?;
    let mut patch = Vec::new();
    child
        .stdout
        .take()
        .ok_or_else(diff_error)?
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut patch)
        .map_err(|_| diff_error())?;
    if patch.len() > max_bytes {
        let _ = child.kill();
        let _ = child.wait();
        return Err(setup_error(size_error_code, None, size_error_message));
    }
    let status = child.wait().map_err(|_| diff_error())?;
    if !status.success() {
        return Err(diff_error());
    }
    Ok(patch)
}

fn git(repo_path: &Path, args: &[&str]) -> std::io::Result<Output> {
    git_command(repo_path).args(args).output()
}

fn git_command(repo_path: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-c")
        .arg("core.quotePath=true")
        .arg("-c")
        .arg("diff.renames=true")
        .arg("-c")
        .arg("color.ui=false")
        .arg("-c")
        .arg("color.diff=false")
        .arg("-C")
        .arg(repo_path);
    command
}

fn parse_name_status(bytes: &[u8]) -> Result<Vec<RawChangedFile>, IncrementalReviewSetupError> {
    let fields = nul_terminated_fields(bytes);
    let mut index = 0;
    let mut files = Vec::new();
    while index < fields.len() {
        let status = fields[index];
        index += 1;
        let kind = match status {
            b"A" => IncrementalChangedFileKind::Added,
            b"M" | b"T" => IncrementalChangedFileKind::Modified,
            b"D" => IncrementalChangedFileKind::Deleted,
            status
                if status.starts_with(b"R")
                    && status.len() > 1
                    && status[1..].iter().all(u8::is_ascii_digit) =>
            {
                IncrementalChangedFileKind::Renamed
            }
            _ => return Err(diff_error()),
        };
        if kind == IncrementalChangedFileKind::Renamed {
            let previous_path = fields.get(index).ok_or_else(diff_error)?;
            let path = fields.get(index + 1).ok_or_else(diff_error)?;
            files.push(RawChangedFile {
                path: GitPath(path.to_vec()),
                previous_path: Some(GitPath(previous_path.to_vec())),
                kind,
            });
            index += 2;
        } else {
            let path = fields.get(index).ok_or_else(diff_error)?;
            files.push(RawChangedFile {
                path: GitPath(path.to_vec()),
                previous_path: None,
                kind,
            });
            index += 1;
        }
    }
    Ok(files)
}

fn parse_numstat(bytes: &[u8]) -> Result<Vec<(GitPath, DiffStats)>, IncrementalReviewSetupError> {
    let mut fields = nul_terminated_fields(bytes).into_iter();
    let mut stats = Vec::new();
    while let Some(record) = fields.next() {
        let mut parts = record.splitn(3, |byte| *byte == b'\t');
        let additions = parts.next().ok_or_else(diff_error)?;
        let deletions = parts.next().ok_or_else(diff_error)?;
        let path = parts.next().ok_or_else(diff_error)?;
        // With `--numstat -z`, a rename has an empty third tab-delimited
        // field in this header, followed by old and new NUL-delimited paths.
        let path = if path.is_empty() {
            let _previous_path = fields.next().ok_or_else(diff_error)?;
            fields.next().ok_or_else(diff_error)?
        } else {
            path
        };
        let additions = parse_stat(additions)?;
        let deletions = parse_stat(deletions)?;
        if additions.is_none() != deletions.is_none() {
            return Err(diff_error());
        }
        stats.push((
            GitPath(path.to_vec()),
            DiffStats {
                additions,
                deletions,
                binary: additions.is_none(),
            },
        ));
    }
    Ok(stats)
}

fn parse_stat(value: &[u8]) -> Result<Option<u64>, IncrementalReviewSetupError> {
    if value == b"-" {
        return Ok(None);
    }
    let value = std::str::from_utf8(value).map_err(|_| diff_error())?;
    value.parse::<u64>().map(Some).map_err(|_| diff_error())
}

fn nul_terminated_fields(bytes: &[u8]) -> Vec<&[u8]> {
    let mut fields: Vec<&[u8]> = bytes.split(|byte| *byte == 0).collect();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    fields
}

fn display_git_path(path: &GitPath) -> Result<String, IncrementalReviewSetupError> {
    String::from_utf8(path.0.clone()).map_err(|_| {
        setup_error(
            IncrementalReviewSetupErrorCode::NonUtf8Path,
            None,
            "Incremental diff contains a non-UTF-8 path.",
        )
    })
}

fn diff_error() -> IncrementalReviewSetupError {
    setup_error(
        IncrementalReviewSetupErrorCode::DiffFailed,
        None,
        "Unable to build incremental review scope.",
    )
}

fn setup_error(
    code: IncrementalReviewSetupErrorCode,
    commit_role: Option<IncrementalCommitRole>,
    message: impl Into<String>,
) -> IncrementalReviewSetupError {
    IncrementalReviewSetupError {
        code,
        commit_role,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    struct RepoFixture {
        path: PathBuf,
    }

    impl RepoFixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "lachesi-incremental-review-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("create fixture directory");
            run_git(&path, &["init", "-q"]);
            run_git(&path, &["config", "user.name", "Lachesi Test"]);
            run_git(&path, &["config", "user.email", "test@lachesi.local"]);
            Self { path }
        }

        fn write(&self, path: &str, contents: &[u8]) {
            let path = self.path.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create fixture parent");
            }
            fs::write(path, contents).expect("write fixture file");
        }

        fn remove(&self, path: &str) {
            fs::remove_file(self.path.join(path)).expect("remove fixture file");
        }

        fn rename(&self, from: &str, to: &str) {
            fs::rename(self.path.join(from), self.path.join(to)).expect("rename fixture file");
        }

        fn commit(&self, message: &str) -> String {
            run_git(&self.path, &["add", "-A"]);
            run_git(&self.path, &["commit", "-q", "-m", message]);
            git_stdout(&self.path, &["rev-parse", "HEAD"])
                .trim()
                .to_string()
        }
    }

    impl Drop for RepoFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn run_git(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .expect("run fixture git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout(path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .expect("run fixture git");
        assert!(output.status.success());
        String::from_utf8(output.stdout).expect("utf8 git output")
    }

    fn fixture_with_incremental_changes() -> (RepoFixture, String, String) {
        let fixture = RepoFixture::new();
        fixture.write("existing.txt", b"base\n");
        fixture.write("rename-me.txt", b"rename content\n");
        fixture.write("delete-me.txt", b"delete content\n");
        fixture.write("binary.bin", &[0, 1, 2, 3]);
        fixture.commit("base");

        fixture.write("existing.txt", b"base\nalready reviewed\n");
        let previous = fixture.commit("previous head");

        fixture.write("existing.txt", b"base\nalready reviewed\ncurrent only\n");
        fixture.write("added.txt", b"new file\n");
        fixture.rename("rename-me.txt", "renamed.txt");
        fixture.remove("delete-me.txt");
        fixture.write("binary.bin", &[0, 9, 8, 7]);
        let current = fixture.commit("current head");

        (fixture, previous, current)
    }

    #[test]
    fn scope_contains_only_incremental_changes_and_preserves_file_kinds() {
        let (fixture, previous, current) = fixture_with_incremental_changes();

        let scope =
            build_incremental_review_scope(&fixture.path, &previous, &current).expect("scope");

        assert!(scope.patch.contains("+current only"));
        assert!(!scope.patch.contains("+already reviewed"));
        assert!(scope.patch.contains("Binary files"));
        assert_eq!(scope.files.len(), 5);
        assert!(scope.files.iter().any(|file| {
            file.path == "added.txt"
                && file.kind == IncrementalChangedFileKind::Added
                && file.additions == Some(1)
                && file.deletions == Some(0)
        }));
        assert!(scope.files.iter().any(|file| {
            file.path == "delete-me.txt"
                && file.kind == IncrementalChangedFileKind::Deleted
                && file.additions == Some(0)
                && file.deletions == Some(1)
        }));
        assert!(scope.files.iter().any(|file| {
            file.path == "renamed.txt"
                && file.previous_path.as_deref() == Some("rename-me.txt")
                && file.kind == IncrementalChangedFileKind::Renamed
                && file.additions == Some(0)
                && file.deletions == Some(0)
                && !file.binary
        }));
        assert!(scope
            .files
            .iter()
            .any(|file| file.path == "binary.bin" && file.binary));
    }

    #[test]
    fn empty_diff_returns_an_empty_scope() {
        let (fixture, _, current) = fixture_with_incremental_changes();

        let scope =
            build_incremental_review_scope(&fixture.path, &current, &current).expect("empty scope");

        assert!(scope.files.is_empty());
        assert!(scope.patch.is_empty());
    }

    #[test]
    fn unavailable_commits_return_structured_setup_errors() {
        let (fixture, previous, current) = fixture_with_incremental_changes();
        let unavailable = "ffffffffffffffffffffffffffffffffffffffff";

        let previous_error = build_incremental_review_scope(&fixture.path, unavailable, &current)
            .expect_err("unavailable previous commit");
        assert_eq!(
            previous_error.code,
            IncrementalReviewSetupErrorCode::CommitUnavailable
        );
        assert_eq!(
            previous_error.commit_role,
            Some(IncrementalCommitRole::PreviousHead)
        );

        let invalid_error = build_incremental_review_scope(&fixture.path, "HEAD", &current)
            .expect_err("invalid previous commit");
        assert_eq!(
            invalid_error.code,
            IncrementalReviewSetupErrorCode::InvalidCommitSha
        );
        assert_eq!(
            invalid_error.commit_role,
            Some(IncrementalCommitRole::PreviousHead)
        );

        let current_error = build_incremental_review_scope(&fixture.path, &previous, unavailable)
            .expect_err("unavailable current commit");
        assert_eq!(
            current_error.code,
            IncrementalReviewSetupErrorCode::CommitUnavailable
        );
        assert_eq!(
            current_error.commit_role,
            Some(IncrementalCommitRole::CurrentHead)
        );
    }

    #[test]
    fn non_utf8_text_patch_returns_a_structured_error() {
        let fixture = RepoFixture::new();
        fixture.write("non-utf8.txt", b"base\n");
        let previous = fixture.commit("base");
        fixture.write("non-utf8.txt", &[b'b', 0xff, b'\n']);
        let current = fixture.commit("non utf8");

        let error = build_incremental_review_scope(&fixture.path, &previous, &current)
            .expect_err("non-utf8 patch");

        assert_eq!(error.code, IncrementalReviewSetupErrorCode::NonUtf8Patch);
        assert_eq!(error.commit_role, None);
    }

    #[test]
    fn parsers_reject_unsupported_statuses_and_mismatched_numstat_rows() {
        assert_eq!(
            parse_name_status(b"MM\0file.txt\0").expect_err("combined status"),
            diff_error()
        );

        let changed = parse_name_status(b"M\0file.txt\0").expect("name status");
        let stats = parse_numstat(b"1\t0\tother.txt\0").expect("numstat");
        assert_eq!(
            materialize_changed_files(changed, stats).expect_err("missing matching stats"),
            diff_error()
        );
    }

    #[test]
    fn non_utf8_paths_return_a_structured_error() {
        let path = GitPath(vec![0xff]);
        let changed = vec![RawChangedFile {
            path: path.clone(),
            previous_path: None,
            kind: IncrementalChangedFileKind::Modified,
        }];
        let stats = vec![(
            path,
            DiffStats {
                additions: Some(1),
                deletions: Some(0),
                binary: false,
            },
        )];

        let error =
            materialize_changed_files(changed, stats).expect_err("non-utf8 changed-file path");

        assert_eq!(error.code, IncrementalReviewSetupErrorCode::NonUtf8Path);
    }

    #[test]
    fn oversized_patch_returns_a_structured_error() {
        let fixture = RepoFixture::new();
        fixture.write("large.txt", b"base\n");
        let previous = fixture.commit("base");
        fixture.write("large.txt", &vec![b'a'; MAX_INCREMENTAL_PATCH_BYTES + 1024]);
        let current = fixture.commit("large patch");

        let error = build_incremental_review_scope(&fixture.path, &previous, &current)
            .expect_err("oversized patch");

        assert_eq!(error.code, IncrementalReviewSetupErrorCode::PatchTooLarge);
    }

    #[test]
    fn bounded_git_reader_reports_oversized_metadata() {
        let (fixture, previous, current) = fixture_with_incremental_changes();
        let error = git_bounded_stdout(
            &fixture.path,
            &["diff", "--name-status", "-z", &previous, &current, "--"],
            1,
            IncrementalReviewSetupErrorCode::MetadataTooLarge,
            "metadata too large",
        )
        .expect_err("bounded metadata");

        assert_eq!(
            error.code,
            IncrementalReviewSetupErrorCode::MetadataTooLarge
        );
    }

    #[test]
    fn scope_construction_does_not_modify_repository_state() {
        let (fixture, previous, current) = fixture_with_incremental_changes();
        run_git(&fixture.path, &["config", "diff.renames", "copies"]);
        run_git(&fixture.path, &["config", "color.ui", "always"]);
        run_git(&fixture.path, &["config", "color.diff", "always"]);
        fixture.write("working-tree.txt", b"leave me alone\n");
        let before_head = git_stdout(&fixture.path, &["rev-parse", "HEAD"]);
        let before_status = git_stdout(&fixture.path, &["status", "--porcelain=v1"]);

        let scope =
            build_incremental_review_scope(&fixture.path, &previous, &current).expect("scope");

        assert!(!scope.patch.contains('\u{1b}'));
        assert_eq!(
            git_stdout(&fixture.path, &["rev-parse", "HEAD"]),
            before_head
        );
        assert_eq!(
            git_stdout(&fixture.path, &["status", "--porcelain=v1"]),
            before_status
        );
        assert_eq!(
            fs::read(fixture.path.join("working-tree.txt")).expect("read working tree file"),
            b"leave me alone\n"
        );
    }
}
