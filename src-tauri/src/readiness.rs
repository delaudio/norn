#![allow(
    non_snake_case,
    reason = "readiness DTO fields preserve the stable camelCase machine-output contract"
)]

use crate::{config, credentials, local_repo, repo_config};
use serde::Serialize;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const NORN_CONFIG_FILE: &str = ".norn.yaml";
const NORN_CONFIG_DIR: &str = ".norn";
const LEGACY_CONFIG_FILE: &str = ".lachesi.yaml";
const LEGACY_CONFIG_DIR: &str = ".lachesi";
const NORN_LOCAL_CONFIG_FILE: &str = ".norn.local.yaml";
const LEGACY_LOCAL_CONFIG_FILE: &str = ".lachesi.local.yaml";
const SCHEMA_VERSION: &str = "norn.readiness.v1";
const MAX_EVIDENCE_SCAN_DEPTH: usize = 3;
const MAX_TEXT_SCAN_BYTES: u64 = 32 * 1024;
const MAX_LEGACY_SCAN_FILES: usize = 10_000;

const LEGACY_SCAN_SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".gradle",
    ".idea",
    "coverage",
    ".parcel-cache",
];

const LEGACY_ALLOWLIST_TEXT: &[&str] = &[
    ".lachesi.yaml",
    ".lachesi.local.yaml",
    ".lachesi.",
    ".lachesi/",
    "app.lachesi.desktop",
    "lachesi.sqlite3",
    "LACHESI_DATA_DIR",
    "LACHESI_REVIEW_DATA_DIR",
    "LACHESI_SERVICE_DATA_DIR",
    "LACHESI_SERVICE_BIND_ADDR",
    "lachesi-pack.yaml",
    ".lachesi",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadinessIssueSeverity {
    Error,
    Warning,
    Info,
}

impl ReadinessIssueSeverity {
    fn sort_rank(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadinessStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadinessIssueScope {
    Machine,
    Repository,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessIssue {
    pub severity: ReadinessIssueSeverity,
    pub scope: ReadinessIssueScope,
    pub code: String,
    pub message: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInfo {
    pub path: String,
    pub exists: bool,
    pub writable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityInfo {
    pub legacyConfigDirExists: bool,
    pub legacyDataDirExists: bool,
    pub usingLegacyConfigAlias: bool,
    pub usingLegacyDataAlias: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialState {
    pub provider: String,
    pub available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliToolState {
    pub provider: String,
    pub required: bool,
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineState {
    pub os: String,
    pub configDirectory: PathInfo,
    pub dataDirectory: PathInfo,
    pub compatibility: CompatibilityInfo,
    pub credentials: Vec<CredentialState>,
    pub cliTools: Vec<CliToolState>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryEvidenceState {
    pub manifestFiles: Vec<String>,
    pub projectTypes: Vec<String>,
    pub taskRunners: Vec<String>,
    pub instructionSources: Vec<String>,
    pub generatedPaths: Vec<String>,
    pub vendorPaths: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRemoteState {
    pub source: String,
    pub provider: String,
    pub workspace: String,
    pub repo: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingTreeState {
    pub dirty: bool,
    pub untrackedFiles: usize,
    pub statusLineCount: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoConfigState {
    pub requestedPath: String,
    pub configPath: Option<String>,
    pub exists: bool,
    pub canonicalSource: bool,
    pub canonicalDirectory: bool,
    pub legacySource: bool,
    pub legacyDirectory: bool,
    pub localOverridesCanonical: bool,
    pub localOverridesLegacy: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoAnalyzerState {
    pub name: String,
    pub command: String,
    pub enabled: bool,
    pub required: bool,
    pub resolved: bool,
    pub commandPath: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryState {
    pub inspected: bool,
    pub requestedPath: String,
    pub skipped: bool,
    pub gitRoot: Option<String>,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub remote: Option<RepositoryRemoteState>,
    pub workingTree: Option<WorkingTreeState>,
    pub config: Option<RepoConfigState>,
    pub selectedProfile: Option<String>,
    pub evidence: RepositoryEvidenceState,
    pub analyzers: Vec<RepoAnalyzerState>,
    pub configWarnings: Vec<String>,
    pub configErrors: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub schemaVersion: &'static str,
    pub status: ReadinessStatus,
    pub timestamp: String,
    pub machine: MachineState,
    pub repository: RepositoryState,
    pub issues: Vec<ReadinessIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorFormat {
    Human,
    Json,
}

#[derive(Debug)]
pub struct DoctorArgs {
    pub repo_path: PathBuf,
    pub machine_only: bool,
    pub format: DoctorFormat,
}

pub fn parse_doctor_args(args: &[String]) -> Result<DoctorArgs, String> {
    if args.first().map(String::as_str) != Some("doctor") {
        return Err("Expected `norn doctor`.".to_string());
    }

    let mut repo_path = PathBuf::from(".");
    let mut machine_only = false;
    let mut format = DoctorFormat::Human;
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
            "--machine-only" => machine_only = true,
            "--format" => {
                index += 1;
                format = match args.get(index).map(String::as_str) {
                    Some("human") | Some("text") => DoctorFormat::Human,
                    Some("json") => DoctorFormat::Json,
                    Some(_) => {
                        return Err("`--format` must be `human`, `text`, or `json`.".to_string())
                    }
                    None => return Err("`--format` requires a value.".to_string()),
                }
            }
            "--json" => format = DoctorFormat::Json,
            unknown => return Err(format!("Unknown doctor option `{unknown}`.")),
        }
        index += 1;
    }

    Ok(DoctorArgs {
        repo_path,
        machine_only,
        format,
    })
}

pub fn run_doctor(
    args: DoctorArgs,
    stdout: &mut dyn std::io::Write,
    _stderr: &mut dyn std::io::Write,
) -> i32 {
    let mut report = collect_report(&args.repo_path, args.machine_only);
    sort_issues(&mut report.issues);
    if report.issues.is_empty() {
        report.issues.push(ReadinessIssue {
            severity: ReadinessIssueSeverity::Info,
            scope: ReadinessIssueScope::Machine,
            code: "readiness.ok".to_string(),
            message: "No actionable readiness issues were found.".to_string(),
            remediation:
                "Review the repository and machine checks, then rerun the onboarding flow."
                    .to_string(),
        });
        report.status = ReadinessStatus::Ok;
    } else {
        report.status = derive_status(&report.issues);
    }

    match args.format {
        DoctorFormat::Json => {
            let rendered = match serde_json::to_string_pretty(&report) {
                Ok(json) => json,
                Err(error) => {
                    let _ = std::io::Write::write_all(_stderr, format!("{error}").as_bytes());
                    return 7;
                }
            };
            let _ = std::io::Write::write_all(stdout, format!("{rendered}\n").as_bytes());
        }
        DoctorFormat::Human => {
            let rendered = format_readiness_human(&report);
            let _ = std::io::Write::write_all(stdout, format!("{rendered}\n").as_bytes());
        }
    }

    match report.status {
        ReadinessStatus::Fail => 2,
        ReadinessStatus::Warn => 1,
        _ => 0,
    }
}

pub fn collect_report(repo_path: &Path, machine_only: bool) -> DoctorReport {
    let mut issues = Vec::new();
    let machine = collect_machine_state(&mut issues);
    let repository = if machine_only {
        collect_repository_state_skipped(repo_path)
    } else {
        collect_repository_state(repo_path, &mut issues)
    };

    DoctorReport {
        schemaVersion: SCHEMA_VERSION,
        status: ReadinessStatus::Ok,
        timestamp: unix_millis_timestamp(),
        machine,
        repository,
        issues,
    }
}

pub fn derive_status(issues: &[ReadinessIssue]) -> ReadinessStatus {
    if issues
        .iter()
        .any(|issue| issue.severity == ReadinessIssueSeverity::Error)
    {
        ReadinessStatus::Fail
    } else if issues
        .iter()
        .any(|issue| issue.severity == ReadinessIssueSeverity::Warning)
    {
        ReadinessStatus::Warn
    } else {
        ReadinessStatus::Ok
    }
}

fn sort_issues(issues: &mut [ReadinessIssue]) {
    issues.sort_by(|lhs, rhs| {
        let lhs_rank = lhs.severity.sort_rank();
        let rhs_rank = rhs.severity.sort_rank();
        lhs_rank
            .cmp(&rhs_rank)
            .then(lhs.scope.cmp(&rhs.scope))
            .then(lhs.code.cmp(&rhs.code))
            .then(lhs.message.cmp(&rhs.message))
    });
}

fn collect_machine_state(issues: &mut Vec<ReadinessIssue>) -> MachineState {
    let config_dir = dirs::config_dir();
    let config_directory = match &config_dir {
        Some(dir) => {
            let dir = dir.join("norn");
            let exists = dir.is_dir();
            let writable = path_writable(&dir);
            if !dir.exists() {
                issues.push(ReadinessIssue {
                    severity: ReadinessIssueSeverity::Info,
                    scope: ReadinessIssueScope::Machine,
                    code: "machine.configDirMissing".to_string(),
                    message: "Norn app config directory is not present yet.".to_string(),
                    remediation: "Run normal app flows once to initialize application data, or create the directory path manually.".to_string(),
                });
            }
            if !writable {
                issues.push(ReadinessIssue {
                    severity: ReadinessIssueSeverity::Warning,
                    scope: ReadinessIssueScope::Machine,
                    code: "machine.configDirNotWritable".to_string(),
                    message: "Norn app config directory is not writable.".to_string(),
                    remediation:
                        "Adjust filesystem permissions so norn can persist settings and reviews."
                            .to_string(),
                });
            }
            PathInfo {
                path: dir.display().to_string(),
                exists,
                writable,
            }
        }
        None => {
            issues.push(ReadinessIssue {
                severity: ReadinessIssueSeverity::Error,
                scope: ReadinessIssueScope::Machine,
                code: "machine.configDirUnavailable".to_string(),
                message: "Cannot resolve OS config directory.".to_string(),
                remediation: "Set a valid OS config directory for this process and retry."
                    .to_string(),
            });
            PathInfo {
                path: String::new(),
                exists: false,
                writable: false,
            }
        }
    };

    let data_directory = resolve_data_directory();
    if !data_directory.exists {
        issues.push(ReadinessIssue {
            severity: ReadinessIssueSeverity::Info,
            scope: ReadinessIssueScope::Machine,
            code: "machine.dataDirMissing".to_string(),
            message: "Norn review data directory is not present yet.".to_string(),
            remediation: "Start a review flow once to initialize local data, or create the directory path manually.".to_string(),
        });
    } else if !data_directory.writable {
        issues.push(ReadinessIssue {
            severity: ReadinessIssueSeverity::Warning,
            scope: ReadinessIssueScope::Machine,
            code: "machine.dataDirNotWritable".to_string(),
            message: "Norn review data directory is not writable.".to_string(),
            remediation: "Adjust filesystem permissions so review artifacts can be persisted."
                .to_string(),
        });
    }

    let app_cfg = config::load();
    let requested_tool = if app_cfg.ai_provider == config::AiProvider::Codex {
        "codex"
    } else {
        "claude"
    };

    let codex = probe_cli_tool("codex", requested_tool == "codex");
    let claude = probe_cli_tool("claude", requested_tool == "claude");
    if !codex.available {
        issues.push(ReadinessIssue {
            severity: ReadinessIssueSeverity::Info,
            scope: ReadinessIssueScope::Machine,
            code: "machine.codexToolUnavailable".to_string(),
            message: "Codex CLI is not configured/available.".to_string(),
            remediation:
                "Install `codex` CLI if you want to use it as your configured review provider."
                    .to_string(),
        });
    }
    if !claude.available {
        issues.push(ReadinessIssue {
            severity: ReadinessIssueSeverity::Info,
            scope: ReadinessIssueScope::Machine,
            code: "machine.claudeToolUnavailable".to_string(),
            message: "Claude CLI is not configured/available.".to_string(),
            remediation:
                "Install `claude` CLI if you want to use it as your configured review provider."
                    .to_string(),
        });
    }

    let compatibility = CompatibilityInfo {
        legacyConfigDirExists: dirs::config_dir()
            .map(|dir| dir.join(LEGACY_CONFIG_DIR).is_dir())
            .unwrap_or(false),
        legacyDataDirExists: data_directory
            .canonical
            .parent()
            .map(|parent| parent.join("lachesi").is_dir())
            .unwrap_or(false),
        usingLegacyConfigAlias: false,
        usingLegacyDataAlias: data_directory.using_legacy_alias,
    };

    if compatibility.legacyConfigDirExists {
        issues.push(ReadinessIssue {
            severity: ReadinessIssueSeverity::Info,
            scope: ReadinessIssueScope::Machine,
            code: "machine.legacyConfigAliasPresent".to_string(),
            message: "Legacy config directory (.lachesi) is present for compatibility.".to_string(),
            remediation: "Remove legacy-only files when migration is complete or keep this note for compatibility mode.".to_string(),
        });
    }

    if compatibility.legacyDataDirExists && !compatibility.usingLegacyDataAlias {
        issues.push(ReadinessIssue {
            severity: ReadinessIssueSeverity::Info,
            scope: ReadinessIssueScope::Machine,
            code: "machine.legacyDataAliasPresent".to_string(),
            message: "Legacy data root (.lachesi) is present for compatibility.".to_string(),
            remediation:
                "Keep until data migration completes, then remove legacy roots if desired."
                    .to_string(),
        });
    }

    MachineState {
        os: std::env::consts::OS.to_string(),
        configDirectory: config_directory,
        dataDirectory: PathInfo {
            path: data_directory.canonical.to_string_lossy().to_string(),
            exists: data_directory.exists,
            writable: data_directory.writable,
        },
        compatibility,
        credentials: vec![
            CredentialState {
                provider: "bitbucket".to_string(),
                available: credentials::has(),
            },
            CredentialState {
                provider: "github".to_string(),
                available: credentials::has_github(),
            },
            CredentialState {
                provider: "jira".to_string(),
                available: credentials::has_jira(),
            },
            CredentialState {
                provider: "notion".to_string(),
                available: credentials::has_notion(),
            },
        ],
        cliTools: vec![codex, claude],
    }
}

fn collect_repository_state(repo_path: &Path, issues: &mut Vec<ReadinessIssue>) -> RepositoryState {
    let requested_path = repo_path.to_string_lossy().to_string();
    let Some(path) = repo_path.to_path_buf().canonicalize().ok() else {
        issues.push(ReadinessIssue {
            severity: ReadinessIssueSeverity::Error,
            scope: ReadinessIssueScope::Repository,
            code: "repository.pathInvalid".to_string(),
            message: "Repository path is not a directory.".to_string(),
            remediation: "Use a valid local repository path with `--repo-path` and retry."
                .to_string(),
        });
        return RepositoryState {
            inspected: false,
            requestedPath: requested_path,
            skipped: false,
            gitRoot: None,
            branch: None,
            head: None,
            remote: None,
            workingTree: None,
            config: None,
            selectedProfile: None,
            evidence: RepositoryEvidenceState {
                manifestFiles: Vec::new(),
                projectTypes: Vec::new(),
                taskRunners: Vec::new(),
                instructionSources: Vec::new(),
                generatedPaths: Vec::new(),
                vendorPaths: Vec::new(),
            },
            analyzers: Vec::new(),
            configWarnings: Vec::new(),
            configErrors: Vec::new(),
        };
    };

    let git_root = match run_git_command(&path, "rev-parse", &["--show-toplevel"]) {
        Ok(root) => PathBuf::from(root),
        Err(error) => {
            issues.push(ReadinessIssue {
                severity: ReadinessIssueSeverity::Error,
                scope: ReadinessIssueScope::Repository,
                code: "repository.notAGitRepo".to_string(),
                message: error,
                remediation:
                    "Run `norn doctor` from within a Git clone or pass `--repo-path` to a valid repository."
                        .to_string(),
            });
            return RepositoryState {
                inspected: false,
                requestedPath: requested_path,
                skipped: false,
                gitRoot: Some(path.to_string_lossy().to_string()),
                branch: None,
                head: None,
                remote: None,
                workingTree: None,
                config: None,
                selectedProfile: None,
                evidence: collect_repository_evidence(&path),
                analyzers: Vec::new(),
                configWarnings: Vec::new(),
                configErrors: Vec::new(),
            };
        }
    };

    let branch = match run_git_command(&git_root, "rev-parse", &["--abbrev-ref", "HEAD"]) {
        Ok(branch) => Some(branch),
        Err(error) => {
            issues.push(ReadinessIssue {
                severity: ReadinessIssueSeverity::Error,
                scope: ReadinessIssueScope::Repository,
                code: "repository.branchUnavailable".to_string(),
                message: error,
                remediation:
                    "Inspect repository HEAD state and retry (run a git command from repository)."
                        .to_string(),
            });
            None
        }
    };

    let head = match run_git_command(&git_root, "rev-parse", &["HEAD"]) {
        Ok(head) => Some(head),
        Err(error) => {
            issues.push(ReadinessIssue {
                severity: ReadinessIssueSeverity::Error,
                scope: ReadinessIssueScope::Repository,
                code: "repository.headUnavailable".to_string(),
                message: error,
                remediation: "Verify HEAD exists and repository has at least one commit."
                    .to_string(),
            });
            None
        }
    };

    let working_tree = match run_git_command(&git_root, "status", &["--porcelain=v1"]) {
        Ok(status) => {
            let lines: Vec<&str> = status
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect();
            let untracked = lines.iter().filter(|line| line.starts_with("??")).count();
            Some(WorkingTreeState {
                dirty: !lines.is_empty(),
                untrackedFiles: untracked,
                statusLineCount: lines.len(),
            })
        }
        Err(error) => {
            issues.push(ReadinessIssue {
                severity: ReadinessIssueSeverity::Warning,
                scope: ReadinessIssueScope::Repository,
                code: "repository.statusUnavailable".to_string(),
                message: error,
                remediation:
                    "Check repository status permissions (read/write) and rerun the check."
                        .to_string(),
            });
            None
        }
    };
    if let Some(state) = &working_tree {
        if state.dirty {
            issues.push(ReadinessIssue {
                severity: ReadinessIssueSeverity::Warning,
                scope: ReadinessIssueScope::Repository,
                code: "repository.workingTreeDirty".to_string(),
                message: "Repository working tree has local changes.".to_string(),
                remediation: "Commit or stash local changes before running onboarding review."
                    .to_string(),
            });
        }
        if state.untrackedFiles > 0 {
            issues.push(ReadinessIssue {
                severity: ReadinessIssueSeverity::Info,
                scope: ReadinessIssueScope::Repository,
                code: "repository.untrackedFiles".to_string(),
                message: format!(
                    "Repository working tree has {} untracked path(s).",
                    state.untrackedFiles
                ),
                remediation:
                    "Review untracked files and include/exclude them according to your policy."
                        .to_string(),
            });
        }
    }

    let (remote, remote_warnings) = read_remote_state(&git_root);
    if let Some(warning) = remote_warnings {
        issues.push(warning);
    }

    let repo_config_path = read_repo_config_state(&git_root);
    let mut selected_profile = None;
    let mut config_errors = Vec::new();
    let mut config_warnings = Vec::new();
    let mut analyzers = Vec::new();

    match repo_config::load_from_repo_path(&git_root) {
        Ok(result) => {
            for warning in result.warnings {
                config_warnings.push(warning.message.clone());
                issues.push(ReadinessIssue {
                    severity: ReadinessIssueSeverity::Warning,
                    scope: ReadinessIssueScope::Repository,
                    code: "repository.configWarning".to_string(),
                    message: warning.message,
                    remediation: "Inspect config content and align it to supported schema."
                        .to_string(),
                });
            }
            for error in result.errors {
                config_errors.push(error.message.clone());
                issues.push(ReadinessIssue {
                    severity: ReadinessIssueSeverity::Error,
                    scope: ReadinessIssueScope::Repository,
                    code: "repository.configError".to_string(),
                    message: error.message,
                    remediation: "Fix repository config schema and values before proceeding."
                        .to_string(),
                });
            }
            if let Some(config) = &result.config {
                for (name, analyzer) in &config.analyzers {
                    let command = analyzer.command.clone().unwrap_or_default();
                    let enabled = analyzer.enabled;
                    let required = analyzer.required;
                    let (resolved, command_path) = resolve_command_path(&command);
                    if !command.is_empty() {
                        analyzers.push(RepoAnalyzerState {
                            name: name.to_string(),
                            command,
                            enabled,
                            required,
                            resolved,
                            commandPath: command_path,
                        });
                        if !resolved {
                            issues.push(ReadinessIssue {
                                severity: ReadinessIssueSeverity::Warning,
                                scope: ReadinessIssueScope::Repository,
                                code: "repository.analyzerUnavailable".to_string(),
                                message: format!(
                                    "Analyzer `{}` command is configured but cannot be resolved."
                                        , name
                                ),
                                remediation:
                                    "Install the analyzer binary or update the command in repository config."
                                        .to_string(),
                            });
                        }
                    } else {
                        analyzers.push(RepoAnalyzerState {
                            name: name.to_string(),
                            command: String::new(),
                            enabled,
                            required,
                            resolved: false,
                            commandPath: None,
                        });
                    }
                }
            }
            selected_profile = result.selected_profile;
        }
        Err(error) => {
            issues.push(ReadinessIssue {
                severity: ReadinessIssueSeverity::Error,
                scope: ReadinessIssueScope::Repository,
                code: "repository.configLoadFailed".to_string(),
                message: error,
                remediation: "Fix config layout/precedence and rerun readiness probe.".to_string(),
            });
            config_errors.push("Config could not be loaded".to_string());
        }
    }

    let config_state = repo_config_path;
    let has_precedence_conflict = config_state.exists
        && (config_state.canonicalDirectory || config_state.canonicalSource)
        && (config_state.legacyDirectory || config_state.legacySource)
        || (config_state.localOverridesCanonical && config_state.localOverridesLegacy);
    if has_precedence_conflict {
        issues.push(ReadinessIssue {
            severity: ReadinessIssueSeverity::Error,
            scope: ReadinessIssueScope::Repository,
            code: "repository.configPrecedenceConflict".to_string(),
            message: "Repository has both canonical and legacy config roots; precedence is ambiguous."
                .to_string(),
            remediation:
                "Keep only `.norn`/`.norn.yaml` or only `.lachesi`/`.lachesi.yaml`, plus a single local override file."
                .to_string(),
        });
    }

    let legacy_reference_hits = find_unapproved_legacy_references(&git_root);
    if !legacy_reference_hits.is_empty() {
        let hit_list = legacy_reference_hits
            .iter()
            .take(5)
            .map(|hit| hit.as_str())
            .collect::<Vec<_>>();
        issues.push(ReadinessIssue {
            severity: ReadinessIssueSeverity::Error,
            scope: ReadinessIssueScope::Repository,
            code: "repository.legacyNameNotAllowed".to_string(),
            message: format!(
                "Found {} unapproved legacy-name references: {}",
                legacy_reference_hits.len(),
                hit_list.join(", ")
            ),
            remediation: "Replace legacy `lachesi` references with canonical `norn` equivalents or add migration gating exceptions before release."
                .to_string(),
        });
    }

    RepositoryState {
        inspected: true,
        requestedPath: requested_path,
        skipped: false,
        gitRoot: Some(git_root.to_string_lossy().to_string()),
        branch,
        head,
        remote,
        workingTree: working_tree,
        config: Some(config_state),
        evidence: collect_repository_evidence(&git_root),
        selectedProfile: selected_profile,
        analyzers,
        configWarnings: config_warnings,
        configErrors: config_errors,
    }
}

fn collect_repository_state_skipped(repo_path: &Path) -> RepositoryState {
    RepositoryState {
        inspected: false,
        requestedPath: repo_path.to_string_lossy().to_string(),
        skipped: true,
        gitRoot: None,
        branch: None,
        head: None,
        remote: None,
        workingTree: None,
        config: None,
        selectedProfile: None,
        evidence: collect_repository_evidence(repo_path),
        analyzers: Vec::new(),
        configWarnings: Vec::new(),
        configErrors: Vec::new(),
    }
}

pub(crate) fn collect_repository_evidence(repo_path: &Path) -> RepositoryEvidenceState {
    let mut manifests = Vec::new();
    let mut project_types = Vec::new();
    let mut task_runners = Vec::new();
    let mut instruction_sources = Vec::new();
    let mut generated_paths = Vec::new();
    let mut vendor_paths = Vec::new();

    const MANIFEST_PATHS: &[(&str, &str)] = &[
        ("Cargo.toml", "rust"),
        ("package.json", "javascript"),
        ("pnpm-lock.yaml", "node"),
        ("yarn.lock", "node"),
        ("package-lock.json", "node"),
        ("pyproject.toml", "python"),
        ("requirements.txt", "python"),
        ("poetry.lock", "python"),
        ("go.mod", "go"),
        ("pom.xml", "java"),
        ("build.gradle", "java"),
        ("gradlew", "java"),
        ("Gemfile", "ruby"),
        ("composer.json", "php"),
        ("mix.exs", "elixir"),
        ("requirements-dev.txt", "python"),
        ("deno.json", "deno"),
    ];
    const TASK_RUNNERS: &[(&str, &str)] = &[
        ("Makefile", "make"),
        ("makefile", "make"),
        ("justfile", "just"),
        ("Taskfile.yml", "task"),
        ("Taskfile.yaml", "task"),
    ];
    const INSTRUCTIONS: &[&str] = &[
        "AGENTS.md",
        "README.instructions.md",
        "agent.md",
        "INSTRUCTIONS.md",
        ".github/copilot-instructions.md",
        "CLAUDE.md",
        ".github/instructions.md",
    ];
    const GENERATED_PATHS: &[&str] = &[
        "dist",
        "build",
        "target",
        "out",
        ".next",
        ".nuxt",
        "coverage",
        ".parcel-cache",
    ];
    const VENDOR_PATHS: &[&str] = &["vendor", "node_modules", ".gradle", "build", "target"];

    let mut evidence_dirs = vec![(repo_path.to_path_buf(), 0usize)];
    while let Some((path, depth)) = evidence_dirs.pop() {
        if depth >= MAX_EVIDENCE_SCAN_DEPTH {
            continue;
        }
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if entry_path.is_file() {
                if MANIFEST_PATHS
                    .iter()
                    .any(|(path, _)| *path == name.as_str())
                {
                    manifests.push(entry_path.to_string_lossy().to_string());
                    if let Some((_, project_type)) = MANIFEST_PATHS
                        .iter()
                        .find(|(path, _)| *path == name.as_str())
                    {
                        project_types.push((*project_type).to_string());
                    }
                }
                if TASK_RUNNERS
                    .iter()
                    .any(|(runner, _)| *runner == name.as_str())
                {
                    if let Some((_, runner_name)) = TASK_RUNNERS
                        .iter()
                        .find(|(runner, _)| *runner == name.as_str())
                    {
                        task_runners.push((*runner_name).to_string());
                    }
                }
                if INSTRUCTIONS.contains(&name.as_str())
                    || entry_path.ends_with(".github/copilot-instructions.md")
                    || entry_path.ends_with(".github/instructions.md")
                {
                    instruction_sources.push(entry_path.to_string_lossy().to_string());
                }
                continue;
            }

            if entry_path.is_dir() {
                if VENDOR_PATHS.iter().any(|vendor| vendor == &name.as_str()) {
                    vendor_paths.push(entry_path.to_string_lossy().to_string());
                    continue;
                }
                if matches!(name.as_str(), ".git" | ".idea" | "node_modules") {
                    continue;
                }
                if depth < MAX_EVIDENCE_SCAN_DEPTH {
                    evidence_dirs.push((entry_path.clone(), depth + 1));
                }
                if GENERATED_PATHS.contains(&name.as_str()) {
                    generated_paths.push(entry_path.to_string_lossy().to_string());
                }
                continue;
            }
        }
    }

    manifests.sort();
    manifests.dedup();
    project_types.sort();
    project_types.dedup();
    task_runners.sort();
    task_runners.dedup();
    instruction_sources.sort();
    instruction_sources.dedup();
    generated_paths.sort();
    generated_paths.dedup();
    vendor_paths.sort();
    vendor_paths.dedup();

    RepositoryEvidenceState {
        manifestFiles: manifests,
        projectTypes: project_types,
        taskRunners: task_runners,
        instructionSources: instruction_sources,
        generatedPaths: generated_paths,
        vendorPaths: vendor_paths,
    }
}

fn find_unapproved_legacy_references(repo_path: &Path) -> Vec<String> {
    let mut scan_stack = vec![(repo_path.to_path_buf(), 0usize)];
    let mut matches = Vec::new();
    let mut scanned_files = 0usize;

    while let Some((path, depth)) = scan_stack.pop() {
        if depth > MAX_EVIDENCE_SCAN_DEPTH {
            continue;
        }
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if LEGACY_SCAN_SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }

            let relative = entry_path
                .strip_prefix(repo_path)
                .unwrap_or(&entry_path)
                .to_string_lossy()
                .to_string();

            if path_contains_legacy_reference(&entry_path)
                && !is_legacy_reference_allowed(&relative)
            {
                matches.push(relative.clone());
            }

            if entry_path.is_dir() {
                if depth < MAX_EVIDENCE_SCAN_DEPTH {
                    scan_stack.push((entry_path, depth + 1));
                }
                continue;
            }

            if !entry_path.is_file() || scanned_files >= MAX_LEGACY_SCAN_FILES {
                continue;
            }
            scanned_files += 1;

            for line in scan_lines_for_unapproved_legacy_references(&entry_path) {
                matches.push(format!("{relative}:{line}"));
            }
        }
    }

    matches.sort();
    matches.dedup();
    matches
}

fn path_contains_legacy_reference(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().to_lowercase().contains("lachesi"))
}

fn is_legacy_reference_allowed(reference: &str) -> bool {
    let reference = reference.to_lowercase();
    LEGACY_ALLOWLIST_TEXT
        .iter()
        .any(|allowed| reference.contains(&allowed.to_lowercase()))
}

fn scan_lines_for_unapproved_legacy_references(path: &Path) -> Vec<String> {
    let content = match fs::read(path) {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };

    if content.len() as u64 > MAX_TEXT_SCAN_BYTES {
        return Vec::new();
    }
    if content.contains(&0u8) {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&content);
    if text.contains('\u{fffd}') {
        return Vec::new();
    }

    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_lower = line.to_lowercase();
            if !line_lower.contains("lachesi") || is_legacy_reference_allowed(line) {
                return None;
            }
            let snippet = line.trim();
            if snippet.is_empty() {
                return None;
            }
            let snippet = snippet.chars().take(120).collect::<String>();
            Some(format!("{:?}:{}", index + 1, snippet))
        })
        .collect()
}

struct DataDirectoryResolution {
    canonical: PathBuf,
    exists: bool,
    writable: bool,
    using_legacy_alias: bool,
}

fn resolve_data_directory() -> DataDirectoryResolution {
    if let Some(dir) =
        std::env::var_os("NORN_REVIEW_DATA_DIR").or_else(|| std::env::var_os("NORN_DATA_DIR"))
    {
        let path = PathBuf::from(dir);
        return DataDirectoryResolution {
            writable: path_writable(&path),
            exists: path.exists(),
            canonical: path,
            using_legacy_alias: false,
        };
    }

    if let Some(dir) =
        std::env::var_os("LACHESI_REVIEW_DATA_DIR").or_else(|| std::env::var_os("LACHESI_DATA_DIR"))
    {
        let path = PathBuf::from(dir);
        return DataDirectoryResolution {
            writable: path_writable(&path),
            exists: path.exists(),
            canonical: path,
            using_legacy_alias: true,
        };
    }

    let base = dirs::data_local_dir();
    let base = match base {
        Some(base) => base,
        None => {
            return DataDirectoryResolution {
                canonical: PathBuf::new(),
                exists: false,
                writable: false,
                using_legacy_alias: false,
            }
        }
    };

    DataDirectoryResolution {
        canonical: base.join("norn"),
        exists: base.join("norn").exists(),
        writable: path_writable(&base.join("norn")),
        using_legacy_alias: false,
    }
}

fn path_writable(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    let permissions = metadata.permissions();
    if permissions.readonly() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.mode() & 0o222 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[derive(Debug)]
struct CommandCheck {
    provider: String,
    required: bool,
    available: bool,
    path: Option<String>,
    version: Option<String>,
}

impl From<CommandCheck> for CliToolState {
    fn from(value: CommandCheck) -> Self {
        Self {
            provider: value.provider,
            required: value.required,
            available: value.available,
            path: value.path,
            version: value.version,
        }
    }
}

fn probe_cli_tool(name: &str, required: bool) -> CliToolState {
    let mut command = installed_cli_command(name);
    command.arg("--version");
    let output = command.output();
    let (available, version) = match output {
        Ok(output) if output.status.success() => {
            let output = format!(
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
            if output.is_empty() {
                (true, None)
            } else {
                (true, Some(output))
            }
        }
        Ok(output) => {
            let output = format!(
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
            .trim()
            .to_string();
            if output.starts_with("Command not found") {
                (false, None)
            } else {
                (false, Some(output))
            }
        }
        Err(_) => (false, None),
    };

    let check = CommandCheck {
        provider: name.to_string(),
        required,
        available,
        path: local_repo::find_in_path(name).map(|path| path.display().to_string()),
        version,
    };
    CliToolState::from(check)
}

#[cfg(target_os = "macos")]
fn installed_cli_command(program: &str) -> Command {
    let mut command = Command::new("/bin/zsh");
    command.args([
        "-lc",
        "export PATH=\"$HOME/.local/bin:$HOME/.npm/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; exec \"$@\"",
        "norn-user-cli",
        program,
    ]);
    command
}

#[cfg(not(target_os = "macos"))]
fn installed_cli_command(program: &str) -> Command {
    Command::new(program)
}

fn read_remote_state(root: &Path) -> (Option<RepositoryRemoteState>, Option<ReadinessIssue>) {
    let remote_name = run_git_command(root, "remote", &["get-url", "origin"])
        .ok()
        .or_else(|| {
            let remotes = run_git_command(root, "remote", &[]).ok()?;
            remotes
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .and_then(|name| run_git_command(root, "remote", &["get-url", name]).ok())
        });

    let Some(remote) = remote_name else {
        return (
            None,
            Some(ReadinessIssue {
                severity: ReadinessIssueSeverity::Error,
                scope: ReadinessIssueScope::Repository,
                code: "repository.remoteMissing".to_string(),
                message: "Repository has no git remotes.".to_string(),
                remediation:
                    "Add a GitHub or Bitbucket remote and rerun `norn doctor` for repository checks.".to_string(),
            }),
        );
    };
    let source = run_git_command(
        root,
        "remote",
        &["name-rev", "--name-only", "HEAD@{upstream}"],
    )
    .ok();
    let label = run_git_command(root, "remote", &["get-url", "origin"])
        .unwrap_or_else(|_| String::from("origin"));
    match local_repo::parse_git_remote(&remote) {
        Ok((provider, workspace, repo)) => (
            Some(RepositoryRemoteState {
                source: source.unwrap_or_else(|| label.clone()),
                provider: provider_name(&provider).to_string(),
                workspace,
                repo,
                url: redact_git_remote(&remote),
            }),
            None,
        ),
        Err(error) => (
            Some(RepositoryRemoteState {
                source: source.unwrap_or_else(|| label.clone()),
                provider: "unknown".to_string(),
                workspace: String::new(),
                repo: String::new(),
                url: redact_git_remote(&remote),
            }),
            Some(ReadinessIssue {
                severity: ReadinessIssueSeverity::Error,
                scope: ReadinessIssueScope::Repository,
                code: "repository.remoteUnsupported".to_string(),
                message: error,
                remediation: "Use a GitHub or Bitbucket remote URL compatible with Norn."
                    .to_string(),
            }),
        ),
    }
}

fn redact_git_remote(remote: &str) -> String {
    let trimmed = remote.trim();
    if let Some((scheme, rest)) = trimmed.split_once("://") {
        if let Some((_auth, after)) = rest.split_once('@') {
            return format!("{scheme}://<redacted>@{after}");
        }
        return trimmed.to_string();
    }

    if let Some((_user, after)) = trimmed.split_once('@') {
        if trimmed.contains(':') {
            return format!("<redacted>@{after}");
        }
        return trimmed.to_string();
    }
    trimmed.to_string()
}

fn provider_name(provider: &config::ReviewProvider) -> &'static str {
    match provider {
        config::ReviewProvider::Bitbucket => "bitbucket",
        config::ReviewProvider::Github => "github",
    }
}

fn read_repo_config_state(repo_path: &Path) -> RepoConfigState {
    let canonical_file = repo_path.join(NORN_CONFIG_FILE);
    let canonical_dir = repo_path.join(NORN_CONFIG_DIR);
    let legacy_file = repo_path.join(LEGACY_CONFIG_FILE);
    let legacy_dir = repo_path.join(LEGACY_CONFIG_DIR);
    let local_canonical = repo_path.join(NORN_LOCAL_CONFIG_FILE);
    let local_legacy = repo_path.join(LEGACY_LOCAL_CONFIG_FILE);

    let mut config = RepoConfigState {
        requestedPath: repo_path.to_string_lossy().to_string(),
        configPath: None,
        exists: canonical_file.exists()
            || canonical_dir.exists()
            || legacy_file.exists()
            || legacy_dir.exists(),
        canonicalSource: canonical_file.is_file(),
        canonicalDirectory: canonical_dir.is_dir(),
        legacySource: legacy_file.is_file(),
        legacyDirectory: legacy_dir.is_dir(),
        localOverridesCanonical: local_canonical.is_file(),
        localOverridesLegacy: local_legacy.is_file(),
    };

    let config_path = if canonical_file.is_file() {
        Some(canonical_file)
    } else if canonical_dir.is_dir() {
        Some(canonical_dir)
    } else if legacy_file.is_file() {
        Some(legacy_file)
    } else if legacy_dir.is_dir() {
        Some(legacy_dir)
    } else {
        None
    };
    if let Some(path) = config_path {
        config.configPath = Some(path.display().to_string());
    }

    if config.configPath.is_none() {
        config.exists = false;
    }
    config
}

fn resolve_command_path(command: &str) -> (bool, Option<String>) {
    let executable = command.split_whitespace().next().unwrap_or_default();
    if executable.is_empty() {
        return (false, None);
    }
    let path = PathBuf::from(executable);
    if path.is_absolute() {
        if path.is_file() {
            (true, Some(path.to_string_lossy().to_string()))
        } else {
            (false, Some(path.to_string_lossy().to_string()))
        }
    } else {
        local_repo::find_in_path(executable)
            .map(|path| (path.is_file(), Some(path.to_string_lossy().to_string())))
            .unwrap_or((false, None))
    }
}

fn run_git_command(path: &Path, command_name: &str, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("/usr/bin/git");
    command.arg("-C");
    command.arg(path);
    command.arg(command_name);
    command.args(args);
    let output = command
        .output()
        .map_err(|error| format!("failed to run git command in {}: {error}", path.display()))?;
    if !output.status.success() {
        let reason = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if reason.is_empty() {
            "git command failed".to_string()
        } else {
            reason
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn unix_millis_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn format_readiness_human(report: &DoctorReport) -> String {
    let mut output = String::new();
    output.push_str("Norn readiness\n");
    output.push_str(&format!("status: {:?}\n", report.status));
    output.push_str(&format!("schemaVersion: {}\n", report.schemaVersion));
    output.push_str(&format!("timestamp: {}\n", report.timestamp));
    output.push_str("\nMachine\n");
    output.push_str(&format!("- os: {}\n", report.machine.os));
    output.push_str(&format!(
        "- config dir: {} (exists={}, writable={})\n",
        report.machine.configDirectory.path,
        report.machine.configDirectory.exists,
        report.machine.configDirectory.writable
    ));
    output.push_str(&format!(
        "- data dir: {} (exists={}, writable={})\n",
        report.machine.dataDirectory.path,
        report.machine.dataDirectory.exists,
        report.machine.dataDirectory.writable
    ));
    for credential in &report.machine.credentials {
        output.push_str(&format!(
            "- credential [{}]: {}\n",
            credential.provider, credential.available
        ));
    }
    for cli in &report.machine.cliTools {
        output.push_str(&format!(
            "- {} cli: available={}\n",
            cli.provider, cli.available
        ));
    }

    if report.repository.inspected {
        output.push_str("\nRepository\n");
        output.push_str(&format!(
            "- requested path: {}\n",
            report.repository.requestedPath
        ));
        if let Some(root) = &report.repository.gitRoot {
            output.push_str(&format!("- git root: {}\n", root));
        }
        if let Some(branch) = &report.repository.branch {
            output.push_str(&format!("- branch: {}\n", branch));
        }
        if let Some(head) = &report.repository.head {
            output.push_str(&format!("- head: {}\n", head));
        }
        if let Some(remote) = &report.repository.remote {
            output.push_str(&format!(
                "- remote: {} {} {}/{}\n",
                remote.provider, remote.source, remote.workspace, remote.repo
            ));
        }
        if let Some(working_tree) = &report.repository.workingTree {
            output.push_str(&format!(
                "- working tree: dirty={} untracked={}\n",
                working_tree.dirty, working_tree.untrackedFiles
            ));
        }
    } else if report.repository.skipped {
        output.push_str("\nRepository check skipped (--machine-only)\n");
    } else {
        output.push_str("\nRepository checks not available\n");
    }

    output.push_str("\nIssues\n");
    for issue in &report.issues {
        output.push_str(&format!(
            "- {:?} {:?}: {} [{}]\n",
            issue.severity, issue.scope, issue.message, issue.code
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{collect_report, parse_doctor_args, run_doctor, DoctorArgs, DoctorFormat};
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_REPO_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_repo() -> PathBuf {
        let nonce = TEMP_REPO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("norn-readiness-{ts}-{nonce}-{}", process::id()));
        fs::create_dir_all(&path).expect("create temp repo");
        path
    }

    fn init_git_repo(path: &PathBuf) {
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(path)
            .args(["init", "--initial-branch", "main"])
            .output()
            .expect("git init");
        fs::write(path.join("README.md"), "ready\n").expect("write README");
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(path)
            .args(["add", "README.md"])
            .output()
            .expect("git add");
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(path)
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
    }

    #[test]
    fn parse_doctor_defaults() {
        let parsed = parse_doctor_args(&["doctor".to_string()]).expect("parse doctor");

        assert_eq!(parsed.repo_path, PathBuf::from("."));
        assert!(!parsed.machine_only);
        match parsed.format {
            DoctorFormat::Human => {}
            _ => panic!("default doctor format should be human"),
        }
    }

    #[test]
    fn doctor_json_includes_schema_and_status() {
        let repo = temp_repo();
        init_git_repo(&repo);
        fs::write(repo.join(".norn.yaml"), "version: 0.1\n").expect("write config");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_doctor(
            DoctorArgs {
                repo_path: repo,
                machine_only: false,
                format: DoctorFormat::Json,
            },
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stderr.is_empty());
        let output: serde_json::Value =
            serde_json::from_slice(&stdout).expect("doctor JSON output");
        assert_eq!(output["schemaVersion"], "norn.readiness.v1");
        assert_eq!(output["status"], "fail");
        assert!(output["issues"]
            .as_array()
            .is_some_and(|issues| !issues.is_empty()));
    }

    #[test]
    fn doctor_json_distinguishes_warn_as_exit_one() {
        let repo = temp_repo();
        init_git_repo(&repo);
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .output()
            .expect("git remote add");
        fs::write(repo.join("next.txt"), "untracked\n").expect("write untracked");

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_doctor(
            DoctorArgs {
                repo_path: repo,
                machine_only: false,
                format: DoctorFormat::Json,
            },
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 1);
        assert!(stderr.is_empty());
        let output: serde_json::Value =
            serde_json::from_slice(&stdout).expect("doctor JSON output");
        assert_eq!(output["status"], "warn");
    }

    #[test]
    fn doctor_with_no_remote_is_error() {
        let repo = temp_repo();
        init_git_repo(&repo);

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_doctor(
            DoctorArgs {
                repo_path: repo,
                machine_only: false,
                format: DoctorFormat::Json,
            },
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stderr.is_empty());
        let output: serde_json::Value =
            serde_json::from_slice(&stdout).expect("doctor JSON output");
        assert_eq!(output["status"], "fail");
        assert!(output["issues"].as_array().is_some_and(|issues| {
            issues
                .iter()
                .any(|issue| issue["code"] == "repository.remoteMissing")
        }));
    }

    #[test]
    fn doctor_detects_untracked_files_and_dirty_tree() {
        let repo = temp_repo();
        init_git_repo(&repo);
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .output()
            .expect("git remote add");
        fs::write(repo.join("next.txt"), "untracked\n").expect("write untracked");

        let mut report = collect_report(&repo, false);
        report.issues.sort_by(|a, b| a.code.cmp(&b.code));

        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "repository.workingTreeDirty"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "repository.untrackedFiles"));
        assert!(report
            .repository
            .workingTree
            .as_ref()
            .is_some_and(|tree| tree.dirty));
        assert_eq!(report.repository.workingTree.unwrap().untrackedFiles, 1);
    }

    #[test]
    fn doctor_reports_legacy_and_canonical_config_precedence_issue() {
        let repo = temp_repo();
        init_git_repo(&repo);
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .output()
            .expect("git remote add");
        fs::write(repo.join(".norn.yaml"), "version: 0.1\n").expect("write norn");
        fs::write(repo.join(".lachesi.yaml"), "version: 0.1\n").expect("write lachesi");

        let report = collect_report(&repo, false);

        assert_eq!(
            super::derive_status(&report.issues),
            super::ReadinessStatus::Fail
        );
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "repository.configPrecedenceConflict"));
    }

    #[test]
    fn doctor_reports_unapproved_legacy_name_occurrences() {
        let repo = temp_repo();
        init_git_repo(&repo);
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .output()
            .expect("git remote add");
        fs::write(repo.join("README.md"), "# Welcome to lachesi\n").expect("legacy readme");

        let report = collect_report(&repo, false);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "repository.legacyNameNotAllowed"));
    }

    #[test]
    fn doctor_allows_intentional_legacy_artifacts() {
        let repo = temp_repo();
        init_git_repo(&repo);
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .output()
            .expect("git remote add");
        fs::write(repo.join(".lachesi.yaml"), "version: 0.1\n").expect("legacy config");

        let report = collect_report(&repo, false);
        assert!(report
            .issues
            .iter()
            .all(|issue| issue.code != "repository.legacyNameNotAllowed"));
    }

    #[test]
    fn doctor_detects_github_and_bitbucket_remote_provider() {
        let repo = temp_repo();
        init_git_repo(&repo);
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .output()
            .expect("git remote add");
        let github_report = collect_report(&repo, false);
        assert_eq!(
            github_report
                .repository
                .remote
                .as_ref()
                .expect("remote exists")
                .provider,
            "github"
        );
        assert_eq!(
            github_report
                .repository
                .remote
                .as_ref()
                .expect("remote exists")
                .workspace,
            "example"
        );

        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "set-url",
                "origin",
                "https://bitbucket.org/example/repo.git",
            ])
            .output()
            .expect("git remote set-url");
        let bitbucket_report = collect_report(&repo, false);
        assert_eq!(
            bitbucket_report
                .repository
                .remote
                .as_ref()
                .expect("remote exists")
                .provider,
            "bitbucket"
        );
    }

    #[test]
    fn doctor_redacts_remote_credentials_in_report() {
        let repo = temp_repo();
        init_git_repo(&repo);
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://ci:super-secret@github.com/example/repo.git",
            ])
            .output()
            .expect("git remote add");
        let report = collect_report(&repo, false);
        let url = &report
            .repository
            .remote
            .as_ref()
            .expect("remote exists")
            .url;
        assert!(url.contains("<redacted>@"));
        assert!(!url.contains("super-secret"));
    }

    #[test]
    fn doctor_detects_repository_evidence_in_a_monorepo_shape() {
        let repo = temp_repo();
        init_git_repo(&repo);
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .output()
            .expect("git remote add");

        fs::create_dir_all(repo.join("packages/frontend")).expect("create package dir");
        fs::write(repo.join("Cargo.toml"), "[package]\nname = \"root\"\n").expect("write cargo");
        fs::write(
            repo.join("packages/frontend/package.json"),
            r#"{"name":"frontend"}"#,
        )
        .expect("write package");
        fs::write(repo.join("Makefile"), "all:\n\techo hi\n").expect("write Makefile");
        fs::create_dir_all(repo.join(".github")).expect("create .github");
        fs::write(repo.join(".github/instructions.md"), "repo instructions")
            .expect("write instructions");

        let report = collect_report(&repo, false);
        assert!(report
            .repository
            .evidence
            .projectTypes
            .iter()
            .any(|project_type| project_type == "javascript" || project_type == "rust"));
        assert!(report
            .repository
            .evidence
            .taskRunners
            .iter()
            .any(|task| task == "make"));
        assert!(report
            .repository
            .evidence
            .manifestFiles
            .iter()
            .any(|manifest| manifest.ends_with("Cargo.toml")));
        assert!(report
            .repository
            .evidence
            .manifestFiles
            .iter()
            .any(|manifest| manifest.ends_with("package.json")));
        assert!(report
            .repository
            .evidence
            .instructionSources
            .iter()
            .any(|instructions| instructions.ends_with("instructions.md")));
    }

    #[test]
    fn doctor_with_no_repository_config_marks_missing_config_as_present_but_not_fatal() {
        let repo = temp_repo();
        init_git_repo(&repo);
        std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .output()
            .expect("git remote add");

        let report = collect_report(&repo, false);
        let config = report
            .repository
            .config
            .as_ref()
            .expect("config state exists");
        assert!(!config.exists);
    }
}
