use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

use serde::{Deserialize, Serialize};

use crate::config::ReviewProvider;
use crate::local_repo::resolve_local_repo_for_provider;
use crate::services::bitbucket::{DiffstatEntry, PrFilePreview, MAX_PR_IMAGE_PREVIEW_BYTES};

pub(crate) const MAX_UNTRACKED_FILE_BYTES: u64 = 512 * 1024;
pub(crate) const MAX_UNTRACKED_TOTAL_BYTES: u64 = 2 * 1024 * 1024;
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
    pub head_sha: Option<String>,
    pub base_sha: String,
    pub diff: String,
    pub diffstat: Vec<DiffstatEntry>,
    pub warnings: Vec<String>,
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
    let base_sha = if let Some(sha) = upstream_sha.as_ref() {
        sha.clone()
    } else if let Some(sha) = head_sha.as_ref() {
        sha.clone()
    } else {
        empty_tree_oid(repo_path)?
    };
    let commits_ahead = if let Some(upstream) = upstream.as_ref() {
        let range = format!("{upstream}..HEAD");
        git_text(repo_path, &["rev-list", "--count", &range])?
            .parse::<u32>()
            .map_err(|_| "Git returned an invalid ahead count.".to_string())?
    } else {
        0
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

    let (diff, diffstat, snapshot_warnings) = collect_local_diff(repo_path, &base_sha)?;
    warnings.extend(snapshot_warnings);

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
    let (verification_diff, verification_diffstat, _) = collect_local_diff(repo_path, &base_sha)?;
    if diff != verification_diff || diffstat != verification_diffstat {
        return Err(
            "Local changes changed while Norn was loading them. Refresh and try again.".to_string(),
        );
    }

    Ok(LocalReviewSnapshot {
        provider,
        workspace: workspace.to_string(),
        repo: repo.to_string(),
        current_branch,
        upstream,
        commits_ahead,
        head_sha,
        base_sha,
        diff,
        diffstat,
        warnings,
    })
}

fn collect_local_diff(
    repo_path: &Path,
    base_sha: &str,
) -> Result<(String, Vec<DiffstatEntry>, Vec<String>), String> {
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
    Ok((diff, diffstat, warnings))
}

pub fn get_local_file_preview_native(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
    base_sha: &str,
    path: &str,
    side: &str,
) -> Result<PrFilePreview, String> {
    if side != "old" && side != "new" {
        return Err("Image preview side must be old or new.".to_string());
    }
    validate_repo_relative_path(path)?;
    let mime_type = raster_mime_type(path)
        .ok_or_else(|| "Only PNG, JPEG, WebP, and GIF previews are supported.".to_string())?;
    let repo_path = resolve_local_repo_for_provider(provider, workspace, repo)?;
    let bytes = if side == "old" {
        let object = format!("{base_sha}:{path}");
        git_bytes_limited(
            &repo_path,
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
        let file = File::open(&resolved)
            .map_err(|error| format!("Failed to open local image preview: {error}"))?;
        read_bounded(file, MAX_PR_IMAGE_PREVIEW_BYTES)?
    };
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    Ok(PrFilePreview {
        path: path.to_string(),
        mime_type: mime_type.to_string(),
        data_url: format!("data:{mime_type};base64,{encoded}"),
        size: bytes.len(),
    })
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
    parse_name_status(&output)
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
    let mut included_bytes = 0_u64;
    let mut warned_total_limit = false;
    for raw_path in output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let relative = String::from_utf8_lossy(raw_path).to_string();
        let display_relative = relative.escape_default().to_string();
        if std::str::from_utf8(raw_path).is_err() {
            warnings.push(format!(
                "Untracked file path `{display_relative}` is not UTF-8 and was rendered lossily."
            ));
        }
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
        if included_bytes >= MAX_UNTRACKED_TOTAL_BYTES {
            if !warned_total_limit {
                warnings.push(format!(
                    "Skipped additional untracked files starting with `{display_relative}` because the total untracked-file byte limit was reached."
                ));
                warned_total_limit = true;
            }
            continue;
        }
        let remaining = MAX_UNTRACKED_TOTAL_BYTES.saturating_sub(included_bytes);
        let allowed = MAX_UNTRACKED_FILE_BYTES.min(remaining);
        let mut contents = Vec::new();
        file.take(allowed.saturating_add(1))
            .read_to_end(&mut contents)
            .map_err(|error| {
                format!("Failed to read untracked file `{display_relative}`: {error}")
            })?;
        if contents.len() as u64 > allowed {
            if allowed < MAX_UNTRACKED_FILE_BYTES {
                if !warned_total_limit {
                    warnings.push(format!(
                        "Skipped additional untracked files starting with `{display_relative}` because the total untracked-file byte limit was reached."
                    ));
                    warned_total_limit = true;
                }
            } else {
                warnings.push(format!(
                    "Skipped large untracked file `{display_relative}`."
                ));
            }
            continue;
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
        included_bytes = included_bytes.saturating_add(contents.len() as u64);
    }
    Ok(())
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
    let mut command = Command::new("/usr/bin/git");
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
        let output = Command::new("/usr/bin/git")
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
        assert_eq!(snapshot.upstream.as_deref(), Some("upstream"));
        assert_eq!(snapshot.diff.matches("diff --git a/tracked.txt").count(), 1);
        assert!(snapshot.diff.contains("diff --git a/committed.txt"));
        assert!(snapshot.diff.contains("diff --git a/untracked.txt"));
        assert_eq!(snapshot.diffstat.len(), 3);
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
}
