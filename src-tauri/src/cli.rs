use std::io::{self, Write};
use std::path::PathBuf;

use serde::Serialize;

use crate::config::{AiProvider, ReviewProvider};
use crate::headless_review::{self, HeadlessReviewRequest, ReviewScope};
use crate::repo_config::{self, LoadedPolicyPack, RepoConfigValidationMessage};
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigValidateArgs {
    repo_path: PathBuf,
    profile: Option<String>,
    format: OutputFormat,
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

struct HeadlessDataDirGuard {
    temp_dir: Option<tempfile::TempDir>,
}

impl HeadlessDataDirGuard {
    fn install() -> Result<Self, String> {
        if std::env::var_os("LACHESI_DATA_DIR").is_some() {
            return Ok(Self { temp_dir: None });
        }
        let temp_dir = create_headless_data_dir()?;
        std::env::set_var("LACHESI_DATA_DIR", temp_dir.path());
        Ok(Self {
            temp_dir: Some(temp_dir),
        })
    }
}

fn create_headless_data_dir() -> Result<tempfile::TempDir, String> {
    let temp_dir = tempfile::Builder::new()
        .prefix("lachesi-headless-storage-")
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
            std::env::remove_var("LACHESI_DATA_DIR");
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
    let _headless_data_dir = if args.first().map(String::as_str) == Some("review") {
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

fn is_cli_command(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("config" | "review" | "--help" | "-h")
    )
}

fn run_args(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if args.first().map(String::as_str) == Some("review")
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--help" || arg == "-h")
    {
        let _ = writeln!(stdout, "{}", review_usage());
        return 0;
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        let _ = writeln!(stdout, "{}", usage());
        return 0;
    }

    match args.first().map(String::as_str) {
        Some("review") => match parse_review_args(args) {
            Ok(args) => run_review(args, stdout, stderr),
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

fn parse_review_args(args: &[String]) -> Result<ReviewArgs, String> {
    if args.first().map(String::as_str) != Some("review") {
        return Err("Expected `lachesi review`.".to_string());
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
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("`{}` requires a value.", args[index.saturating_sub(1)]))
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
        "schemaVersion": "lachesi.headless-review.v1",
        "status": "failed",
        "exitCode": exit_code,
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

fn parse_config_validate_args(args: &[String]) -> Result<ConfigValidateArgs, String> {
    if args.first().map(String::as_str) != Some("config")
        || args.get(1).map(String::as_str) != Some("validate")
    {
        return Err("Expected `lachesi config validate`.".to_string());
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
        writeln!(out, "Lachesi config valid")?;
    } else {
        writeln!(out, "Lachesi config invalid")?;
    }
    writeln!(out, "Repo: {}", output.repo_path)?;
    writeln!(out, "Config: {}", output.config_path)?;
    if !output.exists {
        writeln!(out, "No .lachesi.yaml found; using built-in defaults.")?;
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
  lachesi review [--repo-path <path>] [--scope working-tree|branch|pr]
                 [--base <ref>] [--pr <id>] [--workspace <name>] [--repo <slug>]
                 [--provider github|bitbucket] [--profile <name>]
                 [--ai-provider codex|claude] [--model <name>] [--effort <level>]
                 [--format markdown|json] [--json] [--output <path>]
                 [--fail-on-findings] [--min-severity info|low|medium|high|critical]
                 [--run-analyzers]
Config validation:
  lachesi config validate [--repo-path <path>] [--profile <name>]
                          [--format human|json] [--json]"
}

fn review_usage() -> &'static str {
    "Usage:
  lachesi review [--repo-path <path>] [--scope working-tree|branch|pr]
                 [--base <ref>] [--pr <id>] [--workspace <name>] [--repo <slug>]
                 [--provider github|bitbucket] [--profile <name>]
                 [--ai-provider codex|claude] [--model <name>] [--effort <level>]
                 [--format markdown|json] [--json] [--output <path>]
                 [--fail-on-findings] [--min-severity info|low|medium|high|critical]
                 [--run-analyzers]"
}

#[cfg(test)]
mod tests {
    use super::{create_headless_data_dir, parse_review_args, run_args, ReviewOutputFormat};
    use crate::config::AiProvider;
    use crate::headless_review::ReviewScope;
    use crate::services::review::ReviewFindingSeverity;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_REPO_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_repo() -> PathBuf {
        let nonce = TEMP_REPO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "lachesi-cli-config-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp repo");
        path
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
    fn config_validate_returns_zero_for_valid_config() {
        let repo = temp_repo();
        fs::write(repo.join(".lachesi.yaml"), "version: 0.1\n").expect("write config");
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
            .contains("Lachesi config valid"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn config_validate_returns_two_for_invalid_config_json() {
        let repo = temp_repo();
        fs::write(
            repo.join(".lachesi.yaml"),
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
            repo.join(".lachesi.yaml"),
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
    fn review_defaults_to_working_tree_markdown() {
        let args = parse_review_args(&["review".to_string()]).expect("parse review args");
        assert_eq!(args.scope, ReviewScope::WorkingTree);
        assert_eq!(args.format, ReviewOutputFormat::Markdown);
        assert_eq!(args.repo_path, None);
        assert!(!args.run_analyzers);
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
        ])
        .expect("parse review args");

        assert_eq!(args.scope, ReviewScope::PullRequest);
        assert_eq!(args.pr_id, Some(42));
        assert_eq!(args.format, ReviewOutputFormat::Json);
        assert_eq!(args.ai_provider, Some(AiProvider::Codex));
        assert!(args.fail_on_findings);
        assert_eq!(args.min_severity, Some(ReviewFindingSeverity::Medium));
    }

    #[test]
    fn review_rejects_zero_pull_request_id() {
        let error = parse_review_args(&["review".to_string(), "--pr".to_string(), "0".to_string()])
            .expect_err("zero should not be accepted as a pull request id");

        assert!(error.contains("positive integer"));
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
            "lachesi-hq".to_string(),
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
        assert!(output.contains("lachesi review"));
        assert!(output.contains("--scope working-tree|branch|pr"));
        assert!(output.contains("--run-analyzers"));
        assert!(output.contains("--json"));
        assert!(!output.contains("config validate"));
    }

    #[test]
    fn review_analyzers_are_explicit_opt_in() {
        let args = parse_review_args(&["review".to_string(), "--run-analyzers".to_string()])
            .expect("parse analyzer opt-in");

        assert!(args.run_analyzers);
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
        assert_eq!(output["schemaVersion"], "lachesi.headless-review.v1");
        assert_eq!(output["status"], "failed");
        assert_eq!(output["exitCode"], 4);
        assert!(output["error"]
            .as_str()
            .is_some_and(|error| !error.is_empty()));
        let _ = fs::remove_dir_all(repo);
    }
}
