use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::ReviewProvider;
use crate::local_repo::resolve_local_repo_for_provider;
use crate::services::bitbucket::{DiffstatEntry, PrFilePreview, MAX_PR_IMAGE_PREVIEW_BYTES};

pub(crate) const MAX_UNTRACKED_FILE_BYTES: u64 = 512 * 1024;
pub(crate) const MAX_UNTRACKED_TOTAL_BYTES: u64 = 2 * 1024 * 1024;
const MAX_UNTRACKED_CANDIDATES: usize = 2_000;
const MAX_LOCAL_DIFF_BYTES: usize = 16 * 1024 * 1024;
const MAX_LOCAL_CHANGED_FILES: usize = 2_000;
const MAX_GIT_METADATA_BYTES: usize = 8 * 1024 * 1024;

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
    let head_sha = optional_git_text(repo_path, &["rev-parse", "--verify", "HEAD"]);
    let branch = optional_git_text(repo_path, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
    let current_branch = branch.clone().unwrap_or_else(|| {
        head_sha
            .as_deref()
            .map(|sha| format!("HEAD ({})", &sha[..sha.len().min(12)]))
            .unwrap_or_else(|| "unborn HEAD".to_string())
    });
    let upstream = branch.as_ref().and_then(|_| {
        optional_git_text(
            repo_path,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
        )
    });
    let upstream_sha = upstream
        .as_ref()
        .and_then(|value| optional_git_text(repo_path, &["rev-parse", "--verify", value]));
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
            "The repository has no commits or upstream; showing tracked and eligible untracked files relative to an empty tree."
                .to_string()
        });
    }
    if commits_behind > 0 {
        warnings.push(format!(
            "The current branch is {commits_behind} commit(s) behind its upstream; local changes are shown from their merge base."
        ));
    }

    let collected = collect_local_diff(repo_path, &base_sha)?;
    warnings.extend(collected.warnings.clone());

    let ending_status = git_bytes_limited(
        repo_path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
        MAX_GIT_METADATA_BYTES,
        "Local repository status",
    )?;
    let ending_head = optional_git_text(repo_path, &["rev-parse", "--verify", "HEAD"]);
    let ending_upstream_sha = upstream
        .as_ref()
        .and_then(|value| optional_git_text(repo_path, &["rev-parse", "--verify", value]));
    if starting_status != ending_status
        || head_sha != ending_head
        || upstream_sha != ending_upstream_sha
    {
        return Err(
            "Local changes changed while Norn was loading them. Refresh and try again.".to_string(),
        );
    }
    let verification = collect_local_diff(repo_path, &base_sha)?;
    if collected.diff != verification.diff
        || collected.diffstat != verification.diffstat
        || collected.preview_sha256 != verification.preview_sha256
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
        head_sha.as_deref(),
        &base_sha,
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
    head_sha: Option<&str>,
    base_sha: &str,
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
        head_sha.unwrap_or_default(),
        base_sha,
        diff,
    ] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
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
    let mut diffstat = tracked_diffstat(repo_path, base_sha)?;
    if diffstat.len() > MAX_LOCAL_CHANGED_FILES {
        return Err(format!(
            "Local review contains {} changed files, exceeding the {}-file limit.",
            diffstat.len(),
            MAX_LOCAL_CHANGED_FILES
        ));
    }
    let mut diff = git_text_raw_limited(
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
    let mut warnings = Vec::new();
    append_untracked_files(repo_path, &mut diff, &mut warnings, &mut diffstat)?;
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
    let preview_sha256 = collect_preview_hashes(repo_path, &diffstat)?;
    Ok(CollectedLocalDiff {
        diff,
        diffstat,
        preview_sha256,
        warnings,
    })
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
        let root = repo_path
            .canonicalize()
            .map_err(|error| format!("Failed to resolve local repository: {error}"))?;
        let candidate = root.join(path);
        let resolved = candidate
            .canonicalize()
            .map_err(|error| format!("Failed to resolve local image preview: {error}"))?;
        if !resolved.starts_with(&root) {
            return Err("Local image preview escaped the repository root.".to_string());
        }
        let file = open_untracked_file(&resolved)
            .map_err(|error| format!("Failed to open local image preview: {error}"))?;
        let bytes = read_bounded(file, MAX_PR_IMAGE_PREVIEW_BYTES)?;
        let expected = expected_new_sha256.ok_or_else(|| {
            "The local image preview is unavailable for this snapshot; refresh and try again."
                .to_string()
        })?;
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
    let mut child = git_command(repo_path)
        .args(["hash-object", "-t", "tree", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start git hash-object: {error}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open git hash-object input.".to_string())?
        .write_all(&[])
        .map_err(|error| format!("Failed to write git hash-object input: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("Failed to finish git hash-object: {error}"))?;
    checked_text(output)
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
) -> Result<BTreeMap<String, String>, String> {
    let root = repo_path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve local repository: {error}"))?;
    let mut hashes = BTreeMap::new();
    for path in diffstat
        .iter()
        .filter_map(|entry| entry.new_path.as_deref())
    {
        if raster_mime_type(path).is_none() || validate_repo_relative_path(path).is_err() {
            continue;
        }
        let candidate = root.join(path);
        let Ok(resolved) = candidate.canonicalize() else {
            continue;
        };
        if !resolved.starts_with(&root) {
            continue;
        }
        let Ok(file) = open_untracked_file(&resolved) else {
            continue;
        };
        let Ok(bytes) = read_bounded(file, MAX_PR_IMAGE_PREVIEW_BYTES) else {
            continue;
        };
        hashes.insert(path.to_string(), sha256_hex(&bytes));
    }
    Ok(hashes)
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

pub(crate) fn append_untracked_files(
    repo_path: &Path,
    diff: &mut String,
    warnings: &mut Vec<String>,
    diffstat: &mut Vec<DiffstatEntry>,
) -> Result<(), String> {
    let output = git_bytes_limited(
        repo_path,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        MAX_GIT_METADATA_BYTES,
        "Untracked file metadata",
    )?;
    let mut scanned_bytes = 0_u64;
    for (candidate_index, raw_path) in output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .enumerate()
    {
        if candidate_index >= MAX_UNTRACKED_CANDIDATES {
            warnings.push(format!(
                "Skipped additional untracked files because the {}-file scan limit was reached.",
                MAX_UNTRACKED_CANDIDATES
            ));
            break;
        }
        let Some(relative) = utf8_untracked_path(raw_path, warnings) else {
            continue;
        };
        let display_relative = relative.escape_default().to_string();
        if is_sensitive_untracked_path(&relative) {
            warnings.push(format!(
                "Skipped potentially sensitive untracked file `{display_relative}`."
            ));
            continue;
        }
        if !is_safe_synthetic_diff_path(&relative) {
            warnings.push(format!(
                "Skipped untracked file with a path that cannot be represented safely in a synthetic diff: `{display_relative}`."
            ));
            continue;
        }
        let path = repo_path.join(untracked_relative_path(raw_path));
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(metadata) if metadata.file_type().is_symlink() => {
                warnings.push(format!("Skipped untracked symlink `{display_relative}`."));
                continue;
            }
            _ => continue,
        };
        let file = match open_untracked_file(&path) {
            Ok(file) => file,
            Err(_)
                if fs::symlink_metadata(&path)
                    .is_ok_and(|metadata| metadata.file_type().is_symlink()) =>
            {
                warnings.push(format!("Skipped untracked symlink `{display_relative}`."));
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "Failed to open untracked file `{display_relative}`: {error}"
                ));
            }
        };
        let opened_metadata = file.metadata().map_err(|error| {
            format!("Failed to inspect untracked file `{display_relative}`: {error}")
        })?;
        if !opened_metadata.is_file() {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if opened_metadata.nlink() > 1 {
                warnings.push(format!(
                    "Skipped untracked file with multiple hard links `{display_relative}`."
                ));
                continue;
            }
        }
        if opened_metadata.len() > MAX_UNTRACKED_FILE_BYTES {
            warnings.push(format!(
                "Skipped large untracked file `{display_relative}`."
            ));
            continue;
        }
        if scanned_bytes >= MAX_UNTRACKED_TOTAL_BYTES {
            warnings.push(format!(
                "Skipped additional untracked files starting with `{display_relative}` because the total untracked-file byte limit was reached."
            ));
            break;
        }
        let remaining = MAX_UNTRACKED_TOTAL_BYTES.saturating_sub(scanned_bytes);
        if opened_metadata.len() > remaining {
            warnings.push(format!(
                "Skipped additional untracked files starting with `{display_relative}` because the total untracked-file byte limit was reached."
            ));
            break;
        }
        let mut contents = Vec::new();
        file.take(opened_metadata.len())
            .read_to_end(&mut contents)
            .map_err(|error| {
                format!("Failed to read untracked file `{display_relative}`: {error}")
            })?;
        scanned_bytes = scanned_bytes.saturating_add(contents.len() as u64);
        if contents.len() as u64 != opened_metadata.len() {
            return Err(format!(
                "Untracked file `{display_relative}` changed while Norn was loading it. Refresh and try again."
            ));
        }
        if contents.contains(&0) {
            warnings.push(format!(
                "Skipped binary untracked file `{display_relative}`."
            ));
            continue;
        }
        let text = match std::str::from_utf8(&contents) {
            Ok(text) => text,
            Err(_) => {
                warnings.push(format!(
                    "Skipped non-UTF-8 untracked file `{display_relative}`."
                ));
                continue;
            }
        };
        append_diff(diff, &new_file_patch(&relative, text));
        diffstat.push(DiffstatEntry {
            status: "added".to_string(),
            lines_added: u32::try_from(text.lines().count()).unwrap_or(u32::MAX),
            lines_removed: 0,
            old_path: None,
            new_path: Some(relative),
        });
    }
    Ok(())
}

fn utf8_untracked_path(raw_path: &[u8], warnings: &mut Vec<String>) -> Option<String> {
    match std::str::from_utf8(raw_path) {
        Ok(relative) => Some(relative.to_string()),
        Err(_) => {
            let display_relative = String::from_utf8_lossy(raw_path)
                .escape_default()
                .to_string();
            warnings.push(format!(
                "Skipped untracked file with a non-UTF-8 path `{display_relative}`."
            ));
            None
        }
    }
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

fn open_untracked_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    options.open(path)
}

pub(crate) fn is_safe_synthetic_diff_path(relative: &str) -> bool {
    !relative
        .chars()
        .any(|character| character.is_control() || matches!(character, '"' | '\\'))
}

fn untracked_relative_path(raw_path: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(OsStr::from_bytes(raw_path))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(raw_path).as_ref())
    }
}

fn append_diff(diff: &mut String, patch: &str) {
    if !diff.is_empty() && !diff.ends_with('\n') && !patch.is_empty() {
        diff.push('\n');
    }
    if !diff.is_empty() && !patch.is_empty() {
        diff.push('\n');
    }
    diff.push_str(patch);
}

pub(crate) fn is_sensitive_untracked_path(relative: &str) -> bool {
    let normalized = relative.replace('\\', "/").to_ascii_lowercase();
    let path = Path::new(&normalized);
    let file_name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
    let extension = path.extension().and_then(OsStr::to_str).unwrap_or_default();
    let normalized_file_name = file_name.replace('-', "_");
    let has_sensitive_name_token = file_name
        .trim_start_matches('.')
        .split(['.', '_', '-'])
        .any(|part| {
            matches!(
                part,
                "secret"
                    | "secrets"
                    | "password"
                    | "passwords"
                    | "passwd"
                    | "token"
                    | "tokens"
                    | "credential"
                    | "credentials"
            )
        });
    if normalized.split('/').any(|component| {
        matches!(
            component,
            ".ssh" | ".aws" | ".gnupg" | ".kube" | ".docker" | ".azure" | "gcloud" | ".configstore"
        )
    }) {
        return true;
    }
    if file_name == ".env"
        || file_name == ".envrc"
        || file_name.starts_with(".envrc.")
        || file_name.starts_with(".env.")
        || file_name.ends_with(".env")
        || file_name.starts_with("env.")
        || file_name.contains(".env.")
        || matches!(
            file_name,
            ".npmrc"
                | ".pypirc"
                | ".netrc"
                | ".git-credentials"
                | ".authinfo"
                | ".authinfo.gpg"
                | ".boto"
                | "id_rsa"
                | "id_dsa"
                | "id_ecdsa"
                | "id_ed25519"
                | "credentials"
                | "credentials.json"
                | "auth.json"
                | "application_default_credentials.json"
                | "access_tokens.db"
                | "accesstokens.json"
                | "access_tokens.json"
        )
        || file_name.starts_with(".npmrc.")
        || file_name.starts_with(".pypirc.")
        || file_name.starts_with(".netrc.")
        || file_name == "terraform.tfvars"
        || file_name.ends_with(".auto.tfvars")
        || file_name.ends_with(".tfvars.json")
    {
        return true;
    }
    if matches!(
        extension,
        "pem" | "key" | "p12" | "pfx" | "jks" | "keystore" | "der"
    ) {
        return true;
    }
    let likely_secret_text = file_name.starts_with('.')
        || extension.is_empty()
        || matches!(
            extension,
            "txt"
                | "md"
                | "json"
                | "yaml"
                | "yml"
                | "toml"
                | "ini"
                | "conf"
                | "config"
                | "properties"
                | "csv"
                | "log"
        );
    let first_name_segment = normalized_file_name
        .trim_start_matches('.')
        .split('.')
        .next()
        .unwrap_or_default();
    if likely_secret_text
        && (has_sensitive_name_token
            || matches!(
                first_name_segment,
                "api_key" | "private_key" | "access_token" | "auth_token" | "refresh_token"
            ))
    {
        return true;
    }
    matches!(extension, "json" | "yaml" | "yml" | "toml")
        && (normalized_file_name.contains("secret")
            || normalized_file_name.contains("credential")
            || normalized_file_name.contains("service_account")
            || normalized_file_name.contains("private_key")
            || normalized_file_name.contains("access_token")
            || normalized_file_name.contains("api_token")
            || normalized_file_name.contains("auth_token")
            || normalized_file_name.contains("refresh_token"))
}

pub(crate) fn new_file_patch(path: &str, contents: &str) -> String {
    let escaped_path = path.replace('\\', "/").replace('\n', "\\n");
    let line_count = contents.lines().count();
    let mut patch = format!(
        "diff --git a/{escaped_path} b/{escaped_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{escaped_path}\n@@ -0,0 +1,{line_count} @@\n"
    );
    for line in contents.lines() {
        patch.push('+');
        patch.push_str(line);
        patch.push('\n');
    }
    if !contents.is_empty() && !contents.ends_with('\n') {
        patch.push_str("\\ No newline at end of file\n");
    }
    patch
}

fn git_command(repo_path: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_path);
    command
}

fn git_bytes(repo_path: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = git_command(repo_path)
        .args(args)
        .output()
        .map_err(|error| format!("Failed to run git: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(git_error(&output.stderr, output.status.to_string()))
    }
}

fn git_bytes_limited(
    repo_path: &Path,
    args: &[&str],
    limit: usize,
    description: &str,
) -> Result<Vec<u8>, String> {
    let mut child = git_command(repo_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to run git: {error}"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture git output.".to_string())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture git diagnostics.".to_string())?;
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr.read_to_end(&mut bytes);
        (result, bytes)
    });
    let mut bytes = Vec::new();
    let read_result = stdout
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes);
    if read_result.is_err() || bytes.len() > limit {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .map_err(|error| format!("Failed to finish git: {error}"))?;
    let (stderr_result, stderr_bytes) = stderr_reader
        .join()
        .map_err(|_| "Failed to collect git diagnostics.".to_string())?;
    read_result.map_err(|error| format!("Failed to read git output: {error}"))?;
    stderr_result.map_err(|error| format!("Failed to read git diagnostics: {error}"))?;
    if bytes.len() > limit {
        return Err(format!("{description} exceeds the {limit}-byte limit."));
    }
    if status.success() {
        Ok(bytes)
    } else {
        Err(git_error(&stderr_bytes, status.to_string()))
    }
}

fn git_text(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_bytes(repo_path, args)?;
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

fn optional_git_text(repo_path: &Path, args: &[&str]) -> Option<String> {
    git_text(repo_path, args)
        .ok()
        .filter(|value| !value.is_empty())
}

fn checked_text(output: std::process::Output) -> Result<String, String> {
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
        assert!(snapshot.diff.contains("diff --git a/untracked.txt"));
        assert_eq!(snapshot.diffstat.len(), 3);
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
    fn snapshot_for_unborn_repository_includes_safe_untracked_files() {
        let fixture = Fixture::new("unborn");
        fixture.write("src/new.rs", "fn main() {}\n");

        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");

        assert!(!snapshot.current_branch.is_empty());
        assert!(snapshot.head_sha.is_none());
        assert!(snapshot.diff.contains("diff --git a/src/new.rs"));
        assert_eq!(snapshot.diffstat.len(), 1);
    }

    #[test]
    fn sensitive_untracked_files_are_excluded() {
        let fixture = Fixture::new("sensitive");
        fixture.write("README.md", "base\n");
        fixture.commit_all("base");
        fixture.write(".env", "TOKEN=private\n");
        fixture.write("safe.txt", "visible\n");

        let snapshot =
            local_review_snapshot_for_path(ReviewProvider::Github, "acme", "demo", &fixture.path)
                .expect("snapshot");

        assert!(!snapshot.diff.contains("TOKEN=private"));
        assert!(snapshot.diff.contains("safe.txt"));
        assert!(snapshot
            .warnings
            .iter()
            .any(|warning| warning.contains(".env")));
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
    fn non_utf8_untracked_paths_are_rejected_before_file_access() {
        let mut warnings = Vec::new();

        let path = utf8_untracked_path(b"secret-\xff.txt", &mut warnings);

        assert!(path.is_none());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("non-UTF-8 path"));
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
        let parsed = parse_numstat(b"3\t2\tsrc/lib.rs\0-\t-\timage.png\05\t1\t\0old.rs\0new.rs\0")
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
    fn skipped_binary_untracked_files_consume_the_total_scan_budget() {
        let fixture = Fixture::new("untracked-scan-budget");
        fixture.write("README.md", "base\n");
        fixture.commit_all("base");
        let binary = "\0".repeat(400_000);
        for index in 0..6 {
            fixture.write(&format!("a{index}.bin"), &binary);
        }
        fixture.write("z-safe.txt", "must not be scanned\n");
        let mut diff = String::new();
        let mut warnings = Vec::new();
        let mut diffstat = Vec::new();

        append_untracked_files(&fixture.path, &mut diff, &mut warnings, &mut diffstat)
            .expect("bounded scan");

        assert!(!diff.contains("z-safe.txt"));
        assert!(diffstat.is_empty());
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("total untracked-file byte limit")));
    }

    #[test]
    fn unsafe_and_sensitive_paths_are_rejected() {
        assert!(!is_safe_synthetic_diff_path("src/tab\tfile.ts"));
        assert!(!is_safe_synthetic_diff_path("src/quoted\"file.ts"));
        assert!(is_sensitive_untracked_path("config/client-secrets.json"));
        assert!(!is_sensitive_untracked_path("src/token.rs"));
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
    fn review_identity_changes_with_the_loaded_diff() {
        let previews = BTreeMap::new();
        let (first_hash, first_id) = local_review_identity(
            ReviewProvider::Github,
            "acme",
            "demo",
            "feature/local",
            Some("origin/main"),
            Some("1111111111111111111111111111111111111111"),
            "0000000000000000000000000000000000000000",
            "+first",
            &previews,
        );
        let (second_hash, second_id) = local_review_identity(
            ReviewProvider::Github,
            "acme",
            "demo",
            "feature/local",
            Some("origin/main"),
            Some("1111111111111111111111111111111111111111"),
            "0000000000000000000000000000000000000000",
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
            Some("1111111111111111111111111111111111111111"),
            "0000000000000000000000000000000000000000",
            "binary diff marker",
            &first_previews,
        );
        let second = local_review_identity(
            ReviewProvider::Github,
            "acme",
            "demo",
            "feature/local",
            Some("origin/main"),
            Some("1111111111111111111111111111111111111111"),
            "0000000000000000000000000000000000000000",
            "binary diff marker",
            &second_previews,
        );

        assert_ne!(first, second);
    }
}
