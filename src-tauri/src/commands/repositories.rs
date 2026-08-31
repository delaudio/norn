use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::{self, ReviewProvider};
use crate::local_repo::{configured_repo_path, find_in_path, git_origin_matches};

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryWorktreeStatus {
    Clean,
    Dirty,
    MissingPath,
    InvalidRepo,
    Error,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryBranchKind {
    Local,
    Remote,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryBranchOption {
    pub name: String,
    pub reference: String,
    pub kind: RepositoryBranchKind,
    pub is_current: bool,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryWorktreeState {
    pub workspace: String,
    pub repo: String,
    pub local_path: Option<String>,
    pub status: RepositoryWorktreeStatus,
    pub current_branch: Option<String>,
    pub detached_head: Option<String>,
    pub dirty: bool,
    pub branches: Vec<RepositoryBranchOption>,
    pub error: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryFileEntry {
    pub path: String,
    pub status: RepositoryFileStatus,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryFileStatus {
    Unchanged,
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryFileContent {
    pub path: String,
    pub content: String,
    pub size: u64,
    pub truncated: bool,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryFileDiff {
    pub path: String,
    pub raw_diff: String,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryBlameLine {
    pub line: u32,
    pub sha: String,
    pub short_sha: String,
    pub author: Option<String>,
    pub author_email: Option<String>,
    pub author_time: Option<i64>,
    pub summary: Option<String>,
    pub message: Option<String>,
}

const MAX_REPOSITORY_FILE_BYTES: u64 = 512 * 1024;
const GIT_BLAME_NO_COMMIT_ERROR: &str = "fatal: no such ref: HEAD";
const GIT_BLAME_NO_PATH: &str = "fatal: no such path";
const GIT_SHOW_MESSAGE_MARKER: &str = "\u{1f}NORN_COMMIT_MESSAGE_END\u{1f}";

fn git_command(repo_path: &Path) -> Command {
    let mut cmd = Command::new("/usr/bin/git");
    cmd.current_dir(repo_path);
    cmd
}

fn run_git(repo_path: &Path, args: &[&str]) -> Result<(Option<i32>, String, String), String> {
    let output = git_command(repo_path)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git {}: {e}", args.join(" ")))?;
    Ok((
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    ))
}

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let (code, stdout, stderr) = run_git(repo_path, args)?;
    if code == Some(0) {
        Ok(stdout.trim().to_string())
    } else {
        Err(format!(
            "git {} failed with code {:?}: {}{}",
            args.join(" "),
            code,
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!("\nstdout: {}", stdout.trim())
            }
        ))
    }
}

fn run_git_checked_raw(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let (code, stdout, stderr) = run_git(repo_path, args)?;
    if code == Some(0) {
        Ok(stdout)
    } else {
        Err(format!(
            "git {} failed with code {:?}: {}{}",
            args.join(" "),
            code,
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!("\nstdout: {}", stdout.trim())
            }
        ))
    }
}

fn is_git_repo(repo_path: &Path) -> bool {
    run_git(repo_path, &["rev-parse", "--is-inside-work-tree"])
        .map(|(code, stdout, _)| code == Some(0) && stdout.trim() == "true")
        .unwrap_or(false)
}

fn current_branch(repo_path: &Path) -> Result<(Option<String>, Option<String>), String> {
    let symbolic = run_git(repo_path, &["symbolic-ref", "--short", "HEAD"])?;
    if symbolic.0 == Some(0) {
        return Ok((Some(symbolic.1.trim().to_string()), None));
    }
    let head = run_git_checked(repo_path, &["rev-parse", "--short", "HEAD"])?;
    Ok((None, Some(head)))
}

fn dirty(repo_path: &Path) -> Result<bool, String> {
    Ok(!run_git_checked(repo_path, &["status", "--porcelain"])?.is_empty())
}

fn branch_refs(repo_path: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    Ok(run_git_checked(repo_path, args)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect())
}

fn file_status_from_porcelain(index_status: char, worktree_status: char) -> RepositoryFileStatus {
    if index_status == '?' && worktree_status == '?' {
        return RepositoryFileStatus::Untracked;
    }
    if index_status == 'U'
        || worktree_status == 'U'
        || matches!(
            (index_status, worktree_status),
            ('A', 'A') | ('D', 'D') | ('A', 'U') | ('D', 'U') | ('U', 'A') | ('U', 'D')
        )
    {
        return RepositoryFileStatus::Conflicted;
    }
    if index_status == 'D' || worktree_status == 'D' {
        return RepositoryFileStatus::Deleted;
    }
    if index_status == 'R' || worktree_status == 'R' {
        return RepositoryFileStatus::Renamed;
    }
    if index_status == 'A' || worktree_status == 'A' {
        return RepositoryFileStatus::Added;
    }
    if matches!(index_status, 'M' | 'T') || matches!(worktree_status, 'M' | 'T') {
        return RepositoryFileStatus::Modified;
    }
    RepositoryFileStatus::Unchanged
}

fn parse_git_status_porcelain_z(output: &str) -> BTreeMap<String, RepositoryFileStatus> {
    let mut statuses = BTreeMap::new();
    let mut parts = output.split_terminator('\0');

    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let mut chars = entry.chars();
        let index_status = chars.next().unwrap_or(' ');
        let worktree_status = chars.next().unwrap_or(' ');
        let _space = chars.next();
        let path = chars.as_str();
        if path.is_empty() {
            continue;
        }

        let status = file_status_from_porcelain(index_status, worktree_status);
        if matches!(index_status, 'R' | 'C') || matches!(worktree_status, 'R' | 'C') {
            let _old_path = parts.next();
        }
        statuses.insert(path.to_string(), status);
    }

    statuses
}

fn repository_statuses(repo_path: &Path) -> Result<BTreeMap<String, RepositoryFileStatus>, String> {
    let status_output = run_git_checked_raw(
        repo_path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    Ok(parse_git_status_porcelain_z(&status_output))
}

fn list_branch_options(
    repo_path: &Path,
    current: Option<&str>,
) -> Result<Vec<RepositoryBranchOption>, String> {
    let mut options = Vec::new();
    for branch in branch_refs(repo_path, &["branch", "--format=%(refname:short)"])? {
        options.push(RepositoryBranchOption {
            is_current: current == Some(branch.as_str()),
            reference: branch.clone(),
            name: branch,
            kind: RepositoryBranchKind::Local,
        });
    }
    for branch in branch_refs(repo_path, &["branch", "-r", "--format=%(refname:short)"])? {
        if branch == "origin/HEAD" || branch.starts_with("origin/HEAD ->") {
            continue;
        }
        options.push(RepositoryBranchOption {
            is_current: false,
            reference: branch.clone(),
            name: branch,
            kind: RepositoryBranchKind::Remote,
        });
    }
    options.sort_by(|left, right| left.reference.cmp(&right.reference));
    options.dedup_by(|left, right| left.reference == right.reference);
    Ok(options)
}

fn state_for_repo(
    provider: ReviewProvider,
    workspace: String,
    repo: String,
    local_path: Option<PathBuf>,
) -> RepositoryWorktreeState {
    let Some(path) = local_path else {
        return RepositoryWorktreeState {
            workspace,
            repo,
            local_path: None,
            status: RepositoryWorktreeStatus::MissingPath,
            current_branch: None,
            detached_head: None,
            dirty: false,
            branches: Vec::new(),
            error: Some("No local path configured in Settings.".to_string()),
        };
    };
    let path_string = path.display().to_string();
    if !path.is_dir() {
        return RepositoryWorktreeState {
            workspace,
            repo,
            local_path: Some(path_string),
            status: RepositoryWorktreeStatus::MissingPath,
            current_branch: None,
            detached_head: None,
            dirty: false,
            branches: Vec::new(),
            error: Some("Configured local path does not exist or is not a directory.".to_string()),
        };
    }
    if !is_git_repo(&path) {
        return RepositoryWorktreeState {
            workspace,
            repo,
            local_path: Some(path_string),
            status: RepositoryWorktreeStatus::InvalidRepo,
            current_branch: None,
            detached_head: None,
            dirty: false,
            branches: Vec::new(),
            error: Some("Configured local path is not a git repository.".to_string()),
        };
    }
    if git_origin_matches(&path, provider, &workspace, &repo).ok() != Some(true) {
        let remote = match provider {
            ReviewProvider::Bitbucket => format!("bitbucket.org/{workspace}/{repo}"),
            ReviewProvider::Github => format!("github.com/{workspace}/{repo}"),
        };
        let error = format!("Configured local path does not match {remote}.");
        return RepositoryWorktreeState {
            workspace,
            repo,
            local_path: Some(path_string),
            status: RepositoryWorktreeStatus::InvalidRepo,
            current_branch: None,
            detached_head: None,
            dirty: false,
            branches: Vec::new(),
            error: Some(error),
        };
    }

    match current_branch(&path).and_then(|(branch, detached)| {
        let is_dirty = dirty(&path)?;
        let branches = list_branch_options(&path, branch.as_deref())?;
        Ok((branch, detached, is_dirty, branches))
    }) {
        Ok((current_branch, detached_head, is_dirty, branches)) => RepositoryWorktreeState {
            workspace,
            repo,
            local_path: Some(path_string),
            status: if is_dirty {
                RepositoryWorktreeStatus::Dirty
            } else {
                RepositoryWorktreeStatus::Clean
            },
            current_branch,
            detached_head,
            dirty: is_dirty,
            branches,
            error: None,
        },
        Err(error) => RepositoryWorktreeState {
            workspace,
            repo,
            local_path: Some(path_string),
            status: RepositoryWorktreeStatus::Error,
            current_branch: None,
            detached_head: None,
            dirty: false,
            branches: Vec::new(),
            error: Some(error),
        },
    }
}

fn local_branch_for_remote(reference: &str) -> Option<&str> {
    reference
        .strip_prefix("origin/")
        .filter(|branch| !branch.is_empty())
}

fn checkout_branch(repo_path: &Path, branch_ref: &str) -> Result<(), String> {
    if branch_ref.trim().is_empty() {
        return Err("Choose a branch before checking out.".to_string());
    }
    if dirty(repo_path)? {
        return Err("Cannot checkout while the repository has uncommitted changes.".to_string());
    }
    if let Some(local_branch) = local_branch_for_remote(branch_ref) {
        let local_exists = run_git(
            repo_path,
            &[
                "rev-parse",
                "--verify",
                &format!("refs/heads/{local_branch}"),
            ],
        )?
        .0 == Some(0);
        if local_exists {
            run_git_checked(repo_path, &["checkout", local_branch])?;
        } else {
            run_git_checked(repo_path, &["checkout", "--track", branch_ref])?;
        }
        return Ok(());
    }
    run_git_checked(repo_path, &["checkout", branch_ref])?;
    Ok(())
}

fn configured_repo(workspace: &str, repo: &str) -> Result<PathBuf, String> {
    let cfg = config::load();
    let repo_ref = cfg
        .repos
        .iter()
        .find(|candidate| candidate.workspace == workspace && candidate.repo == repo)
        .ok_or_else(|| format!("Repository {workspace}/{repo} is not configured."))?;
    let repo_path = configured_repo_path(repo_ref)
        .ok_or_else(|| format!("Repository {workspace}/{repo} has no local path configured."))?;
    if !git_origin_matches(&repo_path, repo_ref.provider, workspace, repo)? {
        let remote = match repo_ref.provider {
            ReviewProvider::Bitbucket => format!("bitbucket.org/{workspace}/{repo}"),
            ReviewProvider::Github => format!("github.com/{workspace}/{repo}"),
        };
        return Err(format!("Configured local path does not match {remote}."));
    }
    Ok(repo_path)
}

fn validate_repo_relative_path(path: &str) -> Result<(), String> {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err("Repository file path must be relative.".to_string());
    }
    if path.trim().is_empty() {
        return Err("Repository file path is required.".to_string());
    }
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("Repository file path cannot escape the repository.".to_string());
    }
    Ok(())
}

fn safe_repo_file_path(repo_path: &Path, path: &str) -> Result<PathBuf, String> {
    validate_repo_relative_path(path)?;
    let full_path = repo_path.join(path);
    let canonical_repo = repo_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve repository path: {e}"))?;
    let canonical_file = full_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve repository file path: {e}"))?;
    if !canonical_file.starts_with(&canonical_repo) {
        return Err("Repository file path cannot escape the repository.".to_string());
    }
    Ok(canonical_file)
}

fn repo_relative_file_path(repo_path: &Path, path: &str) -> Result<PathBuf, String> {
    validate_repo_relative_path(path)?;
    Ok(repo_path.join(path))
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(8).collect()
}

fn strip_mail_brackets(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_string()
}

fn parse_blame_header(line: &str) -> Option<(String, u32)> {
    let mut parts = line.split_whitespace();
    let sha = parts.next()?;
    if sha.len() < 7 || !sha.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let _original_line = parts.next()?.parse::<u32>().ok()?;
    let final_line = parts.next()?.parse::<u32>().ok()?;
    Some((sha.to_string(), final_line))
}

fn parse_git_blame_line_porcelain(output: &str) -> Vec<RepositoryBlameLine> {
    let mut entries = Vec::new();
    let mut current: Option<RepositoryBlameLine> = None;

    for line in output.lines() {
        if let Some((sha, final_line)) = parse_blame_header(line) {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(RepositoryBlameLine {
                line: final_line,
                short_sha: short_sha(&sha),
                sha,
                author: None,
                author_email: None,
                author_time: None,
                summary: None,
                message: None,
            });
            continue;
        }

        let Some(entry) = current.as_mut() else {
            continue;
        };

        if line.starts_with('\t') {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
        } else if let Some(author) = line.strip_prefix("author ") {
            entry.author = Some(author.to_string());
        } else if let Some(author_email) = line.strip_prefix("author-mail ") {
            entry.author_email = Some(strip_mail_brackets(author_email));
        } else if let Some(author_time) = line.strip_prefix("author-time ") {
            entry.author_time = author_time.parse::<i64>().ok();
        } else if let Some(summary) = line.strip_prefix("summary ") {
            entry.summary = Some(summary.to_string());
        }
    }

    if let Some(entry) = current {
        entries.push(entry);
    }

    entries.sort_by_key(|entry| entry.line);
    entries
}

fn is_real_commit_sha(sha: &str) -> bool {
    !sha.is_empty() && sha.chars().any(|ch| ch != '0')
}

fn unique_real_shas(entries: &[RepositoryBlameLine]) -> Vec<String> {
    let mut shas = Vec::new();
    for entry in entries {
        if !is_real_commit_sha(&entry.sha) || shas.contains(&entry.sha) {
            continue;
        }
        shas.push(entry.sha.clone());
    }
    shas
}

fn parse_commit_messages(output: &str, shas: &[String]) -> Vec<String> {
    let mut messages = output
        .trim_end()
        .split_terminator(GIT_SHOW_MESSAGE_MARKER)
        .map(|message| message.trim().to_string())
        .collect::<Vec<_>>();
    messages.truncate(shas.len());
    messages
}

fn attach_commit_messages(
    repo_path: &Path,
    entries: &mut [RepositoryBlameLine],
) -> Result<(), String> {
    let shas = unique_real_shas(entries);
    if shas.is_empty() {
        return Ok(());
    }

    let format_arg = format!("--format=%B{}", GIT_SHOW_MESSAGE_MARKER);
    let mut args = vec!["show", "-s", format_arg.as_str()];
    args.extend(shas.iter().map(String::as_str));
    let output = run_git_checked_raw(repo_path, &args)?;
    let messages = parse_commit_messages(&output, &shas);

    for (sha, message) in shas.iter().zip(messages) {
        for entry in entries.iter_mut().filter(|entry| entry.sha == *sha) {
            entry.message = Some(message.clone());
        }
    }

    Ok(())
}

fn editor_file_arg(file_path: &Path, line: Option<u32>, include_column: bool) -> String {
    let path = file_path.to_string_lossy();
    match line.filter(|line| *line > 0) {
        Some(line) if include_column => format!("{path}:{line}:1"),
        Some(line) => format!("{path}:{line}"),
        None => path.to_string(),
    }
}

fn external_editor_command(file_path: &Path, line: Option<u32>) -> Option<(PathBuf, Vec<String>)> {
    if let Some(zed) = find_in_path("zed") {
        return Some((zed, vec![editor_file_arg(file_path, line, true)]));
    }
    if let Some(cursor) = find_in_path("cursor") {
        return Some((
            cursor,
            vec!["-g".to_string(), editor_file_arg(file_path, line, false)],
        ));
    }
    if let Some(code) = find_in_path("code") {
        return Some((
            code,
            vec!["-g".to_string(), editor_file_arg(file_path, line, false)],
        ));
    }
    None
}

fn spawn_command(program: &Path, args: &[String]) -> Result<(), String> {
    Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open external editor: {e}"))
}

fn open_with_system(file_path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("/usr/bin/open")
            .arg(file_path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open file: {e}"))
    }

    #[cfg(target_os = "windows")]
    {
        let file_path = file_path.to_string_lossy().to_string();
        return Command::new("cmd")
            .args(["/C", "start", "", &file_path])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open file: {e}"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(file_path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open file: {e}"))
    }
}

fn fetch_repo(repo_path: &Path) -> Result<(), String> {
    run_git_checked(repo_path, &["fetch", "--prune", "origin"])?;
    Ok(())
}

fn pull_repo(repo_path: &Path) -> Result<(), String> {
    if dirty(repo_path)? {
        return Err("Cannot pull while the repository has uncommitted changes.".to_string());
    }
    run_git_checked(repo_path, &["pull", "--ff-only"])?;
    Ok(())
}

fn stash_repo(repo_path: &Path) -> Result<(), String> {
    if !dirty(repo_path)? {
        return Ok(());
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let message = format!("norn repository view stash {timestamp}");
    run_git_checked(
        repo_path,
        &["stash", "push", "--include-untracked", "-m", &message],
    )?;
    Ok(())
}

#[tauri::command]
pub fn list_repository_worktrees() -> Result<Vec<RepositoryWorktreeState>, String> {
    let cfg = config::load();
    Ok(cfg
        .repos
        .into_iter()
        .map(|repo_ref| {
            let path = configured_repo_path(&repo_ref);
            state_for_repo(repo_ref.provider, repo_ref.workspace, repo_ref.repo, path)
        })
        .collect())
}

#[tauri::command]
pub fn list_repository_files(
    workspace: String,
    repo: String,
) -> Result<Vec<RepositoryFileEntry>, String> {
    let repo_path = configured_repo(&workspace, &repo)?;
    let output = run_git_checked_raw(&repo_path, &["ls-files", "-z", "-co", "--exclude-standard"])?;
    let statuses = repository_statuses(&repo_path)?;
    let mut files = output
        .split_terminator('\0')
        .filter(|path| !path.is_empty())
        .map(|path| {
            let status = statuses
                .get(path)
                .cloned()
                .unwrap_or(RepositoryFileStatus::Unchanged);
            (
                path.to_string(),
                RepositoryFileEntry {
                    path: path.to_string(),
                    status,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (path, status) in statuses {
        files
            .entry(path.clone())
            .or_insert(RepositoryFileEntry { path, status });
    }

    Ok(files.into_values().collect())
}

fn diff_untracked_file(repo_path: &Path, path: &str) -> Result<String, String> {
    let file_path = repo_relative_file_path(repo_path, path)?;
    let file_path_str = file_path.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_git(
        repo_path,
        &[
            "diff",
            "--no-ext-diff",
            "--no-index",
            "--",
            "/dev/null",
            &file_path_str,
        ],
    )?;
    if code == Some(0) || code == Some(1) {
        Ok(stdout)
    } else {
        Err(format!(
            "git diff --no-index failed with code {:?}: {}{}",
            code,
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!("\nstdout: {}", stdout.trim())
            }
        ))
    }
}

#[tauri::command]
pub fn get_repository_file_diff(
    workspace: String,
    repo: String,
    path: String,
) -> Result<RepositoryFileDiff, String> {
    let repo_path = configured_repo(&workspace, &repo)?;
    validate_repo_relative_path(&path)?;
    let statuses = repository_statuses(&repo_path)?;
    let status = statuses
        .get(&path)
        .cloned()
        .unwrap_or(RepositoryFileStatus::Unchanged);
    let raw_diff = if status == RepositoryFileStatus::Untracked {
        diff_untracked_file(&repo_path, &path)?
    } else {
        run_git_checked_raw(&repo_path, &["diff", "--no-ext-diff", "HEAD", "--", &path])?
    };

    Ok(RepositoryFileDiff { path, raw_diff })
}

#[tauri::command]
pub fn read_repository_file(
    workspace: String,
    repo: String,
    path: String,
) -> Result<RepositoryFileContent, String> {
    let repo_path = configured_repo(&workspace, &repo)?;
    let file_path = safe_repo_file_path(&repo_path, &path)?;
    let metadata = fs::metadata(&file_path).map_err(|e| format!("Failed to read file: {e}"))?;
    if !metadata.is_file() {
        return Err("Repository path is not a file.".to_string());
    }
    let size = metadata.len();
    if size > MAX_REPOSITORY_FILE_BYTES {
        return Err(format!(
            "File is too large to preview ({} KB limit).",
            MAX_REPOSITORY_FILE_BYTES / 1024
        ));
    }
    let bytes = fs::read(&file_path).map_err(|e| format!("Failed to read file: {e}"))?;
    let content = String::from_utf8(bytes)
        .map_err(|_| "File is not valid UTF-8 and cannot be previewed.".to_string())?;
    Ok(RepositoryFileContent {
        path,
        content,
        size,
        truncated: false,
    })
}

#[tauri::command]
pub fn get_repository_file_blame(
    workspace: String,
    repo: String,
    path: String,
) -> Result<Vec<RepositoryBlameLine>, String> {
    let repo_path = configured_repo(&workspace, &repo)?;
    let _file_path = safe_repo_file_path(&repo_path, &path)?;
    let (code, stdout, stderr) = run_git(&repo_path, &["blame", "--line-porcelain", "--", &path])?;
    if code == Some(0) {
        let mut entries = parse_git_blame_line_porcelain(&stdout);
        attach_commit_messages(&repo_path, &mut entries)?;
        return Ok(entries);
    }

    let trimmed = stderr.trim();
    if trimmed == GIT_BLAME_NO_COMMIT_ERROR || trimmed.contains(GIT_BLAME_NO_PATH) {
        return Ok(Vec::new());
    }

    Err(format!(
        "git blame failed with code {:?}: {}{}",
        code,
        trimmed,
        if stdout.trim().is_empty() {
            String::new()
        } else {
            format!("\nstdout: {}", stdout.trim())
        }
    ))
}

#[tauri::command]
pub fn open_repository_file_external(
    workspace: String,
    repo: String,
    path: String,
    line: Option<u32>,
) -> Result<(), String> {
    let repo_path = configured_repo(&workspace, &repo)?;
    let file_path = safe_repo_file_path(&repo_path, &path)?;
    let metadata = fs::metadata(&file_path).map_err(|e| format!("Failed to read file: {e}"))?;
    if !metadata.is_file() {
        return Err("Repository path is not a file.".to_string());
    }

    if let Some((program, args)) = external_editor_command(&file_path, line) {
        return spawn_command(&program, &args);
    }

    open_with_system(&file_path)
}

#[tauri::command]
pub fn checkout_repository_branch(
    workspace: String,
    repo: String,
    branch_ref: String,
) -> Result<RepositoryWorktreeState, String> {
    let repo_path = configured_repo(&workspace, &repo)?;
    checkout_branch(&repo_path, &branch_ref)?;
    let provider = config::load()
        .repos
        .iter()
        .find(|candidate| candidate.workspace == workspace && candidate.repo == repo)
        .map(|candidate| candidate.provider)
        .unwrap_or_default();
    Ok(state_for_repo(provider, workspace, repo, Some(repo_path)))
}

#[tauri::command]
pub fn fetch_repository(
    workspace: String,
    repo: String,
) -> Result<RepositoryWorktreeState, String> {
    let repo_path = configured_repo(&workspace, &repo)?;
    fetch_repo(&repo_path)?;
    let provider = config::load()
        .repos
        .iter()
        .find(|candidate| candidate.workspace == workspace && candidate.repo == repo)
        .map(|candidate| candidate.provider)
        .unwrap_or_default();
    Ok(state_for_repo(provider, workspace, repo, Some(repo_path)))
}

#[tauri::command]
pub fn pull_repository(workspace: String, repo: String) -> Result<RepositoryWorktreeState, String> {
    let repo_path = configured_repo(&workspace, &repo)?;
    pull_repo(&repo_path)?;
    let provider = config::load()
        .repos
        .iter()
        .find(|candidate| candidate.workspace == workspace && candidate.repo == repo)
        .map(|candidate| candidate.provider)
        .unwrap_or_default();
    Ok(state_for_repo(provider, workspace, repo, Some(repo_path)))
}

#[tauri::command]
pub fn stash_repository(
    workspace: String,
    repo: String,
) -> Result<RepositoryWorktreeState, String> {
    let repo_path = configured_repo(&workspace, &repo)?;
    stash_repo(&repo_path)?;
    let provider = config::load()
        .repos
        .iter()
        .find(|candidate| candidate.workspace == workspace && candidate.repo == repo)
        .map(|candidate| candidate.provider)
        .unwrap_or_default();
    Ok(state_for_repo(provider, workspace, repo, Some(repo_path)))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        editor_file_arg, local_branch_for_remote, parse_commit_messages,
        parse_git_blame_line_porcelain, parse_git_status_porcelain_z, run_git_checked, stash_repo,
        validate_repo_relative_path, RepositoryFileStatus, GIT_SHOW_MESSAGE_MARKER,
    };

    fn temp_git_repo(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lachesi-{name}-{nonce}"));
        fs::create_dir_all(&path).expect("create temp git repo");
        run_git_checked(&path, &["init"]).expect("git init");
        run_git_checked(&path, &["config", "user.email", "lachesi@example.test"])
            .expect("git config email");
        run_git_checked(&path, &["config", "user.name", "Lachesi Test"]).expect("git config name");
        fs::write(path.join("README.md"), "# Test\n").expect("write initial file");
        run_git_checked(&path, &["add", "README.md"]).expect("git add");
        run_git_checked(&path, &["commit", "-m", "Initial commit"]).expect("git commit");
        path
    }

    #[test]
    fn maps_origin_remote_to_local_branch_name() {
        assert_eq!(
            local_branch_for_remote("origin/feature/test"),
            Some("feature/test")
        );
        assert_eq!(local_branch_for_remote("main"), None);
        assert_eq!(local_branch_for_remote("upstream/main"), None);
    }

    #[test]
    fn validates_repository_relative_paths() {
        assert!(validate_repo_relative_path("src/App.tsx").is_ok());
        assert!(validate_repo_relative_path("../secrets").is_err());
        assert!(validate_repo_relative_path("/tmp/secrets").is_err());
        assert!(validate_repo_relative_path("").is_err());
    }

    #[test]
    fn parses_git_status_porcelain_z() {
        let output = " M src/App.tsx\0A  src/New.tsx\0?? docs/draft.md\0 D src/Removed.tsx\0R  src/NewName.tsx\0src/OldName.tsx\0";

        let statuses = parse_git_status_porcelain_z(output);

        assert_eq!(
            statuses.get("src/App.tsx"),
            Some(&RepositoryFileStatus::Modified)
        );
        assert_eq!(
            statuses.get("src/New.tsx"),
            Some(&RepositoryFileStatus::Added)
        );
        assert_eq!(
            statuses.get("docs/draft.md"),
            Some(&RepositoryFileStatus::Untracked)
        );
        assert_eq!(
            statuses.get("src/Removed.tsx"),
            Some(&RepositoryFileStatus::Deleted)
        );
        assert_eq!(
            statuses.get("src/NewName.tsx"),
            Some(&RepositoryFileStatus::Renamed)
        );
        assert!(!statuses.contains_key("src/OldName.tsx"));
    }

    #[test]
    fn stash_repo_stashes_untracked_changes() {
        let repo = temp_git_repo("stash");
        fs::write(repo.join("draft.txt"), "local draft\n").expect("write untracked file");

        stash_repo(&repo).expect("stash repo");
        let status = run_git_checked(&repo, &["status", "--porcelain"]).expect("git status");
        let stashes = run_git_checked(&repo, &["stash", "list"]).expect("git stash list");

        assert!(status.is_empty());
        assert!(stashes.contains("norn repository view stash"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn parses_git_blame_line_porcelain() {
        let output = r#"6f52c9a1cf5cd075762f13d0b0f8bf8d0f4f3f7d 10 1 1
author Ada Lovelace
author-mail <ada@example.com>
author-time 1710000000
summary Add formatter
filename src/lib/format.ts
	export function formatCurrency() {}
0000000000000000000000000000000000000000 2 2 1
author Not Committed Yet
author-mail <not.committed.yet>
author-time 1710000100
summary Version of src/lib/format.ts from working tree
filename src/lib/format.ts
	return "$0.00";
"#;

        let entries = parse_git_blame_line_porcelain(output);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].line, 1);
        assert_eq!(entries[0].short_sha, "6f52c9a1");
        assert_eq!(entries[0].author.as_deref(), Some("Ada Lovelace"));
        assert_eq!(entries[0].author_email.as_deref(), Some("ada@example.com"));
        assert_eq!(entries[0].author_time, Some(1710000000));
        assert_eq!(entries[0].summary.as_deref(), Some("Add formatter"));
        assert_eq!(entries[0].message, None);
        assert_eq!(entries[1].line, 2);
        assert_eq!(entries[1].short_sha, "00000000");
        assert_eq!(entries[1].author.as_deref(), Some("Not Committed Yet"));
    }

    #[test]
    fn parses_batched_commit_messages() {
        let shas = vec!["6f52c9a1".to_string(), "9a5e2d3c".to_string()];
        let output = format!(
            "Add formatter\n\nUse shared formatting.{}\nFix locale copy\n\nKeep labels concise.{}\n",
            GIT_SHOW_MESSAGE_MARKER, GIT_SHOW_MESSAGE_MARKER
        );

        let messages = parse_commit_messages(&output, &shas);

        assert_eq!(
            messages,
            vec![
                "Add formatter\n\nUse shared formatting.".to_string(),
                "Fix locale copy\n\nKeep labels concise.".to_string(),
            ]
        );
    }

    #[test]
    fn builds_external_editor_file_position_argument() {
        let file_path = Path::new("/tmp/project/src/App.tsx");

        assert_eq!(
            editor_file_arg(file_path, Some(42), true),
            "/tmp/project/src/App.tsx:42:1"
        );
        assert_eq!(
            editor_file_arg(file_path, Some(42), false),
            "/tmp/project/src/App.tsx:42"
        );
        assert_eq!(
            editor_file_arg(file_path, None, true),
            "/tmp/project/src/App.tsx"
        );
    }
}
