use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

use crate::config::{AiProvider, AppConfig, ReviewProvider};
use crate::credentials;
use crate::headless_review::{self, HeadlessReviewRequest, ReviewScope};
use crate::readiness;
use crate::repo_config::{
    self, InitMode as RepoInitMode, LoadedPolicyPack, RepoConfigValidationMessage, RepoInitProposal,
};
use crate::review_evaluation;
use crate::review_event::PullRequestReviewEventProvider;
use crate::review_metrics::{
    ReviewEffectivenessFilter, ReviewEffectivenessReport, ReviewEffectivenessSummary,
};
use crate::review_storage;
use crate::services::review::ReviewFindingSeverity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewOutputFormat {
    Markdown,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReviewArgs {
    repo_path: Option<PathBuf>,
    scope: ReviewScope,
    base: Option<String>,
    workspace: Option<String>,
    repo: Option<String>,
    pr_id: Option<u32>,
    provider: Option<ReviewProvider>,
    profile: Option<String>,
    ai_provider: Option<AiProvider>,
    model: Option<String>,
    effort: Option<String>,
    format: ReviewOutputFormat,
    output: Option<PathBuf>,
    fail_on_findings: bool,
    min_severity: Option<ReviewFindingSeverity>,
    run_analyzers: bool,
    allow_provider_diff: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigValidateArgs {
    repo_path: PathBuf,
    profile: Option<String>,
    format: OutputFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigMigrateArgs {
    repo_path: PathBuf,
    dry_run: bool,
    format: OutputFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetupArgs {
    format: OutputFormat,
    dry_run: bool,
    yes: bool,
    provider_diff_consent: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InitMode {
    Quick,
    Guided,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InitArgs {
    repo_path: PathBuf,
    mode: InitMode,
    dry_run: bool,
    yes: bool,
    format: OutputFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MetricsArgs {
    filter: ReviewEffectivenessFilter,
    format: OutputFormat,
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvaluateArgs {
    corpus: PathBuf,
    baseline: PathBuf,
    output: Option<PathBuf>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigValidateOutput {
    valid: bool,
    repo_path: String,
    config_path: String,
    exists: bool,
    selected_profile: Option<String>,
    prompt_replaces_default: bool,
    loaded_policy_packs: Vec<LoadedPolicyPack>,
    warnings: Vec<RepoConfigValidationMessage>,
    errors: Vec<RepoConfigValidationMessage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupOutput {
    schema_version: &'static str,
    selected_ai_provider: String,
    would_apply: bool,
    config_path: String,
    machine_tools: Vec<SetupToolReport>,
    machine_credentials: Vec<SetupCredentialReport>,
    setup_notes: Vec<String>,
    provider_diff_sharing_allowed: bool,
    proposed_provider_diff_sharing_allowed: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupToolReport {
    provider: String,
    available: bool,
    version: Option<String>,
    required: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupCredentialReport {
    provider: String,
    available: bool,
    source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitOutput {
    schema_version: &'static str,
    repo_path: String,
    mode: &'static str,
    dry_run: bool,
    would_apply: bool,
    proposal: InitProposalOutput,
    actions: Vec<repo_config::RepoConfigMigrationAction>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitProposalOutput {
    mode: &'static str,
    project_types: Vec<String>,
    task_runners: Vec<String>,
    instruction_sources: Vec<String>,
    analyzer_candidates: Vec<InitAnalyzerOutput>,
    suggested_excludes: Vec<String>,
    config_preview: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitAnalyzerOutput {
    id: String,
    command: String,
    required: bool,
    timeout_seconds: u64,
}

struct HeadlessDataDirGuard {
    temp_dir: Option<tempfile::TempDir>,
}

impl HeadlessDataDirGuard {
    fn install() -> Result<Self, String> {
        if std::env::var_os("NORN_REVIEW_DATA_DIR").is_some()
            || std::env::var_os("NORN_DATA_DIR").is_some()
            || std::env::var_os("LACHESI_REVIEW_DATA_DIR").is_some()
            || std::env::var_os("LACHESI_DATA_DIR").is_some()
        {
            return Ok(Self { temp_dir: None });
        }
        let temp_dir = create_headless_data_dir()?;
        std::env::set_var("NORN_REVIEW_DATA_DIR", temp_dir.path());
        Ok(Self {
            temp_dir: Some(temp_dir),
        })
    }
}

fn create_headless_data_dir() -> Result<tempfile::TempDir, String> {
    let temp_dir = tempfile::Builder::new()
        .prefix("norn-headless-storage-")
        .tempdir()
        .map_err(|error| format!("Failed to create temporary headless storage: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o700)).map_err(
            |error| format!("Failed to secure temporary headless storage permissions: {error}"),
        )?;
    }
    Ok(temp_dir)
}

impl Drop for HeadlessDataDirGuard {
    fn drop(&mut self) {
        if self.temp_dir.is_some() {
            std::env::remove_var("NORN_REVIEW_DATA_DIR");
        }
    }
}

pub fn run_from_env_if_cli() -> Option<i32> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !is_cli_command(&args) {
        return None;
    }

    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    let _headless_data_dir = if review_needs_headless_storage(&args) {
        match HeadlessDataDirGuard::install() {
            Ok(guard) => Some(guard),
            Err(error) => {
                let _ = writeln!(stderr, "{error}");
                return Some(7);
            }
        }
    } else {
        None
    };
    Some(run_args(&args, &mut stdout, &mut stderr))
}

pub fn print_usage() -> i32 {
    let mut stdout = io::stdout();
    writeln!(stdout, "{}", usage()).map_or(1, |_| 0)
}

fn review_needs_headless_storage(args: &[String]) -> bool {
    args.first().map(String::as_str) == Some("review")
        && !args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
        && parse_review_args(args).is_ok()
}

fn is_cli_command(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some(
            "auth"
                | "skills"
                | "config"
                | "doctor"
                | "evaluate"
                | "init"
                | "metrics"
                | "setup"
                | "review"
                | "service"
                | "--help"
                | "-h"
                | "--version"
                | "-V"
        )
    )
}

fn run_args(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if matches!(args.first().map(String::as_str), Some("--version" | "-V")) {
        let _ = writeln!(stdout, "norn {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    if args.first().map(String::as_str) == Some("review")
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
    {
        let _ = writeln!(stdout, "{}", review_usage());
        return 0;
    }
    if args.first().map(String::as_str) == Some("metrics")
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
    {
        let _ = writeln!(stdout, "{}", metrics_usage());
        return 0;
    }
    if args.first().map(String::as_str) == Some("evaluate")
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
    {
        let _ = writeln!(stdout, "{}", evaluate_usage());
        return 0;
    }
    if args.first().map(String::as_str) == Some("doctor")
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
    {
        let _ = writeln!(stdout, "{}", doctor_usage());
        return 0;
    }
    if args.first().map(String::as_str) == Some("auth")
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
    {
        return crate::terminal_auth::run(&args[1..], stdout, stderr);
    }
    if args.first().map(String::as_str) == Some("skills")
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
    {
        return crate::agent_skills::run(&args[1..], stdout, stderr);
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        let _ = writeln!(stdout, "{}", usage());
        return 0;
    }

    match args.first().map(String::as_str) {
        Some("review") => match parse_review_args(args) {
            Ok(args) => run_review(args, stdout, stderr),
            Err(error) => write_review_parse_failure(args, &error, stdout, stderr),
        },
        Some("metrics") => match parse_metrics_args(args) {
            Ok(args) => run_metrics(args, stdout, stderr),
            Err(error) => {
                let _ = writeln!(stderr, "{error}\n\n{}", metrics_usage());
                2
            }
        },
        Some("evaluate") => match parse_evaluate_args(args) {
            Ok(args) => run_evaluate(args, stdout, stderr),
            Err(error) => {
                let _ = writeln!(stderr, "{error}\n\n{}", evaluate_usage());
                2
            }
        },
        Some("doctor") => match readiness::parse_doctor_args(args) {
            Ok(args) => readiness::run_doctor(args, stdout, stderr),
            Err(error) => {
                let _ = writeln!(stderr, "{error}\n\n{}", doctor_usage());
                2
            }
        },
        Some("service") => crate::self_hosted_service::run(&args[1..], stdout, stderr),
        Some("auth") => crate::terminal_auth::run(&args[1..], stdout, stderr),
        Some("skills") => crate::agent_skills::run(&args[1..], stdout, stderr),
        Some("config") if args.get(1).map(String::as_str) == Some("migrate") => {
            match parse_config_migrate_args(args) {
                Ok(args) => run_config_migrate(args, stdout, stderr),
                Err(error) => {
                    let _ = writeln!(stderr, "{error}\n\n{}", usage());
                    2
                }
            }
        }
        Some("setup") => match parse_setup_args(args) {
            Ok(args) => run_setup(args, stdout, stderr),
            Err(error) => {
                let _ = writeln!(stderr, "{error}\n\n{}", usage());
                2
            }
        },
        Some("init") => match parse_init_args(args) {
            Ok(args) => run_init(args, stdout, stderr),
            Err(error) => {
                let _ = writeln!(stderr, "{error}\n\n{}", usage());
                2
            }
        },
        _ => match parse_config_validate_args(args) {
            Ok(args) => run_config_validate(args, stdout, stderr),
            Err(error) => {
                let _ = writeln!(stderr, "{error}\n\n{}", usage());
                2
            }
        },
    }
}

fn parse_evaluate_args(args: &[String]) -> Result<EvaluateArgs, String> {
    if args.first().map(String::as_str) != Some("evaluate") {
        return Err("Expected `norn evaluate`.".to_string());
    }
    let mut parsed = EvaluateArgs {
        corpus: PathBuf::from("fixtures/review-evaluation/v1/corpus.json"),
        baseline: PathBuf::from("fixtures/review-evaluation/v1/baseline.json"),
        output: None,
    };
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--corpus" => parsed.corpus = PathBuf::from(next_value(args, &mut index)?),
            "--baseline" => parsed.baseline = PathBuf::from(next_value(args, &mut index)?),
            "--output" => parsed.output = Some(PathBuf::from(next_value(args, &mut index)?)),
            unknown => return Err(format!("Unknown evaluate option `{unknown}`.")),
        }
        index += 1;
    }
    Ok(parsed)
}

fn parse_setup_args(args: &[String]) -> Result<SetupArgs, String> {
    if args.first().map(String::as_str) != Some("setup") {
        return Err("Expected `norn setup`.".to_string());
    }

    let mut format = OutputFormat::Human;
    let mut dry_run = false;
    let mut yes = false;
    let mut explicit_dry_run = false;
    let mut provider_diff_consent = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--format" => {
                index += 1;
                format = match args.get(index).map(String::as_str) {
                    Some("human" | "text") => OutputFormat::Human,
                    Some("json") => OutputFormat::Json,
                    Some(_) => {
                        return Err("`--format` must be `human`, `text`, or `json`.".to_string())
                    }
                    None => return Err("`--format` requires a value.".to_string()),
                };
            }
            "--json" => format = OutputFormat::Json,
            "--dry-run" => {
                explicit_dry_run = true;
                dry_run = true;
            }
            "--yes" => {
                yes = true;
                dry_run = false;
            }
            "--allow-provider-diff" => {
                if provider_diff_consent == Some(false) {
                    return Err(
                        "`--allow-provider-diff` and `--deny-provider-diff` are mutually exclusive for `norn setup`."
                            .to_string(),
                    );
                }
                provider_diff_consent = Some(true);
            }
            "--deny-provider-diff" => {
                if provider_diff_consent == Some(true) {
                    return Err(
                        "`--allow-provider-diff` and `--deny-provider-diff` are mutually exclusive for `norn setup`."
                            .to_string(),
                    );
                }
                provider_diff_consent = Some(false);
            }
            unknown => return Err(format!("Unknown option `{unknown}`.")),
        }
        index += 1;
    }

    if yes && explicit_dry_run {
        return Err("`--yes` and `--dry-run` are mutually exclusive for `norn setup`.".to_string());
    }

    Ok(SetupArgs {
        format,
        dry_run,
        yes,
        provider_diff_consent,
    })
}

fn parse_init_args(args: &[String]) -> Result<InitArgs, String> {
    if args.first().map(String::as_str) != Some("init") {
        return Err("Expected `norn init`.".to_string());
    }
    let mut repo_path = PathBuf::from(".");
    let mut mode = InitMode::Quick;
    let mut dry_run = true;
    let mut yes = false;
    let mut format = OutputFormat::Human;
    let mut explicit_dry_run = false;
    let mut explicit_mode = None::<InitMode>;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--repo-path" => {
                index += 1;
                repo_path = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "`--repo-path` requires a value.".to_string())?,
                );
            }
            "--guided" => {
                if explicit_mode == Some(InitMode::Quick) {
                    return Err(
                        "`--quick` and `--guided` are mutually exclusive for `norn init`."
                            .to_string(),
                    );
                }
                mode = InitMode::Guided;
                explicit_mode = Some(InitMode::Guided);
                dry_run = true;
            }
            "--quick" => {
                if explicit_mode == Some(InitMode::Guided) {
                    return Err(
                        "`--quick` and `--guided` are mutually exclusive for `norn init`."
                            .to_string(),
                    );
                }
                mode = InitMode::Quick;
                explicit_mode = Some(InitMode::Quick);
            }
            "--dry-run" => {
                explicit_dry_run = true;
                dry_run = true;
            }
            "--yes" => {
                yes = true;
                dry_run = false;
            }
            "--format" => {
                index += 1;
                format = match args.get(index).map(String::as_str) {
                    Some("human" | "text") => OutputFormat::Human,
                    Some("json") => OutputFormat::Json,
                    Some(_) => {
                        return Err("`--format` must be `human`, `text`, or `json`.".to_string())
                    }
                    None => return Err("`--format` requires a value.".to_string()),
                };
            }
            "--json" => format = OutputFormat::Json,
            unknown => return Err(format!("Unknown option `{unknown}`.")),
        }
        index += 1;
    }
    if yes && explicit_dry_run {
        return Err("`--yes` and `--dry-run` are mutually exclusive for `norn init`.".to_string());
    }

    Ok(InitArgs {
        repo_path,
        mode,
        dry_run,
        yes,
        format,
    })
}

fn map_repo_init_mode(mode: &InitMode) -> RepoInitMode {
    match mode {
        InitMode::Quick => RepoInitMode::Quick,
        InitMode::Guided => RepoInitMode::Guided,
    }
}

fn init_mode_label(mode: InitMode) -> &'static str {
    match mode {
        InitMode::Quick => "quick",
        InitMode::Guided => "guided",
    }
}

fn build_init_proposal_output(proposal: &RepoInitProposal) -> InitProposalOutput {
    InitProposalOutput {
        mode: match proposal.mode {
            RepoInitMode::Quick => "quick",
            RepoInitMode::Guided => "guided",
        },
        project_types: proposal.project_types.clone(),
        task_runners: proposal.task_runners.clone(),
        instruction_sources: proposal.instruction_sources.clone(),
        analyzer_candidates: proposal
            .analyzer_candidates
            .iter()
            .map(|analyzer| InitAnalyzerOutput {
                id: analyzer.id.clone(),
                command: analyzer.command.clone(),
                required: analyzer.required,
                timeout_seconds: analyzer.timeout_seconds,
            })
            .collect(),
        suggested_excludes: proposal.suggested_excludes.clone(),
        config_preview: proposal.config_contents.clone(),
    }
}

fn run_setup(args: SetupArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let current = crate::config::load();
    let tools = collect_setup_tools(current.ai_provider);
    let machine_credentials = collect_setup_credentials();
    run_setup_with_inventory(args, stdout, stderr, current, tools, machine_credentials)
}

fn run_setup_with_inventory(
    args: SetupArgs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    current: AppConfig,
    tools: Vec<SetupToolReport>,
    machine_credentials: Vec<SetupCredentialReport>,
) -> i32 {
    let selected_ai_provider = select_quick_ai_provider(&tools, current.ai_provider);
    let proposed_provider_diff_sharing_allowed = args
        .provider_diff_consent
        .unwrap_or(current.headless_ai_diff_sharing_allowed);
    let provider_diff_sharing_allowed = if args.yes && !args.dry_run {
        proposed_provider_diff_sharing_allowed
    } else {
        current.headless_ai_diff_sharing_allowed
    };
    let mut notes = Vec::new();
    if !args.yes && selected_ai_provider != current.ai_provider {
        notes.push(format!(
            "Quick default provider would be `{}`. Re-run with `--yes` to persist.",
            selected_ai_provider.to_display_name()
        ));
    }
    for credential in &machine_credentials {
        if !credential.available {
            notes.push(format!(
                "Run `norn auth login {}` to store this provider credential in the OS keychain.",
                credential.provider
            ));
        }
    }
    if !provider_diff_sharing_allowed {
        notes.push(
            "Headless AI review will require `--allow-provider-diff` for each run until diff sharing is enabled locally."
                .to_string(),
        );
    }
    if proposed_provider_diff_sharing_allowed != provider_diff_sharing_allowed {
        let proposed = if proposed_provider_diff_sharing_allowed {
            "allow"
        } else {
            "deny"
        };
        notes.push(format!(
            "Re-run with `--yes` to persist the proposed `{proposed}` headless AI diff-sharing choice locally."
        ));
    }

    let has_setup_notes = !notes.is_empty();
    let output = SetupOutput {
        schema_version: "norn.setup.v1",
        selected_ai_provider: selected_ai_provider.to_display_name().to_string(),
        would_apply: args.yes && !args.dry_run,
        config_path: config_path_display(),
        machine_tools: tools.clone(),
        machine_credentials,
        setup_notes: notes,
        provider_diff_sharing_allowed,
        proposed_provider_diff_sharing_allowed,
    };

    if args.format == OutputFormat::Json {
        let rendered = match serde_json::to_string_pretty(&output) {
            Ok(rendered) => rendered,
            Err(error) => {
                let _ = writeln!(stderr, "Failed to serialize setup output: {error}");
                return 7;
            }
        };
        let _ = writeln!(stdout, "{rendered}");
    } else {
        let _ = writeln!(
            stdout,
            "Norn setup proposal: use {} for local machine configuration.",
            output.selected_ai_provider
        );
        let _ = writeln!(stdout, "Detected machine tooling:");
        for tool in &output.machine_tools {
            let status = if tool.available {
                "available"
            } else {
                "missing"
            };
            let version = tool
                .version
                .clone()
                .unwrap_or_else(|| "unavailable".to_string());
            let _ = writeln!(stdout, "- {} ({status}) {version}", tool.provider);
        }
        let _ = writeln!(stdout, "Detected credential sources:");
        for item in &output.machine_credentials {
            let status = if item.available { "found" } else { "not found" };
            let _ = writeln!(stdout, "- {}: {status} ({})", item.provider, item.source);
        }
        let diff_sharing = if output.provider_diff_sharing_allowed {
            "allowed"
        } else {
            "not allowed"
        };
        let _ = writeln!(
            stdout,
            "Headless AI diff sharing: {diff_sharing} (local setting)."
        );
        if output.proposed_provider_diff_sharing_allowed != output.provider_diff_sharing_allowed {
            let proposed = if output.proposed_provider_diff_sharing_allowed {
                "allowed"
            } else {
                "not allowed"
            };
            let _ = writeln!(
                stdout,
                "Proposed headless AI diff sharing: {proposed} (not persisted)."
            );
        }
        if output.would_apply {
            let _ = writeln!(
                stdout,
                "Would apply provider selection to local app config."
            );
            if !args.yes {
                let _ = writeln!(
                    stdout,
                    "Run with `--yes` (and default path) to persist this setup."
                );
            }
        } else {
            let _ = writeln!(stdout, "Run with `--yes` to persist this setup.");
        }
        if has_setup_notes {
            for note in &output.setup_notes {
                let _ = writeln!(stdout, "- {note}");
            }
        }
    }

    let config_changed = selected_ai_provider != current.ai_provider
        || args
            .provider_diff_consent
            .is_some_and(|allowed| allowed != current.headless_ai_diff_sharing_allowed);
    if args.yes && !args.dry_run && config_changed {
        let mut updated = current;
        updated.ai_provider = selected_ai_provider;
        if let Some(allowed) = args.provider_diff_consent {
            updated.headless_ai_diff_sharing_allowed = allowed;
        }
        if let Err(error) = crate::config::save(&updated) {
            let _ = writeln!(stderr, "Failed to persist setup config: {error}");
            return 7;
        }
    }

    let selected_provider = match selected_ai_provider {
        AiProvider::Codex => "codex",
        AiProvider::Claude => "claude",
    };
    i32::from(!output.machine_tools.iter().any(|tool| {
        tool.provider == selected_provider
            && tool.available
            && tool
                .version
                .as_ref()
                .is_some_and(|version| !version.is_empty())
    }))
}

fn run_init(args: InitArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if args.mode == InitMode::Guided && !args.yes {
        let _ = writeln!(
            stderr,
            "Guided mode requires `--yes` and is not interactive in this release."
        );
        return 2;
    }

    if let Ok(Some(config_source)) = repo_config::discover_repo_config_source(&args.repo_path) {
        match repo_config::load_from_repo_path(&args.repo_path) {
            Ok(result) if !result.errors.is_empty() => {
                let _ = writeln!(
                    stderr,
                    "Cannot run onboarding with repository config at {}.",
                    config_source.display(),
                );
                for error in result.errors {
                    let _ = writeln!(stderr, "  - {}", error.message);
                }
                let _ = writeln!(
                    stderr,
                    "Use `norn doctor --repo-path {}` to repair configuration before retrying.",
                    args.repo_path.display()
                );
                return 2;
            }
            Ok(_) => {}
            Err(error) => {
                let _ = writeln!(
                    stderr,
                    "Cannot run onboarding with repository config at {}: {error}",
                    config_source.display()
                );
                let _ = writeln!(
                    stderr,
                    "Use `norn doctor --repo-path {}` to repair configuration before retrying.",
                    args.repo_path.display()
                );
                return 2;
            }
        }
    }

    let repo_init_mode = map_repo_init_mode(&args.mode);
    let proposal = match repo_config::proposal_for_repo_init(&args.repo_path, repo_init_mode) {
        Ok(proposal) => proposal,
        Err(error) => {
            let _ = writeln!(
                stderr,
                "Failed to generate repository init proposal: {error}"
            );
            return 2;
        }
    };
    let proposal_output = build_init_proposal_output(&proposal);

    let mut result = match repo_config::migrate_repository_config(&args.repo_path, args.dry_run) {
        Ok(result) => result,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return 2;
        }
    };

    match repo_config::default_init_action_if_needed_with_mode(&args.repo_path, repo_init_mode) {
        Ok(Some(action)) => {
            if args.yes && !args.dry_run {
                if let Err(error) = repo_config::write_default_repo_config_if_missing_with_mode(
                    &args.repo_path,
                    repo_init_mode,
                ) {
                    let _ = writeln!(
                        stderr,
                        "Failed to create default repository config: {error}"
                    );
                    return 2;
                }
            }
            result.actions.push(action);
        }
        Ok(None) => {}
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return 2;
        }
    }

    let output = InitOutput {
        schema_version: "norn.init.v1",
        repo_path: result.repo_path,
        mode: if init_mode_label(args.mode) == "guided" {
            "guided"
        } else {
            "quick"
        },
        dry_run: args.dry_run,
        would_apply: args.yes && !args.dry_run,
        proposal: proposal_output,
        actions: result.actions,
    };

    if args.format == OutputFormat::Json {
        let rendered = match serde_json::to_string_pretty(&output) {
            Ok(rendered) => rendered,
            Err(error) => {
                let _ = writeln!(stderr, "Failed to serialize init output: {error}");
                return 7;
            }
        };
        let _ = writeln!(stdout, "{rendered}");
    } else {
        let _ = writeln!(stdout, "Norn repository init in `{}` mode", output.mode);
        let project_types = if output.proposal.project_types.is_empty() {
            "unknown".to_string()
        } else {
            output.proposal.project_types.join(", ")
        };
        let _ = writeln!(stdout, "Detected project types: {project_types}");
        if output.actions.is_empty() {
            let _ = writeln!(stdout, "No repository initialization actions needed.");
        } else {
            for action in output.actions.iter() {
                let _ = writeln!(
                    stdout,
                    "- {} {} -> {}",
                    action.kind, action.source, action.target
                );
                for change in &action.content_changes {
                    let _ = writeln!(stdout, "  - {change}");
                }
            }
            if output.would_apply {
                let _ = writeln!(stdout, "Migration applied.");
            } else {
                let _ = writeln!(stdout, "Run `norn init --yes` to apply.");
            }
        }
        if !output.proposal.analyzer_candidates.is_empty() {
            let _ = writeln!(stdout, "Analyzer candidates (disabled by default):");
            for analyzer in &output.proposal.analyzer_candidates {
                let _ = writeln!(stdout, "  - {}: {}", analyzer.id, analyzer.command);
            }
        }
        let _ = writeln!(stdout, "Proposed config:");
        for line in output.proposal.config_preview.lines() {
            let _ = writeln!(stdout, "  {line}");
        }
    }

    0
}

fn detect_tool_version(provider: &str) -> Option<String> {
    let output = Command::new(provider).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|text| text.lines().next().unwrap_or_default().trim().to_string())
        .filter(|text| !text.is_empty())
}

fn collect_setup_tools(selected: AiProvider) -> Vec<SetupToolReport> {
    let codex = detect_tool_version("codex");
    let claude = detect_tool_version("claude");
    vec![
        SetupToolReport {
            provider: "codex".to_string(),
            available: codex.is_some(),
            version: codex,
            required: selected == AiProvider::Codex,
        },
        SetupToolReport {
            provider: "claude".to_string(),
            available: claude.is_some(),
            version: claude,
            required: selected == AiProvider::Claude,
        },
    ]
}

fn select_quick_ai_provider(tools: &[SetupToolReport], configured: AiProvider) -> AiProvider {
    if tools
        .iter()
        .any(|tool| tool.provider == "codex" && tool.available)
    {
        AiProvider::Codex
    } else if tools
        .iter()
        .any(|tool| tool.provider == "claude" && tool.available)
    {
        AiProvider::Claude
    } else {
        configured
    }
}

fn collect_setup_credentials() -> Vec<SetupCredentialReport> {
    let github_available = credentials::has_github_credential_source();
    let bitbucket_available = credentials::has_bitbucket_credential_source();
    vec![
        SetupCredentialReport {
            provider: "github".to_string(),
            available: github_available,
            source: if github_available {
                "keychain or env reference".to_string()
            } else {
                "none".to_string()
            },
        },
        SetupCredentialReport {
            provider: "bitbucket".to_string(),
            available: bitbucket_available,
            source: if bitbucket_available {
                "keychain or env reference".to_string()
            } else {
                "none".to_string()
            },
        },
    ]
}

fn config_path_display() -> String {
    match dirs::config_dir() {
        Some(dir) => dir.join("norn").join("settings.json").display().to_string(),
        None => ".norn/settings.json".to_string(),
    }
}

trait AiProviderName {
    fn to_display_name(&self) -> &'static str;
}

impl AiProviderName for AiProvider {
    fn to_display_name(&self) -> &'static str {
        match self {
            AiProvider::Claude => "claude",
            AiProvider::Codex => "codex",
        }
    }
}

fn run_evaluate(args: EvaluateArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let result = match review_evaluation::load_and_evaluate(&args.corpus, &args.baseline) {
        Ok(result) => result,
        Err(error) => {
            let _ = writeln!(stderr, "Evaluation failed: {error}");
            return 2;
        }
    };
    let rendered = match serde_json::to_string_pretty(&result) {
        Ok(rendered) => format!("{rendered}\n"),
        Err(error) => {
            let _ = writeln!(stderr, "Evaluation failed to serialize: {error}");
            return 7;
        }
    };
    if let Some(path) = args.output {
        if let Err(error) = std::fs::write(&path, &rendered) {
            let _ = writeln!(
                stderr,
                "Evaluation failed to write {}: {error}",
                path.display()
            );
            return 7;
        }
    } else {
        let _ = write!(stdout, "{rendered}");
    }
    if result.regressions.is_empty() {
        0
    } else {
        1
    }
}

fn write_review_parse_failure(
    args: &[String],
    error: &str,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let json_requested = args.iter().any(|arg| arg == "--json")
        || args
            .windows(2)
            .any(|pair| pair[0] == "--format" && pair[1] == "json");
    if json_requested {
        let rendered = serde_json::json!({
            "schemaVersion": "norn.headless-review.v1",
            "status": "failed",
            "exitCode": 2,
            "error": error,
        });
        if let Some(path) = review_parse_output_path(args) {
            if let Err(write_error) = std::fs::write(&path, format!("{rendered}\n")) {
                let _ = writeln!(
                    stderr,
                    "Failed to write review failure to {}: {write_error}",
                    path.display()
                );
                return 7;
            }
            let _ = writeln!(stderr, "Review failure written to {}.", path.display());
        } else {
            let _ = writeln!(stdout, "{rendered}");
        }
    } else {
        let _ = writeln!(stderr, "{error}\n\n{}", review_usage());
    }
    2
}

fn review_parse_output_path(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .rev()
        .find(|pair| pair[0] == "--output" && !pair[1].starts_with('-'))
        .map(|pair| PathBuf::from(&pair[1]))
}

fn parse_review_args(args: &[String]) -> Result<ReviewArgs, String> {
    if args.first().map(String::as_str) != Some("review") {
        return Err("Expected `norn review`.".to_string());
    }
    let mut parsed = ReviewArgs {
        repo_path: None,
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
        format: ReviewOutputFormat::Markdown,
        output: None,
        fail_on_findings: false,
        min_severity: None,
        run_analyzers: false,
        allow_provider_diff: false,
    };
    let mut scope_explicit = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--repo-path" => {
                parsed.repo_path = Some(PathBuf::from(next_value(args, &mut index)?));
            }
            "--scope" => {
                parsed.scope = match next_value(args, &mut index)?.as_str() {
                    "working-tree" | "worktree" | "local" => ReviewScope::WorkingTree,
                    "branch" => ReviewScope::Branch,
                    "pr" | "pull-request" => ReviewScope::PullRequest,
                    _ => {
                        return Err(
                            "`--scope` must be `working-tree`, `branch`, or `pr`.".to_string()
                        )
                    }
                };
                scope_explicit = true;
            }
            "--base" => parsed.base = Some(next_value(args, &mut index)?),
            "--workspace" => parsed.workspace = Some(next_value(args, &mut index)?),
            "--repo" => parsed.repo = Some(next_value(args, &mut index)?),
            "--pr" => {
                let value = next_value(args, &mut index)?;
                let pr_id = value
                    .parse::<u32>()
                    .map_err(|_| "`--pr` must be a positive integer.".to_string())?;
                if pr_id == 0 {
                    return Err("`--pr` must be a positive integer.".to_string());
                }
                parsed.pr_id = Some(pr_id);
                if !scope_explicit {
                    parsed.scope = ReviewScope::PullRequest;
                }
            }
            "--provider" => {
                parsed.provider = Some(match next_value(args, &mut index)?.as_str() {
                    "github" => ReviewProvider::Github,
                    "bitbucket" => ReviewProvider::Bitbucket,
                    _ => return Err("`--provider` must be `github` or `bitbucket`.".to_string()),
                });
            }
            "--profile" => parsed.profile = Some(next_value(args, &mut index)?),
            "--ai-provider" => {
                parsed.ai_provider = Some(match next_value(args, &mut index)?.as_str() {
                    "codex" => AiProvider::Codex,
                    "claude" => AiProvider::Claude,
                    _ => return Err("`--ai-provider` must be `codex` or `claude`.".to_string()),
                });
            }
            "--model" => parsed.model = Some(next_value(args, &mut index)?),
            "--effort" => parsed.effort = Some(next_value(args, &mut index)?),
            "--format" => {
                parsed.format = match next_value(args, &mut index)?.as_str() {
                    "markdown" | "md" | "human" => ReviewOutputFormat::Markdown,
                    "json" => ReviewOutputFormat::Json,
                    _ => return Err("`--format` must be `markdown` or `json`.".to_string()),
                };
            }
            "--json" => parsed.format = ReviewOutputFormat::Json,
            "--output" => parsed.output = Some(PathBuf::from(next_value(args, &mut index)?)),
            "--fail-on-findings" => parsed.fail_on_findings = true,
            "--run-analyzers" => parsed.run_analyzers = true,
            "--allow-provider-diff" => parsed.allow_provider_diff = true,
            "--min-severity" => {
                parsed.min_severity = Some(parse_severity(&next_value(args, &mut index)?)?);
            }
            unknown => return Err(format!("Unknown review option `{unknown}`.")),
        }
        index += 1;
    }
    if parsed.workspace.is_some() != parsed.repo.is_some() {
        return Err("`--workspace` and `--repo` must be provided together.".to_string());
    }
    if parsed.pr_id.is_some() && parsed.scope != ReviewScope::PullRequest {
        return Err("`--pr` requires `--scope pr` when scope is explicit.".to_string());
    }
    if parsed.base.is_some() && parsed.scope != ReviewScope::Branch {
        return Err("`--base` requires `--scope branch`.".to_string());
    }
    Ok(parsed)
}

fn next_value(args: &[String], index: &mut usize) -> Result<String, String> {
    let option = args
        .get(*index)
        .cloned()
        .unwrap_or_else(|| "option".to_string());
    *index += 1;
    match args.get(*index) {
        Some(value) if !value.starts_with('-') => Ok(value.clone()),
        _ => Err(format!("`{option}` requires a value.")),
    }
}

fn parse_severity(value: &str) -> Result<ReviewFindingSeverity, String> {
    match value {
        "info" => Ok(ReviewFindingSeverity::Info),
        "low" => Ok(ReviewFindingSeverity::Low),
        "medium" => Ok(ReviewFindingSeverity::Medium),
        "high" => Ok(ReviewFindingSeverity::High),
        "critical" => Ok(ReviewFindingSeverity::Critical),
        _ => Err(
            "`--min-severity` must be `info`, `low`, `medium`, `high`, or `critical`.".to_string(),
        ),
    }
}

fn run_review(args: ReviewArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let request = HeadlessReviewRequest {
        repo_path: args.repo_path,
        scope: args.scope,
        base: args.base,
        workspace: args.workspace,
        repo: args.repo,
        pr_id: args.pr_id,
        provider: args.provider,
        profile: args.profile,
        ai_provider: args.ai_provider,
        model: args.model,
        effort: args.effort,
        run_analyzers: args.run_analyzers,
        allow_provider_diff: args.allow_provider_diff,
    };
    let _ = writeln!(stderr, "Starting headless review...");
    let mut execution = match headless_review::run(request) {
        Ok(execution) => execution,
        Err(error) => {
            return write_review_failure(
                error,
                args.format,
                args.output.as_deref(),
                stdout,
                stderr,
            );
        }
    };
    let minimum = args
        .min_severity
        .or(execution.minimum_severity)
        .unwrap_or(ReviewFindingSeverity::High);
    execution.minimum_severity = Some(minimum);
    let exit_code = if args.fail_on_findings
        && headless_review::has_findings_at_or_above(&execution, minimum)
    {
        1
    } else {
        0
    };
    execution.exit_code = exit_code;

    let rendered = match args.format {
        ReviewOutputFormat::Markdown => headless_review::format_markdown(&execution),
        ReviewOutputFormat::Json => match serde_json::to_string_pretty(&execution) {
            Ok(json) => json,
            Err(error) => {
                let _ = writeln!(stderr, "Failed to serialize review output: {error}");
                return 7;
            }
        },
    };
    if let Some(path) = args.output {
        if let Err(error) = std::fs::write(&path, format!("{rendered}\n")) {
            let _ = writeln!(stderr, "Failed to write {}: {error}", path.display());
            return 7;
        }
        let _ = writeln!(stderr, "Review written to {}.", path.display());
    } else {
        let _ = writeln!(stdout, "{rendered}");
    }
    exit_code
}

fn write_review_failure(
    error: headless_review::HeadlessReviewError,
    format: ReviewOutputFormat,
    output: Option<&std::path::Path>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let exit_code = error.exit_code;
    if format == ReviewOutputFormat::Markdown {
        let _ = writeln!(stderr, "{}", error.message);
        return exit_code;
    }

    let rendered = serde_json::json!({
        "schemaVersion": "norn.headless-review.v1",
        "status": "failed",
        "exitCode": exit_code,
        "errorCode": error.code,
        "error": error.message,
    })
    .to_string();
    if let Some(path) = output {
        if let Err(write_error) = std::fs::write(path, format!("{rendered}\n")) {
            let _ = writeln!(
                stderr,
                "Failed to write review failure to {}: {write_error}",
                path.display()
            );
            return 7;
        }
        let _ = writeln!(stderr, "Review failure written to {}.", path.display());
    } else {
        let _ = writeln!(stdout, "{rendered}");
    }
    exit_code
}

fn parse_metrics_args(args: &[String]) -> Result<MetricsArgs, String> {
    if args.first().map(String::as_str) != Some("metrics") {
        return Err("Expected `norn metrics`.".to_string());
    }
    let mut filter = ReviewEffectivenessFilter::default();
    let mut format = OutputFormat::Human;
    let mut output = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--tenant" => filter.tenant_id = next_value(args, &mut index)?,
            "--provider" => {
                filter.provider = Some(match next_value(args, &mut index)?.as_str() {
                    "github" => PullRequestReviewEventProvider::Github,
                    "bitbucket" => PullRequestReviewEventProvider::Bitbucket,
                    _ => return Err("`--provider` must be `github` or `bitbucket`.".to_string()),
                });
            }
            "--workspace" => filter.workspace = Some(next_value(args, &mut index)?),
            "--repo" => filter.repo = Some(next_value(args, &mut index)?),
            "--from" => {
                filter.from_ms = Some(parse_metrics_timestamp(
                    "--from",
                    &next_value(args, &mut index)?,
                )?);
            }
            "--to" => {
                filter.to_ms = Some(parse_metrics_timestamp(
                    "--to",
                    &next_value(args, &mut index)?,
                )?);
            }
            "--format" => {
                format = match next_value(args, &mut index)?.as_str() {
                    "human" | "text" => OutputFormat::Human,
                    "json" => OutputFormat::Json,
                    _ => return Err("`--format` must be `human` or `json`.".to_string()),
                };
            }
            "--json" => format = OutputFormat::Json,
            "--output" => output = Some(PathBuf::from(next_value(args, &mut index)?)),
            unknown => return Err(format!("Unknown metrics option `{unknown}`.")),
        }
        index += 1;
    }
    if filter.repo.is_some() && filter.workspace.is_none() {
        return Err("`--repo` requires `--workspace`.".to_string());
    }
    if matches!((filter.from_ms, filter.to_ms), (Some(from), Some(to)) if from >= to) {
        return Err("`--from` must be less than `--to`.".to_string());
    }
    Ok(MetricsArgs {
        filter,
        format,
        output,
    })
}

fn parse_metrics_timestamp(option: &str, value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .ok()
        .filter(|timestamp| *timestamp >= 0)
        .ok_or_else(|| format!("`{option}` must be non-negative Unix milliseconds."))
}

fn run_metrics(args: MetricsArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let report = match review_storage::review_effectiveness_metrics(args.filter) {
        Ok(report) => report,
        Err(error) => {
            let _ = writeln!(stderr, "Could not aggregate review metrics: {error}");
            return 1;
        }
    };
    let rendered = match args.format {
        OutputFormat::Human => format_metrics_human(&report),
        OutputFormat::Json => match serde_json::to_string_pretty(&report) {
            Ok(json) => json,
            Err(error) => {
                let _ = writeln!(stderr, "Failed to serialize metrics output: {error}");
                return 7;
            }
        },
    };
    if let Some(path) = args.output {
        if let Err(error) = std::fs::write(&path, format!("{rendered}\n")) {
            let _ = writeln!(stderr, "Failed to write {}: {error}", path.display());
            return 7;
        }
        let _ = writeln!(stderr, "Metrics written to {}.", path.display());
    } else {
        let _ = writeln!(stdout, "{rendered}");
    }
    0
}

fn format_metrics_human(report: &ReviewEffectivenessReport) -> String {
    let mut output = String::new();
    let summary = &report.summary;
    output.push_str("Norn review effectiveness\n");
    output.push_str(&format!("Tenant: {}\n", report.filter.tenant_id));
    if let Some(provider) = report.filter.provider {
        output.push_str(&format!("Provider: {}\n", provider.as_str()));
    }
    if let Some(workspace) = report.filter.workspace.as_deref() {
        output.push_str(&format!("Workspace: {workspace}\n"));
    }
    if let Some(repo) = report.filter.repo.as_deref() {
        output.push_str(&format!("Repository: {repo}\n"));
    }
    output.push_str(&format!(
        "Window: {} to {} (completion time, start inclusive/end exclusive)\n",
        report
            .filter
            .from_ms
            .map_or_else(|| "unbounded".to_string(), |value| value.to_string()),
        report
            .filter
            .to_ms
            .map_or_else(|| "unbounded".to_string(), |value| value.to_string())
    ));
    append_metrics_summary(&mut output, summary);
    if !report.repositories.is_empty() {
        output.push_str("Repositories:\n");
        for repository in &report.repositories {
            output.push_str(&format!(
                "- {} {}/{}: {} reviews, {} findings\n",
                repository.provider.as_str(),
                repository.workspace,
                repository.repo,
                repository.summary.review_count,
                repository.summary.finding_count
            ));
        }
    }
    output.trim_end().to_string()
}

fn append_metrics_summary(output: &mut String, summary: &ReviewEffectivenessSummary) {
    output.push_str(&format!("Reviews: {}\n", summary.review_count));
    output.push_str(&format!("Findings: {}\n", summary.finding_count));
    output.push_str(&format!(
        "By severity: {}\n",
        format_counts(&summary.findings_by_severity)
    ));
    output.push_str(&format!(
        "By category: {}\n",
        format_counts(&summary.findings_by_category)
    ));
    output.push_str(&format!(
        "Feedback coverage: {}\n",
        format_rate(&summary.feedback.coverage_rate)
    ));
    output.push_str(&format!(
        "Accepted: {}\n",
        format_rate(&summary.feedback.acceptance_rate)
    ));
    output.push_str(&format!(
        "False positives: {}\n",
        format_rate(&summary.feedback.false_positive_rate)
    ));
    output.push_str(&format!(
        "Fixed: {}\n",
        format_rate(&summary.feedback.fixed_rate)
    ));
    let latency = &summary.time_to_first_review;
    if let Some(average_ms) = latency.average_ms {
        output.push_str(&format!(
            "Time to first review: {average_ms} ms average across {} pull requests\n",
            latency.sample_count
        ));
    } else {
        output.push_str("Time to first review: no completed pull-request samples\n");
    }
}

fn format_counts(counts: &[crate::review_metrics::ReviewMetricCount]) -> String {
    counts
        .iter()
        .map(|item| format!("{}={}", item.key, item.count))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_rate(rate: &crate::review_metrics::ReviewMetricRate) -> String {
    match rate.basis_points {
        Some(basis_points) => format!(
            "{}/{} ({}.{:02}%)",
            rate.numerator,
            rate.denominator,
            basis_points / 100,
            basis_points % 100
        ),
        None => format!("{}/{} (n/a)", rate.numerator, rate.denominator),
    }
}

fn parse_config_validate_args(args: &[String]) -> Result<ConfigValidateArgs, String> {
    if args.first().map(String::as_str) != Some("config")
        || args.get(1).map(String::as_str) != Some("validate")
    {
        return Err("Expected `norn config validate`.".to_string());
    }

    let mut repo_path = PathBuf::from(".");
    let mut profile = None;
    let mut format = OutputFormat::Human;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--repo-path" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "`--repo-path` requires a value.".to_string())?;
                repo_path = PathBuf::from(value);
            }
            "--profile" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "`--profile` requires a value.".to_string())?;
                profile = Some(value.to_string());
            }
            "--format" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "`--format` requires a value.".to_string())?;
                format = match value.as_str() {
                    "human" | "text" => OutputFormat::Human,
                    "json" => OutputFormat::Json,
                    _ => return Err("`--format` must be `human` or `json`.".to_string()),
                };
            }
            "--json" => {
                format = OutputFormat::Json;
            }
            unknown => return Err(format!("Unknown option `{unknown}`.")),
        }
        index += 1;
    }

    Ok(ConfigValidateArgs {
        repo_path,
        profile,
        format,
    })
}

fn parse_config_migrate_args(args: &[String]) -> Result<ConfigMigrateArgs, String> {
    if args.first().map(String::as_str) != Some("config")
        || args.get(1).map(String::as_str) != Some("migrate")
    {
        return Err("Expected `norn config migrate`.".to_string());
    }

    let mut repo_path = PathBuf::from(".");
    let mut dry_run = false;
    let mut format = OutputFormat::Human;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--repo-path" => {
                index += 1;
                repo_path = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "`--repo-path` requires a value.".to_string())?,
                );
            }
            "--dry-run" => dry_run = true,
            "--format" => {
                index += 1;
                format = match args.get(index).map(String::as_str) {
                    Some("human" | "text") => OutputFormat::Human,
                    Some("json") => OutputFormat::Json,
                    Some(_) => return Err("`--format` must be `human` or `json`.".to_string()),
                    None => return Err("`--format` requires a value.".to_string()),
                };
            }
            "--json" => format = OutputFormat::Json,
            unknown => return Err(format!("Unknown option `{unknown}`.")),
        }
        index += 1;
    }

    Ok(ConfigMigrateArgs {
        repo_path,
        dry_run,
        format,
    })
}

fn run_config_migrate(
    args: ConfigMigrateArgs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let result = match repo_config::migrate_repository_config(&args.repo_path, args.dry_run) {
        Ok(result) => result,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return 1;
        }
    };
    match args.format {
        OutputFormat::Json => match serde_json::to_string_pretty(&result) {
            Ok(json) => {
                let _ = writeln!(stdout, "{json}");
            }
            Err(error) => {
                let _ = writeln!(stderr, "Failed to serialize migration output: {error}");
                return 1;
            }
        },
        OutputFormat::Human => {
            let verb = if result.dry_run {
                "Would migrate"
            } else {
                "Migrated"
            };
            if result.actions.is_empty() {
                let _ = writeln!(stdout, "No legacy repository configuration found.");
            }
            for action in result.actions {
                let _ = writeln!(stdout, "{verb} {} -> {}", action.source, action.target);
                for change in action.content_changes {
                    let _ = writeln!(stdout, "  - {change}");
                }
            }
        }
    }
    0
}

fn run_config_validate(
    args: ConfigValidateArgs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let result = match repo_config::load_from_repo_path_with_profile(
        &args.repo_path,
        args.profile.as_deref(),
    ) {
        Ok(result) => result,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return 1;
        }
    };
    let valid = result.errors.is_empty();
    let output = ConfigValidateOutput {
        valid,
        repo_path: result.repo_path,
        config_path: result.config_path,
        exists: result.exists,
        selected_profile: result.selected_profile,
        prompt_replaces_default: result
            .config
            .as_ref()
            .and_then(|config| config.review.as_ref())
            .and_then(|review| review.prompt.as_ref())
            .and_then(|prompt| prompt.replace.as_deref())
            .map(str::trim)
            .is_some_and(|prompt| !prompt.is_empty()),
        loaded_policy_packs: result.loaded_policy_packs,
        warnings: result.warnings,
        errors: result.errors,
    };

    match args.format {
        OutputFormat::Human => {
            let _ = write_human_output(&output, stdout);
        }
        OutputFormat::Json => match serde_json::to_string_pretty(&output) {
            Ok(json) => {
                let _ = writeln!(stdout, "{json}");
            }
            Err(error) => {
                let _ = writeln!(stderr, "Failed to serialize validation output: {error}");
                return 1;
            }
        },
    }

    if valid {
        0
    } else {
        2
    }
}

fn write_human_output(output: &ConfigValidateOutput, out: &mut dyn Write) -> io::Result<()> {
    if output.valid {
        writeln!(out, "Norn config valid")?;
    } else {
        writeln!(out, "Norn config invalid")?;
    }
    writeln!(out, "Repo: {}", output.repo_path)?;
    writeln!(out, "Config: {}", output.config_path)?;
    if !output.exists {
        writeln!(
            out,
            "No .norn.yaml or compatible legacy config found; using built-in defaults."
        )?;
    }
    if let Some(profile) = output.selected_profile.as_deref() {
        writeln!(out, "Profile: {profile}")?;
    }
    if output.prompt_replaces_default {
        writeln!(out, "Prompt: replaces built-in default")?;
    }
    if !output.loaded_policy_packs.is_empty() {
        writeln!(out, "Loaded policy packs:")?;
        for pack in &output.loaded_policy_packs {
            writeln!(out, "- {} ({})", pack.id, pack.path)?;
        }
    }
    if !output.warnings.is_empty() {
        writeln!(out, "Warnings:")?;
        for warning in &output.warnings {
            writeln!(out, "- {}: {}", warning.path, warning.message)?;
        }
    }
    if !output.errors.is_empty() {
        writeln!(out, "Errors:")?;
        for error in &output.errors {
            writeln!(out, "- {}: {}", error.path, error.message)?;
        }
    }
    Ok(())
}

fn usage() -> &'static str {
    "Usage:
Review:
  norn review [--repo-path <path>] [--scope working-tree|branch|pr]
                 [--base <ref>] [--pr <id>] [--workspace <name>] [--repo <slug>]
                 [--provider github|bitbucket] [--profile <name>]
                 [--ai-provider codex|claude] [--model <name>] [--effort <level>]
                 [--format markdown|json] [--json] [--output <path>]
                 [--fail-on-findings] [--min-severity info|low|medium|high|critical]
                 [--run-analyzers] [--allow-provider-diff]
Metrics:
  norn metrics [--tenant <id>] [--provider github|bitbucket]
                  [--workspace <name>] [--repo <slug>]
                  [--from <unix-ms>] [--to <unix-ms>]
                  [--format human|json] [--json] [--output <path>]
Config validation:
  norn config validate [--repo-path <path>] [--profile <name>]
                          [--format human|json] [--json]
Config migration:
  norn config migrate [--repo-path <path>] [--dry-run]
                         [--format human|json] [--json]
Onboarding:
  norn setup [--allow-provider-diff|--deny-provider-diff]
               [--format human|json] [--json] [--dry-run] [--yes]
  norn init [--repo-path <path>] [--quick|--guided] [--dry-run] [--yes]
              [--format human|json] [--json]
Credentials:
  norn auth status [--format human|json] [--json]
  norn auth login github|bitbucket [--username <name>] [--token-stdin]
  norn auth logout github|bitbucket
Agent skills:
  norn skills install|status|uninstall --agent codex|claude|all
                                      [--force] [--format human|json] [--json]
Doctor:
  norn doctor [--repo-path <path>] [--machine-only]
              [--format human|json] [--json]"
}

fn doctor_usage() -> &'static str {
    "Usage:
  norn doctor [--repo-path <path>] [--machine-only]
              [--format human|json] [--json]

`doctor` reports read-only machine and repository readiness, including
provider tooling, credential state, analyzer setup, and git metadata."
}

fn review_usage() -> &'static str {
    "Usage:
  norn review [--repo-path <path>] [--scope working-tree|branch|pr]
                 [--base <ref>] [--pr <id>] [--workspace <name>] [--repo <slug>]
                 [--provider github|bitbucket] [--profile <name>]
                 [--ai-provider codex|claude] [--model <name>] [--effort <level>]
                 [--format markdown|json] [--json] [--output <path>]
                 [--fail-on-findings] [--min-severity info|low|medium|high|critical]
                 [--run-analyzers] [--allow-provider-diff]"
}

fn metrics_usage() -> &'static str {
    "Usage:
  norn metrics [--tenant <id>] [--provider github|bitbucket]
                  [--workspace <name>] [--repo <slug>]
                  [--from <unix-ms>] [--to <unix-ms>]
                  [--format human|json] [--json] [--output <path>]

The completion-time window is start-inclusive and end-exclusive.
The default tenant is `local`."
}

fn evaluate_usage() -> &'static str {
    "Usage:
  norn evaluate [--corpus <path>] [--baseline <path>] [--output <path>]

Runs the versioned offline review-quality corpus and emits JSON.
The command exits 1 when a configured baseline regression is detected."
}

#[cfg(test)]
mod tests {
    use super::{
        collect_setup_credentials, create_headless_data_dir, format_metrics_human,
        parse_evaluate_args, parse_init_args, parse_metrics_args, parse_review_args,
        parse_setup_args, review_needs_headless_storage, run_args, run_setup_with_inventory,
        write_review_failure, InitMode, OutputFormat, ReviewOutputFormat, SetupArgs,
        SetupCredentialReport, SetupToolReport,
    };
    use crate::config::{AiProvider, AppConfig};
    use crate::headless_review::{HeadlessReviewError, ReviewScope};
    use crate::readiness;
    use crate::review_event::PullRequestReviewEventProvider;
    use crate::review_metrics::{aggregate_review_effectiveness, ReviewEffectivenessFilter};
    use crate::services::review::ReviewFindingSeverity;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_REPO_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct EnvVarGuard {
        name: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(name: &'static str, value: &str) -> Self {
            let original = std::env::var_os(name);
            std::env::set_var(name, value);
            Self { name, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.original.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }

    fn temp_repo() -> PathBuf {
        let nonce = TEMP_REPO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "lachesi-cli-config-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp repo");
        path
    }

    fn setup_tools(available: bool) -> Vec<SetupToolReport> {
        let version = available.then(|| "test-provider 1.0.0".to_string());
        vec![
            SetupToolReport {
                provider: "codex".to_string(),
                available: false,
                version: None,
                required: false,
            },
            SetupToolReport {
                provider: "claude".to_string(),
                available,
                version,
                required: true,
            },
        ]
    }

    fn setup_credentials_fixture() -> Vec<SetupCredentialReport> {
        vec![
            SetupCredentialReport {
                provider: "github".to_string(),
                available: true,
                source: "keychain or env reference".to_string(),
            },
            SetupCredentialReport {
                provider: "bitbucket".to_string(),
                available: true,
                source: "keychain or env reference".to_string(),
            },
        ]
    }

    fn run_setup_for_test(
        format: OutputFormat,
        tool_available: bool,
        credentials: Vec<SetupCredentialReport>,
        stdout: &mut Vec<u8>,
        stderr: &mut Vec<u8>,
    ) -> i32 {
        let tools = setup_tools(tool_available);

        run_setup_with_inventory(
            SetupArgs {
                format,
                dry_run: false,
                yes: false,
                provider_diff_consent: None,
            },
            stdout,
            stderr,
            AppConfig::default(),
            tools,
            credentials,
        )
    }

    #[test]
    fn headless_data_dirs_are_unique_and_private() {
        let first = create_headless_data_dir().expect("first temp dir");
        let second = create_headless_data_dir().expect("second temp dir");

        assert_ne!(first.path(), second.path());
        assert!(first.path().is_dir());
        assert!(second.path().is_dir());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(first.path())
                .expect("temp dir metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700);
        }
    }

    #[test]
    fn setup_args_accepts_formats_and_defaults_to_apply_mode() {
        let parsed =
            parse_setup_args(&["setup".to_string(), "--json".to_string()]).expect("setup parse");

        assert_eq!(parsed.format, OutputFormat::Json);
        assert!(!parsed.yes);
        assert!(!parsed.dry_run);
        assert_eq!(parsed.provider_diff_consent, None);

        let parsed = parse_setup_args(&["setup".to_string()]).expect("setup parse");

        assert_eq!(parsed.format, OutputFormat::Human);
        assert!(!parsed.yes);
        assert!(!parsed.dry_run);
        assert_eq!(parsed.provider_diff_consent, None);
    }

    #[test]
    fn setup_args_parse_and_validate_provider_diff_consent() {
        let allow = parse_setup_args(&[
            "setup".to_string(),
            "--allow-provider-diff".to_string(),
            "--yes".to_string(),
        ])
        .expect("allow consent");
        assert_eq!(allow.provider_diff_consent, Some(true));

        let deny = parse_setup_args(&["setup".to_string(), "--deny-provider-diff".to_string()])
            .expect("deny consent");
        assert_eq!(deny.provider_diff_consent, Some(false));

        let error = parse_setup_args(&[
            "setup".to_string(),
            "--allow-provider-diff".to_string(),
            "--deny-provider-diff".to_string(),
        ])
        .expect_err("opposite choices must fail");
        assert!(error.contains("mutually exclusive"));
    }

    #[test]
    fn setup_args_rejects_conflicting_mode_flags() {
        let error = parse_setup_args(&[
            "setup".to_string(),
            "--yes".to_string(),
            "--dry-run".to_string(),
        ])
        .expect_err("conflict should fail");

        assert!(error.contains("`--yes` and `--dry-run`"));
    }

    #[test]
    fn init_args_defaults_and_modes_are_parsed() {
        let parsed = parse_init_args(&[
            "init".to_string(),
            "--repo-path".to_string(),
            "/tmp/x".to_string(),
        ])
        .expect("init parse");

        assert_eq!(parsed.repo_path, PathBuf::from("/tmp/x"));
        assert_eq!(parsed.mode, InitMode::Quick);
        assert!(parsed.dry_run);
        assert!(!parsed.yes);
        assert_eq!(parsed.format, OutputFormat::Human);

        let parsed = parse_init_args(&[
            "init".to_string(),
            "--guided".to_string(),
            "--yes".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ])
        .expect("guided init parse");

        assert_eq!(parsed.mode, InitMode::Guided);
        assert!(!parsed.dry_run);
        assert!(parsed.yes);
        assert_eq!(parsed.format, OutputFormat::Json);
    }

    #[test]
    fn init_args_rejects_conflicting_mode_flags_between_modes() {
        let error = parse_init_args(&[
            "init".to_string(),
            "--yes".to_string(),
            "--dry-run".to_string(),
        ])
        .expect_err("conflict should fail");

        assert!(error.contains("`--yes` and `--dry-run`"));
    }

    #[test]
    fn init_args_rejects_conflicting_mode_flags() {
        let error = parse_init_args(&[
            "init".to_string(),
            "--guided".to_string(),
            "--quick".to_string(),
        ])
        .expect_err("conflict should fail");

        assert!(error.contains("`--quick` and `--guided`"));
    }

    #[test]
    fn init_run_dry_run_does_not_apply_changes() {
        let repo = temp_repo();
        fs::write(repo.join(".lachesi.yaml"), "version: 0.1\n").expect("legacy config");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "init".to_string(),
                "--repo-path".to_string(),
                repo.to_string_lossy().to_string(),
                "--dry-run".to_string(),
                "--json".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(repo.join(".lachesi.yaml").exists());
        assert!(!repo.join(".norn.yaml").exists());
        assert!(stderr.is_empty());

        let output: Value = serde_json::from_slice(&stdout).expect("json output");
        assert_eq!(output["dryRun"], true);
        assert!(!output["actions"].as_array().expect("actions").is_empty());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn init_run_dry_run_suggests_default_repo_config_when_missing() {
        let repo = temp_repo();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "init".to_string(),
                "--repo-path".to_string(),
                repo.to_string_lossy().to_string(),
                "--dry-run".to_string(),
                "--json".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(!repo.join(".norn.yaml").exists());
        assert!(stderr.is_empty());

        let output: Value = serde_json::from_slice(&stdout).expect("json output");
        let actions = output["actions"].as_array().expect("actions");
        let default_init_action = actions.iter().find(|action| {
            action["target"]
                .as_str()
                .is_some_and(|target| target.ends_with(".norn.yaml"))
                && action["kind"].as_str() == Some("file")
        });
        assert!(default_init_action.is_some());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn init_run_yes_writes_default_repo_config() {
        let repo = temp_repo();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "init".to_string(),
                "--repo-path".to_string(),
                repo.to_string_lossy().to_string(),
                "--yes".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(repo.join(".norn.yaml").exists());
        assert!(stderr.is_empty());
        let contents = fs::read_to_string(repo.join(".norn.yaml")).expect("config contents");
        assert!(contents.contains("version: 0.1"));
        assert!(contents.contains("review:\n  mode: balanced"));
        assert!(String::from_utf8_lossy(&stdout).contains("Migration applied."));

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "init".to_string(),
                "--repo-path".to_string(),
                repo.to_string_lossy().to_string(),
                "--yes".to_string(),
                "--json".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        let output: Value = serde_json::from_slice(&stdout).expect("json output");
        assert_eq!(output["dryRun"], false);
        assert!(output["actions"].as_array().expect("actions").is_empty());
        assert!(stderr.is_empty());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn init_run_yes_applies_and_is_idempotent() {
        let repo = temp_repo();
        fs::write(repo.join(".lachesi.yaml"), "version: 0.1\n").expect("legacy config");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "init".to_string(),
                "--repo-path".to_string(),
                repo.to_string_lossy().to_string(),
                "--yes".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(repo.join(".norn.yaml").exists());
        assert!(!repo.join(".lachesi.yaml").exists());
        assert!(stderr.is_empty());
        assert!(String::from_utf8_lossy(&stdout).contains("Migration applied."));

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "init".to_string(),
                "--repo-path".to_string(),
                repo.to_string_lossy().to_string(),
                "--yes".to_string(),
                "--json".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        let output: Value = serde_json::from_slice(&stdout).expect("json output");
        assert_eq!(output["dryRun"], false);
        assert!(output["actions"].as_array().expect("actions").is_empty());
        assert!(stderr.is_empty());

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn init_run_rejects_invalid_repository_config_with_guidance() {
        let repo = temp_repo();
        fs::write(
            repo.join(".norn.yaml"),
            r#"
version: 2.0
review:
  mode: fast
"#,
        )
        .expect("legacy config");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "init".to_string(),
                "--repo-path".to_string(),
                repo.to_string_lossy().to_string(),
                "--yes".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        let stderr = String::from_utf8(stderr).expect("stderr");
        assert!(stderr.contains("Cannot run onboarding with repository config"));
        assert!(stderr.contains("norn doctor --repo-path"));
        let preserved = fs::read_to_string(repo.join(".norn.yaml")).expect("config remains");
        assert!(preserved.contains("version: 2.0"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn init_run_does_not_alter_valid_repo_config_with_unknown_fields() {
        let repo = temp_repo();
        let original = r#"
# Keep unknown keys for compatibility
version: 0.1
review:
  mode: balanced
x-custom:
  keep: yes
prompt:
  replace: |
    keep this prompt override
"#;
        fs::write(repo.join(".norn.yaml"), original).expect("legacy config");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "init".to_string(),
                "--repo-path".to_string(),
                repo.to_string_lossy().to_string(),
                "--dry-run".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let after = fs::read_to_string(repo.join(".norn.yaml")).expect("config remains");
        assert_eq!(after, original);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn init_run_requires_yes_for_guided_mode() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &["init".to_string(), "--guided".to_string()],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        let stderr = String::from_utf8(stderr).expect("stderr");
        assert!(stderr.contains("Guided mode requires `--yes`"));
    }

    #[test]
    fn setup_parser_rejects_unknown_options() {
        let error = parse_setup_args(&["setup".to_string(), "--mystery".to_string()])
            .expect_err("unknown setup flag should fail");
        assert!(error.contains("Unknown option"));
    }

    #[test]
    fn metrics_arguments_select_tenant_repository_range_and_json() {
        let parsed = parse_metrics_args(&[
            "metrics".to_string(),
            "--tenant".to_string(),
            "tenant-acme".to_string(),
            "--provider".to_string(),
            "github".to_string(),
            "--workspace".to_string(),
            "acme".to_string(),
            "--repo".to_string(),
            "payments".to_string(),
            "--from".to_string(),
            "1000".to_string(),
            "--to".to_string(),
            "2000".to_string(),
            "--json".to_string(),
        ])
        .expect("metrics arguments");

        assert_eq!(parsed.filter.tenant_id, "tenant-acme");
        assert_eq!(
            parsed.filter.provider,
            Some(PullRequestReviewEventProvider::Github)
        );
        assert_eq!(parsed.filter.workspace.as_deref(), Some("acme"));
        assert_eq!(parsed.filter.repo.as_deref(), Some("payments"));
        assert_eq!(parsed.filter.from_ms, Some(1000));
        assert_eq!(parsed.filter.to_ms, Some(2000));
        assert_eq!(parsed.format, OutputFormat::Json);
    }

    #[test]
    fn metrics_human_report_documents_zero_feedback_and_latency_samples() {
        let report = aggregate_review_effectiveness(&[], &[], ReviewEffectivenessFilter::default())
            .expect("empty metrics");
        let human = format_metrics_human(&report);

        assert!(human.contains("Tenant: local"));
        assert!(human.contains("By severity: critical=0, high=0, info=0, low=0, medium=0"));
        assert!(human
            .contains("By category: architecture=0, bug=0, docs=0, maintainability=0, other=0"));
        assert!(human.contains("Feedback coverage: 0/0 (n/a)"));
        assert!(human.contains("Time to first review: no completed pull-request samples"));
    }

    #[test]
    fn metrics_help_returns_zero_without_opening_storage() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &["metrics".to_string(), "--help".to_string()],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let output = String::from_utf8(stdout).expect("metrics help");
        assert!(output.contains("norn metrics"));
        assert!(output.contains("start-inclusive and end-exclusive"));
    }

    #[test]
    fn doctor_help_exposes_readiness_options() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &["doctor".to_string(), "--help".to_string()],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let output = String::from_utf8(stdout).expect("doctor help");
        assert!(output.contains("norn doctor"));
        assert!(output.contains("--machine-only"));
        assert!(output.contains("--format human|json"));
    }

    #[test]
    fn doctor_defaults_with_invalid_format_is_rejected() {
        let error = readiness::parse_doctor_args(&[
            "doctor".to_string(),
            "--format".to_string(),
            "xml".to_string(),
        ])
        .expect_err("invalid doctor format should be rejected");

        assert!(error.contains("`--format` must be"));
    }

    #[test]
    fn doctor_json_returns_status_and_schema() {
        let repo = temp_repo();
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args(["init", "--initial-branch", "main"])
            .output()
            .expect("git init");
        fs::write(repo.join("README.md"), "hello\n").expect("write readme");
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "README.md"])
            .output()
            .expect("git add");
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "-c",
                "user.email=ci@example.com",
                "-c",
                "user.name=CI",
                "commit",
                "-m",
                "init",
            ])
            .output()
            .expect("git commit");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "doctor".to_string(),
                "--repo-path".to_string(),
                repo.display().to_string(),
                "--json".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stderr.is_empty());
        let output: serde_json::Value = serde_json::from_slice(&stdout).expect("doctor output");
        assert_eq!(output["schemaVersion"], "norn.readiness.v1");
        assert_eq!(output["status"].as_str(), Some("fail"));
        assert!(output["issues"]
            .as_array()
            .is_some_and(|issues| !issues.is_empty()));
    }

    #[test]
    fn config_validate_returns_zero_for_valid_config() {
        let repo = temp_repo();
        fs::write(repo.join(".norn.yaml"), "version: 0.1\n").expect("write config");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "config".to_string(),
                "validate".to_string(),
                "--repo-path".to_string(),
                repo.display().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(String::from_utf8(stdout)
            .expect("stdout")
            .contains("Norn config valid"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn config_validate_returns_two_for_invalid_config_json() {
        let repo = temp_repo();
        fs::write(
            repo.join(".norn.yaml"),
            r#"
version: 0.1
token: unsafe
"#,
        )
        .expect("write config");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "config".to_string(),
                "validate".to_string(),
                "--repo-path".to_string(),
                repo.display().to_string(),
                "--json".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stderr.is_empty());
        let output = String::from_utf8(stdout).expect("stdout");
        assert!(output.contains("\"valid\": false"));
        assert!(output.contains("looks like a credential"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn init_json_report_includes_repository_path_and_mode() {
        let repo = temp_repo();

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "init".to_string(),
                "--repo-path".to_string(),
                repo.display().to_string(),
                "--dry-run".to_string(),
                "--json".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let output: serde_json::Value = serde_json::from_slice(&stdout).expect("init json output");
        assert_eq!(output["schemaVersion"], "norn.init.v1");
        assert_eq!(output["repoPath"], repo.display().to_string());
        assert_eq!(output["mode"], "quick");
        assert_eq!(output["dryRun"], true);
        let proposal = output["proposal"].as_object().expect("proposal");
        assert_eq!(proposal["mode"], "quick");
        assert!(proposal["projectTypes"].is_array());
        assert!(proposal["taskRunners"].is_array());
        assert!(proposal["instructionSources"].is_array());
        assert!(proposal["analyzerCandidates"].is_array());
        assert!(proposal["configPreview"].is_string());
    }

    #[test]
    fn setup_output_reports_real_env_credential_state_without_secret_values() {
        let github_token = "ghs_live_SECRET_TOKEN_FOR_TEST_DO_NOT_LEAK";
        let bitbucket_token = "bb_secret_TOKEN_FOR_TEST_DO_NOT_LEAK";
        let bitbucket_user = "test-user-for-test";
        let _github_token = EnvVarGuard::set("GITHUB_TOKEN", github_token);
        let _bitbucket_token = EnvVarGuard::set("BITBUCKET_TOKEN", bitbucket_token);
        let _bitbucket_user = EnvVarGuard::set("BITBUCKET_USERNAME", bitbucket_user);
        let credentials = collect_setup_credentials();

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_setup_for_test(
            OutputFormat::Json,
            false,
            credentials.clone(),
            &mut stdout,
            &mut stderr,
        );

        assert!(stderr.is_empty());
        assert_eq!(code, 1);
        let output: Value = serde_json::from_slice(&stdout).expect("setup json output");
        assert_eq!(output["machineCredentials"][0]["provider"], "github");
        assert_eq!(output["machineCredentials"][0]["available"], true);
        assert!(output["machineCredentials"][0].get("token").is_none());
        assert!(output["machineCredentials"][0].get("username").is_none());
        let rendered = output.to_string();
        assert!(!rendered.contains(github_token));
        assert!(!rendered.contains(bitbucket_token));
        assert!(!rendered.contains(bitbucket_user));

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_setup_for_test(
            OutputFormat::Human,
            false,
            credentials,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 1);
        assert!(stderr.is_empty());
        let output = String::from_utf8(stdout).expect("setup human output");
        assert!(output.contains("- github: found (keychain or env reference)"));
        assert!(!output.contains(github_token));
        assert!(!output.contains(bitbucket_token));
        assert!(!output.contains(bitbucket_user));
    }

    #[test]
    fn setup_json_report_includes_schema_and_setup_state() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_setup_for_test(
            OutputFormat::Json,
            false,
            setup_credentials_fixture(),
            &mut stdout,
            &mut stderr,
        );

        assert!(stderr.is_empty());
        let output: serde_json::Value = serde_json::from_slice(&stdout).expect("setup json output");
        assert_eq!(code, 1);
        assert_eq!(output["schemaVersion"], "norn.setup.v1");
        assert_eq!(output["wouldApply"], false);
        assert!(output["machineTools"].as_array().is_some());
        assert!(output["machineCredentials"].as_array().is_some());
        assert!(output["setupNotes"].is_array());
        assert_eq!(output["providerDiffSharingAllowed"], false);
        assert_eq!(output["proposedProviderDiffSharingAllowed"], false);
        assert!(!output["machineTools"]
            .as_array()
            .expect("machine tools")
            .is_empty());
    }

    #[test]
    fn setup_preview_does_not_report_unpersisted_diff_consent_as_active() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_setup_with_inventory(
            SetupArgs {
                format: OutputFormat::Json,
                dry_run: false,
                yes: false,
                provider_diff_consent: Some(true),
            },
            &mut stdout,
            &mut stderr,
            AppConfig::default(),
            setup_tools(true),
            setup_credentials_fixture(),
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let output: Value = serde_json::from_slice(&stdout).expect("setup preview JSON");
        assert_eq!(output["providerDiffSharingAllowed"], false);
        assert_eq!(output["proposedProviderDiffSharingAllowed"], true);
        assert!(output["setupNotes"]
            .as_array()
            .is_some_and(|notes| notes.iter().any(|note| note
                .as_str()
                .is_some_and(|note| note.contains("Re-run with `--yes`")))));
    }

    #[test]
    fn setup_returns_zero_when_the_selected_provider_is_available() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_setup_for_test(
            OutputFormat::Json,
            true,
            setup_credentials_fixture(),
            &mut stdout,
            &mut stderr,
        );

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        let output: Value = serde_json::from_slice(&stdout).expect("setup json output");
        assert_eq!(output["selectedAiProvider"], "claude");
        assert_eq!(output["machineTools"][1]["available"], true);
        assert_eq!(output["machineTools"][1]["version"], "test-provider 1.0.0");
    }

    #[test]
    fn config_usage_errors_return_config_exit_code() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "config".to_string(),
                "validate".to_string(),
                "--unknown".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr)
            .expect("stderr")
            .contains("Unknown option"));
    }

    #[test]
    fn config_validate_accepts_profile_override() {
        let repo = temp_repo();
        fs::write(
            repo.join(".norn.yaml"),
            r#"
version: 0.1
profiles:
  strict:
    mode: strict
"#,
        )
        .expect("write config");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "config".to_string(),
                "validate".to_string(),
                "--repo-path".to_string(),
                repo.display().to_string(),
                "--profile".to_string(),
                "strict".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(String::from_utf8(stdout)
            .expect("stdout")
            .contains("Profile: strict"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn config_migrate_dry_run_previews_without_mutating() {
        let repo = temp_repo();
        fs::write(
            repo.join(".lachesi.yaml"),
            "version: 0.1\npolicy:\n  packs:\n    - .lachesi/packs/team\n",
        )
        .expect("legacy config");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_args(
            &[
                "config".to_string(),
                "migrate".to_string(),
                "--repo-path".to_string(),
                repo.display().to_string(),
                "--dry-run".to_string(),
                "--json".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let output: serde_json::Value = serde_json::from_slice(&stdout).expect("migration JSON");
        assert_eq!(output["dryRun"], true);
        assert_eq!(output["actions"][0]["kind"], "file");
        assert!(output["actions"][0]["contentChanges"]
            .as_array()
            .is_some_and(|changes| !changes.is_empty()));
        assert!(repo.join(".lachesi.yaml").exists());
        assert!(!repo.join(".norn.yaml").exists());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn review_defaults_to_working_tree_markdown() {
        let args = parse_review_args(&["review".to_string()]).expect("parse review args");
        assert_eq!(args.scope, ReviewScope::WorkingTree);
        assert_eq!(args.format, ReviewOutputFormat::Markdown);
        assert_eq!(args.repo_path, None);
        assert!(!args.run_analyzers);
        assert!(!args.allow_provider_diff);
    }

    #[test]
    fn review_pr_flag_selects_pr_scope_and_structured_options() {
        let args = parse_review_args(&[
            "review".to_string(),
            "--pr".to_string(),
            "42".to_string(),
            "--json".to_string(),
            "--ai-provider".to_string(),
            "codex".to_string(),
            "--fail-on-findings".to_string(),
            "--min-severity".to_string(),
            "medium".to_string(),
            "--allow-provider-diff".to_string(),
        ])
        .expect("parse review args");

        assert_eq!(args.scope, ReviewScope::PullRequest);
        assert_eq!(args.pr_id, Some(42));
        assert_eq!(args.format, ReviewOutputFormat::Json);
        assert_eq!(args.ai_provider, Some(AiProvider::Codex));
        assert!(args.fail_on_findings);
        assert_eq!(args.min_severity, Some(ReviewFindingSeverity::Medium));
        assert!(args.allow_provider_diff);
    }

    #[test]
    fn review_rejects_zero_pull_request_id() {
        let error = parse_review_args(&["review".to_string(), "--pr".to_string(), "0".to_string()])
            .expect_err("zero should not be accepted as a pull request id");

        assert!(error.contains("positive integer"));
    }

    #[test]
    fn review_rejects_an_option_token_as_a_value() {
        let error = parse_review_args(&[
            "review".to_string(),
            "--repo-path".to_string(),
            "--json".to_string(),
        ])
        .expect_err("option token must not be accepted as a value");

        assert_eq!(error, "`--repo-path` requires a value.");
    }

    #[test]
    fn review_rejects_conflicting_target_options() {
        let conflicting_pr = parse_review_args(&[
            "review".to_string(),
            "--scope".to_string(),
            "working-tree".to_string(),
            "--pr".to_string(),
            "42".to_string(),
        ])
        .expect_err("explicit working-tree scope must not ignore --pr");
        assert!(conflicting_pr.contains("requires `--scope pr`"));

        let misplaced_base = parse_review_args(&[
            "review".to_string(),
            "--base".to_string(),
            "main".to_string(),
        ])
        .expect_err("working-tree scope must not ignore --base");
        assert!(misplaced_base.contains("requires `--scope branch`"));

        let partial_identity = parse_review_args(&[
            "review".to_string(),
            "--workspace".to_string(),
            "delaudio".to_string(),
        ])
        .expect_err("partial repository identity must be rejected");
        assert!(partial_identity.contains("provided together"));
    }

    #[test]
    fn review_usage_errors_return_config_exit_code() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &["review".to_string(), "--unknown".to_string()],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr)
            .expect("stderr")
            .contains("Unknown review option"));
    }

    #[test]
    fn review_usage_errors_honor_json_output() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "review".to_string(),
                "--json".to_string(),
                "--unknown".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stderr.is_empty());
        let output: serde_json::Value =
            serde_json::from_slice(&stdout).expect("JSON usage failure");
        assert_eq!(output["schemaVersion"], "norn.headless-review.v1");
        assert_eq!(output["status"], "failed");
        assert_eq!(output["exitCode"], 2);
        assert!(output["error"]
            .as_str()
            .is_some_and(|error| error.contains("Unknown review option")));
    }

    #[test]
    fn review_runtime_failures_include_a_machine_readable_code() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = write_review_failure(
            HeadlessReviewError {
                exit_code: 6,
                code: "review.sandboxRestricted",
                message: "Run outside the sandbox.".to_string(),
            },
            ReviewOutputFormat::Json,
            None,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 6);
        assert!(stderr.is_empty());
        let output: serde_json::Value =
            serde_json::from_slice(&stdout).expect("JSON runtime failure");
        assert_eq!(output["errorCode"], "review.sandboxRestricted");
    }

    #[test]
    fn review_usage_errors_write_json_to_requested_output() {
        let temp_dir = tempfile::tempdir().expect("output temp dir");
        let output_path = temp_dir.path().join("review.json");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "review".to_string(),
                "--json".to_string(),
                "--output".to_string(),
                output_path.to_string_lossy().to_string(),
                "--unknown".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr)
            .expect("stderr")
            .contains("Review failure written"));
        let output: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output_path).expect("JSON usage failure output"))
                .expect("JSON usage failure");
        assert_eq!(output["status"], "failed");
        assert_eq!(output["exitCode"], 2);
    }

    #[test]
    fn review_usage_errors_do_not_treat_an_option_as_output_path() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "review".to_string(),
                "--json".to_string(),
                "--output".to_string(),
                "--unknown".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stderr.is_empty());
        let output: serde_json::Value =
            serde_json::from_slice(&stdout).expect("JSON usage failure");
        assert_eq!(output["status"], "failed");
        assert!(output["error"]
            .as_str()
            .is_some_and(|error| error.contains("`--output` requires a value")));
    }

    #[test]
    fn headless_storage_is_only_needed_for_valid_review_runs() {
        assert!(review_needs_headless_storage(&["review".to_string()]));
        assert!(!review_needs_headless_storage(&[
            "review".to_string(),
            "--help".to_string()
        ]));
        assert!(!review_needs_headless_storage(&[
            "review".to_string(),
            "--unknown".to_string()
        ]));
    }

    #[test]
    fn review_help_returns_zero_and_advertises_review_options() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &["review".to_string(), "--help".to_string()],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let output = String::from_utf8(stdout).expect("help output");
        assert!(output.contains("norn review"));
        assert!(output.contains("--scope working-tree|branch|pr"));
        assert!(output.contains("--run-analyzers"));
        assert!(output.contains("--allow-provider-diff"));
        assert!(output.contains("--json"));
        assert!(!output.contains("config validate"));
    }

    #[test]
    fn version_uses_the_canonical_binary_name() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_args(&["--version".to_string()], &mut stdout, &mut stderr);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert_eq!(
            String::from_utf8(stdout).expect("version output"),
            format!("norn {}\n", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn review_analyzers_are_explicit_opt_in() {
        let args = parse_review_args(&["review".to_string(), "--run-analyzers".to_string()])
            .expect("parse analyzer opt-in");

        assert!(args.run_analyzers);
    }

    #[test]
    fn evaluate_defaults_to_the_versioned_corpus_and_baseline() {
        let args = parse_evaluate_args(&["evaluate".to_string()]).expect("parse evaluate");

        assert_eq!(
            args.corpus,
            PathBuf::from("fixtures/review-evaluation/v1/corpus.json")
        );
        assert_eq!(
            args.baseline,
            PathBuf::from("fixtures/review-evaluation/v1/baseline.json")
        );
        assert!(args.output.is_none());
    }

    #[test]
    fn service_command_routes_to_the_self_hosted_runtime() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &["service".to_string(), "invalid".to_string()],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr)
            .expect("usage")
            .contains("norn service <run|smoke|healthcheck|backup|restore>"));
    }

    #[test]
    fn review_json_failure_uses_stable_envelope() {
        let repo = temp_repo();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_args(
            &[
                "review".to_string(),
                "--repo-path".to_string(),
                repo.display().to_string(),
                "--json".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 4);
        let output: serde_json::Value =
            serde_json::from_slice(&stdout).expect("failure should be JSON");
        assert_eq!(output["schemaVersion"], "norn.headless-review.v1");
        assert_eq!(output["status"], "failed");
        assert_eq!(output["exitCode"], 4);
        assert!(output["error"]
            .as_str()
            .is_some_and(|error| !error.is_empty()));
        let _ = fs::remove_dir_all(repo);
    }
}
