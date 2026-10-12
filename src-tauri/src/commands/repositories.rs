use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::config::{self, ReviewProvider};
use crate::local_repo::{configured_repo_path, git_origin_matches};

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

#[cfg(test)]
use crate::services::repository_reader::legacy::{
    parse_commit_messages, parse_git_blame_line_porcelain, parse_git_status_porcelain_z,
    GIT_SHOW_MESSAGE_MARKER,
};
#[cfg(test)]
use crate::services::repository_reader::RepositoryFileStatus;
pub use crate::services::repository_reader::{
    RepositoryBlameLine, RepositoryFileContent, RepositoryFileDiff, RepositoryFileEntry,
};

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

// Legacy desktop calls cannot select a provider. Refuse ambiguity while
// preserving their configured/fallback path resolver until desktop cutover.
fn configured_readonly_repo(workspace: &str, repo: &str) -> Result<PathBuf, String> {
    if config::load()
        .repos
        .iter()
        .filter(|entry| entry.workspace == workspace && entry.repo == repo)
        .count()
        != 1
    {
        return Err("Select one configured repository for this provider.".into());
    }
    configured_repo(workspace, repo)?
        .canonicalize()
        .map_err(|_| "The selected local checkout is unavailable.".into())
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
    let root = configured_readonly_repo(&workspace, &repo)?;
    crate::services::repository_reader::list_at(&root)
}

#[tauri::command]
pub fn get_repository_file_diff(
    workspace: String,
    repo: String,
    path: String,
) -> Result<RepositoryFileDiff, String> {
    let root = configured_readonly_repo(&workspace, &repo)?;
    crate::services::repository_reader::diff_at(&root, &path)
}

#[tauri::command]
pub fn read_repository_file(
    workspace: String,
    repo: String,
    path: String,
) -> Result<RepositoryFileContent, String> {
    let root = configured_readonly_repo(&workspace, &repo)?;
    crate::services::repository_reader::content_at(&root, &path)
}

#[tauri::command]
pub fn get_repository_file_blame(
    workspace: String,
    repo: String,
    path: String,
) -> Result<Vec<RepositoryBlameLine>, String> {
    let root = configured_readonly_repo(&workspace, &repo)?;
    crate::services::repository_reader::blame_at(&root, &path)
}

#[tauri::command]
pub fn open_repository_file_external(
    workspace: String,
    repo: String,
    path: String,
    line: Option<u32>,
) -> Result<(), String> {
    let matches: Vec<_> = config::load()
        .repos
        .into_iter()
        .filter(|entry| entry.workspace == workspace && entry.repo == repo)
        .collect();
    if matches.len() != 1 {
        return Err("Select one configured repository for this provider.".into());
    }
    let entry = &matches[0];
    crate::services::repository_editor::open_graphical(
        crate::services::bitbucket::workspace_prs::RepositoryIdentity {
            provider: entry.provider,
            workspace,
            repo,
        },
        path,
        line.filter(|value| *value > 0),
    )
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
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        local_branch_for_remote, parse_commit_messages, parse_git_blame_line_porcelain,
        parse_git_status_porcelain_z, run_git_checked, stash_repo, RepositoryFileStatus,
        GIT_SHOW_MESSAGE_MARKER,
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
}
