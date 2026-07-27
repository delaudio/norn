use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::config::{self, AiProvider, ReviewProvider as ConfigReviewProvider};
use crate::local_repo::resolve_local_repo;
use crate::repo_config;
use crate::review_storage;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LegacySavedReview {
    pub content: String,
    /// Milliseconds since Unix epoch, stored as a string.
    pub generated_at: String,
}

type SavedReview = LegacySavedReview;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AiReviewTurnKind {
    #[default]
    Initial,
    Reply,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AiReviewMessageRole {
    User,
    Assistant,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewMessage {
    pub id: String,
    pub role: AiReviewMessageRole,
    pub content: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewThread {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub claude_session_id: Option<String>,
    pub messages: Vec<AiReviewMessage>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewStoreData {
    pub active_thread_id: Option<String>,
    pub threads: Vec<AiReviewThread>,
    #[serde(default)]
    pub review_runs: Vec<ReviewRun>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewProvider {
    Bitbucket,
    Github,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewFindingSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewFindingConfidence {
    Low,
    Medium,
    High,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewFindingCategory {
    Bug,
    Security,
    Performance,
    Architecture,
    Typing,
    Test,
    Maintainability,
    Docs,
    Other,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewFindingStatus {
    New,
    Dismissed,
    Accepted,
    Published,
    Fixed,
    Stale,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewFindingSource {
    Llm,
    Analyzer,
    Merged,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewEvidenceKind {
    Conversation,
    Diff,
    Analyzer,
    Doc,
    Manual,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewEvidenceSource {
    Claude,
    Codex,
    BitbucketDiff,
    Jira,
    Notion,
    Tsc,
    Biome,
    Tests,
    Semgrep,
    Other,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewAnchorSide {
    New,
    Old,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewPublicationMode {
    Inline,
    File,
    General,
    LocalOnly,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFindingAnchor {
    pub path: String,
    pub start_line: u32,
    pub end_line: Option<u32>,
    pub side: ReviewAnchorSide,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFindingPublication {
    pub mode: ReviewPublicationMode,
    #[serde(default)]
    pub draft_ids: Vec<String>,
    #[serde(default)]
    pub remote_comment_ids: Vec<u64>,
    pub published_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewEvidenceArtifact {
    pub id: String,
    pub kind: ReviewEvidenceKind,
    pub source: ReviewEvidenceSource,
    pub title: String,
    pub summary: Option<String>,
    pub payload: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFinding {
    pub id: String,
    pub fingerprint: String,
    pub title: String,
    pub severity: ReviewFindingSeverity,
    pub confidence: ReviewFindingConfidence,
    pub category: ReviewFindingCategory,
    pub status: ReviewFindingStatus,
    pub summary: String,
    pub rationale: Option<String>,
    pub rule_id: Option<String>,
    pub source: ReviewFindingSource,
    pub anchor: Option<ReviewFindingAnchor>,
    pub suggested_fix: Option<String>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    pub publication: Option<ReviewFindingPublication>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewFindingPublicationEventKind {
    StageDraft,
    RemoveDraft,
    PublishDraft,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFindingPublicationEvent {
    pub kind: ReviewFindingPublicationEventKind,
    pub review_run_id: String,
    pub finding_fingerprint: String,
    pub mode: ReviewPublicationMode,
    pub draft_id: Option<String>,
    pub remote_comment_id: Option<u64>,
    pub published_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRun {
    pub id: String,
    pub schema_version: String,
    pub provider: ReviewProvider,
    pub workspace: String,
    pub repo: String,
    pub pr_id: u32,
    pub source_branch: String,
    pub destination_branch: String,
    pub status: AiReviewRunStatus,
    pub turn_kind: AiReviewTurnKind,
    #[serde(default)]
    pub review_profile: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub diff_fingerprint: String,
    pub thread_id: Option<String>,
    pub summary_markdown: Option<String>,
    #[serde(default)]
    pub evidence: Vec<ReviewEvidenceArtifact>,
    #[serde(default)]
    pub findings: Vec<ReviewFinding>,
}

#[derive(Debug, Clone)]
pub struct HeadlessNativeReviewRequest {
    pub repo_path: PathBuf,
    pub review_provider: ReviewProvider,
    pub workspace: String,
    pub repo: String,
    pub pr_id: u32,
    pub title: String,
    pub source_branch: String,
    pub destination_branch: String,
    pub payload: String,
    pub ai_provider: AiProvider,
    pub claude_model: Option<String>,
    pub claude_effort: Option<String>,
    pub codex_model: Option<String>,
    pub codex_effort: Option<String>,
    pub review_profile: Option<String>,
    pub run_analyzers: bool,
}

#[derive(Debug)]
pub enum HeadlessNativeReviewError {
    Analyzer(String),
    Provider(String),
    Internal(String),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewPipelineFailureKind {
    Analyzer,
    Provider,
    Internal,
}

#[derive(Debug)]
struct ReviewPipelineFailure {
    kind: ReviewPipelineFailureKind,
    message: String,
}

impl ReviewPipelineFailure {
    fn analyzer(message: impl Into<String>) -> Self {
        Self {
            kind: ReviewPipelineFailureKind::Analyzer,
            message: message.into(),
        }
    }

    fn provider(message: impl Into<String>) -> Self {
        Self {
            kind: ReviewPipelineFailureKind::Provider,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: ReviewPipelineFailureKind::Internal,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ReviewPipelineFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AiReviewRunStatus {
    #[default]
    Idle,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewRunState {
    pub pr_key: String,
    pub pr_title: Option<String>,
    pub thread_id: Option<String>,
    pub turn_kind: Option<AiReviewTurnKind>,
    pub status: AiReviewRunStatus,
    pub logs: Vec<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub generated_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewDraftCommentSuggestion {
    pub path: String,
    pub to: Option<u32>,
    pub from: Option<u32>,
    pub raw: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AiReviewFixStatus {
    #[default]
    Idle,
    Running,
    Succeeded,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AiReviewFixPhase {
    #[default]
    Idle,
    Preflight,
    Stashing,
    SwitchingBranch,
    Syncing,
    MergingDestination,
    RestoringStash,
    ResolvingConflicts,
    RunningClaude,
    VerifyingChanges,
    ReadyToCommit,
    Committing,
    ReadyToPush,
    Pushing,
    Completed,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewFixState {
    pub pr_key: String,
    pub thread_id: Option<String>,
    pub repo_path: Option<String>,
    pub status: AiReviewFixStatus,
    pub phase: AiReviewFixPhase,
    pub logs: Vec<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub suggested_commit_message: Option<String>,
    pub summary: Option<String>,
    pub commit_sha: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub files_touched: Vec<String>,
    #[serde(default)]
    pub tests: Vec<String>,
    pub claude_duration_ms: Option<u64>,
    pub claude_session_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BranchSyncStatus {
    Success,
    Conflict,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BranchSyncResult {
    pub status: BranchSyncStatus,
    pub repo_path: String,
    pub source_branch: String,
    pub destination_branch: String,
    pub summary: String,
    pub sync_commit_sha: Option<String>,
    pub warning: Option<String>,
    #[serde(default)]
    pub conflict_files: Vec<String>,
    #[serde(default)]
    pub logs: Vec<String>,
}

#[derive(Clone, Default)]
struct ReviewRunSessionRecord {
    public: AiReviewRunState,
    run_id: u64,
    child_pid: Option<u32>,
    cancel_requested: bool,
}

#[derive(Default)]
struct AiReviewRunStoreInner {
    sessions: HashMap<String, ReviewRunSessionRecord>,
    next_run_id: u64,
}

#[derive(Clone, Default)]
pub struct AiReviewRunStore(Arc<Mutex<AiReviewRunStoreInner>>);

#[derive(Clone, Default)]
struct FixSessionRecord {
    public: AiReviewFixState,
    source_branch: String,
    destination_branch: String,
    baseline_files: Vec<String>,
}

#[derive(Default)]
struct AiReviewFixStoreInner {
    sessions: HashMap<String, FixSessionRecord>,
    active_key: Option<String>,
}

#[derive(Clone, Default)]
pub struct AiReviewFixStore(Arc<Mutex<AiReviewFixStoreInner>>);

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ClaudeFixResult {
    status: String,
    summary: Option<String>,
    commit_message: Option<String>,
    #[serde(default)]
    tests: Vec<String>,
    #[serde(default)]
    files_touched: Vec<String>,
    failure_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AiReviewDraftCommentResult {
    #[serde(default)]
    comments: Vec<AiReviewDraftCommentSuggestion>,
}

#[derive(Deserialize, Debug)]
struct ClaudeCliEnvelope {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    response_type: Option<String>,
    #[allow(dead_code)]
    subtype: Option<String>,
    #[allow(dead_code)]
    is_error: Option<bool>,
    duration_ms: Option<u64>,
    session_id: Option<String>,
    #[allow(dead_code)]
    result: Option<String>,
    structured_output: Option<ClaudeFixResult>,
    #[serde(default)]
    permission_denials: Vec<serde_json::Value>,
}

#[derive(Debug)]
struct ParsedClaudeFixResponse {
    result: ClaudeFixResult,
    duration_ms: Option<u64>,
    session_id: Option<String>,
    permission_denials: usize,
}

#[derive(Debug)]
struct ParsedClaudeTextResponse {
    content: String,
    duration_ms: Option<u64>,
    session_id: Option<String>,
    permission_denials: usize,
}

#[derive(Debug)]
struct CommandOutput {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalyzerStatus {
    Passed,
    Failed,
    Skipped,
    TimedOut,
    Errored,
}

#[derive(Debug, Clone)]
struct AnalyzerSpec {
    id: String,
    title: String,
    command: String,
    timeout_seconds: u64,
    source: ReviewEvidenceSource,
    required: bool,
}

#[derive(Debug, Clone)]
struct AnalyzerRunResult {
    spec: AnalyzerSpec,
    status: AnalyzerStatus,
    code: Option<i32>,
    duration_ms: u64,
    stdout: String,
    stderr: String,
    error: Option<String>,
}

/// Single-quote a string for safe interpolation into a shell command.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn now_ms() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn human_duration(duration_ms: u64) -> String {
    let total_seconds = duration_ms / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    if minutes == 0 {
        return format!("{seconds}s");
    }
    format!("{minutes}m {seconds:02}s")
}

fn pr_key(workspace: &str, repo: &str, id: u32) -> String {
    format!("{workspace}/{repo}/{id}")
}

fn fix_key(workspace: &str, repo: &str, id: u32, thread_id: Option<&str>) -> String {
    match thread_id.filter(|value| !value.trim().is_empty()) {
        Some(thread_id) => format!("{}/{thread_id}", pr_key(workspace, repo, id)),
        None => format!("{}/default", pr_key(workspace, repo, id)),
    }
}

fn now_id(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{nanos}")
}

fn review_thread_title() -> String {
    "AI review".to_string()
}

fn legacy_review_to_store(review: LegacySavedReview) -> AiReviewStoreData {
    let thread_id = now_id("thread");
    let message_id = now_id("msg");
    AiReviewStoreData {
        active_thread_id: Some(thread_id.clone()),
        threads: vec![AiReviewThread {
            id: thread_id,
            title: review_thread_title(),
            created_at: review.generated_at.clone(),
            updated_at: review.generated_at.clone(),
            claude_session_id: None,
            messages: vec![AiReviewMessage {
                id: message_id,
                role: AiReviewMessageRole::Assistant,
                content: review.content,
                created_at: review.generated_at,
            }],
        }],
        review_runs: Vec::new(),
    }
}

fn normalize_review_store(store: &mut AiReviewStoreData) {
    if store.threads.is_empty() {
        store.active_thread_id = None;
        return;
    }
    if let Some(active) = store.active_thread_id.as_deref() {
        if store.threads.iter().any(|thread| thread.id == active) {
            return;
        }
    }
    store.active_thread_id = store.threads.last().map(|thread| thread.id.clone());
}

fn load_review_store(
    workspace: &str,
    repo: &str,
    id: u32,
) -> Result<Option<AiReviewStoreData>, String> {
    let Some(json) = review_storage::load_review_json(workspace, repo, id)? else {
        return Ok(None);
    };
    if let Ok(mut store) = serde_json::from_str::<AiReviewStoreData>(&json) {
        normalize_review_store(&mut store);
        return Ok(Some(store));
    }
    let legacy = serde_json::from_str::<LegacySavedReview>(&json).map_err(|e| e.to_string())?;
    Ok(Some(legacy_review_to_store(legacy)))
}

fn save_review_store(
    workspace: &str,
    repo: &str,
    id: u32,
    store: &AiReviewStoreData,
) -> Result<(), String> {
    let json = serde_json::to_string(store).map_err(|e| e.to_string())?;
    review_storage::save_review_json(workspace, repo, id, &json)
}

fn analyzer_source(id: &str) -> ReviewEvidenceSource {
    let normalized = id.trim().to_ascii_lowercase();
    if normalized.contains("tsc") || normalized.contains("type") {
        ReviewEvidenceSource::Tsc
    } else if normalized.contains("biome") || normalized.contains("lint") {
        ReviewEvidenceSource::Biome
    } else if normalized.contains("test")
        || normalized.contains("vitest")
        || normalized.contains("jest")
    {
        ReviewEvidenceSource::Tests
    } else if normalized.contains("semgrep") || normalized.contains("opengrep") {
        ReviewEvidenceSource::Semgrep
    } else {
        ReviewEvidenceSource::Other
    }
}

fn analyzer_status_label(status: AnalyzerStatus) -> &'static str {
    match status {
        AnalyzerStatus::Passed => "passed",
        AnalyzerStatus::Failed => "failed",
        AnalyzerStatus::Skipped => "skipped",
        AnalyzerStatus::TimedOut => "timed out",
        AnalyzerStatus::Errored => "errored",
    }
}

fn package_manager(repo_path: &Path) -> &'static str {
    if repo_path.join("pnpm-lock.yaml").is_file() {
        "pnpm"
    } else if repo_path.join("yarn.lock").is_file() {
        "yarn"
    } else {
        "npm"
    }
}

fn package_script_exists(repo_path: &Path, script: &str) -> bool {
    let Ok(contents) = fs::read_to_string(repo_path.join("package.json")) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return false;
    };
    value
        .get("scripts")
        .and_then(|scripts| scripts.get(script))
        .and_then(|value| value.as_str())
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn command_available(repo_path: &Path, binary: &str) -> bool {
    Command::new("/bin/sh")
        .arg("-lc")
        .arg(format!("command -v {}", shell_quote(binary)))
        .current_dir(repo_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn default_analyzer_specs(repo_path: &Path) -> Vec<AnalyzerSpec> {
    let manager = package_manager(repo_path);
    let mut specs = Vec::new();
    let script_specs = [
        (
            "typecheck",
            "TypeScript typecheck",
            ReviewEvidenceSource::Tsc,
        ),
        ("lint", "Lint", ReviewEvidenceSource::Biome),
        ("test", "Tests", ReviewEvidenceSource::Tests),
    ];

    for (script, title, source) in script_specs {
        if package_script_exists(repo_path, script) {
            let command = if script == "test" {
                format!("CI=1 {manager} run {script}")
            } else {
                format!("{manager} run {script}")
            };
            specs.push(AnalyzerSpec {
                id: script.to_string(),
                title: title.to_string(),
                command,
                timeout_seconds: 120,
                source,
                required: false,
            });
        }
    }

    if matches!(manager, "pnpm" | "npm")
        && (repo_path.join("pnpm-lock.yaml").is_file()
            || repo_path.join("package-lock.json").is_file())
    {
        specs.push(AnalyzerSpec {
            id: "dependency-audit".to_string(),
            title: "Dependency audit".to_string(),
            command: format!("{manager} audit --audit-level moderate"),
            timeout_seconds: 120,
            source: ReviewEvidenceSource::Other,
            required: false,
        });
    }

    if command_available(repo_path, "semgrep") {
        specs.push(AnalyzerSpec {
            id: "semgrep".to_string(),
            title: "Semgrep".to_string(),
            command: "semgrep --config auto --error --quiet .".to_string(),
            timeout_seconds: 120,
            source: ReviewEvidenceSource::Semgrep,
            required: false,
        });
    } else if command_available(repo_path, "opengrep") {
        specs.push(AnalyzerSpec {
            id: "opengrep".to_string(),
            title: "OpenGrep".to_string(),
            command: "opengrep --config auto --error --quiet .".to_string(),
            timeout_seconds: 120,
            source: ReviewEvidenceSource::Semgrep,
            required: false,
        });
    }

    specs
}

fn configured_analyzer_specs_with_profile(
    repo_path: &Path,
    review_profile: Option<&str>,
) -> Result<Vec<AnalyzerSpec>, String> {
    let config = repo_config::load_from_repo_path_with_profile(repo_path, review_profile)?;
    if !config.errors.is_empty() {
        return Err(config
            .errors
            .into_iter()
            .map(|error| format!("{}: {}", error.path, error.message))
            .collect::<Vec<_>>()
            .join("\n"));
    }

    let mut specs = Vec::new();
    if let Some(config) = config.config {
        for (id, analyzer) in config.analyzers {
            if !analyzer.enabled {
                continue;
            }
            let command = analyzer.command.unwrap_or_default().trim().to_string();
            if command.is_empty() {
                continue;
            }
            specs.push(AnalyzerSpec {
                title: id.replace(['_', '-'], " "),
                source: analyzer_source(&id),
                id,
                command,
                timeout_seconds: analyzer.timeout_seconds.unwrap_or(120).clamp(1, 900),
                required: analyzer.required,
            });
        }
    }

    Ok(specs)
}

fn analyzer_specs_with_profile(
    repo_path: &Path,
    review_profile: Option<&str>,
) -> Result<Vec<AnalyzerSpec>, String> {
    let configured = configured_analyzer_specs_with_profile(repo_path, review_profile)?;
    if configured.is_empty() {
        Ok(default_analyzer_specs(repo_path))
    } else {
        Ok(configured)
    }
}

fn run_analyzer_command(
    repo_path: &Path,
    spec: &AnalyzerSpec,
    store: &AiReviewRunStore,
    key: &str,
    run_id: u64,
) -> AnalyzerRunResult {
    let started = Instant::now();
    let mut command = Command::new("/bin/sh");
    command
        .arg("-lc")
        .arg(&spec.command)
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return AnalyzerRunResult {
                spec: spec.clone(),
                status: AnalyzerStatus::Errored,
                code: None,
                duration_ms: started.elapsed().as_millis() as u64,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("Failed to start analyzer: {error}")),
            };
        }
    };

    let pid = child.id();
    set_inline_review_pid(store, key, run_id, pid);
    let timeout = Duration::from_secs(spec.timeout_seconds);
    loop {
        if inline_review_cancel_requested(store, key, run_id) {
            let _ = kill_process(pid);
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = kill_process(pid);
                let output = child.wait_with_output().ok();
                clear_inline_review_pid(store, key, run_id);
                return AnalyzerRunResult {
                    spec: spec.clone(),
                    status: AnalyzerStatus::TimedOut,
                    code: output.as_ref().and_then(|output| output.status.code()),
                    duration_ms: started.elapsed().as_millis() as u64,
                    stdout: output
                        .as_ref()
                        .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
                        .unwrap_or_default(),
                    stderr: output
                        .as_ref()
                        .map(|output| String::from_utf8_lossy(&output.stderr).to_string())
                        .unwrap_or_default(),
                    error: Some(format!(
                        "Timed out after {}.",
                        human_duration(timeout.as_millis() as u64)
                    )),
                };
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(error) => {
                clear_inline_review_pid(store, key, run_id);
                return AnalyzerRunResult {
                    spec: spec.clone(),
                    status: AnalyzerStatus::Errored,
                    code: None,
                    duration_ms: started.elapsed().as_millis() as u64,
                    stdout: String::new(),
                    stderr: String::new(),
                    error: Some(format!("Failed while waiting for analyzer: {error}")),
                };
            }
        }
    }

    let output = child.wait_with_output().ok();
    clear_inline_review_pid(store, key, run_id);
    let code = output.as_ref().and_then(|output| output.status.code());
    let stdout = output
        .as_ref()
        .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
        .unwrap_or_default();
    let stderr = output
        .as_ref()
        .map(|output| String::from_utf8_lossy(&output.stderr).to_string())
        .unwrap_or_default();
    let status = if code == Some(0) {
        AnalyzerStatus::Passed
    } else if code == Some(127)
        || stderr.contains("command not found")
        || stdout.contains("command not found")
    {
        AnalyzerStatus::Skipped
    } else {
        AnalyzerStatus::Failed
    };

    AnalyzerRunResult {
        spec: spec.clone(),
        status,
        code,
        duration_ms: started.elapsed().as_millis() as u64,
        stdout,
        stderr,
        error: None,
    }
}

fn trim_evidence_output(value: &str) -> String {
    const MAX_BYTES: usize = 16_000;
    if value.len() <= MAX_BYTES {
        return value.trim().to_string();
    }
    let mut start = value.len().saturating_sub(MAX_BYTES);
    while start < value.len() && !value.is_char_boundary(start) {
        start += 1;
    }
    format!(
        "[truncated to last {MAX_BYTES} bytes]\n{}",
        value[start..].trim()
    )
}

fn analyzer_payload(result: &AnalyzerRunResult) -> String {
    serde_json::json!({
        "id": result.spec.id,
        "command": result.spec.command,
        "status": analyzer_status_label(result.status),
        "exitCode": result.code,
        "durationMs": result.duration_ms,
        "stdout": trim_evidence_output(&result.stdout),
        "stderr": trim_evidence_output(&result.stderr),
        "error": result.error,
    })
    .to_string()
}

fn analyzer_evidence(
    run_id_prefix: &str,
    results: &[AnalyzerRunResult],
) -> Vec<ReviewEvidenceArtifact> {
    results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            let status = analyzer_status_label(result.status);
            let summary = match result.status {
                AnalyzerStatus::Passed => {
                    format!("{status} in {}.", human_duration(result.duration_ms))
                }
                AnalyzerStatus::Skipped => result
                    .error
                    .clone()
                    .unwrap_or_else(|| format!("{status}: command unavailable or not configured.")),
                AnalyzerStatus::TimedOut | AnalyzerStatus::Errored => {
                    result.error.clone().unwrap_or_else(|| {
                        format!("{status} after {}.", human_duration(result.duration_ms))
                    })
                }
                AnalyzerStatus::Failed => format!(
                    "{status} with exit code {:?} in {}.",
                    result.code,
                    human_duration(result.duration_ms)
                ),
            };
            ReviewEvidenceArtifact {
                id: format!("{run_id_prefix}-evidence-analyzer-{}", index + 1),
                kind: ReviewEvidenceKind::Analyzer,
                source: result.spec.source,
                title: result.spec.title.clone(),
                summary: Some(summary),
                payload: Some(analyzer_payload(result)),
            }
        })
        .collect()
}

fn analyzer_prompt_section(results: &[AnalyzerRunResult]) -> String {
    if results.is_empty() {
        return String::new();
    }
    let mut lines = vec![
        "## Local evidence".to_string(),
        "Use these deterministic local analyzer results as evidence. Prefer them over guesses. If an analyzer failed, inspect the output before deciding whether it represents a real PR issue.".to_string(),
    ];
    for result in results {
        let status = analyzer_status_label(result.status);
        lines.push(format!(
            "- {}: {} (command: `{}`, exit: {:?}, duration: {})",
            result.spec.title,
            status,
            result.spec.command,
            result.code,
            human_duration(result.duration_ms)
        ));
        let stderr = trim_evidence_output(&result.stderr);
        if !stderr.is_empty() {
            lines.push(format!(
                "  stderr: {}",
                stderr.lines().take(8).collect::<Vec<_>>().join(" | ")
            ));
        }
        let stdout = trim_evidence_output(&result.stdout);
        if !stdout.is_empty() {
            lines.push(format!(
                "  stdout: {}",
                stdout.lines().take(8).collect::<Vec<_>>().join(" | ")
            ));
        }
        if let Some(error) = result.error.as_deref() {
            lines.push(format!("  error: {error}"));
        }
    }
    lines.join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedReviewResource {
    title: String,
    url: String,
    summary: Option<String>,
}

const STRUCTURED_REVIEW_SCHEMA_VERSION: &str = "lachesi.review.v1";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StructuredReviewOutput {
    schema_version: String,
    #[serde(default)]
    findings: Vec<StructuredReviewFinding>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StructuredReviewFinding {
    title: Option<String>,
    body: Option<String>,
    summary: Option<String>,
    severity: String,
    category: Option<String>,
    confidence: Option<String>,
    file: Option<String>,
    path: Option<String>,
    line: Option<u32>,
    start_line: Option<u32>,
    end_line: Option<u32>,
    suggested_fix: Option<String>,
    rationale: Option<String>,
    rule_id: Option<String>,
}

fn stable_hash_hex(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn normalize_inline_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clean_heading_text(line: &str) -> String {
    line.trim()
        .trim_start_matches('#')
        .trim()
        .trim_matches('*')
        .trim()
        .to_string()
}

fn is_resources_heading(line: &str) -> bool {
    clean_heading_text(line).eq_ignore_ascii_case("resources")
}

fn parse_review_resources(markdown: &str) -> (String, Vec<ParsedReviewResource>) {
    let mut body_lines = Vec::new();
    let mut resource_lines = Vec::new();
    let mut in_resources = false;

    for line in markdown.lines() {
        if !in_resources && is_resources_heading(line) {
            in_resources = true;
            continue;
        }
        if in_resources {
            resource_lines.push(line.trim().to_string());
        } else {
            body_lines.push(line.to_string());
        }
    }

    let mut resources = Vec::new();
    for line in resource_lines {
        let trimmed = line.trim();
        if !(trimmed.starts_with("- [") || trimmed.starts_with("* [")) {
            continue;
        }
        let Some(link_sep) = trimmed.find("](") else {
            continue;
        };
        let Some(title_start) = trimmed.find('[') else {
            continue;
        };
        let title = trimmed[title_start + 1..link_sep].trim();
        if title.is_empty() {
            continue;
        }
        let after_link_sep = &trimmed[link_sep + 2..];
        let Some(url_end) = after_link_sep.find(')') else {
            continue;
        };
        let url = after_link_sep[..url_end].trim();
        if url.is_empty() {
            continue;
        }
        let trailing = after_link_sep[url_end + 1..]
            .trim()
            .trim_start_matches(['—', '-'])
            .trim();
        resources.push(ParsedReviewResource {
            title: title.to_string(),
            url: url.to_string(),
            summary: (!trailing.is_empty()).then(|| trailing.to_string()),
        });
    }

    (body_lines.join("\n").trim().to_string(), resources)
}

fn strip_structured_review_json_block(markdown: &str) -> String {
    let mut output = Vec::new();
    let mut pending_fence = Vec::new();
    let mut in_json = false;
    let mut block = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if in_json {
            if trimmed.starts_with("```") {
                let candidate = block.join("\n");
                if !candidate.contains(STRUCTURED_REVIEW_SCHEMA_VERSION) {
                    output.append(&mut pending_fence);
                    output.append(&mut block);
                    output.push(line.to_string());
                }
                pending_fence.clear();
                block.clear();
                in_json = false;
                continue;
            }
            block.push(line.to_string());
            continue;
        }

        if trimmed.starts_with("```json") {
            pending_fence.push(line.to_string());
            in_json = true;
            continue;
        }

        output.push(line.to_string());
    }

    if in_json {
        output.append(&mut pending_fence);
        output.append(&mut block);
    }

    output.join("\n").trim().to_string()
}

fn strip_list_marker(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return Some(rest.trim_start());
    }
    if let Some(rest) = trimmed.strip_prefix("* ") {
        return Some(rest.trim_start());
    }

    let digit_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count == 0 {
        return None;
    }
    let marker = trimmed.as_bytes().get(digit_count).copied()?;
    if marker != b'.' && marker != b')' {
        return None;
    }
    let rest = trimmed.get(digit_count + 1..)?.trim_start();
    (!rest.is_empty()).then_some(rest)
}

fn finding_line_payload(line: &str) -> Option<&str> {
    if let Some(payload) = strip_list_marker(line) {
        return Some(payload);
    }

    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("**[critical]**")
        || lower.starts_with("**[major]**")
        || lower.starts_with("**[minor]**")
        || lower.starts_with("**[nit]**")
        || lower.starts_with("[critical]")
        || lower.starts_with("[major]")
        || lower.starts_with("[minor]")
        || lower.starts_with("[nit]")
    {
        return Some(trimmed);
    }

    for prefix in [
        "bug:",
        "risk:",
        "issue:",
        "warning:",
        "question:",
        "nit:",
        "security:",
        "performance:",
        "typing:",
        "test:",
        "maintainability:",
        "docs:",
    ] {
        if lower.starts_with(prefix) {
            return Some(trimmed);
        }
    }
    None
}

fn heading_has_finding_signal(line: &str) -> bool {
    let lower = clean_heading_text(line).to_ascii_lowercase();
    [
        "risk",
        "bug",
        "issue",
        "question",
        "nit",
        "security",
        "performance",
        "typing",
        "test",
        "maintainability",
        "docs",
        "suggestion",
        "finding",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

fn finding_has_signal(summary: &str, heading: Option<&str>) -> bool {
    if heading.is_some_and(heading_has_finding_signal) {
        return true;
    }

    let lower = summary.to_ascii_lowercase();
    [
        "bug",
        "risk",
        "regression",
        "should",
        "must",
        "needs",
        "need to",
        "consider",
        "security",
        "performance",
        "typing",
        "test",
        "maintain",
        "docs",
        "inject",
        "critical",
        "major",
        "minor",
        "nit",
        "empty",
        "null",
        "undefined",
        "silently",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
        || summary.contains('`')
}

fn looks_like_file_path(candidate: &str) -> bool {
    if candidate.is_empty() || candidate.contains("://") {
        return false;
    }
    let candidate = candidate.trim_matches(':').trim_matches('/');
    let Some((_, extension)) = candidate.rsplit_once('.') else {
        return false;
    };
    let extension_len = extension.len();
    if extension_len == 0 || extension_len > 8 {
        return false;
    }
    extension.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn parse_line_range(value: &str) -> Option<(u32, Option<u32>)> {
    let cleaned = value.trim().trim_start_matches('L');
    if cleaned.is_empty() {
        return None;
    }
    let mut parts = cleaned.splitn(2, '-');
    let start = parts.next()?.parse::<u32>().ok()?;
    let end = parts
        .next()
        .and_then(|raw| raw.trim_start_matches('L').parse::<u32>().ok());
    Some((start, end))
}

fn extract_finding_anchor(summary: &str) -> Option<ReviewFindingAnchor> {
    for token in summary.split_whitespace() {
        let cleaned = token.trim_matches(|ch: char| {
            matches!(
                ch,
                '`' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | '"' | '\''
            )
        });
        let (path, line_range) = if let Some((path, line_range)) = cleaned.rsplit_once(':') {
            if line_range
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch == '-' || ch == 'L')
            {
                (path, Some(line_range))
            } else {
                (cleaned, None)
            }
        } else {
            (cleaned, None)
        };
        if !looks_like_file_path(path) {
            continue;
        }
        let Some((start_line, end_line)) = line_range.and_then(parse_line_range) else {
            continue;
        };
        return Some(ReviewFindingAnchor {
            path: path.trim_matches('.').to_string(),
            start_line,
            end_line,
            side: ReviewAnchorSide::New,
        });
    }
    None
}

fn derive_finding_title(summary: &str) -> String {
    let first_line = summary
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(summary)
        .trim();
    let mut title = normalize_inline_text(first_line)
        .replace("**", "")
        .replace('`', "");

    for separator in [" — ", ". "] {
        if let Some((prefix, _)) = title.split_once(separator) {
            if prefix.len() >= 12 {
                title = prefix.trim().to_string();
                break;
            }
        }
    }

    if title.len() > 140 {
        title = title
            .chars()
            .take(140)
            .collect::<String>()
            .trim()
            .to_string();
    }
    if title.is_empty() {
        "Finding".to_string()
    } else {
        title
    }
}

fn infer_finding_severity(heading: Option<&str>, summary: &str) -> ReviewFindingSeverity {
    let combined = format!("{} {}", heading.unwrap_or_default(), summary).to_ascii_lowercase();
    if combined.contains("critical") {
        return ReviewFindingSeverity::Critical;
    }
    if combined.contains("major")
        || combined.contains("high risk")
        || combined.starts_with("bug:")
        || combined.contains("bugs / high risk")
    {
        return ReviewFindingSeverity::High;
    }
    if combined.contains("minor") || combined.contains("low risk") {
        return ReviewFindingSeverity::Low;
    }
    if combined.contains("nit") || combined.contains("info") || combined.contains("note") {
        return ReviewFindingSeverity::Info;
    }
    ReviewFindingSeverity::Medium
}

fn infer_finding_category(heading: Option<&str>, summary: &str) -> ReviewFindingCategory {
    let combined = format!("{} {}", heading.unwrap_or_default(), summary).to_ascii_lowercase();
    if combined.contains("security") {
        return ReviewFindingCategory::Security;
    }
    if combined.contains("performance") {
        return ReviewFindingCategory::Performance;
    }
    if combined.contains("architect") {
        return ReviewFindingCategory::Architecture;
    }
    if combined.contains("typing") || combined.contains("typescript") || combined.contains(" type")
    {
        return ReviewFindingCategory::Typing;
    }
    if combined.contains("test") || combined.contains("regression") {
        return ReviewFindingCategory::Test;
    }
    if combined.contains("docs") || combined.contains("documentation") {
        return ReviewFindingCategory::Docs;
    }
    if combined.contains("maintain")
        || combined.contains("readability")
        || combined.contains("duplicate")
    {
        return ReviewFindingCategory::Maintainability;
    }
    if combined.contains("bug") || combined.contains("risk") || combined.contains("issue") {
        return ReviewFindingCategory::Bug;
    }
    ReviewFindingCategory::Other
}

fn severity_key(severity: ReviewFindingSeverity) -> &'static str {
    match severity {
        ReviewFindingSeverity::Info => "info",
        ReviewFindingSeverity::Low => "low",
        ReviewFindingSeverity::Medium => "medium",
        ReviewFindingSeverity::High => "high",
        ReviewFindingSeverity::Critical => "critical",
    }
}

fn category_key(category: ReviewFindingCategory) -> &'static str {
    match category {
        ReviewFindingCategory::Bug => "bug",
        ReviewFindingCategory::Security => "security",
        ReviewFindingCategory::Performance => "performance",
        ReviewFindingCategory::Architecture => "architecture",
        ReviewFindingCategory::Typing => "typing",
        ReviewFindingCategory::Test => "test",
        ReviewFindingCategory::Maintainability => "maintainability",
        ReviewFindingCategory::Docs => "docs",
        ReviewFindingCategory::Other => "other",
    }
}

fn severity_from_structured(value: &str) -> ReviewFindingSeverity {
    match value.trim().to_ascii_lowercase().as_str() {
        "critical" => ReviewFindingSeverity::Critical,
        "major" | "high" => ReviewFindingSeverity::High,
        "minor" | "low" => ReviewFindingSeverity::Low,
        "nit" | "info" => ReviewFindingSeverity::Info,
        _ => ReviewFindingSeverity::Medium,
    }
}

fn category_from_structured(value: Option<&str>) -> ReviewFindingCategory {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "bug" => ReviewFindingCategory::Bug,
        "security" => ReviewFindingCategory::Security,
        "performance" => ReviewFindingCategory::Performance,
        "architecture" => ReviewFindingCategory::Architecture,
        "typing" => ReviewFindingCategory::Typing,
        "test" => ReviewFindingCategory::Test,
        "maintainability" => ReviewFindingCategory::Maintainability,
        "docs" | "documentation" => ReviewFindingCategory::Docs,
        _ => ReviewFindingCategory::Other,
    }
}

fn confidence_from_structured(value: Option<&str>) -> ReviewFindingConfidence {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "low" => ReviewFindingConfidence::Low,
        "high" => ReviewFindingConfidence::High,
        _ => ReviewFindingConfidence::Medium,
    }
}

fn parse_structured_review_json_block(
    markdown: &str,
) -> Result<Option<StructuredReviewOutput>, String> {
    let mut in_json = false;
    let mut block = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if in_json {
            if trimmed.starts_with("```") {
                let candidate = block.join("\n");
                block.clear();
                in_json = false;
                if candidate.contains(STRUCTURED_REVIEW_SCHEMA_VERSION) {
                    let parsed: StructuredReviewOutput = serde_json::from_str(&candidate)
                        .map_err(|error| format!("Invalid structured AI review JSON: {error}"))?;
                    if parsed.schema_version != STRUCTURED_REVIEW_SCHEMA_VERSION {
                        return Err(format!(
                            "Unsupported structured AI review schema version `{}`.",
                            parsed.schema_version
                        ));
                    }
                    return Ok(Some(parsed));
                }
                continue;
            }
            block.push(line.to_string());
            continue;
        }

        if trimmed.starts_with("```json") {
            in_json = true;
        }
    }

    if in_json {
        let candidate = block.join("\n");
        if candidate.contains(STRUCTURED_REVIEW_SCHEMA_VERSION) {
            return Err(
                "Invalid structured AI review JSON: missing closing code fence.".to_string(),
            );
        }
    }

    Ok(None)
}

fn structured_review_findings(
    output: StructuredReviewOutput,
    run_id: &str,
    conversation_evidence_id: &str,
) -> Vec<ReviewFinding> {
    output
        .findings
        .into_iter()
        .enumerate()
        .filter_map(|(index, finding)| {
            let summary = finding
                .body
                .or(finding.summary)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())?;
            let title = finding
                .title
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| derive_finding_title(&summary));
            let path = finding
                .path
                .or(finding.file)
                .map(|value| value.trim().to_string());
            let start_line = finding.start_line.or(finding.line);
            let anchor =
                path.filter(|value| !value.is_empty())
                    .zip(start_line)
                    .map(|(path, start_line)| ReviewFindingAnchor {
                        path,
                        start_line,
                        end_line: finding.end_line.filter(|line| *line != start_line),
                        side: ReviewAnchorSide::New,
                    });
            let severity = severity_from_structured(&finding.severity);
            let category = category_from_structured(finding.category.as_deref());
            let anchor_key = anchor
                .as_ref()
                .map(|anchor| {
                    format!(
                        "{}:{}:{}",
                        anchor.path,
                        anchor.start_line,
                        anchor.end_line.unwrap_or(anchor.start_line)
                    )
                })
                .unwrap_or_else(|| "no-anchor".to_string());
            let fingerprint = stable_hash_hex(&format!(
                "{}|{}|{}|{}",
                category_key(category),
                severity_key(severity),
                anchor_key,
                normalize_inline_text(&title).to_ascii_lowercase()
            ));
            Some(ReviewFinding {
                id: format!("{run_id}-finding-{}", index + 1),
                fingerprint,
                title,
                severity,
                confidence: confidence_from_structured(finding.confidence.as_deref()),
                category,
                status: ReviewFindingStatus::New,
                summary,
                rationale: finding
                    .rationale
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                rule_id: finding
                    .rule_id
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                source: ReviewFindingSource::Llm,
                anchor,
                suggested_fix: finding
                    .suggested_fix
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                evidence_ids: vec![conversation_evidence_id.to_string()],
                publication: None,
            })
        })
        .collect()
}

fn review_findings_from_output(
    markdown: &str,
    run_id: &str,
    conversation_evidence_id: &str,
) -> Result<Vec<ReviewFinding>, String> {
    if let Some(output) = parse_structured_review_json_block(markdown)? {
        return Ok(structured_review_findings(
            output,
            run_id,
            conversation_evidence_id,
        ));
    }

    Ok(extract_review_findings(
        markdown,
        run_id,
        conversation_evidence_id,
    ))
}

fn extract_review_findings(
    markdown: &str,
    run_id: &str,
    conversation_evidence_id: &str,
) -> Vec<ReviewFinding> {
    let (review_body, _) = parse_review_resources(markdown);
    let mut findings = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_item_heading: Option<String> = None;
    let mut current_item_lines: Vec<String> = Vec::new();

    let mut finalize_item = |heading: Option<String>, lines: &mut Vec<String>| {
        let summary = lines.join("\n").trim().to_string();
        lines.clear();
        if summary.is_empty() || !finding_has_signal(&summary, heading.as_deref()) {
            return;
        }

        let anchor = extract_finding_anchor(&summary);
        let severity = infer_finding_severity(heading.as_deref(), &summary);
        let category = infer_finding_category(heading.as_deref(), &summary);
        let title = derive_finding_title(&summary);
        let anchor_key = anchor
            .as_ref()
            .map(|anchor| {
                format!(
                    "{}:{}:{}",
                    anchor.path,
                    anchor.start_line,
                    anchor.end_line.unwrap_or(anchor.start_line)
                )
            })
            .unwrap_or_else(|| "no-anchor".to_string());
        let fingerprint = stable_hash_hex(&format!(
            "{}|{}|{}|{}",
            category_key(category),
            severity_key(severity),
            anchor_key,
            normalize_inline_text(&title).to_ascii_lowercase()
        ));
        findings.push(ReviewFinding {
            id: format!("{run_id}-finding-{}", findings.len() + 1),
            fingerprint,
            title,
            severity,
            confidence: if anchor.is_some() {
                ReviewFindingConfidence::High
            } else {
                ReviewFindingConfidence::Medium
            },
            category,
            status: ReviewFindingStatus::New,
            summary,
            rationale: None,
            rule_id: None,
            source: ReviewFindingSource::Llm,
            anchor,
            suggested_fix: None,
            evidence_ids: vec![conversation_evidence_id.to_string()],
            publication: None,
        });
    };

    for line in review_body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current_item_lines.is_empty() {
                current_item_lines.push(String::new());
            }
            continue;
        }

        if let Some(payload) = finding_line_payload(trimmed) {
            if !current_item_lines.is_empty() {
                finalize_item(current_item_heading.take(), &mut current_item_lines);
            }
            current_item_heading = current_heading.clone();
            current_item_lines.push(payload.to_string());
            continue;
        }

        if !current_item_lines.is_empty() {
            if heading_has_finding_signal(trimmed) && !trimmed.starts_with("```") {
                finalize_item(current_item_heading.take(), &mut current_item_lines);
                current_heading = Some(clean_heading_text(trimmed));
                continue;
            }
            current_item_lines.push(trimmed.to_string());
            continue;
        }

        if heading_has_finding_signal(trimmed) || trimmed.starts_with('#') {
            current_heading = Some(clean_heading_text(trimmed));
        }
    }

    if !current_item_lines.is_empty() {
        finalize_item(current_item_heading.take(), &mut current_item_lines);
    }

    findings
}

fn materialize_review_run(
    workspace: &str,
    repo: &str,
    review_provider_override: Option<ReviewProvider>,
    pr_id: u32,
    source_branch: &str,
    destination_branch: &str,
    thread_id: &str,
    turn_kind: AiReviewTurnKind,
    review_profile: Option<&str>,
    created_at: &str,
    finished_at: &str,
    snapshot_payload: &str,
    summary_markdown: &str,
    assistant_evidence_source: ReviewEvidenceSource,
    analyzer_evidence: Vec<ReviewEvidenceArtifact>,
) -> Result<ReviewRun, String> {
    const REVIEW_SCHEMA_VERSION: &str = "v0.1";

    let run_id = now_id("run");
    let conversation_evidence_id = format!("{run_id}-evidence-conversation");
    let mut findings =
        review_findings_from_output(summary_markdown, &run_id, &conversation_evidence_id)?;
    let display_markdown = strip_structured_review_json_block(summary_markdown);
    let (_, resources) = parse_review_resources(&display_markdown);
    let analyzer_evidence = analyzer_evidence
        .into_iter()
        .enumerate()
        .map(|(index, mut artifact)| {
            artifact.id = format!("{run_id}-evidence-analyzer-{}", index + 1);
            artifact
        })
        .collect::<Vec<_>>();
    let analyzer_evidence_ids = analyzer_evidence
        .iter()
        .map(|artifact| artifact.id.clone())
        .collect::<Vec<_>>();
    for finding in &mut findings {
        finding.evidence_ids.extend(analyzer_evidence_ids.clone());
    }
    let mut evidence = vec![ReviewEvidenceArtifact {
        id: conversation_evidence_id.clone(),
        kind: ReviewEvidenceKind::Conversation,
        source: assistant_evidence_source,
        title: "Assistant review output".to_string(),
        summary: Some("Canonical assistant markdown captured for this review turn.".to_string()),
        payload: Some(display_markdown.clone()),
    }];
    evidence.extend(analyzer_evidence);

    for (index, resource) in resources.into_iter().enumerate() {
        evidence.push(ReviewEvidenceArtifact {
            id: format!("{run_id}-evidence-doc-{}", index + 1),
            kind: ReviewEvidenceKind::Doc,
            source: ReviewEvidenceSource::Other,
            title: resource.title,
            summary: resource.summary,
            payload: Some(resource.url),
        });
    }

    Ok(ReviewRun {
        id: run_id.clone(),
        schema_version: REVIEW_SCHEMA_VERSION.to_string(),
        provider: review_provider_override
            .unwrap_or_else(|| review_provider_for_repo(workspace, repo)),
        workspace: workspace.to_string(),
        repo: repo.to_string(),
        pr_id,
        source_branch: source_branch.to_string(),
        destination_branch: destination_branch.to_string(),
        status: AiReviewRunStatus::Succeeded,
        turn_kind,
        review_profile: review_profile.map(ToOwned::to_owned),
        created_at: created_at.to_string(),
        finished_at: Some(finished_at.to_string()),
        diff_fingerprint: stable_hash_hex(snapshot_payload),
        thread_id: Some(thread_id.to_string()),
        summary_markdown: Some(display_markdown),
        evidence,
        findings,
    })
}

fn ensure_finding_publication(
    finding: &mut ReviewFinding,
    mode: ReviewPublicationMode,
) -> &mut ReviewFindingPublication {
    finding
        .publication
        .get_or_insert_with(|| ReviewFindingPublication {
            mode,
            draft_ids: Vec::new(),
            remote_comment_ids: Vec::new(),
            published_at: None,
        })
}

fn apply_review_finding_publication_event(
    finding: &mut ReviewFinding,
    event: &ReviewFindingPublicationEvent,
) {
    match event.kind {
        ReviewFindingPublicationEventKind::StageDraft => {
            let publication = ensure_finding_publication(finding, event.mode);
            publication.mode = event.mode;
            if let Some(draft_id) = event.draft_id.as_deref() {
                if !publication
                    .draft_ids
                    .iter()
                    .any(|current| current == draft_id)
                {
                    publication.draft_ids.push(draft_id.to_string());
                }
            }
        }
        ReviewFindingPublicationEventKind::RemoveDraft => {
            if let Some(publication) = finding.publication.as_mut() {
                if let Some(draft_id) = event.draft_id.as_deref() {
                    publication.draft_ids.retain(|current| current != draft_id);
                }
                if publication.draft_ids.is_empty()
                    && publication.remote_comment_ids.is_empty()
                    && publication.published_at.is_none()
                {
                    finding.publication = None;
                }
            }
            if finding.publication.is_none() && finding.status == ReviewFindingStatus::Published {
                finding.status = ReviewFindingStatus::New;
            }
        }
        ReviewFindingPublicationEventKind::PublishDraft => {
            let publication = ensure_finding_publication(finding, event.mode);
            publication.mode = event.mode;
            if let Some(draft_id) = event.draft_id.as_deref() {
                publication.draft_ids.retain(|current| current != draft_id);
            }
            if let Some(remote_comment_id) = event.remote_comment_id {
                if !publication
                    .remote_comment_ids
                    .iter()
                    .any(|current| *current == remote_comment_id)
                {
                    publication.remote_comment_ids.push(remote_comment_id);
                }
            }
            publication.published_at = Some(event.published_at.clone().unwrap_or_else(now_ms));
            finding.status = ReviewFindingStatus::Published;
        }
    }
}

fn record_review_finding_publication_events(
    store: &mut AiReviewStoreData,
    events: &[ReviewFindingPublicationEvent],
) -> bool {
    let mut changed = false;
    for event in events {
        let Some(run) = store
            .review_runs
            .iter_mut()
            .find(|run| run.id == event.review_run_id)
        else {
            eprintln!(
                "Skipping review publication event for unknown run {}",
                event.review_run_id
            );
            continue;
        };
        let Some(finding) = run
            .findings
            .iter_mut()
            .find(|finding| finding.fingerprint == event.finding_fingerprint)
        else {
            eprintln!(
                "Skipping review publication event for unknown finding {} in run {}",
                event.finding_fingerprint, event.review_run_id
            );
            continue;
        };
        apply_review_finding_publication_event(finding, event);
        changed = true;
    }
    changed
}

fn find_review_thread_mut<'a>(
    store: &'a mut AiReviewStoreData,
    thread_id: &str,
) -> Result<&'a mut AiReviewThread, String> {
    store
        .threads
        .iter_mut()
        .find(|thread| thread.id == thread_id)
        .ok_or_else(|| format!("Unknown AI review thread: {thread_id}"))
}

fn find_review_thread<'a>(
    store: &'a AiReviewStoreData,
    thread_id: &str,
) -> Result<&'a AiReviewThread, String> {
    store
        .threads
        .iter()
        .find(|thread| thread.id == thread_id)
        .ok_or_else(|| format!("Unknown AI review thread: {thread_id}"))
}

fn append_review_message(
    thread: &mut AiReviewThread,
    role: AiReviewMessageRole,
    content: String,
    created_at: String,
) {
    thread.messages.push(AiReviewMessage {
        id: now_id("msg"),
        role,
        content,
        created_at: created_at.clone(),
    });
    thread.updated_at = created_at;
}

fn build_reply_fallback_payload(
    base_payload: &str,
    thread: &AiReviewThread,
    user_message: &str,
) -> String {
    let mut lines = vec![
        base_payload.trim().to_string(),
        "".to_string(),
        "## Existing review conversation".to_string(),
    ];
    for message in &thread.messages {
        let speaker = match message.role {
            AiReviewMessageRole::User => "Reviewer",
            AiReviewMessageRole::Assistant => "Assistant",
        };
        lines.push(format!("### {speaker}"));
        lines.push(message.content.trim().to_string());
        lines.push("".to_string());
    }
    lines.push("## New follow-up from the reviewer".to_string());
    lines.push(user_message.trim().to_string());
    lines.join("\n")
}

fn with_review_run_store<T, F>(store: &AiReviewRunStore, f: F) -> T
where
    F: FnOnce(&mut AiReviewRunStoreInner) -> T,
{
    let mut inner = store.0.lock().expect("ai review run store poisoned");
    f(&mut inner)
}

fn with_store<T, F>(store: &AiReviewFixStore, f: F) -> T
where
    F: FnOnce(&mut AiReviewFixStoreInner) -> T,
{
    let mut inner = store.0.lock().expect("ai review fix store poisoned");
    f(&mut inner)
}

fn trim_logs(logs: &mut Vec<String>) {
    const MAX_LOGS: usize = 300;
    if logs.len() > MAX_LOGS {
        let drop_count = logs.len() - MAX_LOGS;
        logs.drain(0..drop_count);
    }
}

fn ensure_review_run_session<'a>(
    inner: &'a mut AiReviewRunStoreInner,
    key: &str,
) -> &'a mut ReviewRunSessionRecord {
    inner
        .sessions
        .entry(key.to_string())
        .or_insert_with(|| ReviewRunSessionRecord {
            public: AiReviewRunState {
                pr_key: key.to_string(),
                ..AiReviewRunState::default()
            },
            ..ReviewRunSessionRecord::default()
        })
}

fn begin_inline_review_run(
    store: &AiReviewRunStore,
    key: &str,
    title: String,
    thread_id: String,
    turn_kind: AiReviewTurnKind,
    review_kind: Option<&str>,
) -> Result<(AiReviewRunState, u64), String> {
    with_review_run_store(store, |inner| {
        if matches!(
            inner.sessions.get(key).map(|session| session.public.status),
            Some(AiReviewRunStatus::Running)
        ) {
            return Err("An AI review is already running for this pull request.".to_string());
        }
        inner.next_run_id = inner.next_run_id.saturating_add(1);
        let run_id = inner.next_run_id;
        let session = ensure_review_run_session(inner, key);
        session.run_id = run_id;
        session.child_pid = None;
        session.cancel_requested = false;
        session.public = AiReviewRunState {
            pr_key: key.to_string(),
            pr_title: Some(title.clone()),
            thread_id: Some(thread_id.clone()),
            turn_kind: Some(turn_kind),
            status: AiReviewRunStatus::Running,
            logs: match turn_kind {
                AiReviewTurnKind::Initial => {
                    let starting = if review_kind == Some("lineQuestion") {
                        "Starting line question…"
                    } else {
                        "Starting AI review…"
                    };
                    vec![
                        starting.to_string(),
                        format!("Reviewing PR: {title}"),
                        format!("Saving output to review thread {thread_id}."),
                    ]
                }
                AiReviewTurnKind::Reply => vec![
                    "Continuing AI review chat…".to_string(),
                    format!("Reviewing PR: {title}"),
                    format!("Saving output to review thread {thread_id}."),
                ],
            },
            started_at: Some(now_ms()),
            finished_at: None,
            generated_at: None,
            error: None,
        };
        Ok((session.public.clone(), run_id))
    })
}

fn set_inline_review_pid(store: &AiReviewRunStore, key: &str, run_id: u64, pid: u32) {
    set_inline_review_process_pid(store, key, run_id, pid, "Command");
}

fn set_inline_review_process_pid(
    store: &AiReviewRunStore,
    key: &str,
    run_id: u64,
    pid: u32,
    provider_label: &str,
) {
    with_review_run_store(store, |inner| {
        let Some(session) = inner.sessions.get_mut(key) else {
            return;
        };
        if session.run_id != run_id {
            return;
        }
        session.child_pid = Some(pid);
        session
            .public
            .logs
            .push(format!("{provider_label} process started."));
        trim_logs(&mut session.public.logs);
    });
}

fn clear_inline_review_pid(store: &AiReviewRunStore, key: &str, run_id: u64) {
    with_review_run_store(store, |inner| {
        let Some(session) = inner.sessions.get_mut(key) else {
            return;
        };
        if session.run_id == run_id {
            session.child_pid = None;
        }
    });
}

fn inline_review_cancel_requested(store: &AiReviewRunStore, key: &str, run_id: u64) -> bool {
    with_review_run_store(store, |inner| {
        inner
            .sessions
            .get(key)
            .filter(|session| session.run_id == run_id)
            .map(|session| session.cancel_requested)
            .unwrap_or(true)
    })
}

fn clone_inline_review_state(store: &AiReviewRunStore, key: &str) -> Option<AiReviewRunState> {
    with_review_run_store(store, |inner| {
        inner
            .sessions
            .get(key)
            .map(|session| session.public.clone())
    })
}

pub fn get_ai_review_run_state_native(
    store: &AiReviewRunStore,
    workspace: &str,
    repo: &str,
    id: u32,
) -> Option<AiReviewRunState> {
    clone_inline_review_state(store, &pr_key(workspace, repo, id))
}

fn append_inline_review_log(
    store: &AiReviewRunStore,
    key: &str,
    run_id: u64,
    line: impl Into<String>,
) {
    let line = line.into();
    with_review_run_store(store, |inner| {
        let Some(session) = inner.sessions.get_mut(key) else {
            return;
        };
        if session.run_id != run_id {
            return;
        }
        if session
            .public
            .logs
            .last()
            .map(|current| current == &line)
            .unwrap_or(false)
        {
            return;
        }
        session.public.logs.push(line);
        trim_logs(&mut session.public.logs);
    });
}

fn set_inline_review_failed(
    store: &AiReviewRunStore,
    key: &str,
    run_id: u64,
    error: impl Into<String>,
) {
    let error = error.into();
    with_review_run_store(store, |inner| {
        let Some(session) = inner.sessions.get_mut(key) else {
            return;
        };
        if session.run_id != run_id {
            return;
        }
        session.child_pid = None;
        session.public.status = AiReviewRunStatus::Failed;
        session.public.finished_at = Some(now_ms());
        session.public.error = Some(error.clone());
        session.public.logs.push(format!("ERROR: {error}"));
        trim_logs(&mut session.public.logs);
    });
}

fn finish_inline_review_success(
    store: &AiReviewRunStore,
    key: &str,
    run_id: u64,
    generated_at: String,
    provider_label: &str,
) {
    with_review_run_store(store, |inner| {
        let Some(session) = inner.sessions.get_mut(key) else {
            return;
        };
        if session.run_id != run_id {
            return;
        }
        session.child_pid = None;
        session.public.status = AiReviewRunStatus::Succeeded;
        session.public.finished_at = Some(now_ms());
        session.public.generated_at = Some(generated_at);
        session.public.error = None;
        session
            .public
            .logs
            .push(format!("{provider_label} finished successfully."));
        trim_logs(&mut session.public.logs);
    });
}

fn mark_inline_review_cancelled(store: &AiReviewRunStore, key: &str, run_id: u64) {
    with_review_run_store(store, |inner| {
        let Some(session) = inner.sessions.get_mut(key) else {
            return;
        };
        if session.run_id != run_id {
            return;
        }
        session.cancel_requested = true;
        session.child_pid = None;
        session.public.status = AiReviewRunStatus::Cancelled;
        session.public.finished_at = Some(now_ms());
        session.public.error = None;
        session
            .public
            .logs
            .push("Review cancelled by the user.".to_string());
        trim_logs(&mut session.public.logs);
    });
}

fn ensure_session<'a>(inner: &'a mut AiReviewFixStoreInner, key: &str) -> &'a mut FixSessionRecord {
    inner
        .sessions
        .entry(key.to_string())
        .or_insert_with(|| FixSessionRecord {
            public: AiReviewFixState {
                pr_key: key.to_string(),
                ..AiReviewFixState::default()
            },
            ..FixSessionRecord::default()
        })
}

fn append_log(store: &AiReviewFixStore, key: &str, line: impl Into<String>) {
    let line = line.into();
    with_store(store, |inner| {
        let session = ensure_session(inner, key);
        session.public.logs.push(line);
        trim_logs(&mut session.public.logs);
    });
}

fn set_phase(
    store: &AiReviewFixStore,
    key: &str,
    phase: AiReviewFixPhase,
    status: AiReviewFixStatus,
) {
    with_store(store, |inner| {
        let session = ensure_session(inner, key);
        session.public.phase = phase;
        session.public.status = status;
    });
}

fn set_fix_failed(store: &AiReviewFixStore, key: &str, error: impl Into<String>) {
    let error = error.into();
    with_store(store, |inner| {
        let session = ensure_session(inner, key);
        session.public.status = AiReviewFixStatus::Failed;
        session.public.finished_at = Some(now_ms());
        session.public.error = Some(error.clone());
        session.public.logs.push(format!("ERROR: {error}"));
        trim_logs(&mut session.public.logs);
        if inner.active_key.as_deref() == Some(key) {
            inner.active_key = None;
        }
    });
}

fn finalize_active_key(store: &AiReviewFixStore, key: &str) {
    with_store(store, |inner| {
        if inner.active_key.as_deref() == Some(key) {
            inner.active_key = None;
        }
    });
}

fn begin_operation(
    store: &AiReviewFixStore,
    key: &str,
    pr_key: &str,
    thread_id: Option<String>,
    phase: AiReviewFixPhase,
    repo_path: Option<String>,
) -> Result<AiReviewFixState, String> {
    with_store(store, |inner| {
        if let Some(active) = inner.active_key.as_deref() {
            if active == key {
                return Err(
                    "An AI review operation is already running for this pull request.".to_string(),
                );
            }
            return Err(format!(
                "Another AI review operation is already running for {active}. Wait for it to finish first."
            ));
        }
        inner.active_key = Some(key.to_string());
        let session = ensure_session(inner, key);
        session.public = AiReviewFixState {
            pr_key: pr_key.to_string(),
            thread_id,
            repo_path,
            status: AiReviewFixStatus::Running,
            phase,
            logs: vec!["Starting AI review fix pipeline…".to_string()],
            started_at: Some(now_ms()),
            finished_at: None,
            suggested_commit_message: None,
            summary: None,
            commit_sha: None,
            error: None,
            files_touched: Vec::new(),
            tests: Vec::new(),
            claude_duration_ms: None,
            claude_session_id: None,
        };
        session.source_branch.clear();
        session.destination_branch.clear();
        session.baseline_files.clear();
        Ok(session.public.clone())
    })
}

fn begin_session_step<F>(
    store: &AiReviewFixStore,
    key: &str,
    phase: AiReviewFixPhase,
    mutator: F,
) -> Result<AiReviewFixState, String>
where
    F: FnOnce(&mut FixSessionRecord) -> Result<(), String>,
{
    with_store(store, |inner| {
        if let Some(active) = inner.active_key.as_deref() {
            if active == key {
                return Err(
                    "An AI review operation is already running for this pull request.".to_string(),
                );
            }
            return Err(format!(
                "Another AI review operation is already running for {active}. Wait for it to finish first."
            ));
        }
        let session = inner
            .sessions
            .get_mut(key)
            .ok_or_else(|| "No AI fix session exists for this pull request yet.".to_string())?;
        mutator(session)?;
        session.public.status = AiReviewFixStatus::Running;
        session.public.phase = phase;
        session.public.finished_at = None;
        session.public.error = None;
        inner.active_key = Some(key.to_string());
        Ok(session.public.clone())
    })
}

fn clone_public_state(store: &AiReviewFixStore, key: &str) -> Option<AiReviewFixState> {
    with_store(store, |inner| {
        inner
            .sessions
            .get(key)
            .map(|session| session.public.clone())
    })
}

fn git_command(repo_path: &Path) -> Command {
    let mut cmd = Command::new("/usr/bin/git");
    cmd.current_dir(repo_path);
    cmd
}

fn run_git_capture(repo_path: &Path, args: &[&str]) -> Result<CommandOutput, String> {
    let output = git_command(repo_path)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git {}: {e}", args.join(" ")))?;
    Ok(CommandOutput {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn run_git_checked(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = run_git_capture(repo_path, args)?;
    if output.code == Some(0) {
        Ok(output.stdout.trim().to_string())
    } else {
        Err(format!(
            "git {} failed with code {:?}\nstderr: {}\nstdout: {}",
            args.join(" "),
            output.code,
            output.stderr.trim(),
            output.stdout.trim()
        ))
    }
}

fn read_stream<R: std::io::Read + Send + 'static>(
    reader: R,
    store: AiReviewFixStore,
    key: String,
    stream_name: &'static str,
    sink: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = String::new();
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            append_log(&store, &key, format!("[{stream_name}] {line}"));
            buf.push_str(&line);
            buf.push('\n');
        }
        if let Ok(mut out) = sink.lock() {
            out.push_str(&buf);
        }
    })
}

fn run_logged_command(
    mut command: Command,
    store: &AiReviewFixStore,
    key: &str,
    label: &str,
) -> Result<CommandOutput, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to start {label}: {e}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("{label} stdout was not captured"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("{label} stderr was not captured"))?;

    let stdout_buf = Arc::new(Mutex::new(String::new()));
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let stdout_thread = read_stream(
        stdout,
        store.clone(),
        key.to_string(),
        "stdout",
        stdout_buf.clone(),
    );
    let stderr_thread = read_stream(
        stderr,
        store.clone(),
        key.to_string(),
        "stderr",
        stderr_buf.clone(),
    );

    let status = child
        .wait()
        .map_err(|e| format!("failed while waiting for {label}: {e}"))?;
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    let stdout = stdout_buf.lock().map(|s| s.clone()).unwrap_or_default();
    let stderr = stderr_buf.lock().map(|s| s.clone()).unwrap_or_default();
    Ok(CommandOutput {
        code: status.code(),
        stdout,
        stderr,
    })
}

fn truncate_for_log(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn summarize_json_value_for_log(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Object(map) => {
            let mut keys = map.keys().take(4).cloned().collect::<Vec<_>>();
            if map.len() > 4 {
                keys.push("…".to_string());
            }
            if keys.is_empty() {
                String::new()
            } else {
                format!("({})", keys.join(", "))
            }
        }
        serde_json::Value::Array(items) => format!("({} item(s))", items.len()),
        serde_json::Value::String(text) => format!("({})", truncate_for_log(text, 120)),
        other => format!("({})", truncate_for_log(&other.to_string(), 120)),
    }
}

fn string_field<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn first_string_field<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| string_field(value, key))
}

fn format_claude_tool_log_line(name: &str, input: Option<&serde_json::Value>) -> String {
    let Some(input) = input else {
        return format!("Using Claude tool: {name}");
    };
    match name {
        "Read" => first_string_field(input, &["file_path", "path"])
            .map(|path| format!("Reading file: {path}"))
            .unwrap_or_else(|| "Reading a file.".to_string()),
        "Glob" => {
            let pattern = string_field(input, "pattern");
            let path = string_field(input, "path");
            match (pattern, path) {
                (Some(pattern), Some(path)) => {
                    format!("Finding files matching `{pattern}` under {path}.")
                }
                (Some(pattern), None) => format!("Finding files matching `{pattern}`."),
                _ => "Finding relevant files.".to_string(),
            }
        }
        "Grep" => {
            let pattern = string_field(input, "pattern");
            let path = string_field(input, "path");
            match (pattern, path) {
                (Some(pattern), Some(path)) => {
                    format!(
                        "Searching code for `{}` under {}.",
                        truncate_for_log(pattern, 80),
                        path
                    )
                }
                (Some(pattern), None) => {
                    format!("Searching code for `{}`.", truncate_for_log(pattern, 80))
                }
                _ => "Searching code.".to_string(),
            }
        }
        "Bash" => string_field(input, "command")
            .map(|command| format!("Running command: {}", truncate_for_log(command, 120)))
            .unwrap_or_else(|| "Running a shell command.".to_string()),
        "LS" => first_string_field(input, &["path", "dir"])
            .map(|path| format!("Listing directory: {path}"))
            .unwrap_or_else(|| "Listing files.".to_string()),
        "Edit" | "MultiEdit" | "Write" => "Preparing code changes.".to_string(),
        "TodoWrite" => "Updating review checklist.".to_string(),
        "WebFetch" | "WebSearch" => "Fetching external reference context.".to_string(),
        other => {
            let suffix = summarize_json_value_for_log(input);
            format!("Using Claude tool: {other}{suffix}")
        }
    }
}

fn extract_claude_content_blocks(value: &serde_json::Value) -> Option<&[serde_json::Value]> {
    value
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_array().map(Vec::as_slice))
        .or_else(|| {
            value
                .get("content")
                .and_then(|content| content.as_array().map(Vec::as_slice))
        })
}

fn extract_text_from_content_blocks(blocks: &[serde_json::Value]) -> Option<String> {
    let text = blocks
        .iter()
        .filter_map(|block| {
            (block.get("type").and_then(|value| value.as_str()) == Some("text"))
                .then_some(block.get("text").and_then(|value| value.as_str()))
                .flatten()
        })
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn extract_assistant_text_from_value(value: &serde_json::Value) -> Option<String> {
    extract_claude_content_blocks(value)
        .and_then(extract_text_from_content_blocks)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn permission_denials_from_value(value: &serde_json::Value) -> usize {
    value
        .get("permission_denials")
        .and_then(|value| value.as_array())
        .map(|value| value.len())
        .unwrap_or(0)
}

fn extract_claude_text_response_from_value(
    value: &serde_json::Value,
) -> Option<ParsedClaudeTextResponse> {
    let content = value
        .get("result")
        .and_then(|value| value.as_str())
        .or_else(|| value.get("content").and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| extract_assistant_text_from_value(value));

    content.map(|content| ParsedClaudeTextResponse {
        content,
        duration_ms: value.get("duration_ms").and_then(|value| value.as_u64()),
        session_id: value
            .get("session_id")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        permission_denials: permission_denials_from_value(value),
    })
}

fn format_claude_stream_log_line(stream_name: &'static str, line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if stream_name == "stderr" {
        return vec![format!("[stderr] {trimmed}")];
    }

    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return vec![format!("[stdout] {trimmed}")];
    };

    let event_type = value.get("type").and_then(|value| value.as_str());
    let subtype = value.get("subtype").and_then(|value| value.as_str());

    match event_type {
        Some("system") => {
            if matches!(
                subtype,
                Some(
                    "thinking_tokens"
                        | "content_block_delta"
                        | "content_block_start"
                        | "content_block_stop"
                        | "message_delta"
                        | "message_start"
                        | "message_stop"
                )
            ) {
                return Vec::new();
            }
            let mut label = match subtype {
                Some("init") => "Claude session initialized".to_string(),
                Some(subtype) => format!("Claude system: {subtype}"),
                None => "Claude system event".to_string(),
            };
            if let Some(model) = value.get("model").and_then(|value| value.as_str()) {
                label.push_str(&format!(" ({model})"));
            }
            vec![label]
        }
        Some("assistant") => {
            let mut rendered = Vec::new();
            if let Some(blocks) = extract_claude_content_blocks(&value) {
                for block in blocks {
                    match block.get("type").and_then(|value| value.as_str()) {
                        Some("tool_use") => {
                            let name = block
                                .get("name")
                                .and_then(|value| value.as_str())
                                .unwrap_or("tool");
                            rendered.push(format_claude_tool_log_line(name, block.get("input")));
                        }
                        Some("text") => {
                            if block
                                .get("text")
                                .and_then(|value| value.as_str())
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .is_some()
                            {
                                rendered.push("Claude is drafting the review…".to_string());
                            }
                        }
                        Some(other) => rendered.push(format!("Claude content: {other}")),
                        None => {}
                    }
                }
            }

            if rendered.is_empty() {
                vec!["Claude assistant event".to_string()]
            } else {
                rendered
            }
        }
        Some("result") => match subtype {
            Some("success") => vec!["Claude produced a result.".to_string()],
            Some("error") => vec!["Claude reported an error.".to_string()],
            Some(subtype) => vec![format!("Claude result: {subtype}")],
            None => vec!["Claude produced a result.".to_string()],
        },
        Some("stream_event" | "user") => Vec::new(),
        Some(other) => match subtype {
            Some("thinking_tokens" | "content_block_delta" | "content_block_start")
            | Some("content_block_stop" | "message_delta" | "message_start" | "message_stop") => {
                Vec::new()
            }
            None if matches!(
                other,
                "thinking"
                    | "thinking_tokens"
                    | "content_block_delta"
                    | "content_block_start"
                    | "content_block_stop"
                    | "message_delta"
                    | "message_start"
                    | "message_stop"
            ) =>
            {
                Vec::new()
            }
            Some(subtype) => vec![format!("Claude event: {other}/{subtype}")],
            None => vec![format!("Claude event: {other}")],
        },
        None => vec![format!("[stdout] {trimmed}")],
    }
}

fn read_inline_review_stream_with_formatter<R: std::io::Read + Send + 'static>(
    reader: R,
    store: AiReviewRunStore,
    key: String,
    run_id: u64,
    stream_name: &'static str,
    sink: Arc<Mutex<String>>,
    formatter: fn(&'static str, &str) -> Vec<String>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = String::new();
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            for rendered in formatter(stream_name, &line) {
                append_inline_review_log(&store, &key, run_id, rendered);
            }
            buf.push_str(&line);
            buf.push('\n');
        }
        if let Ok(mut out) = sink.lock() {
            out.push_str(&buf);
        }
    })
}

fn read_inline_review_stream<R: std::io::Read + Send + 'static>(
    reader: R,
    store: AiReviewRunStore,
    key: String,
    run_id: u64,
    stream_name: &'static str,
    sink: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    read_inline_review_stream_with_formatter(
        reader,
        store,
        key,
        run_id,
        stream_name,
        sink,
        format_claude_stream_log_line,
    )
}

fn format_codex_stream_log_line(stream_name: &'static str, line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if stream_name == "stderr" {
        return vec![format!("[stderr] {trimmed}")];
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return vec![format!("[stdout] {trimmed}")];
    };
    let event_type = value
        .get("type")
        .and_then(|value| value.as_str())
        .or_else(|| value.get("event").and_then(|value| value.as_str()));
    match event_type {
        Some("assistant_message" | "message" | "agent_message") => {
            vec!["Codex is drafting the review…".to_string()]
        }
        Some("task_started" | "turn_started") => vec!["Codex review started.".to_string()],
        Some("task_complete" | "turn_complete" | "completed") => {
            vec!["Codex produced a result.".to_string()]
        }
        Some("error") => vec!["Codex reported an error.".to_string()],
        Some(other) => vec![format!("Codex event: {other}")],
        None => Vec::new(),
    }
}

fn read_codex_inline_review_stream<R: std::io::Read + Send + 'static>(
    reader: R,
    store: AiReviewRunStore,
    key: String,
    run_id: u64,
    stream_name: &'static str,
    sink: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    read_inline_review_stream_with_formatter(
        reader,
        store,
        key,
        run_id,
        stream_name,
        sink,
        format_codex_stream_log_line,
    )
}

fn kill_process(pid: u32) -> Result<(), String> {
    let output = Command::new("/bin/kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("failed to send SIGTERM to process (pid {pid}): {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("No such process") {
        return Ok(());
    }
    Err(format!(
        "failed to cancel process (pid {pid})\nstderr: {}\nstdout: {}",
        stderr.trim(),
        String::from_utf8_lossy(&output.stdout).trim()
    ))
}

fn git_status_lines(repo_path: &Path) -> Result<Vec<String>, String> {
    let output = run_git_checked(repo_path, &["status", "--porcelain=v1"])?;
    Ok(output
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn unmerged_files(repo_path: &Path) -> Result<Vec<String>, String> {
    let output = run_git_checked(repo_path, &["diff", "--name-only", "--diff-filter=U"])?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn parse_status_path(line: &str) -> Option<String> {
    let path = line.get(3..)?.trim();
    if path.is_empty() {
        return None;
    }
    if let Some((_, new_path)) = path.split_once(" -> ") {
        return Some(new_path.trim().to_string());
    }
    Some(path.to_string())
}

fn files_from_status(lines: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for line in lines {
        if let Some(path) = parse_status_path(line) {
            if seen.insert(path.clone()) {
                out.push(path);
            }
        }
    }
    out
}

fn find_stash_ref(repo_path: &Path, marker: &str) -> Result<Option<String>, String> {
    let output = run_git_checked(repo_path, &["stash", "list", "--format=%gd:%s"])?;
    for line in output.lines() {
        if let Some((stash_ref, subject)) = line.split_once(':') {
            if subject.contains(marker) {
                return Ok(Some(stash_ref.to_string()));
            }
        }
    }
    Ok(None)
}

fn restore_stash(repo_path: &Path, stash_ref: &str) -> Result<(), String> {
    run_git_checked(repo_path, &["stash", "apply", stash_ref])?;
    run_git_checked(repo_path, &["stash", "drop", stash_ref])?;
    Ok(())
}

fn resolve_branch(repo_path: &Path, branch: &str) -> Result<(), String> {
    let exists = run_git_capture(repo_path, &["rev-parse", "--verify", branch])?;
    if exists.code != Some(0) {
        run_git_checked(
            repo_path,
            &["fetch", "origin", &format!("{branch}:{branch}")],
        )?;
    }
    run_git_checked(repo_path, &["checkout", branch])?;
    Ok(())
}

fn build_claude_text_command(
    repo_path: Option<&Path>,
    payload: &str,
    resume_session_id: Option<&str>,
    claude_model: Option<&str>,
    claude_effort: Option<&str>,
) -> Result<(Command, tempfile::TempDir), String> {
    let temp_dir = private_provider_temp_dir("lachesi-claude-review-")?;
    let tmp_path = temp_dir.path().join("prompt.md");
    fs::write(&tmp_path, payload).map_err(|e| e.to_string())?;

    let resume_arg = resume_session_id
        .map(|session_id| format!(" --resume {}", shell_quote(session_id)))
        .unwrap_or_default();
    let model_arg = claude_model
        .and_then(normalize_claude_model)
        .map(|model| format!(" --model {}", shell_quote(&model)))
        .unwrap_or_default();
    let effort_arg = claude_effort
        .and_then(normalize_claude_effort)
        .map(|effort| format!(" --effort {}", shell_quote(effort)))
        .unwrap_or_default();
    let shell_cmd = format!(
        "export PATH=\"$HOME/.local/bin:$HOME/.npm/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; claude --print --verbose --permission-mode plan --allowedTools Read,Glob,Grep --output-format stream-json --include-partial-messages{model_arg}{effort_arg}{resume_arg} < {}",
        shell_quote(&tmp_path.to_string_lossy())
    );
    let mut command = Command::new("/bin/zsh");
    command
        .arg("-lc")
        .arg(shell_cmd)
        .env("LACHESI_REVIEW_CHILD", "1");
    if let Some(repo_path) = repo_path {
        command.current_dir(repo_path);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    Ok((command, temp_dir))
}

fn private_provider_temp_dir(prefix: &str) -> Result<tempfile::TempDir, String> {
    let temp_dir = tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .map_err(|error| format!("Failed to create private provider storage: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(temp_dir.path(), fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("Failed to secure provider storage: {error}"))?;
    }
    Ok(temp_dir)
}

fn normalize_claude_model(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn normalize_claude_effort(value: &str) -> Option<&'static str> {
    match value.trim() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        "max" => Some("max"),
        _ => None,
    }
}

fn build_codex_text_command(
    repo_path: Option<&Path>,
    payload: &str,
    codex_model: Option<&str>,
    codex_effort: Option<&str>,
) -> Result<(Command, tempfile::TempDir, PathBuf), String> {
    let temp_dir = private_provider_temp_dir("lachesi-codex-review-")?;
    let tmp_path = temp_dir.path().join("prompt.md");
    let output_path = temp_dir.path().join("output.md");
    fs::write(&tmp_path, payload).map_err(|e| e.to_string())?;

    let model_arg = codex_model
        .and_then(normalize_codex_model)
        .map(|model| format!(" --model {}", shell_quote(&model)))
        .unwrap_or_default();
    let effort_arg = codex_effort
        .and_then(normalize_codex_effort)
        .map(|effort| {
            format!(
                " -c {}",
                shell_quote(&format!("model_reasoning_effort={effort}"))
            )
        })
        .unwrap_or_default();
    let repo_arg = if repo_path.is_some() {
        String::new()
    } else {
        " --skip-git-repo-check".to_string()
    };
    let shell_cmd = format!(
        "export PATH=\"$HOME/.local/bin:$HOME/.npm/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; codex exec --sandbox read-only --output-last-message {}{model_arg}{effort_arg}{repo_arg} - < {}",
        shell_quote(&output_path.to_string_lossy()),
        shell_quote(&tmp_path.to_string_lossy())
    );
    let mut command = Command::new("/bin/zsh");
    command
        .arg("-lc")
        .arg(shell_cmd)
        .env("LACHESI_REVIEW_CHILD", "1");
    if let Some(repo_path) = repo_path {
        command.current_dir(repo_path);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    Ok((command, temp_dir, output_path))
}

fn normalize_codex_model(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn normalize_codex_effort(value: &str) -> Option<&'static str> {
    match value.trim() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        _ => None,
    }
}

fn run_claude_fix(
    repo_path: &Path,
    payload: &str,
    store: &AiReviewFixStore,
    key: &str,
) -> Result<ParsedClaudeFixResponse, String> {
    let tmp_path = std::env::temp_dir().join(format!("lachesi-ai-fix-{}.md", now_ms()));
    fs::write(&tmp_path, payload).map_err(|e| e.to_string())?;

    let schema = r#"{"type":"object","properties":{"status":{"type":"string","enum":["success","failed"]},"summary":{"type":"string"},"commitMessage":{"type":"string"},"tests":{"type":"array","items":{"type":"string"}},"filesTouched":{"type":"array","items":{"type":"string"}},"failureReason":{"type":"string"}},"required":["status","summary","commitMessage","tests","filesTouched"],"additionalProperties":false}"#;
    let shell_cmd = format!(
        "export PATH=\"$HOME/.local/bin:$HOME/.npm/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; claude --print --output-format json --permission-mode bypassPermissions --json-schema {} \"$(cat {})\"",
        shell_quote(schema),
        shell_quote(&tmp_path.to_string_lossy())
    );
    let mut command = Command::new("/bin/zsh");
    command.arg("-lc").arg(shell_cmd).current_dir(repo_path);

    append_log(store, key, "Claude is applying the review feedback…");
    let output = run_logged_command(command, store, key, "claude")?;
    let _ = fs::remove_file(&tmp_path);

    if output.code != Some(0) {
        return Err(format!(
            "claude exited with code {:?}\nstderr: {}\nstdout: {}",
            output.code,
            output.stderr.trim(),
            output.stdout.trim()
        ));
    }

    parse_claude_fix_result(&output.stdout)
}

fn run_claude_structured_output<T>(
    repo_path: Option<&Path>,
    payload: &str,
    schema: &str,
) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let tmp_path = std::env::temp_dir().join(format!("lachesi-structured-output-{}.md", now_ms()));
    fs::write(&tmp_path, payload).map_err(|e| e.to_string())?;

    let shell_cmd = format!(
        "export PATH=\"$HOME/.local/bin:$HOME/.npm/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; claude --print --output-format json --permission-mode bypassPermissions --json-schema {} \"$(cat {})\"",
        shell_quote(schema),
        shell_quote(&tmp_path.to_string_lossy())
    );
    let mut command = Command::new("/bin/zsh");
    command.arg("-lc").arg(shell_cmd);
    if let Some(repo_path) = repo_path {
        command.current_dir(repo_path);
    }
    let output = command
        .output()
        .map_err(|e| format!("Failed to run claude: {e}"))?;
    let _ = fs::remove_file(&tmp_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "claude exited with code {:?}.\nstderr: {stderr}\nstdout: {stdout}",
            output.status.code()
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| format!("claude output is not valid UTF-8: {e}"))?;
    parse_claude_structured_json(&stdout)
}

fn parse_claude_structured_json<T>(stdout: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let trimmed = stdout.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(structured_output) = value.get("structured_output") {
            if let Ok(result) = serde_json::from_value::<T>(structured_output.clone()) {
                return Ok(result);
            }
        }
        if let Some(result_value) = value.get("result") {
            if let Some(as_str) = result_value.as_str() {
                if let Ok(result) = serde_json::from_str::<T>(as_str) {
                    return Ok(result);
                }
            }
            if let Ok(result) = serde_json::from_value::<T>(result_value.clone()) {
                return Ok(result);
            }
        }
        if let Some(content) = value.get("content").and_then(|content| content.as_str()) {
            if let Ok(result) = serde_json::from_str::<T>(content) {
                return Ok(result);
            }
        }
        if let Ok(result) = serde_json::from_value::<T>(value.clone()) {
            return Ok(result);
        }
    }
    if let Ok(result) = serde_json::from_str::<T>(trimmed) {
        return Ok(result);
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if end > start {
            let candidate = &trimmed[start..=end];
            if let Ok(result) = serde_json::from_str::<T>(candidate) {
                return Ok(result);
            }
        }
    }
    Err(format!("Could not parse Claude JSON response:\n{trimmed}"))
}

fn parse_claude_text_result(stdout: &str) -> Result<ParsedClaudeTextResponse, String> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err("Claude returned an empty review response.".to_string());
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(parsed) = extract_claude_text_response_from_value(&value) {
            return Ok(parsed);
        }
    }

    let mut content: Option<String> = None;
    let mut duration_ms: Option<u64> = None;
    let mut session_id: Option<String> = None;
    let mut permission_denials = 0usize;
    let mut saw_json_line = false;

    for line in trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        saw_json_line = true;

        if let Some(parsed) = extract_claude_text_response_from_value(&value) {
            content = Some(parsed.content);
            if parsed.duration_ms.is_some() {
                duration_ms = parsed.duration_ms;
            }
            if parsed.session_id.is_some() {
                session_id = parsed.session_id;
            }
            if parsed.permission_denials > 0 {
                permission_denials = parsed.permission_denials;
            }
            continue;
        }

        if duration_ms.is_none() {
            duration_ms = value.get("duration_ms").and_then(|value| value.as_u64());
        }
        if session_id.is_none() {
            session_id = value
                .get("session_id")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
        }
        permission_denials = permission_denials.max(permission_denials_from_value(&value));
    }

    if let Some(content) = content.filter(|content| !content.trim().is_empty()) {
        return Ok(ParsedClaudeTextResponse {
            content,
            duration_ms,
            session_id,
            permission_denials,
        });
    }

    if saw_json_line {
        return Err(format!(
            "Claude JSON stream did not contain review text:\n{trimmed}"
        ));
    }

    Ok(ParsedClaudeTextResponse {
        content: trimmed.to_string(),
        duration_ms: None,
        session_id: None,
        permission_denials: 0,
    })
}

fn parse_claude_fix_result(stdout: &str) -> Result<ParsedClaudeFixResponse, String> {
    let trimmed = stdout.trim();
    if let Ok(result) = serde_json::from_str::<ClaudeFixResult>(trimmed) {
        return Ok(ParsedClaudeFixResponse {
            result,
            duration_ms: None,
            session_id: None,
            permission_denials: 0,
        });
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(result) = extract_claude_result_from_value(&value)? {
            return Ok(result);
        }
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if end > start {
            let candidate = &trimmed[start..=end];
            if let Ok(result) = serde_json::from_str::<ClaudeFixResult>(candidate) {
                return Ok(ParsedClaudeFixResponse {
                    result,
                    duration_ms: None,
                    session_id: None,
                    permission_denials: 0,
                });
            }
        }
    }
    Err(format!("Could not parse Claude JSON response:\n{trimmed}"))
}

fn extract_claude_result_from_value(
    value: &serde_json::Value,
) -> Result<Option<ParsedClaudeFixResponse>, String> {
    if let Ok(result) = serde_json::from_value::<ClaudeFixResult>(value.clone()) {
        return Ok(Some(ParsedClaudeFixResponse {
            result,
            duration_ms: None,
            session_id: None,
            permission_denials: 0,
        }));
    }
    if let Ok(envelope) = serde_json::from_value::<ClaudeCliEnvelope>(value.clone()) {
        if let Some(result) = envelope.structured_output {
            return Ok(Some(ParsedClaudeFixResponse {
                result,
                duration_ms: envelope.duration_ms,
                session_id: envelope.session_id,
                permission_denials: envelope.permission_denials.len(),
            }));
        }
    }
    if let Some(result_value) = value.get("result") {
        if let Some(as_str) = result_value.as_str() {
            if let Ok(result) = serde_json::from_str::<ClaudeFixResult>(as_str) {
                return Ok(Some(ParsedClaudeFixResponse {
                    result,
                    duration_ms: value.get("duration_ms").and_then(|v| v.as_u64()),
                    session_id: value
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .map(ToOwned::to_owned),
                    permission_denials: value
                        .get("permission_denials")
                        .and_then(|v| v.as_array())
                        .map(|v| v.len())
                        .unwrap_or(0),
                }));
            }
        }
        if let Ok(result) = serde_json::from_value::<ClaudeFixResult>(result_value.clone()) {
            return Ok(Some(ParsedClaudeFixResponse {
                result,
                duration_ms: value.get("duration_ms").and_then(|v| v.as_u64()),
                session_id: value
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .map(ToOwned::to_owned),
                permission_denials: value
                    .get("permission_denials")
                    .and_then(|v| v.as_array())
                    .map(|v| v.len())
                    .unwrap_or(0),
            }));
        }
    }
    if let Some(content) = value.get("content").and_then(|content| content.as_str()) {
        if let Ok(result) = serde_json::from_str::<ClaudeFixResult>(content) {
            return Ok(Some(ParsedClaudeFixResponse {
                result,
                duration_ms: value.get("duration_ms").and_then(|v| v.as_u64()),
                session_id: value
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .map(ToOwned::to_owned),
                permission_denials: value
                    .get("permission_denials")
                    .and_then(|v| v.as_array())
                    .map(|v| v.len())
                    .unwrap_or(0),
            }));
        }
    }
    Ok(None)
}

fn fallback_commit_message(id: u32) -> String {
    format!("Address AI review feedback for PR #{id}")
}

fn conflict_resolution_commit_message(source_branch: &str, destination_branch: &str) -> String {
    format!("Merge {destination_branch} into {source_branch}")
}

fn build_conflict_resolution_payload(
    source_branch: &str,
    destination_branch: &str,
    conflict_files: &[String],
    tips: Option<&str>,
) -> String {
    let mut lines = vec![
        "You are resolving Git merge conflicts in an existing repository.".to_string(),
        format!(
            "The repository is currently merging `{destination_branch}` into `{source_branch}`."
        ),
        "Resolve every merge conflict cleanly, preserving the intended behavior from both branches when appropriate.".to_string(),
        "Remove all conflict markers, update the conflicted files, and stage the resolved files with git add.".to_string(),
        "Do not create the commit yourself; stop once the merge is fully resolved and staged.".to_string(),
        "".to_string(),
        "Conflicted files:".to_string(),
    ];
    for file in conflict_files {
        lines.push(format!("- {file}"));
    }
    if let Some(tips) = tips.filter(|value| !value.trim().is_empty()) {
        lines.push("".to_string());
        lines.push("Additional instructions from the reviewer:".to_string());
        lines.push(tips.trim().to_string());
    }
    lines.join("\n")
}

fn run_fix_pipeline(
    store: AiReviewFixStore,
    key: String,
    workspace: String,
    repo: String,
    id: u32,
    payload: String,
    source_branch: String,
    destination_branch: String,
) -> Result<(), String> {
    let repo_path = resolve_local_repo(&workspace, &repo)?;
    with_store(&store, |inner| {
        let session = ensure_session(inner, &key);
        session.public.repo_path = Some(repo_path.to_string_lossy().to_string());
        session.source_branch = source_branch.clone();
        session.destination_branch = destination_branch.clone();
    });
    append_log(
        &store,
        &key,
        format!("Using local clone at {}.", repo_path.display()),
    );

    let original_status = git_status_lines(&repo_path)?;
    let dirty_at_start = !original_status.is_empty();
    let stash_marker = format!("lachesi-ai-review-{id}-{}", now_ms());
    let mut stash_ref = None::<String>;

    if dirty_at_start {
        set_phase(
            &store,
            &key,
            AiReviewFixPhase::Stashing,
            AiReviewFixStatus::Running,
        );
        append_log(
            &store,
            &key,
            "Working tree is dirty; stashing local changes before syncing the branch.",
        );
        run_git_checked(&repo_path, &["stash", "push", "-u", "-m", &stash_marker])?;
        stash_ref = find_stash_ref(&repo_path, &stash_marker)?;
    }

    set_phase(
        &store,
        &key,
        AiReviewFixPhase::SwitchingBranch,
        AiReviewFixStatus::Running,
    );
    append_log(
        &store,
        &key,
        format!("Checking out source branch `{source_branch}`."),
    );
    resolve_branch(&repo_path, &source_branch)?;

    set_phase(
        &store,
        &key,
        AiReviewFixPhase::Syncing,
        AiReviewFixStatus::Running,
    );
    append_log(&store, &key, "Fetching latest commits from origin.");
    run_git_checked(&repo_path, &["fetch", "origin", &source_branch])?;
    append_log(
        &store,
        &key,
        format!("Fast-forwarding `{source_branch}` with `origin/{source_branch}`."),
    );
    run_git_checked(&repo_path, &["pull", "--ff-only", "origin", &source_branch])?;

    if let Some(stash) = stash_ref.as_deref() {
        set_phase(
            &store,
            &key,
            AiReviewFixPhase::RestoringStash,
            AiReviewFixStatus::Running,
        );
        append_log(
            &store,
            &key,
            "Re-applying the stashed local changes before running Claude.",
        );
        restore_stash(&repo_path, stash)?;
    }

    let baseline_status = git_status_lines(&repo_path)?;
    let baseline_files = files_from_status(&baseline_status);
    with_store(&store, |inner| {
        let session = ensure_session(inner, &key);
        session.baseline_files = baseline_files.clone();
    });
    if !baseline_files.is_empty() {
        append_log(
            &store,
            &key,
            format!(
                "Local changes already existed in {} file(s) before Claude started.",
                baseline_files.len()
            ),
        );
    }

    set_phase(
        &store,
        &key,
        AiReviewFixPhase::RunningClaude,
        AiReviewFixStatus::Running,
    );
    let claude = run_claude_fix(&repo_path, &payload, &store, &key)?;
    if let Some(duration_ms) = claude.duration_ms {
        append_log(
            &store,
            &key,
            format!("Claude finished in {}.", human_duration(duration_ms)),
        );
    }
    if claude.permission_denials > 0 {
        append_log(
            &store,
            &key,
            format!(
                "Claude reported {} permission denial(s) during the run.",
                claude.permission_denials
            ),
        );
    }
    if let Some(session_id) = claude.session_id.as_deref() {
        append_log(&store, &key, format!("Claude session: {session_id}"));
    }
    if !claude.result.status.eq_ignore_ascii_case("success") {
        let message = claude
            .result
            .failure_reason
            .or(claude.result.summary)
            .unwrap_or_else(|| "Claude reported a failed fix run.".to_string());
        return Err(message);
    }

    set_phase(
        &store,
        &key,
        AiReviewFixPhase::VerifyingChanges,
        AiReviewFixStatus::Running,
    );
    let after_status = git_status_lines(&repo_path)?;
    if after_status == baseline_status {
        return Err(
            "Claude finished successfully but did not leave any new working tree changes."
                .to_string(),
        );
    }

    let touched_files = if claude.result.files_touched.is_empty() {
        files_from_status(&after_status)
    } else {
        claude.result.files_touched
    };
    let overlap: Vec<String> = touched_files
        .iter()
        .filter(|path| baseline_files.iter().any(|baseline| baseline == *path))
        .cloned()
        .collect();
    if !overlap.is_empty() {
        append_log(
            &store,
            &key,
            format!(
                "Warning: Claude touched files that already had local edits: {}",
                overlap.join(", ")
            ),
        );
    }

    let summary = claude
        .result
        .summary
        .unwrap_or_else(|| "Claude applied the requested fixes.".to_string());
    let commit_message = claude
        .result
        .commit_message
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback_commit_message(id));

    with_store(&store, |inner| {
        let session = ensure_session(inner, &key);
        session.public.status = AiReviewFixStatus::Succeeded;
        session.public.phase = AiReviewFixPhase::ReadyToCommit;
        session.public.finished_at = Some(now_ms());
        session.public.error = None;
        session.public.summary = Some(summary.clone());
        session.public.suggested_commit_message = Some(commit_message.clone());
        session.public.files_touched = touched_files.clone();
        session.public.tests = claude.result.tests;
        session.public.claude_duration_ms = claude.duration_ms;
        session.public.claude_session_id = claude.session_id.clone();
        session.public.commit_sha = None;
        session.public.logs.push("Claude finished successfully. Review the suggested commit message and commit when ready.".to_string());
        trim_logs(&mut session.public.logs);
    });
    finalize_active_key(&store, &key);
    Ok(())
}

fn run_commit_pipeline(
    store: AiReviewFixStore,
    key: String,
    message: String,
) -> Result<(), String> {
    let (repo_path_str, files_to_stage) = with_store(&store, |inner| {
        let session = inner
            .sessions
            .get_mut(&key)
            .ok_or_else(|| "No AI fix session exists for this pull request yet.".to_string())?;
        if session.public.commit_sha.is_some() {
            return Err("A commit was already created for this AI fix session.".to_string());
        }
        let repo_path =
            session.public.repo_path.clone().ok_or_else(|| {
                "The local repo path is missing for this AI fix session.".to_string()
            })?;
        session.public.suggested_commit_message = Some(message.clone());
        Ok((repo_path, session.public.files_touched.clone()))
    })?;
    let repo_path = PathBuf::from(repo_path_str);

    append_log(&store, &key, "Staging Claude's changes.");
    if files_to_stage.is_empty() {
        run_git_checked(&repo_path, &["add", "-A"])?;
    } else {
        let mut add = git_command(&repo_path);
        add.arg("add").arg("--");
        for path in &files_to_stage {
            add.arg(path);
        }
        let output = add
            .output()
            .map_err(|e| format!("failed to stage files for commit: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "git add failed with code {:?}\nstderr: {}\nstdout: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim(),
                String::from_utf8_lossy(&output.stdout).trim()
            ));
        }
    }

    if git_status_lines(&repo_path)?.is_empty() {
        return Err("There are no changes to commit.".to_string());
    }

    append_log(
        &store,
        &key,
        "Creating commit. Pre-commit hooks may be running…",
    );
    let mut commit = git_command(&repo_path);
    commit.arg("commit").arg("-m").arg(&message);
    let output = run_logged_command(commit, &store, &key, "git commit")?;
    if output.code != Some(0) {
        return Err(format!(
            "git commit failed with code {:?}\nstderr: {}\nstdout: {}",
            output.code,
            output.stderr.trim(),
            output.stdout.trim()
        ));
    }

    let sha = run_git_checked(&repo_path, &["rev-parse", "HEAD"])?;
    with_store(&store, |inner| {
        let session = ensure_session(inner, &key);
        session.public.status = AiReviewFixStatus::Succeeded;
        session.public.phase = AiReviewFixPhase::ReadyToPush;
        session.public.finished_at = Some(now_ms());
        session.public.error = None;
        session.public.commit_sha = Some(sha.clone());
        session.public.suggested_commit_message = Some(message.clone());
        session
            .public
            .logs
            .push(format!("Commit created successfully: {}", sha.trim()));
        trim_logs(&mut session.public.logs);
    });
    finalize_active_key(&store, &key);
    Ok(())
}

fn run_push_pipeline(store: AiReviewFixStore, key: String) -> Result<(), String> {
    let (repo_path_str, source_branch, commit_sha) = with_store(
        &store,
        |inner| -> Result<(String, String, String), String> {
            let session = inner
                .sessions
                .get(&key)
                .ok_or_else(|| "No AI fix session exists for this pull request yet.".to_string())?;
            let repo_path = session.public.repo_path.clone().ok_or_else(|| {
                "The local repo path is missing for this AI fix session.".to_string()
            })?;
            let commit_sha = session
                .public
                .commit_sha
                .clone()
                .ok_or_else(|| "Create a commit before pushing.".to_string())?;
            Ok((repo_path, session.source_branch.clone(), commit_sha))
        },
    )?;
    let repo_path = PathBuf::from(repo_path_str);

    append_log(
        &store,
        &key,
        "Pushing the branch. Pre-push hooks may be running…",
    );
    let has_upstream = run_git_capture(
        &repo_path,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )?
    .code
        == Some(0);
    let mut push = git_command(&repo_path);
    push.arg("push");
    if !has_upstream {
        push.arg("-u").arg("origin").arg(&source_branch);
    }
    let output = run_logged_command(push, &store, &key, "git push")?;
    if output.code != Some(0) {
        return Err(format!(
            "git push failed with code {:?}\nstderr: {}\nstdout: {}",
            output.code,
            output.stderr.trim(),
            output.stdout.trim()
        ));
    }

    with_store(&store, |inner| {
        let session = ensure_session(inner, &key);
        session.public.status = AiReviewFixStatus::Succeeded;
        session.public.phase = AiReviewFixPhase::Completed;
        session.public.finished_at = Some(now_ms());
        session.public.error = None;
        session.public.commit_sha = Some(commit_sha.clone());
        session.public.logs.push(format!(
            "Push completed successfully for commit {}.",
            commit_sha.trim()
        ));
        trim_logs(&mut session.public.logs);
    });
    finalize_active_key(&store, &key);
    Ok(())
}

fn sync_branch_pipeline(
    workspace: String,
    repo: String,
    id: u32,
    source_branch: String,
    destination_branch: String,
) -> Result<BranchSyncResult, String> {
    enum SyncOutcome {
        Success {
            summary: String,
            sync_commit_sha: Option<String>,
        },
        Conflict {
            summary: String,
            conflict_files: Vec<String>,
        },
    }

    let repo_path = resolve_local_repo(&workspace, &repo)?;
    let mut logs = Vec::new();
    logs.push(format!("Using local clone at {}.", repo_path.display()));

    let original_branch = run_git_checked(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    logs.push(format!(
        "Current branch before sync: `{}`.",
        original_branch
    ));

    let original_status = git_status_lines(&repo_path)?;
    let stash_marker = format!("lachesi-branch-sync-{id}-{}", now_ms());
    let mut stash_ref = None::<String>;
    if !original_status.is_empty() {
        logs.push(
            "Working tree is dirty; stashing local changes before syncing the branch.".to_string(),
        );
        run_git_checked(&repo_path, &["stash", "push", "-u", "-m", &stash_marker])?;
        stash_ref = find_stash_ref(&repo_path, &stash_marker)?;
    }

    let operation = (|| -> Result<SyncOutcome, String> {
        logs.push(format!("Checking out source branch `{source_branch}`."));
        resolve_branch(&repo_path, &source_branch)?;

        logs.push("Fetching latest commits from origin.".to_string());
        run_git_checked(
            &repo_path,
            &["fetch", "origin", &source_branch, &destination_branch],
        )?;

        logs.push(format!(
            "Fast-forwarding `{source_branch}` with `origin/{source_branch}`."
        ));
        run_git_checked(&repo_path, &["pull", "--ff-only", "origin", &source_branch])?;

        let before_head = run_git_checked(&repo_path, &["rev-parse", "HEAD"])?;
        logs.push(format!(
            "Merging `origin/{destination_branch}` into `{source_branch}`."
        ));
        let merge_output = run_git_capture(
            &repo_path,
            &[
                "merge",
                "--no-edit",
                &format!("origin/{destination_branch}"),
            ],
        )?;
        if merge_output.code != Some(0) {
            let conflict_files = unmerged_files(&repo_path)?;
            if !conflict_files.is_empty() {
                logs.push(format!(
                    "Merge reported conflicts in {} file(s): {}",
                    conflict_files.len(),
                    conflict_files.join(", ")
                ));
                let _ = run_git_capture(&repo_path, &["merge", "--abort"]);
                return Ok(SyncOutcome::Conflict {
                    summary: format!(
                        "`{source_branch}` cannot be synced automatically because merging `{destination_branch}` produces conflicts."
                    ),
                    conflict_files,
                });
            }
            let _ = run_git_capture(&repo_path, &["merge", "--abort"]);
            return Err(format!(
                "git merge failed with code {:?}\nstderr: {}\nstdout: {}",
                merge_output.code,
                merge_output.stderr.trim(),
                merge_output.stdout.trim()
            ));
        }
        if !merge_output.stdout.trim().is_empty() {
            logs.push(merge_output.stdout.trim().to_string());
        }
        if !merge_output.stderr.trim().is_empty() {
            logs.push(merge_output.stderr.trim().to_string());
        }

        let after_head = run_git_checked(&repo_path, &["rev-parse", "HEAD"])?;
        let sync_commit_sha = if before_head.trim() == after_head.trim() {
            None
        } else {
            Some(after_head.trim().to_string())
        };

        logs.push("Pushing synced branch to origin.".to_string());
        let has_upstream = run_git_capture(
            &repo_path,
            &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
        )?
        .code
            == Some(0);
        let push_output = if has_upstream {
            run_git_capture(&repo_path, &["push"])?
        } else {
            run_git_capture(&repo_path, &["push", "-u", "origin", &source_branch])?
        };
        if push_output.code != Some(0) {
            return Err(format!(
                "git push failed with code {:?}\nstderr: {}\nstdout: {}",
                push_output.code,
                push_output.stderr.trim(),
                push_output.stdout.trim()
            ));
        }
        if !push_output.stdout.trim().is_empty() {
            logs.push(push_output.stdout.trim().to_string());
        }
        if !push_output.stderr.trim().is_empty() {
            logs.push(push_output.stderr.trim().to_string());
        }

        let summary = if let Some(sha) = sync_commit_sha.as_deref() {
            format!(
                "Merged {} into {} and pushed the updated branch to origin ({}).",
                destination_branch, source_branch, sha
            )
        } else {
            format!(
                "Fetched the latest commits and pushed `{}`. The branch was already up to date with `{}`.",
                source_branch, destination_branch
            )
        };

        Ok(SyncOutcome::Success {
            summary,
            sync_commit_sha,
        })
    })();

    let mut warning = None::<String>;
    if original_branch.trim() != source_branch {
        match run_git_checked(&repo_path, &["checkout", original_branch.trim()]) {
            Ok(_) => logs.push(format!(
                "Checked out the original branch `{}`.",
                original_branch.trim()
            )),
            Err(error) => {
                warning = Some(format!(
                    "The branch sync finished, but restoring the original branch `{}` failed: {}",
                    original_branch.trim(),
                    error
                ));
            }
        }
    }
    if let Some(stash) = stash_ref.as_deref() {
        match restore_stash(&repo_path, stash) {
            Ok(_) => logs.push("Re-applied the previously stashed local changes.".to_string()),
            Err(error) => {
                let message = format!(
                    "The branch sync finished, but restoring the stashed local changes failed: {}",
                    error
                );
                warning = match warning {
                    Some(existing) => Some(format!("{existing}\n{message}")),
                    None => Some(message),
                };
            }
        }
    }

    match operation {
        Ok(SyncOutcome::Success {
            summary,
            sync_commit_sha,
        }) => Ok(BranchSyncResult {
            status: BranchSyncStatus::Success,
            repo_path: repo_path.to_string_lossy().to_string(),
            source_branch,
            destination_branch,
            summary,
            sync_commit_sha,
            warning,
            conflict_files: Vec::new(),
            logs,
        }),
        Ok(SyncOutcome::Conflict {
            summary,
            conflict_files,
        }) => Ok(BranchSyncResult {
            status: BranchSyncStatus::Conflict,
            repo_path: repo_path.to_string_lossy().to_string(),
            source_branch,
            destination_branch,
            summary,
            sync_commit_sha: None,
            warning,
            conflict_files,
            logs,
        }),
        Err(error) => {
            if let Some(warning) = warning {
                Err(format!("{error}\n\nWorkspace restore warning: {warning}"))
            } else {
                Err(error)
            }
        }
    }
}

fn run_conflict_resolution_pipeline(
    store: AiReviewFixStore,
    key: String,
    workspace: String,
    repo: String,
    id: u32,
    source_branch: String,
    destination_branch: String,
    tips: Option<String>,
) -> Result<(), String> {
    let repo_path = resolve_local_repo(&workspace, &repo)?;
    with_store(&store, |inner| {
        let session = ensure_session(inner, &key);
        session.public.repo_path = Some(repo_path.to_string_lossy().to_string());
        session.source_branch = source_branch.clone();
        session.destination_branch = destination_branch.clone();
    });
    append_log(
        &store,
        &key,
        format!("Using local clone at {}.", repo_path.display()),
    );

    let original_status = git_status_lines(&repo_path)?;
    let dirty_at_start = !original_status.is_empty();
    let stash_marker = format!("lachesi-conflict-resolution-{id}-{}", now_ms());
    let mut stash_ref = None::<String>;

    if dirty_at_start {
        set_phase(
            &store,
            &key,
            AiReviewFixPhase::Stashing,
            AiReviewFixStatus::Running,
        );
        append_log(
            &store,
            &key,
            "Working tree is dirty; stashing local changes before preparing the merge conflict resolution.",
        );
        run_git_checked(&repo_path, &["stash", "push", "-u", "-m", &stash_marker])?;
        stash_ref = find_stash_ref(&repo_path, &stash_marker)?;
    }

    set_phase(
        &store,
        &key,
        AiReviewFixPhase::SwitchingBranch,
        AiReviewFixStatus::Running,
    );
    append_log(
        &store,
        &key,
        format!("Checking out source branch `{source_branch}`."),
    );
    resolve_branch(&repo_path, &source_branch)?;

    set_phase(
        &store,
        &key,
        AiReviewFixPhase::Syncing,
        AiReviewFixStatus::Running,
    );
    append_log(&store, &key, "Fetching latest commits from origin.");
    run_git_checked(
        &repo_path,
        &["fetch", "origin", &source_branch, &destination_branch],
    )?;
    append_log(
        &store,
        &key,
        format!("Fast-forwarding `{source_branch}` with `origin/{source_branch}`."),
    );
    run_git_checked(&repo_path, &["pull", "--ff-only", "origin", &source_branch])?;

    if let Some(stash) = stash_ref.as_deref() {
        set_phase(
            &store,
            &key,
            AiReviewFixPhase::RestoringStash,
            AiReviewFixStatus::Running,
        );
        append_log(
            &store,
            &key,
            "Re-applying the stashed local changes before preparing the merge.",
        );
        restore_stash(&repo_path, stash)?;
    }

    set_phase(
        &store,
        &key,
        AiReviewFixPhase::MergingDestination,
        AiReviewFixStatus::Running,
    );
    append_log(
        &store,
        &key,
        format!(
            "Recreating the merge state with `origin/{destination_branch}` to resolve conflicts."
        ),
    );
    let merge_output = run_git_capture(
        &repo_path,
        &[
            "merge",
            "--no-commit",
            "--no-ff",
            &format!("origin/{destination_branch}"),
        ],
    )?;
    let conflict_files = unmerged_files(&repo_path)?;
    if merge_output.code != Some(0) && conflict_files.is_empty() {
        let _ = run_git_capture(&repo_path, &["merge", "--abort"]);
        return Err(format!(
            "git merge failed with code {:?}\nstderr: {}\nstdout: {}",
            merge_output.code,
            merge_output.stderr.trim(),
            merge_output.stdout.trim()
        ));
    }

    if conflict_files.is_empty() {
        let commit_message =
            conflict_resolution_commit_message(&source_branch, &destination_branch);
        with_store(&store, |inner| {
            let session = ensure_session(inner, &key);
            session.public.status = AiReviewFixStatus::Succeeded;
            session.public.phase = AiReviewFixPhase::ReadyToCommit;
            session.public.finished_at = Some(now_ms());
            session.public.error = None;
            session.public.summary = Some(format!(
                "The branch now merges cleanly. Review the merge commit message and commit when ready."
            ));
            session.public.suggested_commit_message = Some(commit_message);
            session.public.files_touched =
                files_from_status(&git_status_lines(&repo_path).unwrap_or_default());
            session.public.tests = Vec::new();
            session.public.commit_sha = None;
            session.public.logs.push(
                "The branch merged without remaining conflicts. Review the merge commit and commit when ready."
                    .to_string(),
            );
            trim_logs(&mut session.public.logs);
        });
        finalize_active_key(&store, &key);
        return Ok(());
    }

    append_log(
        &store,
        &key,
        format!(
            "Detected merge conflicts in {} file(s): {}",
            conflict_files.len(),
            conflict_files.join(", ")
        ),
    );

    set_phase(
        &store,
        &key,
        AiReviewFixPhase::ResolvingConflicts,
        AiReviewFixStatus::Running,
    );
    let payload = build_conflict_resolution_payload(
        &source_branch,
        &destination_branch,
        &conflict_files,
        tips.as_deref(),
    );
    let claude = run_claude_fix(&repo_path, &payload, &store, &key)?;
    if let Some(duration_ms) = claude.duration_ms {
        append_log(
            &store,
            &key,
            format!("Claude finished in {}.", human_duration(duration_ms)),
        );
    }
    if let Some(session_id) = claude.session_id.as_deref() {
        append_log(&store, &key, format!("Claude session: {session_id}"));
    }
    if !claude.result.status.eq_ignore_ascii_case("success") {
        let _ = run_git_capture(&repo_path, &["merge", "--abort"]);
        let message = claude
            .result
            .failure_reason
            .or(claude.result.summary)
            .unwrap_or_else(|| "Claude reported a failed conflict resolution run.".to_string());
        return Err(message);
    }

    set_phase(
        &store,
        &key,
        AiReviewFixPhase::VerifyingChanges,
        AiReviewFixStatus::Running,
    );
    let remaining_conflicts = unmerged_files(&repo_path)?;
    if !remaining_conflicts.is_empty() {
        return Err(format!(
            "Claude completed, but {} conflict file(s) are still unresolved: {}",
            remaining_conflicts.len(),
            remaining_conflicts.join(", ")
        ));
    }

    let after_status = git_status_lines(&repo_path)?;
    let touched_files = if claude.result.files_touched.is_empty() {
        files_from_status(&after_status)
    } else {
        claude.result.files_touched
    };
    let summary = claude.result.summary.unwrap_or_else(|| {
        format!(
            "Claude resolved the merge conflicts between `{source_branch}` and `{destination_branch}`."
        )
    });
    let commit_message = claude
        .result
        .commit_message
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| conflict_resolution_commit_message(&source_branch, &destination_branch));

    with_store(&store, |inner| {
        let session = ensure_session(inner, &key);
        session.public.status = AiReviewFixStatus::Succeeded;
        session.public.phase = AiReviewFixPhase::ReadyToCommit;
        session.public.finished_at = Some(now_ms());
        session.public.error = None;
        session.public.summary = Some(summary);
        session.public.suggested_commit_message = Some(commit_message);
        session.public.files_touched = touched_files;
        session.public.tests = claude.result.tests;
        session.public.claude_duration_ms = claude.duration_ms;
        session.public.claude_session_id = claude.session_id.clone();
        session.public.commit_sha = None;
        session.public.logs.push(
            "Claude resolved the merge conflicts. Review the merge commit message and commit when ready."
                .to_string(),
        );
        trim_logs(&mut session.public.logs);
    });
    finalize_active_key(&store, &key);
    Ok(())
}

fn run_inline_review_pipeline(
    store: AiReviewRunStore,
    key: String,
    run_id: u64,
    workspace: String,
    repo: String,
    id: u32,
    source_branch: String,
    destination_branch: String,
    thread_id: String,
    turn_kind: AiReviewTurnKind,
    started_at: String,
    snapshot_payload: String,
    payload: String,
    resume_session_id: Option<String>,
    fallback_payload: Option<String>,
    ai_provider: AiProvider,
    claude_model: Option<String>,
    claude_effort: Option<String>,
    codex_model: Option<String>,
    codex_effort: Option<String>,
    skip_analyzers: bool,
    review_profile: Option<String>,
    repo_path_override: Option<PathBuf>,
    review_provider_override: Option<ReviewProvider>,
) -> Result<(), ReviewPipelineFailure> {
    let provider_label = match ai_provider {
        AiProvider::Claude => "Claude",
        AiProvider::Codex => "Codex",
    };
    let assistant_evidence_source = match ai_provider {
        AiProvider::Claude => ReviewEvidenceSource::Claude,
        AiProvider::Codex => ReviewEvidenceSource::Codex,
    };
    let repo_path = repo_path_override.or_else(|| resolve_local_repo(&workspace, &repo).ok());
    let mut analyzer_artifacts = Vec::new();
    let mut effective_payload = payload.clone();
    let mut effective_fallback_payload = fallback_payload.clone();
    let mut effective_snapshot_payload = snapshot_payload.clone();

    if turn_kind == AiReviewTurnKind::Initial && !skip_analyzers {
        if let Some(repo_path) = repo_path.as_deref() {
            append_inline_review_log(&store, &key, run_id, "Running local evidence analyzers.");
            let results = match analyzer_specs_with_profile(repo_path, review_profile.as_deref()) {
                Ok(specs) if specs.is_empty() => {
                    append_inline_review_log(
                        &store,
                        &key,
                        run_id,
                        "No local analyzers configured or auto-detected.",
                    );
                    Vec::new()
                }
                Ok(specs) => {
                    let mut results = Vec::new();
                    for spec in specs {
                        if inline_review_cancel_requested(&store, &key, run_id) {
                            mark_inline_review_cancelled(&store, &key, run_id);
                            return Ok(());
                        }
                        append_inline_review_log(
                            &store,
                            &key,
                            run_id,
                            format!("Analyzer `{}` started: {}", spec.id, spec.command),
                        );
                        let result = run_analyzer_command(repo_path, &spec, &store, &key, run_id);
                        append_inline_review_log(
                            &store,
                            &key,
                            run_id,
                            format!(
                                "Analyzer `{}` {} in {}.",
                                result.spec.id,
                                analyzer_status_label(result.status),
                                human_duration(result.duration_ms)
                            ),
                        );
                        results.push(result);
                    }
                    results
                }
                Err(error) => {
                    append_inline_review_log(
                        &store,
                        &key,
                        run_id,
                        format!("Could not load local analyzer configuration: {error}"),
                    );
                    vec![AnalyzerRunResult {
                        spec: AnalyzerSpec {
                            id: "lachesi-config".to_string(),
                            title: ".lachesi.yaml".to_string(),
                            command: "load .lachesi.yaml".to_string(),
                            timeout_seconds: 0,
                            source: ReviewEvidenceSource::Other,
                            required: true,
                        },
                        status: AnalyzerStatus::Errored,
                        code: None,
                        duration_ms: 0,
                        stdout: String::new(),
                        stderr: String::new(),
                        error: Some(error),
                    }]
                }
            };
            let required_failures = results
                .iter()
                .filter(|result| result.spec.required && result.status != AnalyzerStatus::Passed)
                .map(|result| {
                    format!(
                        "`{}` {}",
                        result.spec.id,
                        analyzer_status_label(result.status)
                    )
                })
                .collect::<Vec<_>>();
            if !required_failures.is_empty() {
                return Err(ReviewPipelineFailure::analyzer(format!(
                    "Required analyzer failure: {}.",
                    required_failures.join(", ")
                )));
            }
            let evidence_section = analyzer_prompt_section(&results);
            if !evidence_section.is_empty() {
                effective_payload = format!("{payload}\n\n{evidence_section}");
                effective_snapshot_payload = format!("{snapshot_payload}\n\n{evidence_section}");
                if let Some(fallback_payload) = fallback_payload.as_deref() {
                    effective_fallback_payload =
                        Some(format!("{fallback_payload}\n\n{evidence_section}"));
                }
            }
            analyzer_artifacts = analyzer_evidence("pending-run", &results);
        } else {
            append_inline_review_log(
                &store,
                &key,
                run_id,
                "No local clone configured; skipping local evidence analyzers.",
            );
        }
    } else if skip_analyzers {
        append_inline_review_log(
            &store,
            &key,
            run_id,
            "Skipping local evidence analyzers for this review.",
        );
    }

    append_inline_review_log(
        &store,
        &key,
        run_id,
        format!("AI provider: {provider_label}"),
    );
    if let Some(profile) = review_profile
        .as_deref()
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
    {
        append_inline_review_log(&store, &key, run_id, format!("Review profile: {profile}"));
    }
    match ai_provider {
        AiProvider::Claude => {
            if let Some(model) = claude_model.as_deref().and_then(normalize_claude_model) {
                append_inline_review_log(&store, &key, run_id, format!("Claude model: {model}"));
            }
            if let Some(effort) = claude_effort.as_deref().and_then(normalize_claude_effort) {
                append_inline_review_log(&store, &key, run_id, format!("Claude effort: {effort}"));
            }
        }
        AiProvider::Codex => {
            if let Some(model) = codex_model.as_deref().and_then(normalize_codex_model) {
                append_inline_review_log(&store, &key, run_id, format!("Codex model: {model}"));
            }
            if let Some(effort) = codex_effort.as_deref().and_then(normalize_codex_effort) {
                append_inline_review_log(&store, &key, run_id, format!("Codex effort: {effort}"));
            }
        }
    }

    let attempt_claude = |prompt: &str,
                          resume_id: Option<&str>|
     -> Result<Option<ParsedClaudeTextResponse>, String> {
        let (mut command, _temp_dir) = build_claude_text_command(
            repo_path.as_deref(),
            prompt,
            resume_id,
            claude_model.as_deref(),
            claude_effort.as_deref(),
        )?;
        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to run claude: {e}"))?;
        let pid = child.id();
        set_inline_review_process_pid(&store, &key, run_id, pid, "Claude");
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "claude stdout was not captured".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "claude stderr was not captured".to_string())?;
        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        let stdout_thread = read_inline_review_stream(
            stdout,
            store.clone(),
            key.clone(),
            run_id,
            "stdout",
            stdout_buf.clone(),
        );
        let stderr_thread = read_inline_review_stream(
            stderr,
            store.clone(),
            key.clone(),
            run_id,
            "stderr",
            stderr_buf.clone(),
        );
        if inline_review_cancel_requested(&store, &key, run_id) {
            let _ = kill_process(pid);
        }

        let status = child
            .wait()
            .map_err(|e| format!("Failed while waiting for claude: {e}"))?;
        let _ = stdout_thread.join();
        let _ = stderr_thread.join();
        clear_inline_review_pid(&store, &key, run_id);

        let output = CommandOutput {
            code: status.code(),
            stdout: stdout_buf.lock().map(|buf| buf.clone()).unwrap_or_default(),
            stderr: stderr_buf.lock().map(|buf| buf.clone()).unwrap_or_default(),
        };

        if inline_review_cancel_requested(&store, &key, run_id) {
            mark_inline_review_cancelled(&store, &key, run_id);
            return Ok(None);
        }

        if output.code != Some(0) {
            return Err(format!(
                "claude exited with code {:?}.\nstderr: {stderr}\nstdout: {stdout}",
                output.code,
                stderr = output.stderr.trim(),
                stdout = output.stdout.trim(),
            ));
        }

        let parsed = parse_claude_text_result(&output.stdout)?;
        if inline_review_cancel_requested(&store, &key, run_id) {
            mark_inline_review_cancelled(&store, &key, run_id);
            return Ok(None);
        }
        Ok(Some(parsed))
    };

    let attempt_codex = |prompt: &str| -> Result<Option<ParsedClaudeTextResponse>, String> {
        let (mut command, _temp_dir, output_path) = build_codex_text_command(
            repo_path.as_deref(),
            prompt,
            codex_model.as_deref(),
            codex_effort.as_deref(),
        )?;
        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to run codex: {e}"))?;
        let pid = child.id();
        set_inline_review_process_pid(&store, &key, run_id, pid, "Codex");
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "codex stdout was not captured".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "codex stderr was not captured".to_string())?;
        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        let stdout_thread = read_codex_inline_review_stream(
            stdout,
            store.clone(),
            key.clone(),
            run_id,
            "stdout",
            stdout_buf.clone(),
        );
        let stderr_thread = read_codex_inline_review_stream(
            stderr,
            store.clone(),
            key.clone(),
            run_id,
            "stderr",
            stderr_buf.clone(),
        );
        if inline_review_cancel_requested(&store, &key, run_id) {
            let _ = kill_process(pid);
        }

        let started = Instant::now();
        let status = child
            .wait()
            .map_err(|e| format!("Failed while waiting for codex: {e}"))?;
        let duration_ms = started.elapsed().as_millis() as u64;
        let _ = stdout_thread.join();
        let _ = stderr_thread.join();
        clear_inline_review_pid(&store, &key, run_id);

        let output = CommandOutput {
            code: status.code(),
            stdout: stdout_buf.lock().map(|buf| buf.clone()).unwrap_or_default(),
            stderr: stderr_buf.lock().map(|buf| buf.clone()).unwrap_or_default(),
        };

        let file_content = fs::read_to_string(&output_path).unwrap_or_default();

        if inline_review_cancel_requested(&store, &key, run_id) {
            mark_inline_review_cancelled(&store, &key, run_id);
            return Ok(None);
        }

        if output.code != Some(0) {
            return Err(format!(
                "codex exited with code {:?}.\nstderr: {stderr}\nstdout: {stdout}",
                output.code,
                stderr = output.stderr.trim(),
                stdout = output.stdout.trim(),
            ));
        }

        let content = if file_content.trim().is_empty() {
            output.stdout.trim().to_string()
        } else {
            file_content.trim().to_string()
        };
        if content.trim().is_empty() {
            return Err("Codex returned an empty review response.".to_string());
        }
        Ok(Some(ParsedClaudeTextResponse {
            content,
            duration_ms: Some(duration_ms),
            session_id: None,
            permission_denials: 0,
        }))
    };

    let response = (|| -> Result<Option<ParsedClaudeTextResponse>, String> {
        match ai_provider {
            AiProvider::Claude => {
                if let Some(resume_session_id) = resume_session_id.as_deref() {
                    match attempt_claude(&effective_payload, Some(resume_session_id)) {
                        Ok(Some(response)) => Ok(Some(response)),
                        Ok(None) => Ok(None),
                        Err(error) => {
                            let fallback_payload =
                                effective_fallback_payload.as_deref().ok_or(error.clone())?;
                            append_inline_review_log(
                                &store,
                                &key,
                                run_id,
                                format!(
                                    "Could not resume Claude session {}; retrying with a fresh session.",
                                    resume_session_id
                                ),
                            );
                            append_inline_review_log(
                                &store,
                                &key,
                                run_id,
                                format!("Resume error: {error}"),
                            );
                            attempt_claude(fallback_payload, None)
                        }
                    }
                } else {
                    attempt_claude(&effective_payload, None)
                }
            }
            AiProvider::Codex => attempt_codex(&effective_payload),
        }
    })()
    .map_err(ReviewPipelineFailure::provider)?;
    let Some(response) = response else {
        return Ok(());
    };

    if response.permission_denials > 0 {
        append_inline_review_log(
            &store,
            &key,
            run_id,
            format!(
                "{provider_label} reported {} permission denial(s) while producing the review.",
                response.permission_denials
            ),
        );
    }
    if let Some(duration_ms) = response.duration_ms {
        append_inline_review_log(
            &store,
            &key,
            run_id,
            format!(
                "{provider_label} finished in {}.",
                human_duration(duration_ms)
            ),
        );
    }
    if ai_provider == AiProvider::Claude {
        if let Some(session_id) = response.session_id.as_deref() {
            append_inline_review_log(
                &store,
                &key,
                run_id,
                format!("Claude session: {session_id}"),
            );
        }
    } else if response.session_id.is_some() {
        append_inline_review_log(
            &store,
            &key,
            run_id,
            format!("{provider_label} session captured."),
        );
    }

    let generated_at = now_ms();
    let mut review_store = load_review_store(&workspace, &repo, id)
        .map_err(ReviewPipelineFailure::internal)?
        .ok_or_else(|| {
            ReviewPipelineFailure::internal("The AI review store could not be loaded.")
        })?;
    let review_run = materialize_review_run(
        &workspace,
        &repo,
        review_provider_override,
        id,
        &source_branch,
        &destination_branch,
        &thread_id,
        turn_kind,
        review_profile.as_deref(),
        &started_at,
        &generated_at,
        &effective_snapshot_payload,
        response.content.trim(),
        assistant_evidence_source,
        analyzer_artifacts,
    )
    .map_err(ReviewPipelineFailure::provider)?;
    let thread = find_review_thread_mut(&mut review_store, &thread_id)
        .map_err(ReviewPipelineFailure::internal)?;
    append_review_message(
        thread,
        AiReviewMessageRole::Assistant,
        response.content.trim().to_string(),
        generated_at.clone(),
    );
    if ai_provider == AiProvider::Claude && response.session_id.is_some() {
        thread.claude_session_id = response.session_id.clone();
    }
    review_store.active_thread_id = Some(thread_id);
    review_store.review_runs.push(review_run);
    save_review_store(&workspace, &repo, id, &review_store)
        .map_err(ReviewPipelineFailure::internal)?;

    finish_inline_review_success(&store, &key, run_id, generated_at, provider_label);
    Ok(())
}

#[tauri::command]
pub fn get_ai_review_run_state(
    store: tauri::State<'_, AiReviewRunStore>,
    workspace: String,
    repo: String,
    id: u32,
) -> Option<AiReviewRunState> {
    clone_inline_review_state(store.inner(), &pr_key(&workspace, &repo, id))
}

#[tauri::command]
pub fn load_ai_review_store(
    workspace: String,
    repo: String,
    id: u32,
) -> Result<Option<AiReviewStoreData>, String> {
    load_ai_review_store_native(&workspace, &repo, id)
}

pub fn load_ai_review_store_native(
    workspace: &str,
    repo: &str,
    id: u32,
) -> Result<Option<AiReviewStoreData>, String> {
    load_review_store(workspace, repo, id)
}

#[allow(clippy::too_many_arguments)]
pub fn start_inline_review_native(
    store: AiReviewRunStore,
    workspace: String,
    repo: String,
    id: u32,
    title: String,
    source_branch: String,
    destination_branch: String,
    payload: String,
    display_message: Option<String>,
    review_kind: Option<String>,
    thread_title: Option<String>,
    skip_analyzers: bool,
    ai_provider: AiProvider,
    claude_model: Option<String>,
    claude_effort: Option<String>,
    codex_model: Option<String>,
    codex_effort: Option<String>,
    review_profile: Option<String>,
) -> Result<AiReviewRunState, String> {
    let key = pr_key(&workspace, &repo, id);
    let thread_id = now_id("thread");
    let (initial, run_id) = begin_inline_review_run(
        &store,
        &key,
        title,
        thread_id.clone(),
        AiReviewTurnKind::Initial,
        review_kind.as_deref(),
    )?;
    let started_at = initial.started_at.clone().unwrap_or_else(now_ms);
    let created_at = now_ms();
    let mut review_store = load_review_store(&workspace, &repo, id)?.unwrap_or_default();
    review_store.active_thread_id = Some(thread_id.clone());
    let mut thread = AiReviewThread {
        id: thread_id.clone(),
        title: thread_title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(review_thread_title),
        created_at: created_at.clone(),
        updated_at: created_at.clone(),
        claude_session_id: None,
        messages: Vec::new(),
    };
    if let Some(message) = display_message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        append_review_message(
            &mut thread,
            AiReviewMessageRole::User,
            message.to_string(),
            created_at,
        );
    }
    review_store.threads.push(thread);
    if let Err(error) = save_review_store(&workspace, &repo, id, &review_store) {
        set_inline_review_failed(&store, &key, run_id, error.clone());
        return Err(error);
    }
    let store_clone = store.clone();
    std::thread::spawn(move || {
        if let Err(error) = run_inline_review_pipeline(
            store_clone.clone(),
            key.clone(),
            run_id,
            workspace,
            repo,
            id,
            source_branch,
            destination_branch,
            thread_id,
            AiReviewTurnKind::Initial,
            started_at,
            payload.clone(),
            payload,
            None,
            None,
            ai_provider,
            claude_model,
            claude_effort,
            codex_model,
            codex_effort,
            skip_analyzers,
            review_profile,
            None,
            None,
        ) {
            set_inline_review_failed(&store_clone, &key, run_id, error.to_string());
        }
    });
    Ok(initial)
}

pub fn run_headless_review_native(
    request: HeadlessNativeReviewRequest,
) -> Result<ReviewRun, HeadlessNativeReviewError> {
    let HeadlessNativeReviewRequest {
        repo_path,
        review_provider,
        workspace,
        repo,
        pr_id,
        title,
        source_branch,
        destination_branch,
        payload,
        ai_provider,
        claude_model,
        claude_effort,
        codex_model,
        codex_effort,
        review_profile,
        run_analyzers,
    } = request;
    let store = AiReviewRunStore::default();
    let key = pr_key(&workspace, &repo, pr_id);
    let thread_id = now_id("thread");
    let (initial, run_id) = begin_inline_review_run(
        &store,
        &key,
        title,
        thread_id.clone(),
        AiReviewTurnKind::Initial,
        Some("headless"),
    )
    .map_err(HeadlessNativeReviewError::Internal)?;
    let started_at = initial.started_at.clone().unwrap_or_else(now_ms);
    let created_at = now_ms();
    let mut review_store = load_review_store(&workspace, &repo, pr_id)
        .map_err(HeadlessNativeReviewError::Internal)?
        .unwrap_or_default();
    review_store.active_thread_id = Some(thread_id.clone());
    review_store.threads.push(AiReviewThread {
        id: thread_id.clone(),
        title: "Headless review".to_string(),
        created_at: created_at.clone(),
        updated_at: created_at,
        claude_session_id: None,
        messages: Vec::new(),
    });
    save_review_store(&workspace, &repo, pr_id, &review_store)
        .map_err(HeadlessNativeReviewError::Internal)?;

    if let Err(error) = run_inline_review_pipeline(
        store.clone(),
        key.clone(),
        run_id,
        workspace.clone(),
        repo.clone(),
        pr_id,
        source_branch,
        destination_branch,
        thread_id.clone(),
        AiReviewTurnKind::Initial,
        started_at,
        payload.clone(),
        payload,
        None,
        None,
        ai_provider,
        claude_model,
        claude_effort,
        codex_model,
        codex_effort,
        !run_analyzers,
        review_profile,
        Some(repo_path),
        Some(review_provider),
    ) {
        let message = error.to_string();
        set_inline_review_failed(&store, &key, run_id, message.clone());
        return Err(match error.kind {
            ReviewPipelineFailureKind::Analyzer => HeadlessNativeReviewError::Analyzer(message),
            ReviewPipelineFailureKind::Provider => HeadlessNativeReviewError::Provider(message),
            ReviewPipelineFailureKind::Internal => HeadlessNativeReviewError::Internal(message),
        });
    }

    let state =
        get_ai_review_run_state_native(&store, &workspace, &repo, pr_id).ok_or_else(|| {
            HeadlessNativeReviewError::Internal(
                "Headless review finished without a run state.".to_string(),
            )
        })?;
    if state.status == AiReviewRunStatus::Cancelled {
        return Err(HeadlessNativeReviewError::Cancelled);
    }
    if state.status != AiReviewRunStatus::Succeeded {
        return Err(HeadlessNativeReviewError::Internal(
            state
                .error
                .unwrap_or_else(|| "Headless review did not complete successfully.".to_string()),
        ));
    }
    let review_store = load_review_store(&workspace, &repo, pr_id)
        .map_err(HeadlessNativeReviewError::Internal)?
        .ok_or_else(|| {
            HeadlessNativeReviewError::Internal(
                "Headless review finished without a saved result.".to_string(),
            )
        })?;
    review_store
        .review_runs
        .into_iter()
        .rev()
        .find(|run| run.thread_id.as_deref() == Some(thread_id.as_str()))
        .ok_or_else(|| {
            HeadlessNativeReviewError::Internal(
                "Headless review result could not be found.".to_string(),
            )
        })
}

#[tauri::command]
pub fn create_ai_review_thread(
    workspace: String,
    repo: String,
    id: u32,
    title: Option<String>,
    initial_message: Option<String>,
) -> Result<AiReviewStoreData, String> {
    let now = now_ms();
    let thread_id = now_id("thread");
    let mut thread = AiReviewThread {
        id: thread_id.clone(),
        title: title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Ask")
            .to_string(),
        created_at: now.clone(),
        updated_at: now.clone(),
        claude_session_id: None,
        messages: Vec::new(),
    };
    if let Some(message) = initial_message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        append_review_message(
            &mut thread,
            AiReviewMessageRole::User,
            message.to_string(),
            now,
        );
    }

    let mut store = load_review_store(&workspace, &repo, id)?.unwrap_or_default();
    store.active_thread_id = Some(thread_id);
    store.threads.push(thread);
    save_review_store(&workspace, &repo, id, &store)?;
    Ok(store)
}

#[tauri::command]
pub fn set_active_ai_review_thread(
    workspace: String,
    repo: String,
    id: u32,
    thread_id: String,
) -> Result<AiReviewStoreData, String> {
    let mut store = load_review_store(&workspace, &repo, id)?
        .ok_or_else(|| "No saved AI review exists for this pull request yet.".to_string())?;
    let _ = find_review_thread(&store, &thread_id)?;
    store.active_thread_id = Some(thread_id);
    save_review_store(&workspace, &repo, id, &store)?;
    Ok(store)
}

#[tauri::command]
pub fn delete_ai_review_thread(
    workspace: String,
    repo: String,
    id: u32,
    thread_id: String,
) -> Result<Option<AiReviewStoreData>, String> {
    let mut store = load_review_store(&workspace, &repo, id)?
        .ok_or_else(|| "No saved AI review exists for this pull request yet.".to_string())?;
    let original_len = store.threads.len();
    store.threads.retain(|thread| thread.id != thread_id);
    if store.threads.len() == original_len {
        return Err(format!("Unknown AI review thread: {thread_id}"));
    }
    store
        .review_runs
        .retain(|run| run.thread_id.as_deref() != Some(thread_id.as_str()));
    normalize_review_store(&mut store);
    if store.threads.is_empty() {
        let _ = review_storage::delete_review(&workspace, &repo, id);
        return Ok(None);
    }
    save_review_store(&workspace, &repo, id, &store)?;
    Ok(Some(store))
}

#[tauri::command]
pub fn record_ai_review_finding_publication(
    workspace: String,
    repo: String,
    id: u32,
    events: Vec<ReviewFindingPublicationEvent>,
) -> Result<Option<AiReviewStoreData>, String> {
    let Some(mut store) = load_review_store(&workspace, &repo, id)? else {
        return Ok(None);
    };
    if events.is_empty() {
        return Ok(Some(store));
    }
    if record_review_finding_publication_events(&mut store, &events) {
        save_review_store(&workspace, &repo, id, &store)?;
    }
    Ok(Some(store))
}

#[tauri::command]
pub async fn start_inline_review(
    store: tauri::State<'_, AiReviewRunStore>,
    workspace: String,
    repo: String,
    id: u32,
    title: String,
    source_branch: String,
    destination_branch: String,
    payload: String,
    display_message: Option<String>,
    review_kind: Option<String>,
    thread_title: Option<String>,
    skip_analyzers: Option<bool>,
    ai_provider: Option<AiProvider>,
    claude_model: Option<String>,
    claude_effort: Option<String>,
    codex_model: Option<String>,
    codex_effort: Option<String>,
    review_profile: Option<String>,
) -> Result<AiReviewRunState, String> {
    let ai_provider = ai_provider.unwrap_or_default();
    let skip_analyzers = resolve_gui_skip_analyzers(skip_analyzers);
    start_inline_review_native(
        store.inner().clone(),
        workspace,
        repo,
        id,
        title,
        source_branch,
        destination_branch,
        payload,
        display_message,
        review_kind,
        thread_title,
        skip_analyzers,
        ai_provider,
        claude_model,
        claude_effort,
        codex_model,
        codex_effort,
        review_profile,
    )
}

fn resolve_gui_skip_analyzers(_requested: Option<bool>) -> bool {
    true
}

#[tauri::command]
pub async fn reply_inline_review(
    store: tauri::State<'_, AiReviewRunStore>,
    workspace: String,
    repo: String,
    id: u32,
    title: String,
    source_branch: String,
    destination_branch: String,
    thread_id: String,
    user_message: String,
    base_payload: String,
    ai_provider: Option<AiProvider>,
    claude_model: Option<String>,
    claude_effort: Option<String>,
    codex_model: Option<String>,
    codex_effort: Option<String>,
) -> Result<AiReviewRunState, String> {
    let ai_provider = ai_provider.unwrap_or_default();
    let trimmed_message = user_message.trim();
    if trimmed_message.is_empty() {
        return Err("A reply message is required.".to_string());
    }
    let key = pr_key(&workspace, &repo, id);
    let mut review_store = load_review_store(&workspace, &repo, id)?
        .ok_or_else(|| "No saved AI review exists for this pull request yet.".to_string())?;
    let thread = find_review_thread(&review_store, &thread_id)?;
    let resume_session_id = if ai_provider == AiProvider::Claude {
        thread.claude_session_id.clone()
    } else {
        None
    };
    let fallback_payload = build_reply_fallback_payload(&base_payload, thread, trimmed_message);
    let (initial, run_id) = begin_inline_review_run(
        store.inner(),
        &key,
        title,
        thread_id.clone(),
        AiReviewTurnKind::Reply,
        None,
    )?;
    let started_at = initial.started_at.clone().unwrap_or_else(now_ms);
    let thread = find_review_thread_mut(&mut review_store, &thread_id)?;
    append_review_message(
        thread,
        AiReviewMessageRole::User,
        trimmed_message.to_string(),
        now_ms(),
    );
    review_store.active_thread_id = Some(thread_id.clone());
    if let Err(error) = save_review_store(&workspace, &repo, id, &review_store) {
        set_inline_review_failed(store.inner(), &key, run_id, error.clone());
        return Err(error);
    }

    let primary_payload = if resume_session_id.is_some() {
        trimmed_message.to_string()
    } else {
        fallback_payload.clone()
    };
    let resume_for_pipeline = resume_session_id.clone();
    let fallback_for_pipeline = resume_session_id.map(|_| fallback_payload);

    let store_clone = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = run_inline_review_pipeline(
            store_clone.clone(),
            key.clone(),
            run_id,
            workspace,
            repo,
            id,
            source_branch,
            destination_branch,
            thread_id,
            AiReviewTurnKind::Reply,
            started_at,
            base_payload.clone(),
            primary_payload,
            resume_for_pipeline,
            fallback_for_pipeline,
            ai_provider,
            claude_model,
            claude_effort,
            codex_model,
            codex_effort,
            false,
            None,
            None,
            None,
        ) {
            set_inline_review_failed(&store_clone, &key, run_id, error.to_string());
        }
    });
    Ok(initial)
}

#[tauri::command]
pub fn cancel_inline_review(
    store: tauri::State<'_, AiReviewRunStore>,
    workspace: String,
    repo: String,
    id: u32,
) -> Result<AiReviewRunState, String> {
    let key = pr_key(&workspace, &repo, id);
    let (state, run_id, pid) = with_review_run_store(store.inner(), |inner| {
        let session = inner
            .sessions
            .get_mut(&key)
            .ok_or_else(|| "No AI review is active for this pull request.".to_string())?;
        if session.public.status != AiReviewRunStatus::Running {
            return Err("There is no running AI review to cancel.".to_string());
        }
        session.cancel_requested = true;
        session.public.status = AiReviewRunStatus::Cancelled;
        session.public.finished_at = Some(now_ms());
        session.public.error = None;
        session
            .public
            .logs
            .push("Cancellation requested. Waiting for Claude to exit…".to_string());
        trim_logs(&mut session.public.logs);
        let run_id = session.run_id;
        let pid = session.child_pid;
        Ok((session.public.clone(), run_id, pid))
    })?;

    if let Some(pid) = pid {
        if let Err(error) = kill_process(pid) {
            set_inline_review_failed(store.inner(), &key, run_id, error.clone());
            return Err(error);
        }
    }

    Ok(state)
}

/// Run `claude --print` headlessly, capture stdout, persist to disk, return the review.
#[tauri::command]
pub async fn run_inline_review(
    workspace: String,
    repo: String,
    id: u32,
    payload: String,
) -> Result<SavedReview, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<SavedReview, String> {
        let ts = now_ms();
        let tmp_path = std::env::temp_dir().join(format!("lachesi-review-{ts}.md"));
        fs::write(&tmp_path, &payload).map_err(|e| e.to_string())?;

        let shell_cmd = format!(
            "export PATH=\"$HOME/.local/bin:$HOME/.npm/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; claude --print \"$(cat {})\"",
            shell_quote(&tmp_path.to_string_lossy())
        );
        let output = Command::new("/bin/zsh")
            .arg("-lc")
            .arg(&shell_cmd)
            .output()
            .map_err(|e| format!("Failed to run claude: {e}"))?;

        let _ = fs::remove_file(&tmp_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!(
                "claude exited with code {:?}.\nstderr: {stderr}\nstdout: {stdout}",
                output.status.code()
            ));
        }

        let content = String::from_utf8(output.stdout)
            .map_err(|e| format!("claude output is not valid UTF-8: {e}"))?;

        let review = SavedReview {
            content: content.trim().to_string(),
            generated_at: ts,
        };

        let json = serde_json::to_string(&review).map_err(|e| e.to_string())?;
        review_storage::save_review_json(&workspace, &repo, id, &json)?;

        Ok(review)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn draft_ai_review_comments(
    workspace: String,
    repo: String,
    _id: u32,
    payload: String,
) -> Result<Vec<AiReviewDraftCommentSuggestion>, String> {
    tauri::async_runtime::spawn_blocking(
        move || -> Result<Vec<AiReviewDraftCommentSuggestion>, String> {
            let repo_path = resolve_local_repo(&workspace, &repo).ok();
            let schema = r#"{"type":"object","properties":{"comments":{"type":"array","items":{"type":"object","properties":{"path":{"type":"string"},"to":{"type":["integer","null"]},"from":{"type":["integer","null"]},"raw":{"type":"string"}},"required":["path","to","from","raw"],"additionalProperties":false}}},"required":["comments"],"additionalProperties":false}"#;
            let result: AiReviewDraftCommentResult =
                run_claude_structured_output(repo_path.as_deref(), &payload, schema)?;
            Ok(result
                .comments
                .into_iter()
                .filter(|comment| {
                    !comment.path.trim().is_empty() && !comment.raw.trim().is_empty()
                })
                .collect())
        },
    )
    .await
    .map_err(|e| e.to_string())?
}

/// Load a previously saved review from disk; returns null if none exists.
#[tauri::command]
pub fn load_saved_review(workspace: String, repo: String, id: u32) -> Option<SavedReview> {
    let store = load_review_store(&workspace, &repo, id).ok()??;
    let thread = store.threads.last()?;
    let message = thread
        .messages
        .iter()
        .rev()
        .find(|message| message.role == AiReviewMessageRole::Assistant)?;
    Some(SavedReview {
        content: message.content.clone(),
        generated_at: message.created_at.clone(),
    })
}

/// Delete a saved review.
#[tauri::command]
pub fn delete_saved_review(workspace: String, repo: String, id: u32) {
    let _ = review_storage::delete_review(&workspace, &repo, id);
}

/// Remove every review whose key is NOT in `keep_keys`.
/// `keep_keys` contains strings of the form `{workspace}_{repo}_{id}`.
#[tauri::command]
pub fn cleanup_stale_reviews(keep_keys: Vec<String>) {
    let _ = review_storage::cleanup_stale_reviews(&keep_keys);
}

#[tauri::command]
pub fn create_ai_review_job(
    workspace: String,
    repo: String,
    pr_id: u32,
    pr_title: String,
    source_branch: String,
    destination_branch: String,
    trigger: String,
) -> Result<review_storage::ReviewJob, String> {
    review_storage::create_review_job(
        &workspace,
        &repo,
        pr_id,
        &pr_title,
        &source_branch,
        &destination_branch,
        &trigger,
    )
}

#[tauri::command]
pub fn update_ai_review_job_status(
    job_id: String,
    status: review_storage::ReviewJobStatus,
    thread_id: Option<String>,
    error: Option<String>,
) -> Result<review_storage::ReviewJob, String> {
    review_storage::update_review_job_status(
        &job_id,
        status,
        thread_id.as_deref(),
        error.as_deref(),
    )
}

#[tauri::command]
pub fn list_ai_review_jobs(limit: Option<u32>) -> Result<Vec<review_storage::ReviewJob>, String> {
    review_storage::list_recent_review_jobs(limit.unwrap_or(20).clamp(1, 100))
}

#[tauri::command]
pub fn get_ai_review_fix_state(
    store: tauri::State<'_, AiReviewFixStore>,
    workspace: String,
    repo: String,
    id: u32,
    thread_id: Option<String>,
) -> Option<AiReviewFixState> {
    clone_public_state(
        store.inner(),
        &fix_key(&workspace, &repo, id, thread_id.as_deref()),
    )
}

#[tauri::command]
pub async fn start_ai_review_fix(
    store: tauri::State<'_, AiReviewFixStore>,
    workspace: String,
    repo: String,
    id: u32,
    thread_id: Option<String>,
    payload: String,
    source_branch: String,
    destination_branch: String,
) -> Result<AiReviewFixState, String> {
    let pr_key = pr_key(&workspace, &repo, id);
    let key = fix_key(&workspace, &repo, id, thread_id.as_deref());
    let repo_path = resolve_local_repo(&workspace, &repo)
        .ok()
        .map(|path| path.to_string_lossy().to_string());
    let initial = begin_operation(
        store.inner(),
        &key,
        &pr_key,
        thread_id.clone(),
        AiReviewFixPhase::Preflight,
        repo_path,
    )?;
    append_log(
        store.inner(),
        &key,
        "Preparing to address the active AI review thread.",
    );
    let store_clone = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = run_fix_pipeline(
            store_clone.clone(),
            key.clone(),
            workspace,
            repo,
            id,
            payload,
            source_branch,
            destination_branch,
        ) {
            set_fix_failed(&store_clone, &key, error);
        }
    });
    Ok(initial)
}

#[tauri::command]
pub async fn start_ai_review_commit(
    store: tauri::State<'_, AiReviewFixStore>,
    workspace: String,
    repo: String,
    id: u32,
    thread_id: Option<String>,
    message: String,
) -> Result<AiReviewFixState, String> {
    if message.trim().is_empty() {
        return Err("A commit message is required.".to_string());
    }
    let key = fix_key(&workspace, &repo, id, thread_id.as_deref());
    let initial = begin_session_step(
        store.inner(),
        &key,
        AiReviewFixPhase::Committing,
        |session| {
            if session.public.status != AiReviewFixStatus::Succeeded
                || session.public.phase != AiReviewFixPhase::ReadyToCommit
            {
                return Err(
                    "The AI fix must complete successfully before you can commit it.".to_string(),
                );
            }
            session.public.suggested_commit_message = Some(message.clone());
            Ok(())
        },
    )?;
    append_log(
        store.inner(),
        &key,
        "Commit requested from the AI review panel.",
    );
    let store_clone = store.inner().clone();
    let message = message.trim().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = run_commit_pipeline(store_clone.clone(), key.clone(), message) {
            set_fix_failed(&store_clone, &key, error);
        }
    });
    Ok(initial)
}

#[tauri::command]
pub async fn start_ai_review_push(
    store: tauri::State<'_, AiReviewFixStore>,
    workspace: String,
    repo: String,
    id: u32,
    thread_id: Option<String>,
) -> Result<AiReviewFixState, String> {
    let key = fix_key(&workspace, &repo, id, thread_id.as_deref());
    let initial = begin_session_step(store.inner(), &key, AiReviewFixPhase::Pushing, |session| {
        if session.public.commit_sha.is_none()
            || session.public.phase != AiReviewFixPhase::ReadyToPush
        {
            return Err("Create a successful commit before pushing it.".to_string());
        }
        Ok(())
    })?;
    append_log(
        store.inner(),
        &key,
        "Push requested from the AI review panel.",
    );
    let store_clone = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = run_push_pipeline(store_clone.clone(), key.clone()) {
            set_fix_failed(&store_clone, &key, error);
        }
    });
    Ok(initial)
}

#[tauri::command]
pub async fn start_ai_conflict_resolution(
    store: tauri::State<'_, AiReviewFixStore>,
    workspace: String,
    repo: String,
    id: u32,
    thread_id: Option<String>,
    source_branch: String,
    destination_branch: String,
    tips: Option<String>,
) -> Result<AiReviewFixState, String> {
    let pr_key = pr_key(&workspace, &repo, id);
    let key = fix_key(&workspace, &repo, id, thread_id.as_deref());
    let repo_path = resolve_local_repo(&workspace, &repo)
        .ok()
        .map(|path| path.to_string_lossy().to_string());
    let initial = begin_operation(
        store.inner(),
        &key,
        &pr_key,
        thread_id,
        AiReviewFixPhase::Preflight,
        repo_path,
    )?;
    append_log(
        store.inner(),
        &key,
        format!(
            "Preparing to resolve merge conflicts between `{}` and `{}` with Claude.",
            source_branch, destination_branch
        ),
    );
    if let Some(tips) = tips.as_deref().filter(|value| !value.trim().is_empty()) {
        append_log(
            store.inner(),
            &key,
            format!("Applying reviewer tips: {}", tips.trim()),
        );
    }
    let store_clone = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = run_conflict_resolution_pipeline(
            store_clone.clone(),
            key.clone(),
            workspace,
            repo,
            id,
            source_branch,
            destination_branch,
            tips,
        ) {
            set_fix_failed(&store_clone, &key, error);
        }
    });
    Ok(initial)
}

#[tauri::command]
pub async fn sync_pr_branch(
    workspace: String,
    repo: String,
    id: u32,
    source_branch: String,
    destination_branch: String,
) -> Result<BranchSyncResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        sync_branch_pipeline(workspace, repo, id, source_branch, destination_branch)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn reset_ai_review_fix_state(
    store: tauri::State<'_, AiReviewFixStore>,
    workspace: String,
    repo: String,
    id: u32,
    thread_id: Option<String>,
) -> Result<(), String> {
    let key = fix_key(&workspace, &repo, id, thread_id.as_deref());
    with_store(store.inner(), |inner| {
        if inner.active_key.as_deref() == Some(&key) {
            return Err(
                "Cannot reset the AI fix session while an operation is running.".to_string(),
            );
        }
        inner.sessions.remove(&key);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::{
        apply_review_finding_publication_event, begin_inline_review_run, build_claude_text_command,
        build_codex_text_command, extract_review_findings, format_claude_stream_log_line,
        get_ai_review_run_state_native, human_duration, materialize_review_run,
        normalize_codex_effort, normalize_codex_model, parse_claude_fix_result,
        parse_claude_structured_json, parse_claude_text_result, parse_review_resources,
        resolve_gui_skip_analyzers, review_findings_from_output, trim_evidence_output,
        AiReviewDraftCommentResult, AiReviewRunStatus, AiReviewRunStore, AiReviewTurnKind,
        ReviewEvidenceArtifact, ReviewEvidenceKind, ReviewEvidenceSource, ReviewFindingCategory,
        ReviewFindingConfidence, ReviewFindingPublicationEvent, ReviewFindingPublicationEventKind,
        ReviewFindingSeverity, ReviewProvider, ReviewPublicationMode,
        STRUCTURED_REVIEW_SCHEMA_VERSION,
    };

    #[test]
    fn parses_structured_output_from_claude_json_envelope() {
        let stdout = r#"{
            "type":"result",
            "subtype":"success",
            "duration_ms":355960,
            "session_id":"abc-session",
            "permission_denials":[{"tool_name":"Edit"}],
            "result":"Human readable summary",
            "structured_output":{
                "status":"success",
                "summary":"Applied fixes.",
                "commitMessage":"fix: apply ai review feedback",
                "tests":["pnpm test"],
                "filesTouched":["src/file.ts"]
            }
        }"#;

        let parsed = parse_claude_fix_result(stdout).expect("expected structured output to parse");
        assert_eq!(parsed.result.status, "success");
        assert_eq!(
            parsed.result.commit_message.as_deref(),
            Some("fix: apply ai review feedback")
        );
        assert_eq!(parsed.duration_ms, Some(355960));
        assert_eq!(parsed.session_id.as_deref(), Some("abc-session"));
        assert_eq!(parsed.permission_denials, 1);
    }

    #[test]
    fn parses_text_output_from_claude_json_envelope() {
        let stdout = r###"{
            "type":"result",
            "subtype":"success",
            "duration_ms":1420,
            "session_id":"review-session",
            "result":"## Review\n\n- Looks correct."
        }"###;

        let parsed = parse_claude_text_result(stdout).expect("expected text output to parse");
        assert_eq!(parsed.content, "## Review\n\n- Looks correct.");
        assert_eq!(parsed.duration_ms, Some(1420));
        assert_eq!(parsed.session_id.as_deref(), Some("review-session"));
    }

    #[test]
    fn normalizes_codex_model_and_effort_settings() {
        assert_eq!(
            normalize_codex_model("gpt-5-codex").as_deref(),
            Some("gpt-5-codex")
        );
        assert_eq!(normalize_codex_model("gpt 5"), None);
        assert_eq!(normalize_codex_effort("low"), Some("low"));
        assert_eq!(normalize_codex_effort("medium"), Some("medium"));
        assert_eq!(normalize_codex_effort("high"), Some("high"));
        assert_eq!(normalize_codex_effort("max"), None);
    }

    #[test]
    fn builds_codex_review_command_with_model_effort_and_read_only_sandbox() {
        let (command, temp_dir, output_path) =
            build_codex_text_command(None, "review prompt", Some("gpt-5-codex"), Some("high"))
                .expect("codex command should build");
        let shell = command
            .get_args()
            .nth(1)
            .expect("zsh command should include shell payload")
            .to_string_lossy();

        assert!(shell.contains("codex exec"));
        assert!(shell.contains("--sandbox read-only"));
        assert!(!shell.contains("--ask-for-approval"));
        assert!(shell.contains("--output-last-message"));
        assert!(shell.contains("gpt-5-codex"));
        assert!(shell.contains("model_reasoning_effort=high"));
        assert!(shell.contains("--skip-git-repo-check"));
        assert!(command.get_envs().any(|(key, value)| {
            key == "LACHESI_REVIEW_CHILD"
                && value.is_some_and(|value| value.to_string_lossy() == "1")
        }));
        assert!(temp_dir.path().join("prompt.md").is_file());
        assert_eq!(output_path, temp_dir.path().join("output.md"));
        assert_private_temp_dir(&temp_dir);
    }

    #[test]
    fn builds_claude_review_command_with_read_only_tools() {
        let (command, temp_dir) =
            build_claude_text_command(None, "review prompt", None, None, None)
                .expect("claude command should build");
        let shell = command
            .get_args()
            .nth(1)
            .expect("zsh command should include shell payload")
            .to_string_lossy();

        assert!(shell.contains("--permission-mode plan"));
        assert!(shell.contains("--allowedTools Read,Glob,Grep"));
        assert!(shell.contains(" < "));
        assert!(!shell.contains("$(cat"));
        assert!(command.get_envs().any(|(key, value)| {
            key == "LACHESI_REVIEW_CHILD"
                && value.is_some_and(|value| value.to_string_lossy() == "1")
        }));
        assert!(temp_dir.path().join("prompt.md").is_file());
        assert_private_temp_dir(&temp_dir);
    }

    fn assert_private_temp_dir(temp_dir: &tempfile::TempDir) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = std::fs::metadata(temp_dir.path())
                .expect("private temp dir metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700);
        }
    }

    #[test]
    fn gui_review_always_skips_analyzers() {
        assert!(resolve_gui_skip_analyzers(None));
        assert!(resolve_gui_skip_analyzers(Some(true)));
        assert!(resolve_gui_skip_analyzers(Some(false)));
    }

    #[test]
    fn review_run_store_allows_parallel_reviews_for_different_pull_requests() {
        let store = AiReviewRunStore::default();

        let (first, first_run_id) = begin_inline_review_run(
            &store,
            "workspace/repo/1",
            "First".to_string(),
            "thread-1".to_string(),
            AiReviewTurnKind::Initial,
            None,
        )
        .expect("first review should start");
        let (second, second_run_id) = begin_inline_review_run(
            &store,
            "workspace/repo/2",
            "Second".to_string(),
            "thread-2".to_string(),
            AiReviewTurnKind::Initial,
            None,
        )
        .expect("second PR review should start concurrently");

        assert_eq!(first.status, AiReviewRunStatus::Running);
        assert_eq!(second.status, AiReviewRunStatus::Running);
        assert_ne!(first_run_id, second_run_id);
        assert_eq!(
            get_ai_review_run_state_native(&store, "workspace", "repo", 1)
                .expect("first state")
                .status,
            AiReviewRunStatus::Running
        );
        assert_eq!(
            get_ai_review_run_state_native(&store, "workspace", "repo", 2)
                .expect("second state")
                .status,
            AiReviewRunStatus::Running
        );
    }

    #[test]
    fn review_run_store_rejects_parallel_reviews_for_same_pull_request() {
        let store = AiReviewRunStore::default();

        begin_inline_review_run(
            &store,
            "workspace/repo/1",
            "First".to_string(),
            "thread-1".to_string(),
            AiReviewTurnKind::Initial,
            None,
        )
        .expect("first review should start");
        let error = begin_inline_review_run(
            &store,
            "workspace/repo/1",
            "Duplicate".to_string(),
            "thread-2".to_string(),
            AiReviewTurnKind::Initial,
            None,
        )
        .expect_err("same PR should still be locked");

        assert_eq!(
            error,
            "An AI review is already running for this pull request."
        );
    }

    #[test]
    fn formats_claude_tool_stream_events_as_readable_activity() {
        let stdout = r#"{
            "type":"assistant",
            "message":{
                "content":[
                    {
                        "type":"tool_use",
                        "name":"Read",
                        "input":{"file_path":"src/app.tsx","offset":10,"limit":50}
                    }
                ]
            }
        }"#;

        assert_eq!(
            format_claude_stream_log_line("stdout", stdout),
            vec!["Reading file: src/app.tsx"]
        );
    }

    #[test]
    fn suppresses_noisy_claude_stream_events() {
        assert!(format_claude_stream_log_line("stdout", r#"{"type":"stream_event"}"#).is_empty());
        assert!(format_claude_stream_log_line(
            "stdout",
            r#"{"type":"system","subtype":"thinking_tokens"}"#
        )
        .is_empty());
        assert!(format_claude_stream_log_line("stdout", r#"{"type":"user"}"#).is_empty());
    }

    #[test]
    fn parses_text_output_from_claude_stream_json() {
        let stdout = r###"
{"type":"system","subtype":"init","session_id":"review-session","model":"claude-sonnet-4"}
{"type":"assistant","message":{"content":[{"type":"text","text":"## Review\n\n- Draft"}]}}
{"type":"result","subtype":"success","duration_ms":3210,"session_id":"review-session","permission_denials":[{"tool_name":"Edit"}],"result":"## Review\n\n- Final"}"###;

        let parsed = parse_claude_text_result(stdout).expect("expected stream output to parse");
        assert_eq!(parsed.content, "## Review\n\n- Final");
        assert_eq!(parsed.duration_ms, Some(3210));
        assert_eq!(parsed.session_id.as_deref(), Some("review-session"));
        assert_eq!(parsed.permission_denials, 1);
    }

    #[test]
    fn parses_structured_draft_comments_from_claude_envelope() {
        let stdout = r###"{
            "type":"result",
            "subtype":"success",
            "structured_output":{
                "comments":[
                    {
                        "path":"src/file.ts",
                        "to":17,
                        "from":null,
                        "raw":"This branch should add a regression test for the new code path."
                    }
                ]
            }
        }"###;

        let parsed: AiReviewDraftCommentResult =
            parse_claude_structured_json(stdout).expect("expected structured draft comments");
        assert_eq!(parsed.comments.len(), 1);
        assert_eq!(parsed.comments[0].path, "src/file.ts");
        assert_eq!(parsed.comments[0].to, Some(17));
        assert_eq!(parsed.comments[0].from, None);
    }

    #[test]
    fn formats_human_duration() {
        assert_eq!(human_duration(900), "0s");
        assert_eq!(human_duration(12_000), "12s");
        assert_eq!(human_duration(355_960), "5m 55s");
    }

    #[test]
    fn truncates_analyzer_output_on_a_utf8_boundary() {
        let value = format!("{}️{}", "a".repeat(20_000), "tail");
        let truncated = trim_evidence_output(&value);

        assert!(truncated.starts_with("[truncated to last 16000 bytes]"));
        assert!(truncated.ends_with("tail"));
    }

    #[test]
    fn parses_review_resources_section() {
        let review = r#"## Review

🔴 Bugs / High Risk

1. `src/services/pagination.ts:44` can silently drop the final page when `hasMore` and `pages` disagree.

## Resources

- [MDN: Array.prototype.sort()](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/sort) — Documents sort semantics.
- [NestJS Controllers](https://docs.nestjs.com/controllers) — Explains controller response defaults."#;

        let (body, resources) = parse_review_resources(review);
        assert!(body.contains("Bugs / High Risk"));
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].title, "MDN: Array.prototype.sort()");
        assert_eq!(
            resources[0].url,
            "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/sort"
        );
        assert_eq!(
            resources[1].summary.as_deref(),
            Some("Explains controller response defaults.")
        );
    }

    #[test]
    fn extracts_structured_findings_from_review_markdown() {
        let review = r#"## Review

🔴 Bugs / High Risk

1. `src/services/pagination.ts:44-46` can silently drop the final page when `hasMore` and `pages` disagree.

🟡 Tests / Medium Risk

1. Add a regression test showing the final page is still returned when the API reports `hasMore = false`.

## Resources

- [Example](https://example.com) — Reference doc."#;

        let findings = extract_review_findings(review, "run-1", "run-1-evidence-conversation");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].severity, ReviewFindingSeverity::High);
        assert_eq!(findings[0].category, ReviewFindingCategory::Bug);
        assert_eq!(
            findings[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.path.as_str()),
            Some("src/services/pagination.ts")
        );
        assert_eq!(
            findings[0].anchor.as_ref().map(|anchor| anchor.start_line),
            Some(44)
        );
        assert_eq!(
            findings[0]
                .anchor
                .as_ref()
                .and_then(|anchor| anchor.end_line),
            Some(46)
        );
        assert_eq!(findings[1].category, ReviewFindingCategory::Test);
        assert!(findings[1].anchor.is_none());
    }

    #[test]
    fn extracts_findings_from_severity_bracket_review_markdown() {
        let review = r#"**[MAJOR]** `src/services/orders.ts:27` — stale cached orders can be returned after checkout.
Fix: invalidate the query after the mutation succeeds.

**[MINOR]** `src/services/orders.ts:48` — missing empty-state handling can hide a valid zero result.
Fix: handle empty arrays separately from missing responses.

**[NIT]** `src/services/orders.ts:52` — helper name is ambiguous.
Fix: rename it to describe the returned shape."#;

        let findings = extract_review_findings(review, "run-1", "run-1-evidence-conversation");
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0].severity, ReviewFindingSeverity::High);
        assert_eq!(
            findings[0].anchor.as_ref().map(|anchor| anchor.start_line),
            Some(27)
        );
        assert_eq!(findings[1].severity, ReviewFindingSeverity::Low);
        assert_eq!(findings[2].severity, ReviewFindingSeverity::Info);
    }

    #[test]
    fn extracts_findings_from_structured_review_json() {
        let review = r#"## Review

**[MAJOR]** `src/services/orders.ts:27` — stale cached orders can be returned after checkout.
Fix: invalidate the query after the mutation succeeds.

```json
{
  "schemaVersion": "lachesi.review.v1",
  "findings": [
    {
      "title": "Stale cached orders after checkout",
      "body": "The mutation succeeds without invalidating the order query, so users can keep seeing stale data.",
      "severity": "major",
      "category": "bug",
      "confidence": "high",
      "file": "src/services/orders.ts",
      "line": 27,
      "suggestedFix": "Invalidate the orders query in the mutation success handler."
    }
  ]
}
```
"#;

        let findings = review_findings_from_output(review, "run-1", "run-1-evidence-conversation")
            .expect("structured review should parse");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].title, "Stale cached orders after checkout");
        assert_eq!(findings[0].severity, ReviewFindingSeverity::High);
        assert_eq!(findings[0].confidence, ReviewFindingConfidence::High);
        assert_eq!(findings[0].category, ReviewFindingCategory::Bug);
        assert_eq!(
            findings[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.path.as_str()),
            Some("src/services/orders.ts")
        );
        assert_eq!(
            findings[0].suggested_fix.as_deref(),
            Some("Invalidate the orders query in the mutation success handler.")
        );
    }

    #[test]
    fn rejects_malformed_structured_review_json() {
        let review = r#"## Review

```json
{
  "schemaVersion": "lachesi.review.v1",
  "findings": [
}
```
"#;

        let error = review_findings_from_output(review, "run-1", "run-1-evidence-conversation")
            .expect_err("malformed structured review should fail");
        assert!(error.contains("Invalid structured AI review JSON"));
    }

    #[test]
    fn materialized_review_hides_structured_json_from_display_markdown() {
        let review = format!(
            r#"## Review

**[MINOR]** `src/file.ts:12` — missing empty-state handling.
Fix: render a useful empty state.

```json
{{
  "schemaVersion": "{STRUCTURED_REVIEW_SCHEMA_VERSION}",
  "findings": [
    {{
      "title": "Missing empty state",
      "body": "The component renders nothing for an empty list.",
      "severity": "minor",
      "category": "bug",
      "confidence": "medium",
      "file": "src/file.ts",
      "line": 12,
      "suggestedFix": "Render an explicit empty state."
    }}
  ]
}}
```

## Resources

- [React conditional rendering](https://react.dev/learn/conditional-rendering) — Documents rendering branches.
"#
        );

        let run = materialize_review_run(
            "acme",
            "lachesi",
            Some(ReviewProvider::Github),
            1731,
            "feature/review-schema",
            "main",
            "thread-1",
            AiReviewTurnKind::Initial,
            Some("agentic-balanced"),
            "1750076400000",
            "1750076460000",
            "diff payload snapshot",
            &review,
            ReviewEvidenceSource::Claude,
            Vec::new(),
        )
        .expect("review run should materialize");

        assert_eq!(run.findings.len(), 1);
        assert_eq!(run.provider, ReviewProvider::Github);
        assert_eq!(run.review_profile.as_deref(), Some("agentic-balanced"));
        let summary = run.summary_markdown.as_deref().unwrap_or_default();
        assert!(summary.contains("## Review"));
        assert!(summary.contains("## Resources"));
        assert!(!summary.contains(STRUCTURED_REVIEW_SCHEMA_VERSION));
        assert!(!summary.contains("```json"));
        assert_eq!(
            run.evidence[0].payload.as_deref(),
            run.summary_markdown.as_deref()
        );
    }

    #[test]
    fn materializes_review_run_with_evidence_and_findings() {
        let review = r#"## Review

🔴 Bugs / High Risk

1. `src/services/pagination.ts:44` can silently drop the final page when `hasMore` and `pages` disagree.

## Resources

- [MDN](https://developer.mozilla.org/) — Reference doc."#;

        let run = materialize_review_run(
            "acme",
            "lachesi",
            None,
            1731,
            "feature/review-schema",
            "main",
            "thread-1",
            AiReviewTurnKind::Initial,
            None,
            "1750076400000",
            "1750076460000",
            "diff payload snapshot",
            review,
            ReviewEvidenceSource::Claude,
            Vec::new(),
        )
        .expect("review run should materialize");

        assert_eq!(run.schema_version, "v0.1");
        assert_eq!(run.workspace, "acme");
        assert_eq!(run.repo, "lachesi");
        assert_eq!(run.pr_id, 1731);
        assert_eq!(run.thread_id.as_deref(), Some("thread-1"));
        assert_eq!(run.evidence.len(), 2);
        assert_eq!(run.findings.len(), 1);
        assert_eq!(run.findings[0].evidence_ids.len(), 1);
    }

    #[test]
    fn materializes_review_run_with_analyzer_evidence() {
        let review = r#"## Review

**[MAJOR]** `src/services/orders.ts:27` — stale cached orders can be returned after checkout.
Fix: invalidate the query after the mutation succeeds."#;

        let run = materialize_review_run(
            "acme",
            "lachesi",
            None,
            1731,
            "feature/review-schema",
            "main",
            "thread-1",
            AiReviewTurnKind::Initial,
            None,
            "1750076400000",
            "1750076460000",
            "diff payload snapshot",
            review,
            ReviewEvidenceSource::Claude,
            vec![ReviewEvidenceArtifact {
                id: "run-1-evidence-analyzer-1".to_string(),
                kind: ReviewEvidenceKind::Analyzer,
                source: ReviewEvidenceSource::Tsc,
                title: "TypeScript typecheck".to_string(),
                summary: Some("failed with exit code 2.".to_string()),
                payload: Some("{\"status\":\"failed\"}".to_string()),
            }],
        )
        .expect("review run should materialize");

        assert_eq!(run.evidence.len(), 2);
        assert_eq!(run.evidence[1].kind, ReviewEvidenceKind::Analyzer);
        assert!(run.evidence[1].id.ends_with("-evidence-analyzer-1"));
        assert!(run.findings[0].evidence_ids.contains(&run.evidence[1].id));
    }

    #[test]
    fn records_stage_and_publish_publication_events() {
        let review = r#"## Review

🔴 Bugs / High Risk

1. `src/services/pagination.ts:44` can silently drop the final page when `hasMore` and `pages` disagree."#;

        let mut run = materialize_review_run(
            "acme",
            "lachesi",
            None,
            1731,
            "feature/review-schema",
            "main",
            "thread-1",
            AiReviewTurnKind::Initial,
            None,
            "1750076400000",
            "1750076460000",
            "diff payload snapshot",
            review,
            ReviewEvidenceSource::Claude,
            Vec::new(),
        )
        .expect("review run should materialize");

        let run_id = run.id.clone();
        let finding_fingerprint = run.findings[0].fingerprint.clone();
        let finding = run.findings.first_mut().expect("finding");
        let stage = ReviewFindingPublicationEvent {
            kind: ReviewFindingPublicationEventKind::StageDraft,
            review_run_id: run_id.clone(),
            finding_fingerprint: finding_fingerprint.clone(),
            mode: ReviewPublicationMode::Inline,
            draft_id: Some("draft-1".to_string()),
            remote_comment_id: None,
            published_at: None,
        };
        apply_review_finding_publication_event(finding, &stage);
        assert_eq!(
            finding
                .publication
                .as_ref()
                .map(|publication| publication.draft_ids.as_slice()),
            Some(&["draft-1".to_string()][..])
        );

        let publish = ReviewFindingPublicationEvent {
            kind: ReviewFindingPublicationEventKind::PublishDraft,
            review_run_id: run_id,
            finding_fingerprint,
            mode: ReviewPublicationMode::Inline,
            draft_id: Some("draft-1".to_string()),
            remote_comment_id: Some(42),
            published_at: Some("1750076500000".to_string()),
        };
        apply_review_finding_publication_event(finding, &publish);
        assert_eq!(finding.status, super::ReviewFindingStatus::Published);
        assert_eq!(
            finding
                .publication
                .as_ref()
                .map(|publication| publication.draft_ids.is_empty()),
            Some(true)
        );
        assert_eq!(
            finding
                .publication
                .as_ref()
                .map(|publication| publication.remote_comment_ids.as_slice()),
            Some(&[42][..])
        );
        assert_eq!(
            finding
                .publication
                .as_ref()
                .and_then(|publication| publication.published_at.as_deref()),
            Some("1750076500000")
        );
    }
}
fn review_provider_for_repo(workspace: &str, repo: &str) -> ReviewProvider {
    let cfg = config::load();
    match cfg
        .repos
        .iter()
        .find(|candidate| candidate.workspace == workspace && candidate.repo == repo)
        .map(|candidate| candidate.provider)
        .unwrap_or(cfg.review_provider)
    {
        ConfigReviewProvider::Bitbucket => ReviewProvider::Bitbucket,
        ConfigReviewProvider::Github => ReviewProvider::Github,
    }
}
