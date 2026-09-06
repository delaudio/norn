use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::config::ReviewProvider;
use crate::local_repo::{resolve_local_repo_for_provider, trusted_git_path};
use crate::services::bitbucket::{DiffstatEntry, PrFilePreview, MAX_PR_IMAGE_PREVIEW_BYTES};

const MAX_LOCAL_DIFF_BYTES: usize = 16 * 1024 * 1024;
const MAX_LOCAL_CHANGED_FILES: usize = 2_000;
const MAX_GIT_METADATA_BYTES: usize = 8 * 1024 * 1024;
const MAX_GIT_DIAGNOSTIC_BYTES: usize = 256 * 1024;
const LOCAL_GIT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_LOCAL_PREVIEW_CANDIDATES: usize = 64;
const MAX_LOCAL_PREVIEW_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
#[cfg(not(windows))]
const NULL_GIT_CONFIG_PATH: &str = "/dev/null";
#[cfg(windows)]
const NULL_GIT_CONFIG_PATH: &str = "NUL";

#[derive(Clone, Debug, Default)]
pub(crate) struct LocalReviewCancellation(Arc<AtomicBool>);

impl LocalReviewCancellation {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub(crate) struct GitRunControl {
    deadline: Instant,
    timeout: Duration,
    cancellation: LocalReviewCancellation,
    operation: &'static str,
    disabled_filter_drivers: Vec<String>,
}

impl GitRunControl {
    pub(crate) fn new(
        timeout: Duration,
        cancellation: LocalReviewCancellation,
        operation: &'static str,
    ) -> Self {
        Self {
            deadline: Instant::now() + timeout,
            timeout,
            cancellation,
            operation,
            disabled_filter_drivers: Vec::new(),
        }
    }

    fn with_disabled_filter_drivers(mut self, drivers: Vec<String>) -> Self {
        self.disabled_filter_drivers = drivers;
        self
    }

    pub(crate) fn check(&self) -> Result<(), String> {
        if self.cancellation.is_cancelled() {
            return Err(format!("{} was cancelled.", self.operation));
        }
        if Instant::now() >= self.deadline {
            return Err(format!(
                "{} timed out after {} seconds.",
                self.operation,
                self.timeout.as_secs_f64()
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalReviewSnapshot {
    pub provider: ReviewProvider,
    pub workspace: String,
    pub repo: String,
    pub current_branch: String,
    pub upstream: Option<String>,
    pub commits_ahead: u32,
    pub commits_behind: u32,
    pub head_sha: Option<String>,
    pub base_sha: String,
    pub snapshot_sha256: String,
    pub review_id: u32,
    pub diff: String,
    pub diffstat: Vec<DiffstatEntry>,
    pub preview_oid: BTreeMap<String, String>,
    pub warnings: Vec<String>,
}

struct CollectedLocalDiff {
    diff: String,
    diffstat: Vec<DiffstatEntry>,
    preview_oid: BTreeMap<String, String>,
    warnings: Vec<String>,
}

#[tauri::command]
pub fn get_local_review_snapshot(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
) -> Result<LocalReviewSnapshot, String> {
    get_local_review_snapshot_native(
        provider.unwrap_or_default(),
        workspace.as_str(),
        repo.as_str(),
    )
}

pub fn get_local_review_snapshot_native(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
) -> Result<LocalReviewSnapshot, String> {
    get_local_review_snapshot_native_with_cancellation(
        provider,
        workspace,
        repo,
        LocalReviewCancellation::new(),
    )
}

pub(crate) fn get_local_review_snapshot_native_with_cancellation(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
    cancellation: LocalReviewCancellation,
) -> Result<LocalReviewSnapshot, String> {
    let repo_path = resolve_local_repo_for_provider(provider, workspace, repo)?;
    local_review_snapshot_for_path_with_control(
        provider,
        workspace,
        repo,
        &repo_path,
        cancellation,
        LOCAL_GIT_TIMEOUT,
        false,
    )
}

pub(crate) fn local_review_snapshot_for_configured_path(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
    repo_path: &Path,
    cancellation: LocalReviewCancellation,
) -> Result<LocalReviewSnapshot, String> {
    local_review_snapshot_for_path_with_control(
        provider,
        workspace,
        repo,
        repo_path,
        cancellation,
        LOCAL_GIT_TIMEOUT,
        true,
    )
}

#[cfg(test)]
pub(crate) fn local_review_snapshot_for_path(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
    repo_path: &Path,
) -> Result<LocalReviewSnapshot, String> {
    local_review_snapshot_for_path_with_control(
        provider,
        workspace,
        repo,
        repo_path,
        LocalReviewCancellation::new(),
        LOCAL_GIT_TIMEOUT,
        false,
    )
}

fn local_review_snapshot_for_path_with_control(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
    repo_path: &Path,
    cancellation: LocalReviewCancellation,
    timeout: Duration,
    validate_origin: bool,
) -> Result<LocalReviewSnapshot, String> {
    let control = GitRunControl::new(timeout, cancellation, "Local review snapshot");
    ensure_git_repository(repo_path, &control)?;
    let disabled_filter_drivers = configured_filter_drivers(repo_path, &control)?;
    let control = control.with_disabled_filter_drivers(disabled_filter_drivers);
    if validate_origin {
        let origin = git_text_with_control(repo_path, &["remote", "get-url", "origin"], &control)?;
        if !crate::local_repo::matches_remote(&origin, provider, workspace, repo) {
            return Err(format!(
                "Configured local path does not match the selected repository: {}.",
                repo_path.display()
            ));
        }
    }
    let starting_tracked_status = git_bytes_limited_with_control(
        repo_path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=no"],
        MAX_GIT_METADATA_BYTES,
        "Local repository status",
        &control,
    )?;
    let starting_has_untracked = has_untracked_files(repo_path, &control)?;
    let head_sha =
        optional_git_text_with_control(repo_path, &["rev-parse", "--verify", "HEAD"], &control)?;
    let branch = optional_git_text_with_control(
        repo_path,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        &control,
    )?;
    let current_branch = branch.clone().unwrap_or_else(|| {
        head_sha
            .as_deref()
            .map(|sha| format!("HEAD ({})", &sha[..sha.len().min(12)]))
            .unwrap_or_else(|| "unborn HEAD".to_string())
    });
    let upstream = if branch.is_some() {
        optional_git_text_with_control(
            repo_path,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
            &control,
        )?
    } else {
        None
    };
    let upstream_sha = upstream
        .as_ref()
        .map(|value| {
            optional_git_text_with_control(repo_path, &["rev-parse", "--verify", value], &control)
        })
        .transpose()?
        .flatten();
    let base_sha =
        if let (Some(upstream_sha), Some(head_sha)) = (upstream_sha.as_ref(), head_sha.as_ref()) {
            git_text_with_control(repo_path, &["merge-base", upstream_sha, head_sha], &control)?
        } else if let Some(sha) = head_sha.as_ref() {
            sha.clone()
        } else {
            empty_tree_oid(repo_path, &control)?
        };
    let (commits_ahead, commits_behind) = if let Some(upstream) = upstream.as_ref() {
        ahead_behind(repo_path, upstream, &control)?
    } else {
        (0, 0)
    };
    let mut warnings = Vec::new();
    if upstream.is_none() {
        warnings.push(if head_sha.is_some() {
            "The current branch has no upstream; showing working-tree changes relative to HEAD."
                .to_string()
        } else {
            "The repository has no commits or upstream; showing tracked changes relative to an empty tree."
                .to_string()
        });
    }
    if starting_has_untracked {
        warnings.push(
            "Untracked files are excluded from local review until they are staged or committed."
                .to_string(),
        );
    }
    if commits_behind > 0 {
        warnings.push(format!(
            "The current branch is {commits_behind} commit(s) behind its upstream; local changes are shown from their merge base."
        ));
    }

    let mut collected = collect_local_diff(repo_path, &base_sha, &control)?;
    let (preview_oid, preview_warnings) =
        collect_preview_oids_with_control(repo_path, &collected.diffstat, &control)?;
    collected.preview_oid = preview_oid;
    collected.warnings.extend(preview_warnings);
    warnings.extend(collected.warnings.clone());

    let ending_tracked_status = git_bytes_limited_with_control(
        repo_path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=no"],
        MAX_GIT_METADATA_BYTES,
        "Local repository status",
        &control,
    )?;
    let ending_has_untracked = has_untracked_files(repo_path, &control)?;
    let ending_head =
        optional_git_text_with_control(repo_path, &["rev-parse", "--verify", "HEAD"], &control)?;
    let ending_branch = optional_git_text_with_control(
        repo_path,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        &control,
    )?;
    let ending_upstream = if ending_branch.is_some() {
        optional_git_text_with_control(
            repo_path,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
            &control,
        )?
    } else {
        None
    };
    let ending_upstream_sha = ending_upstream
        .as_ref()
        .map(|value| {
            optional_git_text_with_control(repo_path, &["rev-parse", "--verify", value], &control)
        })
        .transpose()?
        .flatten();
    if starting_tracked_status != ending_tracked_status
        || starting_has_untracked != ending_has_untracked
        || head_sha != ending_head
        || branch != ending_branch
        || upstream != ending_upstream
        || upstream_sha != ending_upstream_sha
    {
        return Err(
            "Local changes changed while Norn was loading them. Refresh and try again.".to_string(),
        );
    }
    let verification = collect_local_diff(repo_path, &base_sha, &control)?;
    let (verification_preview_oid, _) =
        collect_preview_oids_with_control(repo_path, &verification.diffstat, &control)?;
    if collected.diff != verification.diff
        || collected.diffstat != verification.diffstat
        || collected.preview_oid != verification_preview_oid
    {
        return Err(
            "Local changes changed while Norn was loading them. Refresh and try again.".to_string(),
        );
    }
    let (snapshot_sha256, review_id) = local_review_identity(
        provider,
        workspace,
        repo,
        &current_branch,
        upstream.as_deref(),
        upstream_sha.as_deref(),
        head_sha.as_deref(),
        &base_sha,
        commits_ahead,
        commits_behind,
        &collected.diff,
        &collected.preview_oid,
    );

    Ok(LocalReviewSnapshot {
        provider,
        workspace: workspace.to_string(),
        repo: repo.to_string(),
        current_branch,
        upstream,
        commits_ahead,
        commits_behind,
        head_sha,
        base_sha,
        snapshot_sha256,
        review_id,
        diff: collected.diff,
        diffstat: collected.diffstat,
        preview_oid: collected.preview_oid,
        warnings,
    })
}

fn ahead_behind(
    repo_path: &Path,
    upstream: &str,
    control: &GitRunControl,
) -> Result<(u32, u32), String> {
    let range = format!("{upstream}...HEAD");
    let output = git_text_with_control(
        repo_path,
        &["rev-list", "--left-right", "--count", &range],
        control,
    )?;
    let mut counts = output.split_whitespace();
    let behind = counts
        .next()
        .ok_or_else(|| "Git returned an invalid upstream comparison.".to_string())?
        .parse::<u32>()
        .map_err(|_| "Git returned an invalid behind count.".to_string())?;
    let ahead = counts
        .next()
        .ok_or_else(|| "Git returned an invalid upstream comparison.".to_string())?
        .parse::<u32>()
        .map_err(|_| "Git returned an invalid ahead count.".to_string())?;
    if counts.next().is_some() {
        return Err("Git returned an invalid upstream comparison.".to_string());
    }
    Ok((ahead, behind))
}

#[allow(clippy::too_many_arguments)]
fn local_review_identity(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
    current_branch: &str,
    upstream: Option<&str>,
    upstream_sha: Option<&str>,
    head_sha: Option<&str>,
    base_sha: &str,
    commits_ahead: u32,
    commits_behind: u32,
    diff: &str,
    preview_oids: &BTreeMap<String, String>,
) -> (String, u32) {
    let mut hasher = Sha256::new();
    for part in [
        match provider {
            ReviewProvider::Bitbucket => "bitbucket",
            ReviewProvider::Github => "github",
        },
        workspace,
        repo,
        current_branch,
        upstream.unwrap_or_default(),
        upstream_sha.unwrap_or_default(),
        head_sha.unwrap_or_default(),
        base_sha,
        diff,
    ] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    hasher.update(commits_ahead.to_be_bytes());
    hasher.update(commits_behind.to_be_bytes());
    hasher.update((preview_oids.len() as u64).to_be_bytes());
    for (path, oid) in preview_oids {
        for part in [path.as_str(), oid.as_str()] {
            hasher.update((part.len() as u64).to_be_bytes());
            hasher.update(part.as_bytes());
        }
    }
    let digest = hasher.finalize();
    let review_id = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) | 0x8000_0000;
    (hex::encode(digest), review_id)
}

fn collect_local_diff(
    repo_path: &Path,
    base_sha: &str,
    control: &GitRunControl,
) -> Result<CollectedLocalDiff, String> {
    let diffstat = tracked_diffstat(repo_path, base_sha, control)?;
    if diffstat.len() > MAX_LOCAL_CHANGED_FILES {
        return Err(format!(
            "Local review contains {} changed files, exceeding the {}-file limit.",
            diffstat.len(),
            MAX_LOCAL_CHANGED_FILES
        ));
    }
    let diff = git_text_raw_limited_with_control(
        repo_path,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames",
            "--no-color",
            base_sha,
            "--",
        ],
        MAX_LOCAL_DIFF_BYTES,
        "Local review diff",
        control,
    )?;
    let warnings = Vec::new();
    if diffstat.len() > MAX_LOCAL_CHANGED_FILES {
        return Err(format!(
            "Local review contains {} changed files, exceeding the {}-file limit.",
            diffstat.len(),
            MAX_LOCAL_CHANGED_FILES
        ));
    }
    if diff.len() > MAX_LOCAL_DIFF_BYTES {
        return Err(format!(
            "Local review diff exceeds the {} MiB limit.",
            MAX_LOCAL_DIFF_BYTES / (1024 * 1024)
        ));
    }
    Ok(CollectedLocalDiff {
        diff,
        diffstat,
        preview_oid: BTreeMap::new(),
        warnings,
    })
}

fn has_untracked_files(repo_path: &Path, control: &GitRunControl) -> Result<bool, String> {
    git_output_exists_with_control(
        repo_path,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        control,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn get_local_file_preview_native(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
    base_sha: &str,
    diffstat: &[DiffstatEntry],
    expected_new_oid: Option<&str>,
    path: &str,
    side: &str,
) -> Result<PrFilePreview, String> {
    if side != "old" && side != "new" {
        return Err("Image preview side must be old or new.".to_string());
    }
    ensure_snapshot_preview_path(diffstat, path, side)?;
    validate_repo_relative_path(path)?;
    let mime_type = raster_mime_type(path)
        .ok_or_else(|| "Only PNG, JPEG, WebP, and GIF previews are supported.".to_string())?;
    let repo_path = resolve_local_repo_for_provider(provider, workspace, repo)?;
    local_file_preview_for_path(
        &repo_path,
        base_sha,
        diffstat,
        expected_new_oid,
        path,
        side,
        mime_type,
    )
}

fn local_file_preview_for_path(
    repo_path: &Path,
    base_sha: &str,
    diffstat: &[DiffstatEntry],
    expected_new_oid: Option<&str>,
    path: &str,
    side: &str,
    mime_type: &str,
) -> Result<PrFilePreview, String> {
    ensure_snapshot_preview_path(diffstat, path, side)?;
    let bytes = if side == "old" {
        let object = format!("{base_sha}:{path}");
        git_bytes_limited(
            repo_path,
            &["cat-file", "blob", object.as_str()],
            MAX_PR_IMAGE_PREVIEW_BYTES,
            "Image preview",
        )?
    } else {
        let expected = expected_new_oid.ok_or_else(|| {
            "The local image preview is unavailable for this snapshot; refresh and try again."
                .to_string()
        })?;
        if !valid_git_object_id(expected) {
            return Err("The local image preview fingerprint is invalid.".to_string());
        }
        let expected = expected.to_ascii_lowercase();
        let control = GitRunControl::new(
            LOCAL_GIT_TIMEOUT,
            LocalReviewCancellation::new(),
            "Image preview",
        );
        let current = index_blob_oids(repo_path, &[path.to_string()], &control)?;
        if current.get(path) != Some(&expected) {
            return Err(
                "The local image changed after this snapshot was loaded; refresh and try again."
                    .to_string(),
            );
        }
        let bytes = git_bytes_limited_with_control(
            repo_path,
            &["cat-file", "blob", &expected],
            MAX_PR_IMAGE_PREVIEW_BYTES,
            "Image preview",
            &control,
        )?;
        if git_object_id_for_bytes(&bytes, expected.len())? != expected {
            return Err(
                "The local image changed after this snapshot was loaded; refresh and try again."
                    .to_string(),
            );
        }
        bytes
    };
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    Ok(PrFilePreview {
        path: path.to_string(),
        mime_type: mime_type.to_string(),
        data_url: format!("data:{mime_type};base64,{encoded}"),
        size: bytes.len(),
    })
}

fn ensure_snapshot_preview_path(
    diffstat: &[DiffstatEntry],
    path: &str,
    side: &str,
) -> Result<(), String> {
    let allowed = diffstat.iter().any(|entry| match side {
        "old" => entry.old_path.as_deref() == Some(path),
        "new" => entry.new_path.as_deref() == Some(path),
        _ => false,
    });
    if allowed {
        Ok(())
    } else {
        Err("Image preview is not part of this local review snapshot.".to_string())
    }
}

fn ensure_git_repository(repo_path: &Path, control: &GitRunControl) -> Result<(), String> {
    if git_text_with_control(repo_path, &["rev-parse", "--is-inside-work-tree"], control)? == "true"
    {
        Ok(())
    } else {
        Err("Configured local path is not a Git working tree.".to_string())
    }
}

fn empty_tree_oid(repo_path: &Path, control: &GitRunControl) -> Result<String, String> {
    let output = run_git_bounded_with_control(
        repo_path,
        &["hash-object", "-t", "tree", "--stdin"],
        Some(&[]),
        MAX_GIT_METADATA_BYTES,
        "Git hash-object output",
        control,
    )?;
    checked_git_text(output)
}

fn tracked_diffstat(
    repo_path: &Path,
    base_sha: &str,
    control: &GitRunControl,
) -> Result<Vec<DiffstatEntry>, String> {
    let output = git_bytes_limited_with_control(
        repo_path,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--name-status",
            "-z",
            "--find-renames",
            base_sha,
            "--",
        ],
        MAX_GIT_METADATA_BYTES,
        "Local review file metadata",
        control,
    )?;
    let mut entries = parse_name_status(&output)?;
    let numstat = git_bytes_limited_with_control(
        repo_path,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--numstat",
            "-z",
            "--find-renames",
            base_sha,
            "--",
        ],
        MAX_GIT_METADATA_BYTES,
        "Local review line metadata",
        control,
    )?;
    let counts = parse_numstat(&numstat)?;
    for entry in &mut entries {
        let path = entry.new_path.as_ref().or(entry.old_path.as_ref());
        if let Some((_, added, removed)) =
            path.and_then(|path| counts.iter().find(|(candidate, _, _)| candidate == path))
        {
            entry.lines_added = *added;
            entry.lines_removed = *removed;
        }
    }
    Ok(entries)
}

fn parse_numstat(output: &[u8]) -> Result<Vec<(String, u32, u32)>, String> {
    let mut records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty());
    let mut counts = Vec::new();
    while let Some(record) = records.next() {
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let added = parse_numstat_count(fields.next())?;
        let removed = parse_numstat_count(fields.next())?;
        let path_field = fields
            .next()
            .ok_or_else(|| "Git returned incomplete line metadata.".to_string())?;
        let path = if path_field.is_empty() {
            let _old_path = utf8_path(records.next())?;
            utf8_path(records.next())?
        } else {
            utf8_path(Some(path_field))?
        };
        counts.push((path, added, removed));
    }
    Ok(counts)
}

fn parse_numstat_count(raw: Option<&[u8]>) -> Result<u32, String> {
    let raw = raw.ok_or_else(|| "Git returned incomplete line metadata.".to_string())?;
    if raw == b"-" {
        return Ok(0);
    }
    std::str::from_utf8(raw)
        .map_err(|_| "Git returned invalid line metadata.".to_string())?
        .parse::<u32>()
        .map_err(|_| "Git returned invalid line metadata.".to_string())
}

#[cfg(test)]
fn collect_preview_oids(
    repo_path: &Path,
    diffstat: &[DiffstatEntry],
) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    let control = GitRunControl::new(
        LOCAL_GIT_TIMEOUT,
        LocalReviewCancellation::new(),
        "Git command",
    );
    collect_preview_oids_with_limits(
        repo_path,
        diffstat,
        MAX_LOCAL_PREVIEW_CANDIDATES,
        MAX_LOCAL_PREVIEW_TOTAL_BYTES,
        MAX_PR_IMAGE_PREVIEW_BYTES as u64,
        &control,
    )
}

fn collect_preview_oids_with_control(
    repo_path: &Path,
    diffstat: &[DiffstatEntry],
    control: &GitRunControl,
) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    collect_preview_oids_with_limits(
        repo_path,
        diffstat,
        MAX_LOCAL_PREVIEW_CANDIDATES,
        MAX_LOCAL_PREVIEW_TOTAL_BYTES,
        MAX_PR_IMAGE_PREVIEW_BYTES as u64,
        control,
    )
}

#[cfg(test)]
fn collect_preview_oids_with_test_limits(
    repo_path: &Path,
    diffstat: &[DiffstatEntry],
    candidate_limit: usize,
    total_byte_limit: u64,
    file_byte_limit: u64,
) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    let control = GitRunControl::new(
        LOCAL_GIT_TIMEOUT,
        LocalReviewCancellation::new(),
        "Git command",
    );
    collect_preview_oids_with_limits(
        repo_path,
        diffstat,
        candidate_limit,
        total_byte_limit,
        file_byte_limit,
        &control,
    )
}

fn collect_preview_oids_with_limits(
    repo_path: &Path,
    diffstat: &[DiffstatEntry],
    candidate_limit: usize,
    total_byte_limit: u64,
    file_byte_limit: u64,
    control: &GitRunControl,
) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    control.check()?;
    let mut oids = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut total_bytes = 0_u64;
    let mut candidates = Vec::new();
    for (candidate_index, path) in diffstat
        .iter()
        .filter_map(|entry| entry.new_path.as_deref())
        .filter(|path| raster_mime_type(path).is_some())
        .enumerate()
    {
        control.check()?;
        if candidate_index >= candidate_limit {
            warnings.push(format!(
                "Skipped additional image previews because the {candidate_limit}-file preview limit was reached."
            ));
            break;
        }
        if validate_repo_relative_path(path).is_err() {
            warnings.push(format!(
                "Skipped image preview with an unsafe repository path `{}`.",
                path.escape_default()
            ));
            continue;
        }
        candidates.push(path.to_string());
    }
    if candidates.is_empty() {
        return Ok((oids, warnings));
    }

    let unstaged = unstaged_paths(repo_path, &candidates, control)?;
    let index_oids = index_blob_oids(repo_path, &candidates, control)?;
    let object_sizes = object_sizes(repo_path, index_oids.values(), control)?;
    for path in candidates {
        control.check()?;
        if unstaged.contains(&path) {
            warnings.push(format!(
                "Skipped image preview with unstaged content `{}`; stage it to create an immutable preview.",
                path.escape_default()
            ));
            continue;
        }
        let Some(oid) = index_oids.get(&path) else {
            warnings.push(format!(
                "Skipped image preview that is unavailable from the Git index `{}`.",
                path.escape_default()
            ));
            continue;
        };
        let Some(size) = object_sizes.get(oid).copied() else {
            warnings.push(format!(
                "Skipped image preview whose Git object could not be inspected `{}`.",
                path.escape_default()
            ));
            continue;
        };
        if size > file_byte_limit {
            warnings.push(format!(
                "Skipped oversized image preview `{}`.",
                path.escape_default()
            ));
            continue;
        }
        let remaining = total_byte_limit.saturating_sub(total_bytes);
        if size > remaining {
            warnings.push(format!(
                "Skipped additional image previews starting with `{}` because the total preview byte limit was reached.",
                path.escape_default()
            ));
            break;
        }
        total_bytes = total_bytes.saturating_add(size);
        oids.insert(path, oid.clone());
    }
    control.check()?;
    Ok((oids, warnings))
}

fn unstaged_paths(
    repo_path: &Path,
    candidates: &[String],
    control: &GitRunControl,
) -> Result<BTreeSet<String>, String> {
    let mut args = vec![
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--name-only",
        "-z",
        "--",
    ];
    args.extend(candidates.iter().map(String::as_str));
    let output = git_bytes_limited_with_control(
        repo_path,
        &args,
        MAX_GIT_METADATA_BYTES,
        "Local image preview status",
        control,
    )?;
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            std::str::from_utf8(path)
                .map(str::to_string)
                .map_err(|_| "Git returned a non-UTF-8 image preview path.".to_string())
        })
        .collect()
}

fn index_blob_oids(
    repo_path: &Path,
    candidates: &[String],
    control: &GitRunControl,
) -> Result<BTreeMap<String, String>, String> {
    let mut args = vec!["ls-files", "--stage", "-z", "--"];
    args.extend(candidates.iter().map(String::as_str));
    let output = git_bytes_limited_with_control(
        repo_path,
        &args,
        MAX_GIT_METADATA_BYTES,
        "Local image preview index",
        control,
    )?;
    let mut oids = BTreeMap::new();
    for record in output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
            return Err("Git returned invalid image preview index metadata.".to_string());
        };
        let header = std::str::from_utf8(&record[..tab])
            .map_err(|_| "Git returned invalid image preview index metadata.".to_string())?;
        let path = std::str::from_utf8(&record[tab + 1..])
            .map_err(|_| "Git returned a non-UTF-8 image preview path.".to_string())?;
        let mut fields = header.split_ascii_whitespace();
        let _mode = fields.next();
        let oid = fields.next();
        let stage = fields.next();
        if fields.next().is_some() || stage != Some("0") {
            continue;
        }
        if let Some(oid) = oid.filter(|oid| valid_git_object_id(oid)) {
            oids.insert(path.to_string(), oid.to_ascii_lowercase());
        }
    }
    Ok(oids)
}

fn object_sizes<'a>(
    repo_path: &Path,
    oids: impl Iterator<Item = &'a String>,
    control: &GitRunControl,
) -> Result<BTreeMap<String, u64>, String> {
    let mut input = Vec::new();
    for oid in oids {
        input.extend_from_slice(oid.as_bytes());
        input.push(b'\n');
    }
    if input.is_empty() {
        return Ok(BTreeMap::new());
    }
    let output = run_git_bounded_with_control(
        repo_path,
        &[
            "cat-file",
            "--batch-check=%(objectname) %(objecttype) %(objectsize)",
        ],
        Some(&input),
        MAX_GIT_METADATA_BYTES,
        "Local image preview object metadata",
        control,
    )?;
    let output = checked_git_text(output)?;
    let mut sizes = BTreeMap::new();
    for line in output.lines() {
        let mut fields = line.split_ascii_whitespace();
        if let (Some(oid), Some("blob"), Some(size), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        {
            if valid_git_object_id(oid) {
                if let Ok(size) = size.parse::<u64>() {
                    sizes.insert(oid.to_ascii_lowercase(), size);
                }
            }
        }
    }
    Ok(sizes)
}

fn valid_git_object_id(oid: &str) -> bool {
    (oid.len() == 40 || oid.len() == 64) && oid.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn git_object_id_for_bytes(bytes: &[u8], oid_len: usize) -> Result<String, String> {
    let header = format!("blob {}\0", bytes.len());
    match oid_len {
        40 => {
            let mut hasher = Sha1::new();
            hasher.update(header.as_bytes());
            hasher.update(bytes);
            Ok(hex::encode(hasher.finalize()))
        }
        64 => {
            let mut hasher = Sha256::new();
            hasher.update(header.as_bytes());
            hasher.update(bytes);
            Ok(hex::encode(hasher.finalize()))
        }
        _ => Err("Git returned an unsupported image fingerprint.".to_string()),
    }
}

fn parse_name_status(output: &[u8]) -> Result<Vec<DiffstatEntry>, String> {
    let mut parts = output
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty());
    let mut entries = Vec::new();
    while let Some(raw_status) = parts.next() {
        let status = std::str::from_utf8(raw_status)
            .map_err(|_| "Git returned a non-UTF-8 file status.".to_string())?;
        let code = status.chars().next().unwrap_or('M');
        let first = utf8_path(parts.next())?;
        let (public_status, old_path, new_path) = match code {
            'A' => ("added", None, Some(first)),
            'D' => ("removed", Some(first), None),
            'R' | 'C' => {
                let second = utf8_path(parts.next())?;
                ("renamed", Some(first), Some(second))
            }
            _ => ("modified", Some(first.clone()), Some(first)),
        };
        entries.push(DiffstatEntry {
            status: public_status.to_string(),
            lines_added: 0,
            lines_removed: 0,
            old_path,
            new_path,
        });
    }
    Ok(entries)
}

fn utf8_path(raw: Option<&[u8]>) -> Result<String, String> {
    let raw = raw.ok_or_else(|| "Git returned an incomplete file status.".to_string())?;
    std::str::from_utf8(raw)
        .map(str::to_string)
        .map_err(|_| "Local review does not support non-UTF-8 changed paths.".to_string())
}

fn raster_mime_type(path: &str) -> Option<&'static str> {
    match Path::new(path)
        .extension()
        .and_then(OsStr::to_str)?
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

fn validate_repo_relative_path(path: &str) -> Result<(), String> {
    let candidate = Path::new(path);
    if path.is_empty()
        || candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err("Image preview path must stay inside the repository.".to_string());
    }
    Ok(())
}

fn configured_filter_drivers(
    repo_path: &Path,
    control: &GitRunControl,
) -> Result<Vec<String>, String> {
    let output = run_git_bounded_with_control(
        repo_path,
        &[
            "config",
            "--includes",
            "--name-only",
            "--null",
            "--get-regexp",
            r"^filter\..*\.(clean|process|required)$",
        ],
        None,
        MAX_GIT_METADATA_BYTES,
        "Git filter configuration",
        control,
    )?;
    if !output.status.success() {
        if output.status.code() == Some(1) && output.stdout.is_empty() {
            return Ok(Vec::new());
        }
        return Err(git_error(&output.stderr, output.status.to_string()));
    }

    let mut drivers = BTreeSet::new();
    for raw_key in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|key| !key.is_empty())
    {
        let key = std::str::from_utf8(raw_key)
            .map_err(|_| "Git returned a non-UTF-8 filter configuration key.".to_string())?;
        let normalized = key.to_ascii_lowercase();
        let suffix = [".clean", ".process", ".required"]
            .into_iter()
            .find(|suffix| normalized.ends_with(suffix))
            .ok_or_else(|| "Git returned an invalid filter configuration key.".to_string())?;
        let driver = key
            .get("filter.".len()..key.len().saturating_sub(suffix.len()))
            .filter(|driver| !driver.is_empty())
            .ok_or_else(|| "Git returned an invalid filter driver name.".to_string())?;
        drivers.insert(driver.to_string());
    }
    Ok(drivers.into_iter().collect())
}

fn git_command(repo_path: &Path, control: &GitRunControl) -> Result<Command, String> {
    let repo_path = repo_path.canonicalize().map_err(|error| {
        format!(
            "Failed to resolve local repository path {}: {error}",
            repo_path.display()
        )
    })?;
    let mut work_tree = OsString::from("--work-tree=");
    work_tree.push(&repo_path);

    let mut command = Command::new(trusted_git_path()?);
    command
        .arg("-C")
        .arg(&repo_path)
        .arg(work_tree)
        .arg("--no-optional-locks")
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg("diff.external=")
        .arg("-c")
        .arg(format!("core.attributesFile={NULL_GIT_CONFIG_PATH}"))
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_SYSTEM", NULL_GIT_CONFIG_PATH)
        .env("GIT_CONFIG_GLOBAL", NULL_GIT_CONFIG_PATH);
    for driver in &control.disabled_filter_drivers {
        command
            .arg("-c")
            .arg(format!("filter.{driver}.clean="))
            .arg("-c")
            .arg(format!("filter.{driver}.process="))
            .arg("-c")
            .arg(format!("filter.{driver}.required=false"));
    }
    for key in [
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_CONFIG",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
        "GIT_DIFF_OPTS",
        "GIT_DIR",
        "GIT_EXEC_PATH",
        "GIT_EXTERNAL_DIFF",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_WORK_TREE",
    ] {
        command.env_remove(key);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;
        command.creation_flags(CREATE_SUSPENDED);
    }
    Ok(command)
}

#[derive(Debug)]
pub(crate) struct BoundedGitOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

fn git_bytes_limited(
    repo_path: &Path,
    args: &[&str],
    limit: usize,
    description: &str,
) -> Result<Vec<u8>, String> {
    let control = GitRunControl::new(
        LOCAL_GIT_TIMEOUT,
        LocalReviewCancellation::new(),
        "Git command",
    );
    git_bytes_limited_with_control(repo_path, args, limit, description, &control)
}

fn git_bytes_limited_with_control(
    repo_path: &Path,
    args: &[&str],
    limit: usize,
    description: &str,
    control: &GitRunControl,
) -> Result<Vec<u8>, String> {
    let output = run_git_bounded_with_control(repo_path, args, None, limit, description, control)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(git_error(&output.stderr, output.status.to_string()))
    }
}

fn git_output_exists_with_control(
    repo_path: &Path,
    args: &[&str],
    control: &GitRunControl,
) -> Result<bool, String> {
    control.check()?;
    let mut command = git_command(repo_path, control)?;
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to run git: {error}"))?;
    let process_tree = GitProcessTree::attach_and_resume(&child).map_err(|error| {
        let _ = child.kill();
        let _ = child.wait();
        format!("Failed to contain git process tree: {error}")
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture git output.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture git diagnostics.".to_string())?;
    let stdout_reader = spawn_output_presence_reader(stdout);
    let stderr_reader = spawn_bounded_reader(stderr, MAX_GIT_DIAGNOSTIC_BYTES);
    let mut status = None;
    let mut stdout_present = None;
    let mut stderr_bytes = None;
    let mut failure = None;
    loop {
        if let Err(error) =
            poll_output_presence_reader(&stdout_reader, &mut stdout_present, "git output")
        {
            failure = Some(error);
        }
        if let Err(error) =
            poll_bounded_reader(&stderr_reader, &mut stderr_bytes, "git diagnostics")
        {
            failure = Some(error);
        }
        if stderr_bytes
            .as_ref()
            .is_some_and(|bytes| bytes.len() > MAX_GIT_DIAGNOSTIC_BYTES)
        {
            failure = Some(format!(
                "Git diagnostics exceed the {MAX_GIT_DIAGNOSTIC_BYTES}-byte limit."
            ));
        }
        if failure.is_none() {
            if let Err(error) = control.check() {
                failure = Some(error);
            }
        }
        if let Some(failure) = failure {
            terminate_git_child(&mut child, &process_tree);
            return Err(failure);
        }
        if stdout_present == Some(true) {
            terminate_git_child(&mut child, &process_tree);
            return Ok(true);
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(Some(exit_status)) => status = Some(exit_status),
                Ok(None) => {}
                Err(error) => {
                    terminate_git_child(&mut child, &process_tree);
                    return Err(format!("Failed while waiting for git: {error}"));
                }
            }
        }
        if status.is_some() && stdout_present.is_some() && stderr_bytes.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let status = status.ok_or_else(|| "Git did not report an exit status.".to_string())?;
    if status.success() {
        Ok(false)
    } else {
        Err(git_error(
            stderr_bytes.as_deref().unwrap_or_default(),
            status.to_string(),
        ))
    }
}

#[cfg(test)]
fn run_git_bounded(
    repo_path: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
    stdout_limit: usize,
    description: &str,
    timeout: Duration,
) -> Result<BoundedGitOutput, String> {
    let control = GitRunControl::new(timeout, LocalReviewCancellation::new(), "Git command");
    run_git_bounded_with_control(repo_path, args, stdin, stdout_limit, description, &control)
}

pub(crate) fn run_git_bounded_with_control(
    repo_path: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
    stdout_limit: usize,
    description: &str,
    control: &GitRunControl,
) -> Result<BoundedGitOutput, String> {
    control.check()?;
    let mut command = git_command(repo_path, control)?;
    let mut child = command
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to run git: {error}"))?;
    let process_tree = GitProcessTree::attach_and_resume(&child).map_err(|error| {
        let _ = child.kill();
        let _ = child.wait();
        format!("Failed to contain git process tree: {error}")
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture git output.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture git diagnostics.".to_string())?;
    let stdout_reader = spawn_bounded_reader(stdout, stdout_limit);
    let stderr_reader = spawn_bounded_reader(stderr, MAX_GIT_DIAGNOSTIC_BYTES);
    let stdin_writer = stdin
        .map(|input| {
            child
                .stdin
                .take()
                .map(|writer| spawn_stdin_writer(writer, input.to_vec()))
                .ok_or_else(|| "Failed to open git input.".to_string())
        })
        .transpose()?;
    let mut status = None;
    let mut stdout_bytes = None;
    let mut stderr_bytes = None;
    let mut stdin_complete = stdin_writer.is_none();
    let mut failure = None;
    loop {
        if let Err(error) = poll_bounded_reader(&stdout_reader, &mut stdout_bytes, "git output") {
            failure = Some(error);
        }
        if let Err(error) =
            poll_bounded_reader(&stderr_reader, &mut stderr_bytes, "git diagnostics")
        {
            failure = Some(error);
        }
        if let Some(writer) = &stdin_writer {
            if let Err(error) = poll_stdin_writer(writer, &mut stdin_complete) {
                failure = Some(error);
            }
        }
        if stdout_bytes
            .as_ref()
            .is_some_and(|bytes| bytes.len() > stdout_limit)
        {
            failure = Some(format!(
                "{description} exceeds the {stdout_limit}-byte limit."
            ));
        }
        if stderr_bytes
            .as_ref()
            .is_some_and(|bytes| bytes.len() > MAX_GIT_DIAGNOSTIC_BYTES)
        {
            failure = Some(format!(
                "Git diagnostics exceed the {MAX_GIT_DIAGNOSTIC_BYTES}-byte limit."
            ));
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(Some(exit_status)) => status = Some(exit_status),
                Ok(None) => {}
                Err(error) => failure = Some(format!("Failed while waiting for git: {error}")),
            }
        }
        if failure.is_none() {
            if let Err(error) = control.check() {
                failure = Some(error);
            }
        }
        if let Some(failure) = failure {
            terminate_git_child(&mut child, &process_tree);
            return Err(failure);
        }
        if status.is_some() && stdout_bytes.is_some() && stderr_bytes.is_some() && stdin_complete {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(BoundedGitOutput {
        status: status.ok_or_else(|| "Git did not report an exit status.".to_string())?,
        stdout: stdout_bytes.ok_or_else(|| "Git output did not close.".to_string())?,
        stderr: stderr_bytes.ok_or_else(|| "Git diagnostics did not close.".to_string())?,
    })
}

fn spawn_stdin_writer(
    mut writer: impl Write + Send + 'static,
    input: Vec<u8>,
) -> Receiver<io::Result<()>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = writer.write_all(&input);
        let _ = sender.send(result);
    });
    receiver
}

fn poll_stdin_writer(
    receiver: &Receiver<io::Result<()>>,
    complete: &mut bool,
) -> Result<(), String> {
    if *complete {
        return Ok(());
    }
    match receiver.try_recv() {
        Ok(Ok(())) => {
            *complete = true;
            Ok(())
        }
        Ok(Err(error)) => Err(format!("Failed to write git input: {error}")),
        Err(TryRecvError::Empty) => Ok(()),
        Err(TryRecvError::Disconnected) => Err("Failed to write git input.".to_string()),
    }
}

fn spawn_bounded_reader(
    reader: impl Read + Send + 'static,
    limit: usize,
) -> Receiver<io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = reader
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(result);
    });
    receiver
}

fn spawn_output_presence_reader(
    mut reader: impl Read + Send + 'static,
) -> Receiver<io::Result<bool>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut first_byte = [0_u8; 1];
        let result = reader.read(&mut first_byte).map(|count| count > 0);
        let _ = sender.send(result);
    });
    receiver
}

fn poll_output_presence_reader(
    receiver: &Receiver<io::Result<bool>>,
    output: &mut Option<bool>,
    label: &str,
) -> Result<(), String> {
    if output.is_some() {
        return Ok(());
    }
    match receiver.try_recv() {
        Ok(Ok(present)) => {
            *output = Some(present);
            Ok(())
        }
        Ok(Err(error)) => Err(format!("Failed to read {label}: {error}")),
        Err(TryRecvError::Empty) => Ok(()),
        Err(TryRecvError::Disconnected) => Err(format!("Failed to inspect {label}.")),
    }
}

fn poll_bounded_reader(
    receiver: &Receiver<io::Result<Vec<u8>>>,
    output: &mut Option<Vec<u8>>,
    label: &str,
) -> Result<(), String> {
    if output.is_some() {
        return Ok(());
    }
    match receiver.try_recv() {
        Ok(Ok(bytes)) => {
            *output = Some(bytes);
            Ok(())
        }
        Ok(Err(error)) => Err(format!("Failed to read {label}: {error}")),
        Err(TryRecvError::Empty) => Ok(()),
        Err(TryRecvError::Disconnected) => Err(format!("Failed to collect {label}.")),
    }
}

fn terminate_git_child(child: &mut std::process::Child, process_tree: &GitProcessTree) {
    process_tree.terminate();
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(windows))]
struct GitProcessTree;

#[cfg(not(windows))]
impl GitProcessTree {
    fn attach_and_resume(_child: &std::process::Child) -> io::Result<Self> {
        Ok(Self)
    }

    fn terminate(&self) {}
}

#[cfg(windows)]
struct GitProcessTree {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl GitProcessTree {
    fn attach_and_resume(child: &std::process::Child) -> io::Result<Self> {
        use std::mem::size_of;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Self { handle };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(io::Error::last_os_error());
        }
        let assigned = unsafe {
            AssignProcessToJobObject(
                job.handle,
                child.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE,
            )
        };
        if assigned == 0 {
            return Err(io::Error::last_os_error());
        }
        resume_process_thread(child.id())?;
        Ok(job)
    }

    fn terminate(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.handle, 1);
        }
    }
}

#[cfg(windows)]
fn resume_process_thread(process_id: u32) -> io::Result<()> {
    use std::mem::size_of;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                THREADENTRY32,
            },
            Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut found = unsafe { Thread32First(snapshot, &mut entry) } != 0;
    let mut result = Err(io::Error::new(
        io::ErrorKind::NotFound,
        "suspended git process thread was not found",
    ));
    while found {
        if entry.th32OwnerProcessID == process_id {
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if thread.is_null() {
                result = Err(io::Error::last_os_error());
            } else {
                let resumed = unsafe { ResumeThread(thread) };
                unsafe { CloseHandle(thread) };
                result = if resumed == u32::MAX {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                };
            }
            break;
        }
        found = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
    }
    unsafe { CloseHandle(snapshot) };
    result
}

#[cfg(windows)]
impl Drop for GitProcessTree {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
fn git_text(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_bytes_limited(
        repo_path,
        args,
        MAX_GIT_METADATA_BYTES,
        "Git metadata output",
    )?;
    String::from_utf8(output)
        .map(|value| value.trim().to_string())
        .map_err(|_| "Git returned non-UTF-8 output.".to_string())
}

fn git_text_with_control(
    repo_path: &Path,
    args: &[&str],
    control: &GitRunControl,
) -> Result<String, String> {
    let output = git_bytes_limited_with_control(
        repo_path,
        args,
        MAX_GIT_METADATA_BYTES,
        "Git metadata output",
        control,
    )?;
    String::from_utf8(output)
        .map(|value| value.trim().to_string())
        .map_err(|_| "Git returned non-UTF-8 output.".to_string())
}

fn git_text_raw_limited_with_control(
    repo_path: &Path,
    args: &[&str],
    limit: usize,
    description: &str,
    control: &GitRunControl,
) -> Result<String, String> {
    String::from_utf8(git_bytes_limited_with_control(
        repo_path,
        args,
        limit,
        description,
        control,
    )?)
    .map_err(|_| "Git returned a non-UTF-8 diff.".to_string())
}

fn optional_git_text_with_control(
    repo_path: &Path,
    args: &[&str],
    control: &GitRunControl,
) -> Result<Option<String>, String> {
    let output = run_git_bounded_with_control(
        repo_path,
        args,
        None,
        MAX_GIT_METADATA_BYTES,
        "Git metadata output",
        control,
    )?;
    if !output.status.success() {
        return Ok(None);
    }
    String::from_utf8(output.stdout)
        .map(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
        .map_err(|_| "Git returned non-UTF-8 output.".to_string())
}

fn checked_git_text(output: BoundedGitOutput) -> Result<String, String> {
    if output.status.success() {
        String::from_utf8(output.stdout)
            .map(|value| value.trim().to_string())
            .map_err(|_| "Git returned non-UTF-8 output.".to_string())
    } else {
        Err(git_error(&output.stderr, output.status.to_string()))
    }
}

fn git_error(stderr: &[u8], status: String) -> String {
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if stderr.is_empty() {
        format!("Git exited with status {status}.")
    } else {
        stderr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        path: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("norn-local-review-{name}-{stamp}-{nonce}"));
            fs::create_dir_all(&path).expect("create fixture");
            run(&path, &["init", "-q"]);
            run(&path, &["config", "user.email", "norn@example.com"]);
            run(&path, &["config", "user.name", "Norn"]);
            Self { path }
        }

        fn write(&self, path: &str, value: &str) {
            let target = self.path.join(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::write(target, value).expect("write fixture");
        }

        fn commit_all(&self, message: &str) {
            run(&self.path, &["add", "."]);
            run(&self.path, &["commit", "-qm", message]);
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn run(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn git_command_disables_mutating_and_injected_git_features() {
        let fixture = Fixture::new("git-command-boundary");
        let canonical_path = fixture.path.canonicalize().expect("canonical fixture path");
        let control = GitRunControl::new(
            LOCAL_GIT_TIMEOUT,
            LocalReviewCancellation::new(),
            "Git command",
        )
        .with_disabled_filter_drivers(vec!["unsafe".to_string()]);
        let command = git_command(&fixture.path, &control).expect("git command");
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(args
            .iter()
            .any(|arg| arg == &format!("--work-tree={}", canonical_path.display())));
        assert!(args
            .windows(2)
            .any(|args| args == ["-c", "core.fsmonitor=false"]));
        assert!(args.windows(2).any(|args| args == ["-c", "diff.external="]));
        assert!(args
            .windows(2)
            .any(|args| args == ["-c", "filter.unsafe.clean="]));
        assert!(args
            .windows(2)
            .any(|args| args == ["-c", "filter.unsafe.process="]));
        assert!(args
            .windows(2)
            .any(|args| args == ["-c", "filter.unsafe.required=false"]));
        assert!(args.iter().any(|arg| arg == "--no-optional-locks"));

        let environment = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            environment.get("GIT_OPTIONAL_LOCKS"),
            Some(&Some("0".to_string()))
        );
        assert_eq!(
            environment.get("GIT_NO_LAZY_FETCH"),
            Some(&Some("1".to_string()))
        );
        assert_eq!(environment.get("GIT_CONFIG_COUNT"), Some(&None));
        assert_eq!(environment.get("GIT_CONFIG_PARAMETERS"), Some(&None));
        assert_eq!(environment.get("GIT_EXTERNAL_DIFF"), Some(&None));
        assert_eq!(
            environment.get("GIT_CONFIG_GLOBAL"),
            Some(&Some(NULL_GIT_CONFIG_PATH.to_string()))
        );
        assert_eq!(
            environment.get("GIT_CONFIG_SYSTEM"),
            Some(&Some(NULL_GIT_CONFIG_PATH.to_string()))
        );
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_does_not_execute_repository_configured_git_helpers() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new("disabled-git-helpers");
        fixture.write(".gitattributes", "*.txt diff=unsafe filter=unsafe-clean\n");
        fixture.write("tracked.txt", "base\n");
        fixture.commit_all("base");

        let fsmonitor_marker = fixture.path.join("fsmonitor-ran");
        let fsmonitor_helper = fixture.path.join("fsmonitor-helper.sh");
        fs::write(
            &fsmonitor_helper,
            format!("#!/bin/sh\n: > '{}'\n", fsmonitor_marker.display()),
        )
        .expect("write fsmonitor helper");
        let mut permissions = fs::metadata(&fsmonitor_helper)
            .expect("fsmonitor helper metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&fsmonitor_helper, permissions).expect("chmod fsmonitor helper");

        let textconv_marker = fixture.path.join("textconv-ran");
        let textconv_helper = fixture.path.join("textconv-helper.sh");
        fs::write(
            &textconv_helper,
            format!(
                "#!/bin/sh\n: > '{}'\ncat \"$1\"\n",
                textconv_marker.display()
            ),
        )
        .expect("write textconv helper");
        let mut permissions = fs::metadata(&textconv_helper)
            .expect("textconv helper metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&textconv_helper, permissions).expect("chmod textconv helper");

        let clean_marker = fixture.path.join("clean-filter-ran");
        let clean_helper = fixture.path.join("clean-filter-helper.sh");
        fs::write(
            &clean_helper,
            format!("#!/bin/sh\n: > '{}'\ncat\n", clean_marker.display()),
        )
        .expect("write clean-filter helper");
        let mut permissions = fs::metadata(&clean_helper)
            .expect("clean-filter helper metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&clean_helper, permissions).expect("chmod clean-filter helper");

        run(
            &fixture.path,
            &[
                "config",
                "core.fsmonitor",
                fsmonitor_helper.to_str().expect("utf-8 helper path"),
            ],
        );
        run(
            &fixture.path,
            &[
                "config",
                "diff.unsafe.textconv",
                textconv_helper.to_str().expect("utf-8 helper path"),
            ],
        );
        run(
            &fixture.path,
            &[
                "config",
                "filter.unsafe-clean.clean",
                clean_helper.to_str().expect("utf-8 helper path"),
            ],
        );
        run(
            &fixture.path,
            &["config", "filter.unsafe-clean.required", "true"],
        );
        fixture.write("tracked.txt", "changed\n");

        local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
            .expect("snapshot");

        assert!(!fsmonitor_marker.exists());
        assert!(!textconv_marker.exists());
        assert!(!clean_marker.exists());
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_does_not_start_repository_process_filters() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new("disabled-process-filter");
        fixture.write(".gitattributes", "*.txt filter=unsafe-process\n");
        fixture.write("tracked.txt", "base\n");
        fixture.commit_all("base");

        let marker = fixture.path.join("process-filter-ran");
        let helper = fixture.path.join("process-filter-helper.sh");
        fs::write(
            &helper,
            format!("#!/bin/sh\n: > '{}'\nsleep 5\n", marker.display()),
        )
        .expect("write process-filter helper");
        let mut permissions = fs::metadata(&helper)
            .expect("process-filter helper metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&helper, permissions).expect("chmod process-filter helper");
        run(
            &fixture.path,
            &[
                "config",
                "filter.unsafe-process.process",
                helper.to_str().expect("utf-8 helper path"),
            ],
        );
        run(
            &fixture.path,
            &["config", "filter.unsafe-process.required", "true"],
        );
        fixture.write("tracked.txt", "changed\n");
        let started = Instant::now();

        local_review_snapshot_for_path_with_control(
            ReviewProvider::Github,
            "acme",
            "demo",
            &fixture.path,
            LocalReviewCancellation::new(),
            Duration::from_secs(2),
            false,
        )
        .expect("snapshot");

        assert!(!marker.exists());
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn repository_config_cannot_redirect_the_reviewed_worktree() {
        let fixture = Fixture::new("configured-worktree-boundary");
        fixture.write("tracked.txt", "base\n");
        fixture.commit_all("base");

        let external = tempfile::tempdir().expect("external worktree");
        fs::write(external.path().join("tracked.txt"), "outside-secret\n")
            .expect("write external worktree file");
        run(
            &fixture.path,
            &[
                "config",
                "core.worktree",
                external.path().to_str().expect("utf-8 external path"),
            ],
        );
        fixture.write("tracked.txt", "local-worktree\n");

        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");

        assert!(snapshot.diff.contains("+local-worktree"));
        assert!(!snapshot.diff.contains("outside-secret"));
    }

    #[test]
    fn snapshot_combines_unpushed_and_worktree_changes_once() {
        let fixture = Fixture::new("upstream");
        fixture.write("tracked.txt", "base\n");
        fixture.commit_all("base");
        run(&fixture.path, &["branch", "upstream"]);
        fixture.write("committed.txt", "committed\n");
        fixture.commit_all("local commit");
        run(&fixture.path, &["branch", "--set-upstream-to", "upstream"]);
        fixture.write("tracked.txt", "working\n");
        fixture.write("untracked.txt", "new\n");

        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");

        assert_eq!(snapshot.commits_ahead, 1);
        assert_eq!(snapshot.commits_behind, 0);
        assert_eq!(snapshot.upstream.as_deref(), Some("upstream"));
        assert_eq!(snapshot.diff.matches("diff --git a/tracked.txt").count(), 1);
        assert!(snapshot.diff.contains("diff --git a/committed.txt"));
        assert!(!snapshot.diff.contains("untracked.txt"));
        assert_eq!(snapshot.diffstat.len(), 2);
        assert!(snapshot
            .warnings
            .iter()
            .any(|warning| warning.contains("Untracked files are excluded")));
        let tracked = snapshot
            .diffstat
            .iter()
            .find(|entry| entry.new_path.as_deref() == Some("tracked.txt"))
            .expect("tracked diffstat");
        assert_eq!((tracked.lines_added, tracked.lines_removed), (1, 1));
    }

    #[test]
    fn snapshot_without_upstream_falls_back_to_head_and_warns() {
        let fixture = Fixture::new("no-upstream");
        fixture.write("tracked.txt", "base\n");
        fixture.commit_all("base");
        fixture.write("tracked.txt", "changed\n");

        let snapshot = local_review_snapshot_for_path(
            ReviewProvider::Bitbucket,
            "acme",
            "demo",
            &fixture.path,
        )
        .expect("snapshot");

        assert!(snapshot.upstream.is_none());
        assert_eq!(snapshot.commits_ahead, 0);
        assert_eq!(snapshot.commits_behind, 0);
        assert!(snapshot.diff.contains("+changed"));
        assert!(snapshot.warnings[0].contains("no upstream"));
    }

    #[test]
    fn snapshot_for_unborn_repository_excludes_untracked_files() {
        let fixture = Fixture::new("unborn");
        fixture.write("src/new.rs", "fn main() {}\n");

        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");

        assert!(!snapshot.current_branch.is_empty());
        assert!(snapshot.head_sha.is_none());
        assert!(snapshot.diff.is_empty());
        assert!(snapshot.diffstat.is_empty());
        assert!(snapshot
            .warnings
            .iter()
            .any(|warning| warning.contains("Untracked files are excluded")));
    }

    #[test]
    fn untracked_files_are_excluded_without_revealing_their_names() {
        let fixture = Fixture::new("sensitive");
        fixture.write("README.md", "base\n");
        fixture.commit_all("base");
        fixture.write(".env", "TOKEN=private\n");
        fixture.write("safe.txt", "visible\n");

        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");

        assert!(!snapshot.diff.contains("TOKEN=private"));
        assert!(!snapshot.diff.contains("safe.txt"));
        assert!(snapshot
            .warnings
            .iter()
            .all(|warning| !warning.contains(".env") && !warning.contains("safe.txt")));
    }

    #[test]
    fn divergent_upstream_changes_are_not_reported_as_local_changes() {
        let fixture = Fixture::new("diverged");
        fixture.write("base.txt", "base\n");
        fixture.commit_all("base");
        let current_branch =
            git_text(&fixture.path, &["symbolic-ref", "--short", "HEAD"]).expect("current branch");
        run(&fixture.path, &["branch", "upstream"]);
        run(&fixture.path, &["checkout", "-q", "upstream"]);
        fixture.write("upstream-only.txt", "remote work\n");
        fixture.commit_all("upstream work");
        run(&fixture.path, &["checkout", "-q", &current_branch]);
        fixture.write("local-only.txt", "local work\n");
        fixture.commit_all("local work");
        run(&fixture.path, &["branch", "--set-upstream-to", "upstream"]);

        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");

        assert_eq!(snapshot.commits_ahead, 1);
        assert_eq!(snapshot.commits_behind, 1);
        assert!(snapshot.diff.contains("local-only.txt"));
        assert!(!snapshot.diff.contains("upstream-only.txt"));
        assert!(snapshot
            .warnings
            .iter()
            .any(|warning| warning.contains("behind its upstream")));
    }

    #[test]
    fn name_status_preserves_added_deleted_and_renamed_paths() {
        let parsed = parse_name_status(b"A\0new.png\0D\0old.png\0R100\0before.png\0after.png\0")
            .expect("parse");
        assert_eq!(parsed[0].old_path, None);
        assert_eq!(parsed[0].new_path.as_deref(), Some("new.png"));
        assert_eq!(parsed[1].old_path.as_deref(), Some("old.png"));
        assert_eq!(parsed[1].new_path, None);
        assert_eq!(parsed[2].old_path.as_deref(), Some("before.png"));
        assert_eq!(parsed[2].new_path.as_deref(), Some("after.png"));
    }

    #[test]
    fn numstat_preserves_counts_for_normal_binary_and_renamed_paths() {
        let parsed =
            parse_numstat(b"3\t2\tsrc/lib.rs\0-\t-\timage.png\x005\t1\t\0old.rs\0new.rs\0")
                .expect("parse numstat");

        assert_eq!(parsed[0], ("src/lib.rs".to_string(), 3, 2));
        assert_eq!(parsed[1], ("image.png".to_string(), 0, 0));
        assert_eq!(parsed[2], ("new.rs".to_string(), 5, 1));
    }

    #[test]
    fn new_image_preview_reads_the_immutable_snapshot_blob_after_worktree_drift() {
        let fixture = Fixture::new("image-drift");
        fixture.write("preview.png", "base-image");
        fixture.commit_all("base image");
        fixture.write("preview.png", "snapshot-image");
        run(&fixture.path, &["add", "preview.png"]);
        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");
        let expected = snapshot
            .preview_oid
            .get("preview.png")
            .expect("preview fingerprint");

        let preview = local_file_preview_for_path(
            &fixture.path,
            &snapshot.base_sha,
            &snapshot.diffstat,
            Some(expected),
            "preview.png",
            "new",
            "image/png",
        )
        .expect("matching preview");
        assert_eq!(preview.size, "snapshot-image".len());

        fixture.write("preview.png", "changed-after-snapshot");
        let preview = local_file_preview_for_path(
            &fixture.path,
            &snapshot.base_sha,
            &snapshot.diffstat,
            Some(expected),
            "preview.png",
            "new",
            "image/png",
        )
        .expect("snapshot blob remains available");
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            preview.data_url.split_once(',').expect("data URL").1,
        )
        .expect("preview bytes");
        assert_eq!(decoded, b"snapshot-image");
    }

    #[test]
    fn image_preview_rejects_paths_outside_the_snapshot_diffstat() {
        let fixture = Fixture::new("image-allowlist");
        fixture.write("changed.png", "base-changed");
        fixture.write("unrelated.png", "base-unrelated");
        fixture.commit_all("base images");
        fixture.write("changed.png", "updated");
        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");

        let error = local_file_preview_for_path(
            &fixture.path,
            &snapshot.base_sha,
            &snapshot.diffstat,
            None,
            "unrelated.png",
            "old",
            "image/png",
        )
        .expect_err("path outside snapshot");

        assert!(error.contains("not part of this local review snapshot"));
    }

    #[test]
    fn new_image_preview_requires_a_fingerprint_before_file_access() {
        let fixture = Fixture::new("missing-preview-fingerprint");
        let diffstat = vec![DiffstatEntry {
            status: "added".to_string(),
            lines_added: 0,
            lines_removed: 0,
            old_path: None,
            new_path: Some("missing.png".to_string()),
        }];

        let error = local_file_preview_for_path(
            &fixture.path,
            "0000000000000000000000000000000000000000",
            &diffstat,
            None,
            "missing.png",
            "new",
            "image/png",
        )
        .expect_err("missing fingerprint");

        assert!(error.contains("unavailable for this snapshot"));
        assert!(!error.contains("open"));
    }

    #[test]
    fn preview_hashing_enforces_cumulative_byte_and_candidate_limits() {
        let fixture = Fixture::new("preview-budget");
        let paths = ["a.png", "b.png", "c.png"];
        for path in paths {
            fixture.write(path, "1234");
        }
        run(&fixture.path, &["add", "a.png", "b.png", "c.png"]);
        let diffstat = paths
            .into_iter()
            .map(|path| DiffstatEntry {
                status: "added".to_string(),
                lines_added: 0,
                lines_removed: 0,
                old_path: None,
                new_path: Some(path.to_string()),
            })
            .collect::<Vec<_>>();

        let (byte_limited, byte_warnings) =
            collect_preview_oids_with_test_limits(&fixture.path, &diffstat, 3, 8, 4)
                .expect("byte-limited previews");
        let (candidate_limited, candidate_warnings) =
            collect_preview_oids_with_test_limits(&fixture.path, &diffstat, 1, 64, 4)
                .expect("candidate-limited previews");

        assert_eq!(byte_limited.len(), 2);
        assert!(byte_warnings
            .iter()
            .any(|warning| warning.contains("total preview byte limit")));
        assert_eq!(candidate_limited.len(), 1);
        assert!(candidate_warnings
            .iter()
            .any(|warning| warning.contains("1-file preview limit")));
    }

    #[test]
    fn preview_fingerprints_change_when_content_changes() {
        let fixture = Fixture::new("preview-revalidation");
        fixture.write("preview.png", "first");
        run(&fixture.path, &["add", "preview.png"]);
        let diffstat = vec![DiffstatEntry {
            status: "modified".to_string(),
            lines_added: 0,
            lines_removed: 0,
            old_path: Some("preview.png".to_string()),
            new_path: Some("preview.png".to_string()),
        }];
        let (first, _) = collect_preview_oids(&fixture.path, &diffstat).expect("first hash");

        fixture.write("preview.png", "other");
        run(&fixture.path, &["add", "preview.png"]);
        let (second, _) = collect_preview_oids(&fixture.path, &diffstat).expect("second hash");

        assert_ne!(first, second);
    }

    #[test]
    fn unstaged_image_content_is_not_read_into_the_snapshot() {
        let fixture = Fixture::new("unstaged-preview");
        fs::write(fixture.path.join("preview.png"), b"base\0").expect("base image");
        fixture.commit_all("base image");
        fs::write(
            fixture.path.join("preview.png"),
            b"unstaged-image-content\0",
        )
        .expect("unstaged image");

        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");

        assert!(snapshot.preview_oid.is_empty());
        assert!(snapshot
            .warnings
            .iter()
            .any(|warning| warning.contains("unstaged content")));
        assert!(!snapshot.diff.contains("unstaged-image-content"));
    }

    #[test]
    fn previews_missing_from_the_index_remain_non_fatal_warnings() {
        let fixture = Fixture::new("preview-read-warning");
        let diffstat = vec![DiffstatEntry {
            status: "added".to_string(),
            lines_added: 0,
            lines_removed: 0,
            old_path: None,
            new_path: Some("missing.png".to_string()),
        }];

        let (oids, warnings) = collect_preview_oids(&fixture.path, &diffstat)
            .expect("missing preview should be recoverable");

        assert!(oids.is_empty());
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("unavailable from the Git index")));
    }

    #[test]
    fn in_memory_blob_fingerprint_matches_git_sha1() {
        let fixture = Fixture::new("preview-blob-oid");
        let bytes = b"preview contents";
        let expected = checked_git_text(
            run_git_bounded(
                &fixture.path,
                &["hash-object", "--stdin"],
                Some(bytes),
                MAX_GIT_METADATA_BYTES,
                "Test image fingerprint",
                LOCAL_GIT_TIMEOUT,
            )
            .expect("git blob oid"),
        )
        .expect("git oid");

        assert_eq!(
            git_object_id_for_bytes(bytes, expected.len()).expect("blob oid"),
            expected
        );
    }

    #[test]
    fn git_output_is_rejected_before_exceeding_the_requested_bound() {
        let fixture = Fixture::new("bounded-output");
        fixture.write("large.txt", "0123456789abcdef\n");
        fixture.commit_all("large object");

        let error = git_bytes_limited(&fixture.path, &["show", "HEAD:large.txt"], 8, "Test output")
            .expect_err("oversized output");

        assert_eq!(error, "Test output exceeds the 8-byte limit.");
    }

    #[test]
    fn untracked_file_probe_reports_presence_without_collecting_paths() {
        let fixture = Fixture::new("untracked-presence");
        let clean_control = GitRunControl::new(
            LOCAL_GIT_TIMEOUT,
            LocalReviewCancellation::new(),
            "Local review snapshot",
        );
        assert!(!has_untracked_files(&fixture.path, &clean_control).expect("clean probe"));

        fixture.write("untracked.txt", "untracked\n");
        let dirty_control = GitRunControl::new(
            LOCAL_GIT_TIMEOUT,
            LocalReviewCancellation::new(),
            "Local review snapshot",
        );
        assert!(has_untracked_files(&fixture.path, &dirty_control).expect("dirty probe"));
    }

    #[cfg(unix)]
    #[test]
    fn output_presence_probe_terminates_an_unbounded_producer() {
        let fixture = Fixture::new("output-presence-short-circuit");
        let control = GitRunControl::new(
            Duration::from_secs(2),
            LocalReviewCancellation::new(),
            "Git output presence",
        );
        let started = Instant::now();

        let present = git_output_exists_with_control(
            &fixture.path,
            &["-c", "alias.noisy=!yes untracked", "noisy"],
            &control,
        )
        .expect("presence result");

        assert!(present);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn git_diagnostics_are_bounded_and_terminate_the_process_group() {
        let fixture = Fixture::new("bounded-git-stderr");

        let error = run_git_bounded(
            &fixture.path,
            &["-c", "alias.noisy=!yes diagnostic >&2", "noisy"],
            None,
            1_024,
            "Test output",
            Duration::from_secs(2),
        )
        .expect_err("oversized diagnostics");

        assert!(error.contains("Git diagnostics exceed"));
    }

    #[cfg(unix)]
    #[test]
    fn git_commands_are_terminated_after_the_deadline() {
        let fixture = Fixture::new("git-timeout");
        let started = Instant::now();

        let error = run_git_bounded(
            &fixture.path,
            &["-c", "alias.pause=!sleep 5", "pause"],
            None,
            1_024,
            "Test output",
            Duration::from_millis(50),
        )
        .expect_err("timed out command");

        assert!(error.contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn git_stdin_writes_are_supervised_by_the_deadline() {
        let fixture = Fixture::new("git-stdin-timeout");
        let input = vec![b'x'; 8 * 1024 * 1024];
        let started = Instant::now();

        let error = run_git_bounded(
            &fixture.path,
            &["-c", "alias.pause=!sleep 5", "pause"],
            Some(&input),
            1_024,
            "Test output",
            Duration::from_millis(50),
        )
        .expect_err("blocked stdin write timed out");

        assert!(error.contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn git_deadline_includes_pipes_held_by_descendants() {
        let fixture = Fixture::new("git-descendant-pipe-timeout");
        let started = Instant::now();

        let error = run_git_bounded(
            &fixture.path,
            &["-c", "alias.leak=!sleep 5 &", "leak"],
            None,
            1_024,
            "Test output",
            Duration::from_millis(100),
        )
        .expect_err("descendant-held pipe timed out");

        assert!(error.contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn cancelled_git_commands_terminate_the_process_group() {
        let fixture = Fixture::new("git-cancellation");
        let cancellation = LocalReviewCancellation::new();
        let cancel_from_worker = cancellation.clone();
        let control = GitRunControl::new(
            Duration::from_secs(5),
            cancellation,
            "Local review snapshot",
        );
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            cancel_from_worker.cancel();
        });
        let started = Instant::now();

        let error = run_git_bounded_with_control(
            &fixture.path,
            &["-c", "alias.pause=!sleep 5", "pause"],
            None,
            1_024,
            "Test output",
            &control,
        )
        .expect_err("cancelled command");

        assert!(error.contains("cancelled"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn git_commands_share_one_operation_deadline() {
        let fixture = Fixture::new("git-shared-deadline");
        let control = GitRunControl::new(
            Duration::from_secs(1),
            LocalReviewCancellation::new(),
            "Local review snapshot",
        );
        let started = Instant::now();

        run_git_bounded_with_control(
            &fixture.path,
            &["-c", "alias.pause=!sleep 0.2", "pause"],
            None,
            1_024,
            "Test output",
            &control,
        )
        .expect("first command before shared deadline");
        let error = run_git_bounded_with_control(
            &fixture.path,
            &["-c", "alias.pause=!sleep 1", "pause"],
            None,
            1_024,
            "Test output",
            &control,
        )
        .expect_err("second command exceeds shared deadline");

        assert!(error.contains("timed out"));
        assert!(started.elapsed() < Duration::from_millis(1_500));
    }

    #[test]
    fn review_identity_changes_with_the_loaded_diff() {
        let previews = BTreeMap::new();
        let (first_hash, first_id) = local_review_identity(
            ReviewProvider::Github,
            "acme",
            "demo",
            "feature/local",
            Some("origin/main"),
            Some("2222222222222222222222222222222222222222"),
            Some("1111111111111111111111111111111111111111"),
            "0000000000000000000000000000000000000000",
            1,
            0,
            "+first",
            &previews,
        );
        let (second_hash, second_id) = local_review_identity(
            ReviewProvider::Github,
            "acme",
            "demo",
            "feature/local",
            Some("origin/main"),
            Some("2222222222222222222222222222222222222222"),
            Some("1111111111111111111111111111111111111111"),
            "0000000000000000000000000000000000000000",
            1,
            0,
            "+second",
            &previews,
        );

        assert_ne!(first_hash, second_hash);
        assert_ne!(first_id, second_id);
        assert_eq!(first_hash.len(), 64);
        assert_eq!(second_hash.len(), 64);
        assert_ne!(first_id, 0);
        assert_ne!(second_id, 0);
    }

    #[test]
    fn review_identity_changes_with_binary_preview_content() {
        let first_previews = BTreeMap::from([("preview.png".to_string(), "first".to_string())]);
        let second_previews = BTreeMap::from([("preview.png".to_string(), "second".to_string())]);

        let first = local_review_identity(
            ReviewProvider::Github,
            "acme",
            "demo",
            "feature/local",
            Some("origin/main"),
            Some("2222222222222222222222222222222222222222"),
            Some("1111111111111111111111111111111111111111"),
            "0000000000000000000000000000000000000000",
            1,
            0,
            "binary diff marker",
            &first_previews,
        );
        let second = local_review_identity(
            ReviewProvider::Github,
            "acme",
            "demo",
            "feature/local",
            Some("origin/main"),
            Some("2222222222222222222222222222222222222222"),
            Some("1111111111111111111111111111111111111111"),
            "0000000000000000000000000000000000000000",
            1,
            0,
            "binary diff marker",
            &second_previews,
        );

        assert_ne!(first, second);
    }

    #[test]
    fn review_identity_changes_with_upstream_state() {
        let previews = BTreeMap::new();
        let first = local_review_identity(
            ReviewProvider::Github,
            "acme",
            "demo",
            "feature/local",
            Some("origin/main"),
            Some("2222222222222222222222222222222222222222"),
            Some("1111111111111111111111111111111111111111"),
            "0000000000000000000000000000000000000000",
            1,
            0,
            "+same",
            &previews,
        );
        let second = local_review_identity(
            ReviewProvider::Github,
            "acme",
            "demo",
            "feature/local",
            Some("origin/main"),
            Some("3333333333333333333333333333333333333333"),
            Some("1111111111111111111111111111111111111111"),
            "0000000000000000000000000000000000000000",
            2,
            1,
            "+same",
            &previews,
        );

        assert_ne!(first, second);
    }
}
