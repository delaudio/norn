use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{self, RepoRef, ReviewProvider};

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "vendor",
    ".next",
    ".cache",
    ".venv",
    "venv",
    "Library",
    "Applications",
];

fn matches_remote(
    config_contents: &str,
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
) -> bool {
    let lower = config_contents.to_lowercase();
    let ws = workspace.to_lowercase();
    let rp = repo.to_lowercase();
    match provider {
        ReviewProvider::Bitbucket => {
            lower.contains(&format!("bitbucket.org/{ws}/{rp}"))
                || lower.contains(&format!("bitbucket.org:{ws}/{rp}"))
        }
        ReviewProvider::Github => {
            lower.contains(&format!("github.com/{ws}/{rp}"))
                || lower.contains(&format!("github.com:{ws}/{rp}"))
        }
    }
}

fn search_dir(
    dir: &Path,
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
    depth: u32,
) -> Option<PathBuf> {
    let git_config = dir.join(".git").join("config");
    if git_config.is_file() {
        if let Ok(contents) = fs::read_to_string(&git_config) {
            if matches_remote(&contents, provider, workspace, repo) {
                return Some(dir.to_path_buf());
            }
        }
        return None;
    }
    if depth == 0 {
        return None;
    }
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.is_empty() || name.starts_with('.') || SKIP_DIRS.contains(&name) {
            continue;
        }
        if let Some(found) = search_dir(&path, provider, workspace, repo, depth - 1) {
            return Some(found);
        }
    }
    None
}

pub fn autodiscover_local_repo(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let roots = [
        "dev",
        "code",
        "projects",
        "work",
        "src",
        "Developer",
        "repos",
        "git",
        "Sites",
        "Documents",
    ];
    for root in roots {
        let dir = home.join(root);
        if dir.is_dir() {
            if let Some(found) = search_dir(&dir, provider, workspace, repo, 5) {
                return Some(found);
            }
        }
    }
    None
}

pub fn git_origin_matches(
    path: &Path,
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
) -> Result<bool, String> {
    let output = Command::new("/usr/bin/git")
        .arg("-C")
        .arg(path)
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .output()
        .map_err(|e| format!("failed to inspect git remote for {}: {e}", path.display()))?;
    if !output.status.success() {
        return Ok(false);
    }
    let remote = String::from_utf8_lossy(&output.stdout);
    Ok(matches_remote(&remote, provider, workspace, repo))
}

pub fn parse_git_remote(remote: &str) -> Result<(ReviewProvider, String, String), String> {
    let trimmed = remote.trim();
    if trimmed.is_empty() {
        return Err("Git remote URL is empty.".to_string());
    }
    let display_remote = redact_git_remote(trimmed);

    let without_suffix = trimmed
        .split(['?', '#'])
        .next()
        .unwrap_or(trimmed)
        .trim_end_matches('/')
        .trim_end_matches(".git");

    let (host, path) = if let Some(rest) = without_suffix.strip_prefix("git@") {
        let (host, path) = rest
            .split_once(':')
            .ok_or_else(|| format!("Cannot parse git remote URL: {display_remote}."))?;
        (host, path)
    } else if let Some(rest) = without_suffix.strip_prefix("ssh://git@") {
        let (host, path) = rest
            .split_once('/')
            .ok_or_else(|| format!("Cannot parse git remote URL: {display_remote}."))?;
        (host, path)
    } else if let Some(rest) = without_suffix.strip_prefix("https://") {
        let rest = rest.split_once('@').map(|(_, after)| after).unwrap_or(rest);
        let (host, path) = rest
            .split_once('/')
            .ok_or_else(|| format!("Cannot parse git remote URL: {display_remote}."))?;
        (host, path)
    } else if let Some(rest) = without_suffix.strip_prefix("http://") {
        let rest = rest.split_once('@').map(|(_, after)| after).unwrap_or(rest);
        let (host, path) = rest
            .split_once('/')
            .ok_or_else(|| format!("Cannot parse git remote URL: {display_remote}."))?;
        (host, path)
    } else {
        return Err(format!(
            "Unsupported git remote URL: {display_remote}. Run `norn-tui` from a clone whose remote is on github.com or bitbucket.org, or use `norn-tui --workspace` to open configured repositories."
        ));
    };

    let provider = match host.to_lowercase().as_str() {
        "bitbucket.org" => ReviewProvider::Bitbucket,
        "github.com" => ReviewProvider::Github,
        _ => {
            return Err(format!(
                "Unsupported git remote host `{host}`. Run `norn-tui` from a clone whose remote is on github.com or bitbucket.org, or use `norn-tui --workspace` to open configured repositories."
            ));
        }
    };

    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let workspace = parts.next().ok_or_else(|| {
        format!("Cannot find repository owner in git remote URL: {display_remote}.")
    })?;
    let repo = parts.next().ok_or_else(|| {
        format!("Cannot find repository name in git remote URL: {display_remote}.")
    })?;
    if parts.next().is_some() {
        return Err(format!("Cannot parse git remote URL: {display_remote}."));
    }

    Ok((provider, workspace.to_string(), repo.to_string()))
}

fn redact_git_remote(remote: &str) -> String {
    for scheme in ["https://", "http://", "ssh://"] {
        if let Some(rest) = remote.strip_prefix(scheme) {
            if let Some((_userinfo, after)) = rest.split_once('@') {
                return format!("{scheme}<redacted>@{after}");
            }
        }
    }
    remote.to_string()
}

fn git_output(args: &[&str], working_dir: &Path) -> Result<String, String> {
    let output = Command::new("/usr/bin/git")
        .arg("-C")
        .arg(working_dir)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git in {}: {e}", working_dir.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "git command failed".to_string()
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn current_repo_remote(root: &Path) -> Result<(String, String), String> {
    if let Ok(origin) = git_output(&["remote", "get-url", "origin"], root) {
        if !origin.trim().is_empty() {
            return Ok(("origin".to_string(), origin));
        }
    }

    let remotes = git_output(&["remote"], root).map_err(|_| missing_remote_message())?;
    let Some(remote_name) = remotes.lines().find(|line| !line.trim().is_empty()) else {
        return Err(missing_remote_message());
    };
    let remote_url = git_output(&["remote", "get-url", remote_name.trim()], root)?;
    Ok((remote_name.trim().to_string(), remote_url))
}

pub fn resolve_current_repo_from_dir(dir: &Path) -> Result<RepoRef, String> {
    let root = git_output(&["rev-parse", "--show-toplevel"], dir).map_err(|_| {
        format!(
            "{} is not inside a git repository. Run `norn-tui` from a local clone with a GitHub or Bitbucket remote, or use `norn-tui --workspace` to open configured repositories.",
            dir.display()
        )
    })?;
    let root = PathBuf::from(root);
    let (_remote_name, remote_url) = current_repo_remote(&root)?;
    let (provider, workspace, repo) = parse_git_remote(&remote_url)?;

    Ok(RepoRef {
        provider,
        workspace,
        repo,
        local_path: Some(root.display().to_string()),
    })
}

pub fn resolve_current_repo() -> Result<RepoRef, String> {
    let cwd = env::current_dir().map_err(|e| format!("Cannot read current directory: {e}"))?;
    resolve_current_repo_from_dir(&cwd)
}

fn missing_remote_message() -> String {
    "This git repository has no remotes configured. Add an origin remote for GitHub or Bitbucket, or use `norn-tui --workspace` to open configured repositories."
        .to_string()
}

pub fn configured_repo_path(repo_ref: &RepoRef) -> Option<PathBuf> {
    repo_ref
        .local_path
        .as_ref()
        .map(|path| PathBuf::from(path.trim()))
        .filter(|path| !path.as_os_str().is_empty())
}

pub fn configured_or_discovered_repo(workspace: &str, repo: &str) -> Option<PathBuf> {
    let cfg = config::load();
    if let Some(repo_ref) = cfg
        .repos
        .iter()
        .find(|candidate| candidate.workspace == workspace && candidate.repo == repo)
    {
        if let Some(path) = configured_repo_path(repo_ref) {
            if path.is_dir()
                && git_origin_matches(&path, repo_ref.provider, workspace, repo).ok() == Some(true)
            {
                return Some(path);
            }
        }
    }
    autodiscover_local_repo(cfg.review_provider, workspace, repo)
}

pub fn resolve_local_repo(workspace: &str, repo: &str) -> Result<PathBuf, String> {
    let cfg = config::load();
    let provider = cfg
        .repos
        .iter()
        .find(|candidate| candidate.workspace == workspace && candidate.repo == repo)
        .map(|candidate| candidate.provider)
        .unwrap_or(cfg.review_provider);
    resolve_local_repo_for_provider_with_config(&cfg, provider, workspace, repo)
}

pub fn resolve_local_repo_for_provider(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
) -> Result<PathBuf, String> {
    let cfg = config::load();
    resolve_local_repo_for_provider_with_config(&cfg, provider, workspace, repo)
}

fn resolve_local_repo_for_provider_with_config(
    cfg: &config::AppConfig,
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
) -> Result<PathBuf, String> {
    let configured = cfg.repos.iter().find(|candidate| {
        candidate.provider == provider && candidate.workspace == workspace && candidate.repo == repo
    });
    let configured = configured.and_then(configured_repo_path);
    let mut configured_error = None;

    if let Some(path) = configured {
        if !path.is_dir() {
            configured_error = Some(format!(
                "Configured local path does not exist or is not a directory: {}.",
                path.display()
            ));
        } else if git_origin_matches(&path, provider, workspace, repo)? {
            return Ok(path);
        } else {
            configured_error = Some(format!(
                "Configured local path does not match {}: {}.",
                remote_label(provider, workspace, repo),
                path.display()
            ));
        }
    }

    if let Some(found) = autodiscover_local_repo(provider, workspace, repo) {
        return Ok(found);
    }

    Err(match configured_error {
        Some(message) => format!("{message} No matching local clone was auto-discovered."),
        None => format!(
            "No local clone found for {}. Configure a local path in Settings or clone the repo under a scanned directory.",
            remote_label(provider, workspace, repo)
        ),
    })
}

fn remote_label(provider: ReviewProvider, workspace: &str, repo: &str) -> String {
    match provider {
        ReviewProvider::Bitbucket => format!("bitbucket.org/{workspace}/{repo}"),
        ReviewProvider::Github => format!("github.com/{workspace}/{repo}"),
    }
}

pub fn find_in_path(bin: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(bin))
        .find(|candidate| {
            fs::metadata(candidate)
                .map(|meta| meta.is_file())
                .unwrap_or(false)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        env::temp_dir().join(format!(
            "lachesi-local-repo-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn parses_github_and_bitbucket_remote_urls() {
        assert_eq!(
            parse_git_remote("git@github.com:lachesi-hq/lachesi.git").unwrap(),
            (
                ReviewProvider::Github,
                "lachesi-hq".to_string(),
                "lachesi".to_string()
            )
        );
        assert_eq!(
            parse_git_remote("https://bitbucket.org/compri-vcs/procurement-frontend.git").unwrap(),
            (
                ReviewProvider::Bitbucket,
                "compri-vcs".to_string(),
                "procurement-frontend".to_string()
            )
        );
        assert_eq!(
            parse_git_remote("ssh://git@github.com/lachesi-hq/lachesi.git").unwrap(),
            (
                ReviewProvider::Github,
                "lachesi-hq".to_string(),
                "lachesi".to_string()
            )
        );
    }

    #[test]
    fn rejects_unsupported_remote_hosts() {
        let error = parse_git_remote("git@example.com:owner/repo.git").unwrap_err();
        assert!(error.contains("Unsupported git remote host"));
    }

    #[test]
    fn resolves_current_repo_from_git_remote() {
        let path = temp_path("current");
        fs::create_dir_all(&path).expect("temp repo dir");
        Command::new("/usr/bin/git")
            .arg("init")
            .arg(&path)
            .output()
            .expect("git init");
        Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&path)
            .arg("remote")
            .arg("add")
            .arg("origin")
            .arg("git@github.com:lachesi-hq/lachesi.git")
            .output()
            .expect("git remote add");

        let resolved = resolve_current_repo_from_dir(&path).expect("current repo");

        assert_eq!(resolved.provider, ReviewProvider::Github);
        assert_eq!(resolved.workspace, "lachesi-hq");
        assert_eq!(resolved.repo, "lachesi");
        assert_eq!(
            fs::canonicalize(resolved.local_path.as_deref().unwrap()).expect("resolved path"),
            fs::canonicalize(&path).expect("expected path")
        );

        fs::remove_dir_all(path).expect("cleanup temp repo");
    }

    #[test]
    fn resolves_configured_repo_for_explicit_provider() {
        let path = temp_path("configured-provider");
        fs::create_dir_all(&path).expect("temp repo dir");
        Command::new("/usr/bin/git")
            .arg("init")
            .arg(&path)
            .output()
            .expect("git init");
        Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&path)
            .args([
                "remote",
                "add",
                "origin",
                "git@github.com:lachesi-hq/lachesi.git",
            ])
            .output()
            .expect("git remote add");
        let cfg = config::AppConfig {
            repos: vec![config::RepoRef {
                provider: ReviewProvider::Github,
                workspace: "lachesi-hq".to_string(),
                repo: "lachesi".to_string(),
                local_path: Some(path.display().to_string()),
            }],
            review_provider: ReviewProvider::Bitbucket,
            ..config::AppConfig::default()
        };

        let resolved = resolve_local_repo_for_provider_with_config(
            &cfg,
            ReviewProvider::Github,
            "lachesi-hq",
            "lachesi",
        )
        .expect("configured GitHub repo");

        assert_eq!(
            fs::canonicalize(resolved).expect("resolved path"),
            fs::canonicalize(&path).expect("expected path")
        );
        fs::remove_dir_all(path).expect("cleanup temp repo");
    }

    #[test]
    fn resolving_current_repo_reports_non_git_directory() {
        let path = temp_path("not-git");
        fs::create_dir_all(&path).expect("temp dir");

        let error = match resolve_current_repo_from_dir(&path) {
            Ok(_) => panic!("expected non-git error"),
            Err(error) => error,
        };

        assert!(error.contains("not inside a git repository"));
        assert!(error.contains("norn-tui --workspace"));
        fs::remove_dir_all(path).expect("cleanup temp dir");
    }

    #[test]
    fn resolving_current_repo_reports_missing_remote() {
        let path = temp_path("no-remote");
        fs::create_dir_all(&path).expect("temp repo dir");
        Command::new("/usr/bin/git")
            .arg("init")
            .arg(&path)
            .output()
            .expect("git init");

        let error = match resolve_current_repo_from_dir(&path) {
            Ok(_) => panic!("expected missing remote error"),
            Err(error) => error,
        };

        assert!(error.contains("no remotes configured"));
        assert!(error.contains("norn-tui --workspace"));
        fs::remove_dir_all(path).expect("cleanup temp repo");
    }

    #[test]
    fn remote_parse_errors_redact_userinfo() {
        let error = parse_git_remote("https://user:secret@github.com/owner").unwrap_err();

        assert!(error.contains("<redacted>@github.com"));
        assert!(!error.contains("secret"));
    }
}
