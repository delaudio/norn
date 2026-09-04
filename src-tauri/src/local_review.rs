use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(unix)]
use cap_fs_ext::OpenOptionsSyncExt;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{ambient_authority, fs::Dir};

use crate::config::ReviewProvider;
use crate::local_repo::resolve_local_repo_for_provider;
use crate::services::bitbucket::{DiffstatEntry, PrFilePreview, MAX_PR_IMAGE_PREVIEW_BYTES};

const MAX_LOCAL_DIFF_BYTES: usize = 16 * 1024 * 1024;
const MAX_LOCAL_CHANGED_FILES: usize = 2_000;
const MAX_GIT_METADATA_BYTES: usize = 8 * 1024 * 1024;
const MAX_GIT_DIAGNOSTIC_BYTES: usize = 256 * 1024;
const LOCAL_GIT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_LOCAL_PREVIEW_CANDIDATES: usize = 64;
const MAX_LOCAL_PREVIEW_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

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
    pub preview_sha256: BTreeMap<String, String>,
    pub warnings: Vec<String>,
}

struct CollectedLocalDiff {
    diff: String,
    diffstat: Vec<DiffstatEntry>,
    preview_sha256: BTreeMap<String, String>,
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
    let repo_path = resolve_local_repo_for_provider(provider, workspace, repo)?;
    local_review_snapshot_for_path(provider, workspace, repo, &repo_path)
}

pub(crate) fn local_review_snapshot_for_path(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
    repo_path: &Path,
) -> Result<LocalReviewSnapshot, String> {
    ensure_git_repository(repo_path)?;
    let starting_status = git_bytes_limited(
        repo_path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
        MAX_GIT_METADATA_BYTES,
        "Local repository status",
    )?;
    let head_sha = optional_git_text(repo_path, &["rev-parse", "--verify", "HEAD"])?;
    let branch = optional_git_text(repo_path, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    let current_branch = branch.clone().unwrap_or_else(|| {
        head_sha
            .as_deref()
            .map(|sha| format!("HEAD ({})", &sha[..sha.len().min(12)]))
            .unwrap_or_else(|| "unborn HEAD".to_string())
    });
    let upstream = if branch.is_some() {
        optional_git_text(
            repo_path,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
        )?
    } else {
        None
    };
    let upstream_sha = upstream
        .as_ref()
        .map(|value| optional_git_text(repo_path, &["rev-parse", "--verify", value]))
        .transpose()?
        .flatten();
    let base_sha =
        if let (Some(upstream_sha), Some(head_sha)) = (upstream_sha.as_ref(), head_sha.as_ref()) {
            git_text(repo_path, &["merge-base", upstream_sha, head_sha])?
        } else if let Some(sha) = head_sha.as_ref() {
            sha.clone()
        } else {
            empty_tree_oid(repo_path)?
        };
    let (commits_ahead, commits_behind) = if let Some(upstream) = upstream.as_ref() {
        ahead_behind(repo_path, upstream)?
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
    if status_contains_untracked(&starting_status) {
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

    let mut collected = collect_local_diff(repo_path, &base_sha)?;
    let (preview_sha256, preview_warnings) =
        collect_preview_hashes(repo_path, &collected.diffstat)?;
    collected.preview_sha256 = preview_sha256;
    collected.warnings.extend(preview_warnings);
    warnings.extend(collected.warnings.clone());

    let ending_status = git_bytes_limited(
        repo_path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
        MAX_GIT_METADATA_BYTES,
        "Local repository status",
    )?;
    let ending_head = optional_git_text(repo_path, &["rev-parse", "--verify", "HEAD"])?;
    let ending_branch =
        optional_git_text(repo_path, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    let ending_upstream = if ending_branch.is_some() {
        optional_git_text(
            repo_path,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
        )?
    } else {
        None
    };
    let ending_upstream_sha = ending_upstream
        .as_ref()
        .map(|value| optional_git_text(repo_path, &["rev-parse", "--verify", value]))
        .transpose()?
        .flatten();
    if starting_status != ending_status
        || head_sha != ending_head
        || branch != ending_branch
        || upstream != ending_upstream
        || upstream_sha != ending_upstream_sha
    {
        return Err(
            "Local changes changed while Norn was loading them. Refresh and try again.".to_string(),
        );
    }
    let verification = collect_local_diff(repo_path, &base_sha)?;
    let (verification_preview_sha256, _) =
        collect_preview_hashes(repo_path, &verification.diffstat)?;
    if collected.diff != verification.diff
        || collected.diffstat != verification.diffstat
        || collected.preview_sha256 != verification_preview_sha256
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
        &collected.preview_sha256,
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
        preview_sha256: collected.preview_sha256,
        warnings,
    })
}

fn ahead_behind(repo_path: &Path, upstream: &str) -> Result<(u32, u32), String> {
    let range = format!("{upstream}...HEAD");
    let output = git_text(repo_path, &["rev-list", "--left-right", "--count", &range])?;
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
    preview_sha256: &BTreeMap<String, String>,
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
    hasher.update((preview_sha256.len() as u64).to_be_bytes());
    for (path, sha256) in preview_sha256 {
        for part in [path.as_str(), sha256.as_str()] {
            hasher.update((part.len() as u64).to_be_bytes());
            hasher.update(part.as_bytes());
        }
    }
    let digest = hasher.finalize();
    let review_id = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) | 0x8000_0000;
    (hex::encode(digest), review_id)
}

fn collect_local_diff(repo_path: &Path, base_sha: &str) -> Result<CollectedLocalDiff, String> {
    let diffstat = tracked_diffstat(repo_path, base_sha)?;
    if diffstat.len() > MAX_LOCAL_CHANGED_FILES {
        return Err(format!(
            "Local review contains {} changed files, exceeding the {}-file limit.",
            diffstat.len(),
            MAX_LOCAL_CHANGED_FILES
        ));
    }
    let diff = git_text_raw_limited(
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
        preview_sha256: BTreeMap::new(),
        warnings,
    })
}

fn status_contains_untracked(status: &[u8]) -> bool {
    status
        .split(|byte| *byte == 0)
        .any(|entry| entry.starts_with(b"?? "))
}

#[allow(clippy::too_many_arguments)]
pub fn get_local_file_preview_native(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
    base_sha: &str,
    diffstat: &[DiffstatEntry],
    expected_new_sha256: Option<&str>,
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
        expected_new_sha256,
        path,
        side,
        mime_type,
    )
}

fn local_file_preview_for_path(
    repo_path: &Path,
    base_sha: &str,
    diffstat: &[DiffstatEntry],
    expected_new_sha256: Option<&str>,
    path: &str,
    side: &str,
    mime_type: &str,
) -> Result<PrFilePreview, String> {
    ensure_snapshot_preview_path(diffstat, path, side)?;
    let bytes = if side == "old" {
        let object = format!("{base_sha}:{path}");
        git_bytes_limited(
            repo_path,
            &["show", object.as_str()],
            MAX_PR_IMAGE_PREVIEW_BYTES,
            "Image preview",
        )?
    } else {
        let expected = expected_new_sha256.ok_or_else(|| {
            "The local image preview is unavailable for this snapshot; refresh and try again."
                .to_string()
        })?;
        let root = open_repo_dir(repo_path)?;
        let file = open_repo_file(&root, Path::new(path))
            .map_err(|error| format!("Failed to open local image preview: {error}"))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("Failed to inspect local image preview: {error}"))?;
        if has_multiple_hard_links(&file, &metadata)
            .map_err(|error| format!("Failed to inspect local image preview links: {error}"))?
        {
            return Err(
                "Local image previews cannot use files with multiple hard links.".to_string(),
            );
        }
        let bytes = read_bounded(file, MAX_PR_IMAGE_PREVIEW_BYTES)?;
        if sha256_hex(&bytes) != expected {
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

fn ensure_git_repository(repo_path: &Path) -> Result<(), String> {
    if git_text(repo_path, &["rev-parse", "--is-inside-work-tree"])? == "true" {
        Ok(())
    } else {
        Err("Configured local path is not a Git working tree.".to_string())
    }
}

fn empty_tree_oid(repo_path: &Path) -> Result<String, String> {
    let output = run_git_bounded(
        repo_path,
        &["hash-object", "-t", "tree", "--stdin"],
        Some(&[]),
        MAX_GIT_METADATA_BYTES,
        "Git hash-object output",
        LOCAL_GIT_TIMEOUT,
    )?;
    checked_git_text(output)
}

fn tracked_diffstat(repo_path: &Path, base_sha: &str) -> Result<Vec<DiffstatEntry>, String> {
    let output = git_bytes_limited(
        repo_path,
        &[
            "diff",
            "--name-status",
            "-z",
            "--find-renames",
            base_sha,
            "--",
        ],
        MAX_GIT_METADATA_BYTES,
        "Local review file metadata",
    )?;
    let mut entries = parse_name_status(&output)?;
    let numstat = git_bytes_limited(
        repo_path,
        &["diff", "--numstat", "-z", "--find-renames", base_sha, "--"],
        MAX_GIT_METADATA_BYTES,
        "Local review line metadata",
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

fn collect_preview_hashes(
    repo_path: &Path,
    diffstat: &[DiffstatEntry],
) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    collect_preview_hashes_with_limits(
        repo_path,
        diffstat,
        MAX_LOCAL_PREVIEW_CANDIDATES,
        MAX_LOCAL_PREVIEW_TOTAL_BYTES,
        MAX_PR_IMAGE_PREVIEW_BYTES as u64,
    )
}

fn collect_preview_hashes_with_limits(
    repo_path: &Path,
    diffstat: &[DiffstatEntry],
    candidate_limit: usize,
    total_byte_limit: u64,
    file_byte_limit: u64,
) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    let root = open_repo_dir(repo_path)?;
    let mut hashes = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut total_bytes = 0_u64;
    for (candidate_index, path) in diffstat
        .iter()
        .filter_map(|entry| entry.new_path.as_deref())
        .filter(|path| raster_mime_type(path).is_some())
        .enumerate()
    {
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
        let Ok(file) = open_repo_file(&root, Path::new(path)) else {
            warnings.push(format!(
                "Skipped image preview that could not be opened safely `{}`.",
                path.escape_default()
            ));
            continue;
        };
        let metadata = match file.metadata() {
            Ok(metadata) => metadata,
            Err(_) => {
                warnings.push(format!(
                    "Skipped image preview that could not be inspected `{}`.",
                    path.escape_default()
                ));
                continue;
            }
        };
        if has_multiple_hard_links(&file, &metadata).unwrap_or(true) {
            warnings.push(format!(
                "Skipped image preview with unsafe link metadata `{}`.",
                path.escape_default()
            ));
            continue;
        }
        if !metadata.is_file() || metadata.len() > file_byte_limit {
            warnings.push(format!(
                "Skipped oversized image preview `{}`.",
                path.escape_default()
            ));
            continue;
        }
        let remaining = total_byte_limit.saturating_sub(total_bytes);
        if metadata.len() > remaining {
            warnings.push(format!(
                "Skipped additional image previews starting with `{}` because the total preview byte limit was reached.",
                path.escape_default()
            ));
            break;
        }
        let mut bytes = Vec::new();
        if file.take(metadata.len()).read_to_end(&mut bytes).is_err() {
            warnings.push(format!(
                "Skipped image preview that could not be read `{}`.",
                path.escape_default()
            ));
            continue;
        }
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        if bytes.len() as u64 != metadata.len() {
            return Err(format!(
                "Local image `{}` changed while Norn was loading it. Refresh and try again.",
                path.escape_default()
            ));
        }
        hashes.insert(path.to_string(), sha256_hex(&bytes));
    }
    Ok((hashes, warnings))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
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

#[cfg(unix)]
fn has_multiple_hard_links(_file: &File, metadata: &fs::Metadata) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;

    Ok(metadata.nlink() > 1)
}

#[cfg(windows)]
fn has_multiple_hard_links(file: &File, _metadata: &fs::Metadata) -> io::Result<bool> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    let succeeded =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        let information = unsafe { information.assume_init() };
        Ok(information.nNumberOfLinks > 1)
    }
}

#[cfg(not(any(unix, windows)))]
fn has_multiple_hard_links(_file: &File, _metadata: &fs::Metadata) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "hard-link inspection is unavailable on this platform",
    ))
}

fn read_bounded(reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read image preview: {error}"))?;
    bounded_bytes(bytes, limit)
}

fn bounded_bytes(bytes: Vec<u8>, limit: usize) -> Result<Vec<u8>, String> {
    if bytes.len() > limit {
        Err(format!("Image preview exceeds the {limit}-byte limit."))
    } else {
        Ok(bytes)
    }
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

fn open_repo_dir(repo_path: &Path) -> Result<Dir, String> {
    Dir::open_ambient_dir(repo_path, ambient_authority())
        .map_err(|error| format!("Failed to open local repository safely: {error}"))
}

fn open_repo_file(root: &Dir, relative: &Path) -> io::Result<File> {
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    options.nonblock(true);
    root.open_with(relative, &options)
        .map(|file| file.into_std())
}

fn git_command(repo_path: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_path);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

#[derive(Debug)]
struct BoundedGitOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn git_bytes_limited(
    repo_path: &Path,
    args: &[&str],
    limit: usize,
    description: &str,
) -> Result<Vec<u8>, String> {
    let output = run_git_bounded(repo_path, args, None, limit, description, LOCAL_GIT_TIMEOUT)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(git_error(&output.stderr, output.status.to_string()))
    }
}

fn run_git_bounded(
    repo_path: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
    stdout_limit: usize,
    description: &str,
    timeout: Duration,
) -> Result<BoundedGitOutput, String> {
    let mut child = git_command(repo_path)
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
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .ok_or_else(|| "Failed to open git input.".to_string())?
            .write_all(input)
            .map_err(|error| format!("Failed to write git input: {error}"))?;
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture git output.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture git diagnostics.".to_string())?;
    let output_exceeded = Arc::new(AtomicBool::new(false));
    let stdout_reader = spawn_bounded_reader(stdout, stdout_limit, Arc::clone(&output_exceeded));
    let stderr_reader = spawn_bounded_reader(
        stderr,
        MAX_GIT_DIAGNOSTIC_BYTES,
        Arc::clone(&output_exceeded),
    );
    let started = Instant::now();
    let mut status = None;
    let mut failure = None;
    loop {
        match child.try_wait() {
            Ok(Some(exit_status)) => {
                status = Some(exit_status);
                break;
            }
            Ok(None) if output_exceeded.load(Ordering::Acquire) => {
                terminate_git_child(&mut child);
                failure = Some("Git output exceeded a configured byte limit.".to_string());
                break;
            }
            Ok(None) if started.elapsed() >= timeout => {
                terminate_git_child(&mut child);
                failure = Some(format!(
                    "Git command timed out after {} seconds.",
                    timeout.as_secs_f64()
                ));
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                terminate_git_child(&mut child);
                failure = Some(format!("Failed while waiting for git: {error}"));
                break;
            }
        }
    }
    let stdout_bytes = stdout_reader
        .join()
        .map_err(|_| "Failed to collect git output.".to_string())?
        .map_err(|error| format!("Failed to read git output: {error}"))?;
    let stderr_bytes = stderr_reader
        .join()
        .map_err(|_| "Failed to collect git diagnostics.".to_string())?
        .map_err(|error| format!("Failed to read git diagnostics: {error}"))?;
    if stdout_bytes.len() > stdout_limit {
        return Err(format!(
            "{description} exceeds the {stdout_limit}-byte limit."
        ));
    }
    if stderr_bytes.len() > MAX_GIT_DIAGNOSTIC_BYTES {
        return Err(format!(
            "Git diagnostics exceed the {MAX_GIT_DIAGNOSTIC_BYTES}-byte limit."
        ));
    }
    if let Some(failure) = failure {
        return Err(failure);
    }
    Ok(BoundedGitOutput {
        status: status.ok_or_else(|| "Git did not report an exit status.".to_string())?,
        stdout: stdout_bytes,
        stderr: stderr_bytes,
    })
}

fn spawn_bounded_reader(
    reader: impl Read + Send + 'static,
    limit: usize,
    exceeded: Arc<AtomicBool>,
) -> thread::JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        reader.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            exceeded.store(true, Ordering::Release);
        }
        Ok(bytes)
    })
}

fn terminate_git_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

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

fn git_text_raw_limited(
    repo_path: &Path,
    args: &[&str],
    limit: usize,
    description: &str,
) -> Result<String, String> {
    String::from_utf8(git_bytes_limited(repo_path, args, limit, description)?)
        .map_err(|_| "Git returned a non-UTF-8 diff.".to_string())
}

fn optional_git_text(repo_path: &Path, args: &[&str]) -> Result<Option<String>, String> {
    let output = run_git_bounded(
        repo_path,
        args,
        None,
        MAX_GIT_METADATA_BYTES,
        "Git metadata output",
        LOCAL_GIT_TIMEOUT,
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
    fn new_image_preview_rejects_content_that_drifted_after_snapshot() {
        let fixture = Fixture::new("image-drift");
        fixture.write("preview.png", "base-image");
        fixture.commit_all("base image");
        fixture.write("preview.png", "snapshot-image");
        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");
        let expected = snapshot
            .preview_sha256
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
        let error = local_file_preview_for_path(
            &fixture.path,
            &snapshot.base_sha,
            &snapshot.diffstat,
            Some(expected),
            "preview.png",
            "new",
            "image/png",
        )
        .expect_err("drifted preview");
        assert!(error.contains("changed after this snapshot"));
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

    #[cfg(unix)]
    #[test]
    fn capability_open_rejects_intermediate_symlinks_that_escape_the_repository() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new("intermediate-link");
        let external = tempfile::tempdir().expect("external directory");
        fs::write(external.path().join("secret.txt"), "private\n").expect("external secret");
        symlink(external.path(), fixture.path.join("redirect")).expect("directory symlink");
        let root = open_repo_dir(&fixture.path).expect("repository capability");

        let error = open_repo_file(&root, Path::new("redirect/secret.txt"))
            .expect_err("outside path must be rejected");

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[cfg(unix)]
    #[test]
    fn capability_open_does_not_block_on_a_fifo() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::time::{Duration, Instant};

        let fixture = Fixture::new("fifo");
        let fifo_path = fixture.path.join("preview.png");
        let fifo_path_c = CString::new(fifo_path.as_os_str().as_bytes()).expect("fifo path");
        assert_eq!(unsafe { libc::mkfifo(fifo_path_c.as_ptr(), 0o600) }, 0);
        let writer_path = fifo_path.clone();
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(250));
            let _ = fs::OpenOptions::new().write(true).open(writer_path);
        });
        let root = open_repo_dir(&fixture.path).expect("repository capability");

        let started = Instant::now();
        let file = open_repo_file(&root, Path::new("preview.png")).expect("open fifo");
        let elapsed = started.elapsed();

        assert!(elapsed < Duration::from_millis(100));
        assert!(!file.metadata().expect("fifo metadata").is_file());
        writer.join().expect("writer thread");
    }

    #[test]
    fn preview_hashing_enforces_cumulative_byte_and_candidate_limits() {
        let fixture = Fixture::new("preview-budget");
        let paths = ["a.png", "b.png", "c.png"];
        for path in paths {
            fixture.write(path, "1234");
        }
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
            collect_preview_hashes_with_limits(&fixture.path, &diffstat, 3, 8, 4)
                .expect("byte-limited previews");
        let (candidate_limited, candidate_warnings) =
            collect_preview_hashes_with_limits(&fixture.path, &diffstat, 1, 64, 4)
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
        let diffstat = vec![DiffstatEntry {
            status: "modified".to_string(),
            lines_added: 0,
            lines_removed: 0,
            old_path: Some("preview.png".to_string()),
            new_path: Some("preview.png".to_string()),
        }];
        let (first, _) = collect_preview_hashes(&fixture.path, &diffstat).expect("first hash");

        fixture.write("preview.png", "other");
        let (second, _) = collect_preview_hashes(&fixture.path, &diffstat).expect("second hash");

        assert_ne!(first, second);
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
