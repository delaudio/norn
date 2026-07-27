use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::config::{self, AiProvider, ReviewProvider};
use crate::local_repo;
use crate::repo_config::{self, ReviewSeverity};
use crate::services::bitbucket::{get_pr_diff_native, get_pull_request_native};
use crate::services::review::{
    run_headless_review_native, HeadlessNativeReviewError, HeadlessNativeReviewRequest,
    ReviewFinding, ReviewFindingSeverity, ReviewProvider as ReviewRunProvider, ReviewRun,
};

const DEFAULT_REVIEW_PROMPT: &str = include_str!("../../src/lib/defaultReviewPrompt.md");
const MAX_UNTRACKED_FILE_BYTES: u64 = 512 * 1024;
const MAX_UNTRACKED_TOTAL_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewScope {
    WorkingTree,
    Branch,
    PullRequest,
}

impl ReviewScope {
    fn label(self) -> &'static str {
        match self {
            Self::WorkingTree => "working-tree",
            Self::Branch => "branch",
            Self::PullRequest => "pr",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeadlessReviewRequest {
    pub repo_path: Option<PathBuf>,
    pub scope: ReviewScope,
    pub base: Option<String>,
    pub workspace: Option<String>,
    pub repo: Option<String>,
    pub pr_id: Option<u32>,
    pub provider: Option<ReviewProvider>,
    pub profile: Option<String>,
    pub ai_provider: Option<AiProvider>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub run_analyzers: bool,
}

#[derive(Debug)]
pub struct HeadlessReviewError {
    pub exit_code: i32,
    pub message: String,
}

impl HeadlessReviewError {
    fn config(message: impl Into<String>) -> Self {
        Self {
            exit_code: 2,
            message: message.into(),
        }
    }

    fn target(message: impl Into<String>) -> Self {
        Self {
            exit_code: 4,
            message: message.into(),
        }
    }

    fn auth(message: impl Into<String>) -> Self {
        Self {
            exit_code: 3,
            message: message.into(),
        }
    }

    fn model(message: impl Into<String>) -> Self {
        Self {
            exit_code: 6,
            message: message.into(),
        }
    }

    fn analyzer(message: impl Into<String>) -> Self {
        Self {
            exit_code: 5,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            exit_code: 7,
            message: message.into(),
        }
    }

    fn cancelled(message: impl Into<String>) -> Self {
        Self {
            exit_code: 130,
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadlessReviewTarget {
    pub scope: String,
    pub repo_path: String,
    pub workspace: Option<String>,
    pub repo: String,
    pub pr_id: Option<u32>,
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadlessReviewExecution {
    pub schema_version: String,
    pub status: String,
    pub exit_code: i32,
    pub warnings: Vec<String>,
    pub minimum_severity: Option<ReviewFindingSeverity>,
    pub analyzers_ran: bool,
    pub target: HeadlessReviewTarget,
    pub review_run: Option<ReviewRun>,
}

struct ResolvedTarget {
    target: HeadlessReviewTarget,
    provider: ReviewProvider,
    workspace: String,
    repo: String,
    pr_id: u32,
    title: String,
    source_branch: String,
    destination_branch: String,
    diff: String,
    warnings: Vec<String>,
}

pub fn run(request: HeadlessReviewRequest) -> Result<HeadlessReviewExecution, HeadlessReviewError> {
    let repo_path = resolve_repo_root_for_request(&request)?;
    let config_result =
        repo_config::load_from_repo_path_with_profile(&repo_path, request.profile.as_deref())
            .map_err(HeadlessReviewError::config)?;
    if !config_result.errors.is_empty() {
        return Err(HeadlessReviewError::config(
            config_result
                .errors
                .iter()
                .map(|error| format!("{}: {}", error.path, error.message))
                .collect::<Vec<_>>()
                .join("\n"),
        ));
    }

    let mut resolved = resolve_target(&request, &repo_path)?;
    resolved.warnings.extend(
        config_result
            .warnings
            .iter()
            .map(|warning| format!("{}: {}", warning.path, warning.message)),
    );
    let prompt = resolve_prompt(config_result.config.as_ref());
    let minimum_severity = config_result
        .config
        .as_ref()
        .and_then(|config| config.review.as_ref())
        .and_then(|review| review.findings.as_ref())
        .and_then(|findings| findings.min_severity)
        .map(severity_from_repo);
    let selected_profile = config_result.selected_profile.clone();

    if resolved.diff.trim().is_empty() {
        return Ok(HeadlessReviewExecution {
            schema_version: "lachesi.headless-review.v1".to_string(),
            status: "succeeded".to_string(),
            exit_code: 0,
            warnings: resolved.warnings,
            minimum_severity,
            analyzers_ran: false,
            target: resolved.target,
            review_run: None,
        });
    }

    let app_config = config::load();
    let ai_provider = request.ai_provider.unwrap_or(app_config.ai_provider);
    let (claude_model, claude_effort, codex_model, codex_effort) = match ai_provider {
        AiProvider::Claude => (
            request.model.or(app_config.claude_model),
            request.effort.or(app_config.claude_effort),
            None,
            None,
        ),
        AiProvider::Codex => (
            None,
            None,
            request.model.or(app_config.codex_model),
            request.effort.or(app_config.codex_effort),
        ),
    };
    let payload = build_review_payload(
        &prompt,
        &resolved.title,
        &resolved.source_branch,
        &resolved.destination_branch,
        &resolved.diff,
        request.scope,
    );
    let review_run = run_headless_review_native(HeadlessNativeReviewRequest {
        repo_path: repo_path.clone(),
        review_provider: review_run_provider(resolved.provider),
        workspace: resolved.workspace,
        repo: resolved.repo,
        pr_id: resolved.pr_id,
        title: resolved.title,
        source_branch: resolved.source_branch,
        destination_branch: resolved.destination_branch,
        payload,
        ai_provider,
        claude_model,
        claude_effort,
        codex_model,
        codex_effort,
        review_profile: selected_profile,
        run_analyzers: request.run_analyzers,
    })
    .map_err(map_native_review_error)?;

    Ok(HeadlessReviewExecution {
        schema_version: "lachesi.headless-review.v1".to_string(),
        status: "succeeded".to_string(),
        exit_code: 0,
        warnings: resolved.warnings,
        minimum_severity,
        analyzers_ran: request.run_analyzers,
        target: resolved.target,
        review_run: Some(review_run),
    })
}

fn map_native_review_error(error: HeadlessNativeReviewError) -> HeadlessReviewError {
    match error {
        HeadlessNativeReviewError::Analyzer(message) => HeadlessReviewError::analyzer(message),
        HeadlessNativeReviewError::Provider(message) => HeadlessReviewError::model(message),
        HeadlessNativeReviewError::Internal(message) => HeadlessReviewError::internal(message),
        HeadlessNativeReviewError::Cancelled => HeadlessReviewError::cancelled("Review cancelled."),
    }
}

pub fn format_markdown(execution: &HeadlessReviewExecution) -> String {
    let mut output = vec![
        format!("# Lachesi review: {}", execution.target.repo),
        String::new(),
        format!(
            "Target: {} ({} -> {})",
            execution.target.scope, execution.target.source, execution.target.destination
        ),
        format!(
            "Analyzers: {}",
            if execution.analyzers_ran {
                "executed"
            } else {
                "skipped"
            }
        ),
    ];
    for warning in &execution.warnings {
        output.push(format!("Warning: {warning}"));
    }
    match execution.review_run.as_ref() {
        Some(run) => {
            output.push(String::new());
            output.extend(format_findings_markdown(&run.findings));
            if let Some(summary) = run
                .summary_markdown
                .as_deref()
                .map(str::trim)
                .filter(|summary| !summary.is_empty())
            {
                output.extend([
                    String::new(),
                    "## Assistant summary".to_string(),
                    String::new(),
                    summary.to_string(),
                ]);
            }
            output.extend([
                String::new(),
                format!("Run: {} ({})", run.id, run.schema_version),
            ]);
        }
        None => {
            output.extend([String::new(), "No changes to review.".to_string()]);
        }
    }
    output.join("\n")
}

fn format_findings_markdown(findings: &[ReviewFinding]) -> Vec<String> {
    let mut output = vec!["## Findings".to_string()];
    if findings.is_empty() {
        output.extend([String::new(), "No findings.".to_string()]);
        return output;
    }

    for severity in [
        ReviewFindingSeverity::Critical,
        ReviewFindingSeverity::High,
        ReviewFindingSeverity::Medium,
        ReviewFindingSeverity::Low,
        ReviewFindingSeverity::Info,
    ] {
        let group = findings
            .iter()
            .filter(|finding| finding.severity == severity)
            .collect::<Vec<_>>();
        if group.is_empty() {
            continue;
        }
        output.extend([String::new(), format!("### {}", severity_label(severity))]);
        for finding in group {
            let anchor = finding.anchor.as_ref().map(|anchor| {
                let lines = match anchor.end_line {
                    Some(end) if end != anchor.start_line => {
                        format!("{}-{}", anchor.start_line, end)
                    }
                    _ => anchor.start_line.to_string(),
                };
                format!(" (`{}:{lines}`)", anchor.path)
            });
            output.push(format!(
                "- **{}**{}: {}",
                finding.title,
                anchor.unwrap_or_default(),
                finding.summary
            ));
            if let Some(fix) = finding.suggested_fix.as_deref() {
                output.push(format!("  Fix: {fix}"));
            }
        }
    }
    output
}

fn severity_label(severity: ReviewFindingSeverity) -> &'static str {
    match severity {
        ReviewFindingSeverity::Info => "Info",
        ReviewFindingSeverity::Low => "Low",
        ReviewFindingSeverity::Medium => "Medium",
        ReviewFindingSeverity::High => "High",
        ReviewFindingSeverity::Critical => "Critical",
    }
}

pub fn has_findings_at_or_above(
    execution: &HeadlessReviewExecution,
    minimum: ReviewFindingSeverity,
) -> bool {
    execution.review_run.as_ref().is_some_and(|run| {
        run.findings
            .iter()
            .any(|finding| severity_rank(finding.severity) >= severity_rank(minimum))
    })
}

pub fn severity_from_repo(value: ReviewSeverity) -> ReviewFindingSeverity {
    match value {
        ReviewSeverity::Info => ReviewFindingSeverity::Info,
        ReviewSeverity::Low => ReviewFindingSeverity::Low,
        ReviewSeverity::Medium => ReviewFindingSeverity::Medium,
        ReviewSeverity::High => ReviewFindingSeverity::High,
        ReviewSeverity::Critical => ReviewFindingSeverity::Critical,
    }
}

fn severity_rank(value: ReviewFindingSeverity) -> u8 {
    match value {
        ReviewFindingSeverity::Info => 0,
        ReviewFindingSeverity::Low => 1,
        ReviewFindingSeverity::Medium => 2,
        ReviewFindingSeverity::High => 3,
        ReviewFindingSeverity::Critical => 4,
    }
}

fn resolve_repo_root(path: &Path) -> Result<PathBuf, HeadlessReviewError> {
    let output = git_output(path, ["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(output.trim()))
}

fn resolve_repo_root_for_request(
    request: &HeadlessReviewRequest,
) -> Result<PathBuf, HeadlessReviewError> {
    if let Some(repo_path) = request.repo_path.as_deref() {
        return resolve_repo_root(repo_path);
    }

    let cwd = std::env::current_dir().map_err(|error| {
        HeadlessReviewError::target(format!("Cannot read current directory: {error}"))
    })?;
    if request.scope != ReviewScope::PullRequest {
        return resolve_repo_root(&cwd);
    }

    let (Some(workspace), Some(repo)) = (request.workspace.as_deref(), request.repo.as_deref())
    else {
        return resolve_repo_root(&cwd);
    };
    let discovered = match request.provider {
        Some(provider) => local_repo::resolve_local_repo_for_provider(provider, workspace, repo),
        None => local_repo::resolve_local_repo(workspace, repo),
    };
    match discovered {
        Ok(path) => resolve_repo_root(&path),
        Err(discovery_error) => {
            let identity =
                local_repo::resolve_current_repo_from_dir(&cwd).map_err(|cwd_error| {
                    HeadlessReviewError::target(format!(
                        "{discovery_error} Current-directory fallback failed: {cwd_error}"
                    ))
                })?;
            if !repo_identity_matches_target(&identity, request.provider, workspace, repo) {
                return Err(HeadlessReviewError::target(format!(
                    "{discovery_error} The current directory does not match the requested repository. Pass `--repo-path` or configure its local path."
                )));
            }
            resolve_repo_root(&cwd)
        }
    }
}

fn repo_identity_matches_target(
    identity: &config::RepoRef,
    provider: Option<ReviewProvider>,
    workspace: &str,
    repo: &str,
) -> bool {
    identity.workspace == workspace
        && identity.repo == repo
        && provider.is_none_or(|provider| identity.provider == provider)
}

fn resolve_target(
    request: &HeadlessReviewRequest,
    repo_path: &Path,
) -> Result<ResolvedTarget, HeadlessReviewError> {
    let identity = local_repo::resolve_current_repo_from_dir(repo_path).ok();
    let workspace = request
        .workspace
        .clone()
        .or_else(|| identity.as_ref().map(|repo| repo.workspace.clone()))
        .unwrap_or_else(|| "local".to_string());
    let repo = request
        .repo
        .clone()
        .or_else(|| identity.as_ref().map(|repo| repo.repo.clone()))
        .or_else(|| {
            repo_path
                .file_name()
                .and_then(OsStr::to_str)
                .map(ToOwned::to_owned)
        })
        .ok_or_else(|| HeadlessReviewError::target("Cannot determine repository name."))?;
    let provider = request
        .provider
        .or_else(|| identity.as_ref().map(|repo| repo.provider))
        .unwrap_or_else(|| config::load().review_provider);

    match request.scope {
        ReviewScope::WorkingTree => {
            let source = current_branch(repo_path);
            let (diff, warnings) = working_tree_diff(repo_path)?;
            Ok(local_target(
                repo_path,
                workspace,
                repo,
                source,
                "HEAD".to_string(),
                "Working tree changes".to_string(),
                diff,
                warnings,
                request.scope,
                provider,
            ))
        }
        ReviewScope::Branch => {
            let source = current_branch(repo_path);
            let base = match request.base.as_deref() {
                Some(base) if !base.trim().is_empty() => base.trim().to_string(),
                _ => resolve_default_base(repo_path)?,
            };
            let merge_base = git_output(repo_path, ["merge-base", base.as_str(), "HEAD"])?;
            let diff = git_output(
                repo_path,
                [
                    "diff",
                    "--no-ext-diff",
                    "--find-renames",
                    merge_base.trim(),
                    "HEAD",
                    "--",
                ],
            )?;
            Ok(local_target(
                repo_path,
                workspace,
                repo,
                source,
                base,
                "Current branch changes".to_string(),
                diff,
                Vec::new(),
                request.scope,
                provider,
            ))
        }
        ReviewScope::PullRequest => {
            let pr_id = request
                .pr_id
                .ok_or_else(|| HeadlessReviewError::target("`--pr` is required for PR scope."))?;
            let detail = get_pull_request_native(Some(provider), &workspace, &repo, pr_id)
                .map_err(map_provider_target_error)?;
            let diff = get_pr_diff_native(Some(provider), &workspace, &repo, pr_id)
                .map_err(map_provider_target_error)?;
            Ok(ResolvedTarget {
                target: HeadlessReviewTarget {
                    scope: request.scope.label().to_string(),
                    repo_path: repo_path.display().to_string(),
                    workspace: Some(workspace.clone()),
                    repo: repo.clone(),
                    pr_id: Some(pr_id),
                    source: detail.source_branch.clone(),
                    destination: detail.destination_branch.clone(),
                },
                provider,
                workspace,
                repo,
                pr_id,
                title: detail.title,
                source_branch: detail.source_branch,
                destination_branch: detail.destination_branch,
                diff,
                warnings: Vec::new(),
            })
        }
    }
}

fn map_provider_target_error(message: String) -> HeadlessReviewError {
    let normalized = message.to_ascii_lowercase();
    let is_auth_error = normalized.contains("no bitbucket credentials configured")
        || normalized.contains("no github token configured")
        || normalized.contains("api error 401")
        || normalized.contains("api error 403")
        || normalized.contains("401 unauthorized")
        || normalized.contains("403 forbidden");
    if is_auth_error {
        HeadlessReviewError::auth(message)
    } else {
        HeadlessReviewError::target(message)
    }
}

#[allow(clippy::too_many_arguments)]
fn local_target(
    repo_path: &Path,
    workspace: String,
    repo: String,
    source: String,
    destination: String,
    title: String,
    diff: String,
    warnings: Vec<String>,
    scope: ReviewScope,
    provider: ReviewProvider,
) -> ResolvedTarget {
    ResolvedTarget {
        target: HeadlessReviewTarget {
            scope: scope.label().to_string(),
            repo_path: repo_path.display().to_string(),
            workspace: (workspace != "local").then_some(workspace.clone()),
            repo: repo.clone(),
            pr_id: None,
            source: source.clone(),
            destination: destination.clone(),
        },
        provider,
        workspace,
        repo,
        pr_id: 0,
        title,
        source_branch: source,
        destination_branch: destination,
        diff,
        warnings,
    }
}

fn review_run_provider(provider: ReviewProvider) -> ReviewRunProvider {
    match provider {
        ReviewProvider::Bitbucket => ReviewRunProvider::Bitbucket,
        ReviewProvider::Github => ReviewRunProvider::Github,
    }
}

fn current_branch(repo_path: &Path) -> String {
    git_output(repo_path, ["branch", "--show-current"])
        .map(|branch| {
            let branch = branch.trim();
            if branch.is_empty() {
                "HEAD".to_string()
            } else {
                branch.to_string()
            }
        })
        .unwrap_or_else(|_| "HEAD".to_string())
}

fn resolve_default_base(repo_path: &Path) -> Result<String, HeadlessReviewError> {
    if let Ok(base) = git_output(
        repo_path,
        [
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    ) {
        if !base.trim().is_empty() {
            return Ok(base.trim().to_string());
        }
    }
    for candidate in ["origin/main", "main", "origin/master", "master"] {
        if git_output(repo_path, ["rev-parse", "--verify", "--quiet", candidate]).is_ok() {
            return Ok(candidate.to_string());
        }
    }
    Err(HeadlessReviewError::target(
        "Cannot resolve a base branch. Pass `--base <ref>`.",
    ))
}

fn working_tree_diff(repo_path: &Path) -> Result<(String, Vec<String>), HeadlessReviewError> {
    let mut diff = git_output(
        repo_path,
        ["diff", "--no-ext-diff", "--find-renames", "HEAD", "--"],
    )?;
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .output()
        .map_err(|error| HeadlessReviewError::target(format!("Failed to run git: {error}")))?;
    if !output.status.success() {
        return Err(HeadlessReviewError::target(git_error_message(&output)));
    }

    let mut warnings = Vec::new();
    let mut included_bytes = 0_u64;
    for raw_path in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let relative = String::from_utf8_lossy(raw_path).to_string();
        let path = repo_path.join(&relative);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(metadata) if metadata.file_type().is_symlink() => {
                warnings.push(format!("Skipped untracked symlink `{relative}`."));
                continue;
            }
            _ => continue,
        };
        if metadata.len() > MAX_UNTRACKED_FILE_BYTES
            || included_bytes.saturating_add(metadata.len()) > MAX_UNTRACKED_TOTAL_BYTES
        {
            warnings.push(format!("Skipped large untracked file `{relative}`."));
            continue;
        }
        let contents = fs::read(&path).map_err(|error| {
            HeadlessReviewError::target(format!(
                "Failed to read untracked file `{relative}`: {error}"
            ))
        })?;
        if contents.contains(&0) {
            warnings.push(format!("Skipped binary untracked file `{relative}`."));
            continue;
        }
        let text = String::from_utf8_lossy(&contents);
        if !diff.is_empty() && !diff.ends_with('\n') {
            diff.push('\n');
        }
        diff.push_str(&new_file_patch(&relative, &text));
        included_bytes = included_bytes.saturating_add(metadata.len());
    }
    Ok((diff, warnings))
}

fn new_file_patch(path: &str, contents: &str) -> String {
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

fn git_output<const N: usize>(
    repo_path: &Path,
    args: [&str; N],
) -> Result<String, HeadlessReviewError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map_err(|error| HeadlessReviewError::target(format!("Failed to run git: {error}")))?;
    if !output.status.success() {
        return Err(HeadlessReviewError::target(git_error_message(&output)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_error_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("git exited with status {}.", output.status)
    } else {
        stderr
    }
}

fn resolve_prompt(config: Option<&repo_config::RepoReviewConfig>) -> String {
    let prompt = config
        .and_then(|config| config.review.as_ref())
        .and_then(|review| review.prompt.as_ref())
        .cloned()
        .unwrap_or_default();
    let replacement = prompt
        .replace
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let extension = prompt
        .extend
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let base = replacement.unwrap_or_else(|| DEFAULT_REVIEW_PROMPT.trim().to_string());
    match extension {
        Some(extension) => format!("{base}\n\n## Repository review policy\n{extension}"),
        None => base,
    }
}

fn build_review_payload(
    prompt: &str,
    title: &str,
    source: &str,
    destination: &str,
    diff: &str,
    scope: ReviewScope,
) -> String {
    let scope_note = match scope {
        ReviewScope::WorkingTree => {
            "This target can contain staged, unstaged, and untracked files. Do not describe a file as committed unless the supplied evidence proves that it is committed."
        }
        ReviewScope::Branch => {
            "This target contains commits on the current branch relative to the selected base."
        }
        ReviewScope::PullRequest => {
            "This target contains the provider pull-request diff."
        }
    };
    let mut payload = [
        prompt.trim(),
        "",
        "## Review target",
        &format!("{title} ({})", scope.label()),
        &format!("Branch: {source} -> {destination}"),
        scope_note,
        "",
        "## Diff",
        "```diff",
    ]
    .join("\n");
    payload.push('\n');
    payload.push_str(diff);
    if !diff.ends_with('\n') {
        payload.push('\n');
    }
    payload.push_str("```");
    payload
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        build_review_payload, format_findings_markdown, map_native_review_error,
        map_provider_target_error, new_file_patch, repo_identity_matches_target, working_tree_diff,
        ReviewScope,
    };
    use crate::config::{RepoRef, ReviewProvider};
    use crate::services::review::{
        HeadlessNativeReviewError, ReviewAnchorSide, ReviewFinding, ReviewFindingAnchor,
        ReviewFindingCategory, ReviewFindingConfidence, ReviewFindingSeverity, ReviewFindingSource,
        ReviewFindingStatus,
    };

    static TEMP_REPO_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn temp_git_repo() -> std::path::PathBuf {
        let nonce = TEMP_REPO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let repo = std::env::temp_dir().join(format!(
            "lachesi-headless-review-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&repo).expect("create temp repo");
        git(&repo, &["init"]);
        git(&repo, &["config", "user.name", "Lachesi Test"]);
        git(&repo, &["config", "user.email", "test@lachesi.invalid"]);
        fs::write(repo.join("staged.txt"), "before staged\n").expect("write staged fixture");
        fs::write(repo.join("unstaged.txt"), "before unstaged\n").expect("write unstaged fixture");
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "test: baseline"]);
        repo
    }

    #[test]
    fn new_file_patch_marks_every_line_as_added() {
        let patch = new_file_patch("src/new file.ts", "one\ntwo\n");
        assert!(patch.contains("diff --git a/src/new file.ts b/src/new file.ts"));
        assert!(patch.contains("@@ -0,0 +1,2 @@"));
        assert!(patch.contains("+one\n+two\n"));
    }

    #[test]
    fn review_scope_labels_are_stable() {
        assert_eq!(ReviewScope::WorkingTree.label(), "working-tree");
        assert_eq!(ReviewScope::Branch.label(), "branch");
        assert_eq!(ReviewScope::PullRequest.label(), "pr");
    }

    #[test]
    fn cwd_fallback_requires_matching_repository_identity() {
        let identity = RepoRef {
            provider: ReviewProvider::Github,
            workspace: "lachesi-hq".to_string(),
            repo: "lachesi".to_string(),
            local_path: Some("/tmp/lachesi".to_string()),
        };

        assert!(repo_identity_matches_target(
            &identity,
            Some(ReviewProvider::Github),
            "lachesi-hq",
            "lachesi",
        ));
        assert!(!repo_identity_matches_target(
            &identity,
            Some(ReviewProvider::Bitbucket),
            "lachesi-hq",
            "lachesi",
        ));
        assert!(!repo_identity_matches_target(
            &identity,
            Some(ReviewProvider::Github),
            "other",
            "lachesi",
        ));
    }

    #[test]
    fn markdown_findings_are_grouped_and_anchored() {
        let finding = ReviewFinding {
            id: "finding-1".to_string(),
            fingerprint: "fingerprint-1".to_string(),
            title: "Unsafe fallback".to_string(),
            severity: ReviewFindingSeverity::High,
            confidence: ReviewFindingConfidence::High,
            category: ReviewFindingCategory::Security,
            status: ReviewFindingStatus::New,
            summary: "The fallback can select the wrong repository.".to_string(),
            rationale: None,
            rule_id: None,
            source: ReviewFindingSource::Llm,
            anchor: Some(ReviewFindingAnchor {
                path: "src/review.rs".to_string(),
                start_line: 42,
                end_line: Some(44),
                side: ReviewAnchorSide::New,
            }),
            suggested_fix: Some("Validate the remote identity.".to_string()),
            evidence_ids: Vec::new(),
            publication: None,
        };

        let markdown = format_findings_markdown(&[finding]).join("\n");

        assert!(markdown.contains("### High"));
        assert!(markdown.contains("`src/review.rs:42-44`"));
        assert!(markdown.contains("Fix: Validate the remote identity."));
    }

    #[test]
    fn working_tree_payload_does_not_imply_untracked_files_are_committed() {
        let payload = build_review_payload(
            "Review carefully.",
            "Working tree changes",
            "feature",
            "HEAD",
            "diff --git a/new.txt b/new.txt",
            ReviewScope::WorkingTree,
        );

        assert!(payload.contains("staged, unstaged, and untracked files"));
        assert!(payload.contains("Do not describe a file as committed"));
    }

    #[test]
    fn review_payload_preserves_diff_whitespace() {
        let diff = " diff header\n+trailing spaces  \n+   \n";
        let payload = build_review_payload(
            "Review carefully.",
            "Whitespace changes",
            "feature",
            "main",
            diff,
            ReviewScope::Branch,
        );

        assert!(payload.contains(&format!("```diff\n{diff}```")));
    }

    #[test]
    fn working_tree_diff_includes_staged_unstaged_and_untracked_text() {
        let repo = temp_git_repo();
        fs::write(repo.join("staged.txt"), "after staged\n").expect("update staged file");
        git(&repo, &["add", "staged.txt"]);
        fs::write(repo.join("unstaged.txt"), "after unstaged\n").expect("update unstaged file");
        fs::write(repo.join("untracked.txt"), "new untracked\n").expect("write untracked file");

        let (diff, warnings) = working_tree_diff(&repo).expect("collect working tree diff");

        assert!(warnings.is_empty());
        assert!(diff.contains("+after staged"));
        assert!(diff.contains("+after unstaged"));
        assert!(diff.contains("diff --git a/untracked.txt b/untracked.txt"));
        assert!(diff.contains("+new untracked"));
        let _ = fs::remove_dir_all(repo);
    }

    #[cfg(unix)]
    #[test]
    fn working_tree_diff_does_not_follow_untracked_symlinks() {
        use std::os::unix::fs::symlink;

        let repo = temp_git_repo();
        let secret_path = repo.with_extension("outside-secret");
        fs::write(&secret_path, "must not be reviewed\n").expect("write external fixture");
        symlink(&secret_path, repo.join("external-link")).expect("create symlink");

        let (diff, warnings) = working_tree_diff(&repo).expect("collect working tree diff");

        assert!(!diff.contains("must not be reviewed"));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("Skipped untracked symlink")));
        let _ = fs::remove_dir_all(repo);
        let _ = fs::remove_file(secret_path);
    }

    #[test]
    fn native_review_errors_map_to_stable_exit_codes() {
        assert_eq!(
            map_native_review_error(HeadlessNativeReviewError::Analyzer("failed".to_string()))
                .exit_code,
            5
        );
        assert_eq!(
            map_native_review_error(HeadlessNativeReviewError::Provider("failed".to_string()))
                .exit_code,
            6
        );
        assert_eq!(
            map_native_review_error(HeadlessNativeReviewError::Internal("failed".to_string()))
                .exit_code,
            7
        );
        assert_eq!(
            map_native_review_error(HeadlessNativeReviewError::Cancelled).exit_code,
            130
        );
    }

    #[test]
    fn provider_auth_errors_map_to_auth_exit_code() {
        for message in [
            "No Bitbucket credentials configured.",
            "No GitHub token configured.",
            "Bitbucket API error 401 Unauthorized: denied",
            "GitHub API error 403 Forbidden: denied",
        ] {
            assert_eq!(map_provider_target_error(message.to_string()).exit_code, 3);
        }
        assert_eq!(
            map_provider_target_error("GitHub API error 404 Not Found".to_string()).exit_code,
            4
        );
    }
}
