use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
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
const HEADLESS_REVIEW_BOUNDARY: &str = "## Headless reviewer boundary\n\nReview only the supplied policy, context, evidence, and diff. Do not inspect the filesystem or run commands.";
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
    validate_requested_identity_shape(&request)?;
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
        if request.run_analyzers {
            return Err(HeadlessReviewError::target(
                "No changes to review; local analyzers were not run.",
            ));
        }
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
    let payload = format!("{payload}\n\n{HEADLESS_REVIEW_BOUNDARY}");
    let mut review_run = run_headless_review_native(HeadlessNativeReviewRequest {
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
    strip_private_evidence_payloads(&mut review_run);

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

fn strip_private_evidence_payloads(review_run: &mut ReviewRun) {
    for evidence in &mut review_run.evidence {
        evidence.payload = None;
    }
}

fn map_native_review_error(error: HeadlessNativeReviewError) -> HeadlessReviewError {
    match error {
        HeadlessNativeReviewError::Analyzer(message) => HeadlessReviewError::analyzer(message),
        HeadlessNativeReviewError::Provider(message) => {
            HeadlessReviewError::model(public_provider_error(&message))
        }
        HeadlessNativeReviewError::Internal(message) => HeadlessReviewError::internal(message),
        HeadlessNativeReviewError::Cancelled => HeadlessReviewError::cancelled("Review cancelled."),
    }
}

fn public_provider_error(message: &str) -> &'static str {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("empty review response") {
        "AI provider returned an empty review response."
    } else if normalized.contains("structured review")
        || normalized.contains("invalid json")
        || normalized.contains("failed to parse")
    {
        "AI provider returned invalid review output."
    } else if normalized.contains("failed to run")
        || normalized.contains("not found")
        || normalized.contains("was not captured")
    {
        "AI provider CLI could not be started."
    } else {
        "AI provider review failed."
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
    if let Some(pr_id) = execution.target.pr_id {
        output.push(format!("Pull request: #{pr_id}"));
    }
    for warning in &execution.warnings {
        output.push(format!("Warning: {warning}"));
    }
    match execution.review_run.as_ref() {
        Some(run) => {
            if let Some(profile) = run.review_profile.as_deref() {
                output.push(format!("Profile: {profile}"));
            }
            output.push(String::new());
            output.extend(format_findings_markdown(&run.findings));
            if !run.evidence.is_empty() {
                output.extend([String::new(), "## Evidence".to_string(), String::new()]);
                output.extend(run.evidence.iter().map(|evidence| {
                    let summary = evidence
                        .summary
                        .as_deref()
                        .map(|summary| format!(": {summary}"))
                        .unwrap_or_default();
                    format!("- **{}**{summary}", evidence.title)
                }));
            }
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
        let repo_root = resolve_repo_root(repo_path)?;
        validate_explicit_repo_identity(&repo_root, request)?;
        return Ok(repo_root);
    }

    let cwd = std::env::current_dir().map_err(|error| {
        HeadlessReviewError::target(format!("Cannot read current directory: {error}"))
    })?;
    let Some((workspace, repo)) = requested_repo_identity(request) else {
        let repo_root = resolve_repo_root(&cwd)?;
        validate_explicit_repo_identity(&repo_root, request)?;
        return Ok(repo_root);
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

fn validate_requested_identity_shape(
    request: &HeadlessReviewRequest,
) -> Result<(), HeadlessReviewError> {
    if request.workspace.is_some() != request.repo.is_some() {
        return Err(HeadlessReviewError::target(
            "`--workspace` and `--repo` must be provided together.",
        ));
    }
    Ok(())
}

fn validate_explicit_repo_identity(
    repo_root: &Path,
    request: &HeadlessReviewRequest,
) -> Result<(), HeadlessReviewError> {
    if request.provider.is_none() && request.workspace.is_none() && request.repo.is_none() {
        return Ok(());
    }
    let identity = local_repo::resolve_current_repo_from_dir(repo_root).map_err(|error| {
        HeadlessReviewError::target(format!(
            "Cannot verify `--repo-path` against the requested repository: {error}"
        ))
    })?;
    if request
        .provider
        .is_some_and(|provider| identity.provider != provider)
        || request
            .workspace
            .as_deref()
            .is_some_and(|workspace| identity.workspace != workspace)
        || request
            .repo
            .as_deref()
            .is_some_and(|repo| identity.repo != repo)
    {
        return Err(HeadlessReviewError::target(
            "`--repo-path` does not match the requested provider, workspace, or repository.",
        ));
    }
    Ok(())
}

fn requested_repo_identity(request: &HeadlessReviewRequest) -> Option<(&str, &str)> {
    Some((request.workspace.as_deref()?, request.repo.as_deref()?))
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
            let warnings = branch_scope_warnings(repo_path)?;
            Ok(local_target(
                repo_path,
                workspace,
                repo,
                source,
                base,
                "Current branch changes".to_string(),
                diff,
                warnings,
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

fn branch_scope_warnings(repo_path: &Path) -> Result<Vec<String>, HeadlessReviewError> {
    let status = git_output(
        repo_path,
        ["status", "--porcelain=v1", "--untracked-files=normal"],
    )?;
    if status.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(vec![
            "Branch scope reviews committed changes only; staged, unstaged, and untracked files were excluded. Run `--scope working-tree` separately to review them."
                .to_string(),
        ])
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
    let mut warnings = Vec::new();
    let has_head = git_output(repo_path, ["rev-parse", "--verify", "--quiet", "HEAD"]).is_ok();
    if !has_head {
        warnings.push(
            "Repository has no commits; staged and unstaged changes are shown separately."
                .to_string(),
        );
    }
    let staged = if has_head {
        git_output(
            repo_path,
            [
                "diff",
                "--cached",
                "--no-ext-diff",
                "--find-renames",
                "HEAD",
                "--",
            ],
        )?
    } else {
        git_output(
            repo_path,
            ["diff", "--cached", "--no-ext-diff", "--find-renames", "--"],
        )?
    };
    let unstaged = git_output(repo_path, ["diff", "--no-ext-diff", "--find-renames", "--"])?;
    let mut diff = String::new();
    let staged_label = if has_head {
        "Staged changes (HEAD -> index)"
    } else {
        "Staged changes (empty tree -> index)"
    };
    append_diff_section(&mut diff, staged_label, &staged);
    append_diff_section(
        &mut diff,
        "Unstaged changes (index -> working tree)",
        &unstaged,
    );
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .output()
        .map_err(|error| HeadlessReviewError::target(format!("Failed to run git: {error}")))?;
    if !output.status.success() {
        return Err(HeadlessReviewError::target(git_error_message(&output)));
    }

    let mut included_bytes = 0_u64;
    let mut has_untracked_section = false;
    let mut warned_total_limit = false;
    for raw_path in output
        .stdout
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
        let relative_path = untracked_relative_path(raw_path);
        let path = repo_path.join(relative_path);
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
            Err(_error)
                if fs::symlink_metadata(&path)
                    .is_ok_and(|metadata| metadata.file_type().is_symlink()) =>
            {
                warnings.push(format!("Skipped untracked symlink `{display_relative}`."));
                continue;
            }
            Err(error) => {
                return Err(HeadlessReviewError::target(format!(
                    "Failed to open untracked file `{display_relative}`: {error}"
                )));
            }
        };
        let opened_metadata = match file.metadata() {
            Ok(metadata) if metadata.is_file() => metadata,
            _ => continue,
        };
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
        #[cfg(not(unix))]
        let _ = opened_metadata;
        if included_bytes >= MAX_UNTRACKED_TOTAL_BYTES {
            if !warned_total_limit {
                warnings.push(format!(
                    "Skipped additional untracked files starting with `{display_relative}` because the total untracked-file byte limit was reached."
                ));
                warned_total_limit = true;
            }
            continue;
        }
        let remaining_bytes = MAX_UNTRACKED_TOTAL_BYTES.saturating_sub(included_bytes);
        let allowed_bytes = MAX_UNTRACKED_FILE_BYTES.min(remaining_bytes);
        let mut contents = Vec::new();
        file.take(allowed_bytes.saturating_add(1))
            .read_to_end(&mut contents)
            .map_err(|error| {
                HeadlessReviewError::target(format!(
                    "Failed to read untracked file `{display_relative}`: {error}"
                ))
            })?;
        if contents.len() as u64 > allowed_bytes {
            if allowed_bytes < MAX_UNTRACKED_FILE_BYTES {
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
        if !has_untracked_section {
            append_diff_section_header(&mut diff, "Untracked files (new files)");
            has_untracked_section = true;
        }
        append_diff(&mut diff, &new_file_patch(&relative, &text));
        included_bytes = included_bytes.saturating_add(contents.len() as u64);
    }
    Ok((diff, warnings))
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

        // FILE_FLAG_OPEN_REPARSE_POINT opens the link itself instead of its target.
        options.custom_flags(0x0020_0000);
    }
    options.open(path)
}

fn is_safe_synthetic_diff_path(relative: &str) -> bool {
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

fn append_diff_section(diff: &mut String, label: &str, patch: &str) {
    if patch.is_empty() {
        return;
    }
    append_diff_section_header(diff, label);
    append_diff(diff, patch);
}

fn append_diff_section_header(diff: &mut String, label: &str) {
    if !diff.is_empty() && !diff.ends_with('\n') {
        diff.push('\n');
    }
    if !diff.is_empty() {
        diff.push('\n');
    }
    diff.push_str("# Lachesi diff section: ");
    diff.push_str(label);
    diff.push('\n');
}

fn append_diff(diff: &mut String, patch: &str) {
    if !diff.is_empty() && !diff.ends_with('\n') && !patch.is_empty() {
        diff.push('\n');
    }
    diff.push_str(patch);
}

fn is_sensitive_untracked_path(relative: &str) -> bool {
    let normalized = relative.replace('\\', "/").to_ascii_lowercase();
    let path = Path::new(&normalized);
    let file_name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
    let extension = path.extension().and_then(OsStr::to_str).unwrap_or_default();
    let normalized_file_name = file_name.replace('-', "_");
    let normalized_stem = file_name
        .split('.')
        .next()
        .unwrap_or_default()
        .replace('-', "_");
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
    let likely_secret_text = extension.is_empty()
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
    if likely_secret_text
        && (normalized_stem.split('_').any(|part| {
            matches!(
                part,
                "secret" | "secrets" | "password" | "passwords" | "passwd"
            )
        }) || matches!(
            normalized_stem.as_str(),
            "token"
                | "tokens"
                | "credential"
                | "credentials"
                | "api_key"
                | "private_key"
                | "access_token"
                | "auth_token"
                | "refresh_token"
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
    let fence = markdown_fence(diff);
    let opening_fence = format!("{fence}diff");
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
        &opening_fence,
    ]
    .join("\n");
    payload.push('\n');
    payload.push_str(diff);
    if !diff.ends_with('\n') {
        payload.push('\n');
    }
    payload.push_str(&fence);
    payload
}

fn markdown_fence(content: &str) -> String {
    let max_run = content
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    "`".repeat(max_run.saturating_add(1).max(3))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        branch_scope_warnings, build_review_payload, format_findings_markdown, format_markdown,
        is_safe_synthetic_diff_path, is_sensitive_untracked_path, map_native_review_error,
        map_provider_target_error, markdown_fence, new_file_patch, public_provider_error,
        repo_identity_matches_target, requested_repo_identity, run,
        strip_private_evidence_payloads, untracked_relative_path, validate_explicit_repo_identity,
        validate_requested_identity_shape, working_tree_diff, HeadlessReviewExecution,
        HeadlessReviewRequest, HeadlessReviewTarget, ReviewScope, HEADLESS_REVIEW_BOUNDARY,
        MAX_UNTRACKED_FILE_BYTES,
    };
    use crate::config::{RepoRef, ReviewProvider};
    use crate::services::review::{
        AiReviewRunStatus, AiReviewTurnKind, HeadlessNativeReviewError, ReviewAnchorSide,
        ReviewEvidenceArtifact, ReviewEvidenceKind, ReviewEvidenceSource, ReviewFinding,
        ReviewFindingAnchor, ReviewFindingCategory, ReviewFindingConfidence, ReviewFindingSeverity,
        ReviewFindingSource, ReviewFindingStatus, ReviewProvider as ReviewRunProvider, ReviewRun,
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
    fn synthetic_diff_paths_reject_ambiguous_header_characters() {
        assert!(is_safe_synthetic_diff_path("src/new file.ts"));
        assert!(!is_safe_synthetic_diff_path("src/tab\tfile.ts"));
        assert!(!is_safe_synthetic_diff_path("src/quoted\"file.ts"));
        assert!(!is_safe_synthetic_diff_path("src/newline\nfile.ts"));
        assert!(!is_safe_synthetic_diff_path("src\\config.txt"));
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
    fn explicit_repo_identity_drives_discovery_for_every_scope() {
        for scope in [
            ReviewScope::WorkingTree,
            ReviewScope::Branch,
            ReviewScope::PullRequest,
        ] {
            let request = HeadlessReviewRequest {
                repo_path: None,
                scope,
                base: None,
                workspace: Some("lachesi-hq".to_string()),
                repo: Some("lachesi".to_string()),
                pr_id: None,
                provider: Some(ReviewProvider::Github),
                profile: None,
                ai_provider: None,
                model: None,
                effort: None,
                run_analyzers: false,
            };

            assert_eq!(
                requested_repo_identity(&request),
                Some(("lachesi-hq", "lachesi"))
            );
        }
    }

    #[test]
    fn explicit_repo_path_must_match_requested_identity() {
        let repo = temp_git_repo();
        git(
            &repo,
            &[
                "remote",
                "add",
                "origin",
                "git@github.com:lachesi-hq/lachesi.git",
            ],
        );
        let request = HeadlessReviewRequest {
            repo_path: Some(repo.clone()),
            scope: ReviewScope::Branch,
            base: Some("HEAD".to_string()),
            workspace: Some("different-workspace".to_string()),
            repo: Some("lachesi".to_string()),
            pr_id: None,
            provider: Some(ReviewProvider::Github),
            profile: None,
            ai_provider: None,
            model: None,
            effort: None,
            run_analyzers: false,
        };

        let error = validate_explicit_repo_identity(&repo, &request)
            .expect_err("mismatched explicit identity must be rejected");

        assert_eq!(error.exit_code, 4);
        assert!(error.message.contains("does not match"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn partial_requested_repo_identity_is_rejected() {
        let request = HeadlessReviewRequest {
            repo_path: None,
            scope: ReviewScope::WorkingTree,
            base: None,
            workspace: Some("lachesi-hq".to_string()),
            repo: None,
            pr_id: None,
            provider: None,
            profile: None,
            ai_provider: None,
            model: None,
            effort: None,
            run_analyzers: false,
        };

        let error = validate_requested_identity_shape(&request)
            .expect_err("partial repository identity must be rejected");

        assert_eq!(error.exit_code, 4);
        assert!(error.message.contains("provided together"));
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
    fn headless_output_omits_raw_evidence_payloads() {
        let mut run = ReviewRun {
            id: "run-1".to_string(),
            schema_version: "v0.1".to_string(),
            provider: ReviewRunProvider::Github,
            workspace: "lachesi-hq".to_string(),
            repo: "lachesi".to_string(),
            pr_id: 0,
            source_branch: "feature".to_string(),
            destination_branch: "main".to_string(),
            status: AiReviewRunStatus::Succeeded,
            turn_kind: AiReviewTurnKind::Initial,
            review_profile: None,
            created_at: "1".to_string(),
            finished_at: Some("2".to_string()),
            diff_fingerprint: "fingerprint".to_string(),
            thread_id: None,
            summary_markdown: Some("Review summary".to_string()),
            evidence: vec![ReviewEvidenceArtifact {
                id: "evidence-1".to_string(),
                kind: ReviewEvidenceKind::Analyzer,
                source: ReviewEvidenceSource::Tests,
                title: "Analyzer output".to_string(),
                summary: Some("Analyzer completed.".to_string()),
                payload: Some("TOKEN=secret-value".to_string()),
            }],
            findings: Vec::new(),
        };

        strip_private_evidence_payloads(&mut run);

        assert_eq!(run.evidence.len(), 1);
        assert_eq!(
            run.evidence[0].summary.as_deref(),
            Some("Analyzer completed.")
        );
        assert_eq!(run.evidence[0].payload, None);
        let json = serde_json::to_string(&run).expect("serialize sanitized review run");
        assert!(!json.contains("secret-value"));
    }

    #[test]
    fn markdown_includes_pr_profile_and_evidence_context() {
        let execution = HeadlessReviewExecution {
            schema_version: "lachesi.headless-review.v1".to_string(),
            status: "succeeded".to_string(),
            exit_code: 0,
            warnings: Vec::new(),
            minimum_severity: None,
            analyzers_ran: false,
            target: HeadlessReviewTarget {
                scope: "pr".to_string(),
                repo_path: "/tmp/lachesi".to_string(),
                workspace: Some("lachesi-hq".to_string()),
                repo: "lachesi".to_string(),
                pr_id: Some(42),
                source: "feature".to_string(),
                destination: "main".to_string(),
            },
            review_run: Some(ReviewRun {
                id: "run-1".to_string(),
                schema_version: "v0.1".to_string(),
                provider: ReviewRunProvider::Github,
                workspace: "lachesi-hq".to_string(),
                repo: "lachesi".to_string(),
                pr_id: 42,
                source_branch: "feature".to_string(),
                destination_branch: "main".to_string(),
                status: AiReviewRunStatus::Succeeded,
                turn_kind: AiReviewTurnKind::Initial,
                review_profile: Some("strict".to_string()),
                created_at: "1".to_string(),
                finished_at: Some("2".to_string()),
                diff_fingerprint: "fingerprint".to_string(),
                thread_id: None,
                summary_markdown: Some("Looks good.".to_string()),
                evidence: vec![ReviewEvidenceArtifact {
                    id: "evidence-1".to_string(),
                    kind: ReviewEvidenceKind::Analyzer,
                    source: ReviewEvidenceSource::Tests,
                    title: "Test suite".to_string(),
                    summary: Some("All tests passed.".to_string()),
                    payload: None,
                }],
                findings: Vec::new(),
            }),
        };

        let markdown = format_markdown(&execution);

        assert!(markdown.contains("Pull request: #42"));
        assert!(markdown.contains("Profile: strict"));
        assert!(markdown.contains("## Evidence"));
        assert!(markdown.contains("Test suite"));
        assert!(markdown.contains("All tests passed."));
    }

    #[test]
    fn provider_errors_expose_only_stable_public_messages() {
        let message = public_provider_error(
            "codex exited with code 1.\nstderr: TOKEN=secret-value\nstdout: private output",
        );

        assert_eq!(message, "AI provider review failed.");
        assert!(!message.contains("secret-value"));
        assert_eq!(
            public_provider_error("failed to parse structured review JSON"),
            "AI provider returned invalid review output."
        );
    }

    #[test]
    fn analyzer_opt_in_rejects_an_empty_review_target() {
        let repo = temp_git_repo();
        let error = run(HeadlessReviewRequest {
            repo_path: Some(repo.clone()),
            scope: ReviewScope::WorkingTree,
            base: None,
            workspace: None,
            repo: None,
            pr_id: None,
            provider: None,
            profile: None,
            ai_provider: None,
            model: None,
            effort: None,
            run_analyzers: true,
        })
        .expect_err("analyzer opt-in must not silently pass without a review target");

        assert_eq!(error.exit_code, 4);
        assert!(error.message.contains("analyzers were not run"));
        let _ = fs::remove_dir_all(repo);
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
    fn headless_payload_boundary_forbids_filesystem_inspection() {
        let payload = build_review_payload(
            "Review carefully.",
            "Local changes",
            "feature",
            "HEAD",
            "+change\n",
            ReviewScope::WorkingTree,
        );
        let payload = format!("{payload}\n\n{HEADLESS_REVIEW_BOUNDARY}");

        assert!(payload.contains("Do not inspect the filesystem or run commands."));
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
    fn review_payload_uses_a_fence_longer_than_diff_backticks() {
        let diff = "+const example = \"```\";\n";
        let payload = build_review_payload(
            "Review carefully.",
            "Fence changes",
            "feature",
            "main",
            diff,
            ReviewScope::Branch,
        );

        assert_eq!(markdown_fence(diff), "````");
        assert!(payload.contains(&format!("````diff\n{diff}````")));
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
        assert!(diff.contains("# Lachesi diff section: Staged changes (HEAD -> index)"));
        assert!(diff.contains("# Lachesi diff section: Unstaged changes (index -> working tree)"));
        assert!(diff.contains("# Lachesi diff section: Untracked files (new files)"));
        assert!(diff.contains("+after staged"));
        assert!(diff.contains("+after unstaged"));
        assert!(diff.contains("diff --git a/untracked.txt b/untracked.txt"));
        assert!(diff.contains("+new untracked"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn branch_scope_warns_when_local_changes_are_excluded() {
        let repo = temp_git_repo();
        assert!(branch_scope_warnings(&repo)
            .expect("clean warnings")
            .is_empty());
        fs::write(repo.join("unstaged.txt"), "after unstaged\n")
            .expect("write local branch change");

        let warnings = branch_scope_warnings(&repo).expect("dirty warnings");

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("committed changes only"));
        assert!(warnings[0].contains("--scope working-tree"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn working_tree_diff_keeps_staged_change_cancelled_by_worktree() {
        let repo = temp_git_repo();
        fs::write(repo.join("staged.txt"), "after staged\n").expect("update staged file");
        git(&repo, &["add", "staged.txt"]);
        fs::write(repo.join("staged.txt"), "before staged\n").expect("restore worktree content");

        let (diff, warnings) = working_tree_diff(&repo).expect("collect working tree diff");

        assert!(warnings.is_empty());
        assert!(diff.contains("# Lachesi diff section: Staged changes (HEAD -> index)"));
        assert!(diff.contains("# Lachesi diff section: Unstaged changes (index -> working tree)"));
        assert_eq!(
            diff.matches("diff --git a/staged.txt b/staged.txt").count(),
            2
        );
        assert!(diff.contains("+after staged"));
        assert!(diff.contains("-after staged"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn working_tree_diff_skips_sensitive_untracked_files() {
        let repo = temp_git_repo();
        fs::write(repo.join(".env.local"), "API_TOKEN=must-not-leak\n")
            .expect("write sensitive untracked file");
        fs::write(repo.join("notes.txt"), "review this\n").expect("write safe untracked file");
        fs::write(repo.join("invalid.txt"), [0xff, 0xfe, 0xfd])
            .expect("write invalid UTF-8 fixture");

        let (diff, warnings) = working_tree_diff(&repo).expect("collect working tree diff");

        assert!(!diff.contains("must-not-leak"));
        assert!(diff.contains("+review this"));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains(".env.local")));
        assert!(warnings.iter().any(|warning| warning.contains("non-UTF-8")));
        assert!(!diff.contains("invalid.txt"));
        assert!(is_sensitive_untracked_path("config/client-secrets.json"));
        assert!(is_sensitive_untracked_path("certs/signing.key"));
        assert!(is_sensitive_untracked_path(".envrc"));
        assert!(is_sensitive_untracked_path(".envrc.local"));
        assert!(is_sensitive_untracked_path("config/prod.env"));
        assert!(is_sensitive_untracked_path("config/prod.env.local"));
        assert!(is_sensitive_untracked_path("config/env.production"));
        assert!(is_sensitive_untracked_path("config/service_account.json"));
        assert!(is_sensitive_untracked_path("config/client_secret.json"));
        assert!(is_sensitive_untracked_path(".kube/config"));
        assert!(is_sensitive_untracked_path(".docker/config.json"));
        assert!(is_sensitive_untracked_path(".git-credentials"));
        assert!(is_sensitive_untracked_path("terraform.tfvars"));
        assert!(is_sensitive_untracked_path("production.auto.tfvars"));
        assert!(is_sensitive_untracked_path(
            ".config/gcloud/application_default_credentials.json"
        ));
        assert!(is_sensitive_untracked_path(".azure/accessTokens.json"));
        assert!(is_sensitive_untracked_path("cache/accessTokens.json"));
        assert!(is_sensitive_untracked_path("secrets.txt"));
        assert!(is_sensitive_untracked_path("passwords.md"));
        assert!(is_sensitive_untracked_path("token.txt"));
        assert!(is_sensitive_untracked_path("client-secrets.ini"));
        assert!(is_sensitive_untracked_path(".npmrc.local"));
        assert!(!is_sensitive_untracked_path("src/token.rs"));
        assert!(!is_sensitive_untracked_path("src/credentials.rs"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn working_tree_diff_supports_repositories_without_commits() {
        let nonce = TEMP_REPO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let repo = std::env::temp_dir().join(format!(
            "lachesi-headless-unborn-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&repo).expect("create unborn repo");
        git(&repo, &["init"]);
        fs::write(repo.join("staged.txt"), "staged content\n").expect("write staged fixture");
        git(&repo, &["add", "staged.txt"]);
        fs::write(repo.join("untracked.txt"), "untracked content\n")
            .expect("write untracked fixture");

        let (diff, warnings) = working_tree_diff(&repo).expect("collect unborn repository diff");

        assert!(diff.contains("+staged content"));
        assert!(diff.contains("+untracked content"));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("no commits")));
        let _ = fs::remove_dir_all(repo);
    }

    #[cfg(unix)]
    #[test]
    fn untracked_relative_path_preserves_non_utf8_bytes() {
        use std::os::unix::ffi::OsStrExt;

        let raw_path = [b'n', b'o', b't', b'e', 0xff];
        let path = untracked_relative_path(&raw_path);

        assert_eq!(path.as_os_str().as_bytes(), raw_path);
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

    #[cfg(unix)]
    #[test]
    fn working_tree_diff_skips_untracked_hard_links() {
        let repo = temp_git_repo();
        let outside_dir = tempfile::tempdir().expect("outside temp dir");
        let outside_file = outside_dir.path().join("outside.txt");
        fs::write(&outside_file, "outside secret\n").expect("write outside fixture");
        fs::hard_link(&outside_file, repo.join("notes.txt")).expect("create hard link");

        let (diff, warnings) = working_tree_diff(&repo).expect("collect working tree diff");

        assert!(!diff.contains("outside secret"));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("multiple hard links")));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn working_tree_diff_warns_when_total_untracked_budget_is_reached() {
        let repo = temp_git_repo();
        let contents = vec![b'a'; MAX_UNTRACKED_FILE_BYTES as usize];
        for index in 0..5 {
            fs::write(repo.join(format!("large-{index}.txt")), &contents)
                .expect("write large untracked fixture");
        }

        let (diff, warnings) = working_tree_diff(&repo).expect("collect working tree diff");

        assert!(diff.contains("large-0.txt"));
        assert!(!diff.contains("large-4.txt"));
        assert_eq!(
            warnings
                .iter()
                .filter(|warning| warning.contains("total untracked-file byte limit"))
                .count(),
            1
        );
        let _ = fs::remove_dir_all(repo);
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
