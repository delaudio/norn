use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as JsonValue;
use serde_yaml::Value;

const CONFIG_FILE: &str = ".norn.yaml";
const LEGACY_CONFIG_FILE: &str = ".lachesi.yaml";
const CONFIG_DIR: &str = ".norn";
const LEGACY_CONFIG_DIR: &str = ".lachesi";
const LOCAL_CONFIG_FILE: &str = ".norn.local.yaml";
const LEGACY_LOCAL_CONFIG_FILE: &str = ".lachesi.local.yaml";
const DEFAULT_REPO_INIT_CONFIG_FILE: &str = ".norn.yaml";
const SUPPORTED_VERSION: &str = "0.1";

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoReviewConfig {
    #[serde(deserialize_with = "deserialize_version")]
    pub version: String,
    #[serde(default)]
    pub review: Option<ReviewConfig>,
    #[serde(default)]
    pub profiles: BTreeMap<String, ReviewProfileConfig>,
    #[serde(default)]
    pub paths: Option<PathFilters>,
    #[serde(default)]
    pub policy: Option<PolicyConfig>,
    #[serde(default)]
    pub analyzers: BTreeMap<String, AnalyzerConfig>,
    #[serde(default)]
    pub publish: Option<PublishConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewConfig {
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub mode: Option<ReviewMode>,
    #[serde(default)]
    pub prompt: Option<PromptConfig>,
    #[serde(default)]
    pub findings: Option<FindingConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProfileConfig {
    #[serde(default)]
    pub mode: Option<ReviewMode>,
    #[serde(default)]
    pub min_severity: Option<ReviewSeverity>,
    #[serde(default)]
    pub prompt: Option<PromptConfig>,
    #[serde(default)]
    pub policy_packs: Vec<String>,
    #[serde(default)]
    pub analyzers: BTreeMap<String, ProfileAnalyzerRequirement>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProfileAnalyzerRequirement {
    #[default]
    Optional,
    Required,
    Disabled,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewMode {
    Fast,
    #[default]
    Balanced,
    Strict,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptConfig {
    #[serde(default)]
    pub extend: Option<String>,
    #[serde(default)]
    pub replace: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FindingConfig {
    #[serde(default)]
    pub min_severity: Option<ReviewSeverity>,
    #[serde(default)]
    pub require_anchors: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewSeverity {
    Info,
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PathFilters {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolicyConfig {
    #[serde(default)]
    pub packs: Vec<String>,
    #[serde(default)]
    pub sources: Vec<PolicySource>,
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
    #[serde(default)]
    pub path_rules: Vec<PathRule>,
    #[serde(default)]
    pub ast_rules: Vec<AstRule>,
    #[serde(default)]
    pub suppressions: Vec<PolicySuppression>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    pub id: String,
    #[serde(default)]
    pub source: Option<String>,
    pub severity: ReviewSeverity,
    #[serde(default)]
    pub confidence: Option<ReviewConfidence>,
    #[serde(default)]
    pub applies_to: Option<PathFilters>,
    pub instruction: String,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub remediation: Option<String>,
    #[serde(default)]
    pub enforcement: Option<PolicyEnforcement>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewConfidence {
    Low,
    #[default]
    Medium,
    High,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PolicyEnforcement {
    #[default]
    Prompt,
    Analyzer,
    Ast,
    Manual,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PathRule {
    pub id: String,
    pub severity: ReviewSeverity,
    pub paths: PathFilters,
    pub instruction: String,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub remediation: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AstRule {
    pub id: String,
    pub language: String,
    pub severity: ReviewSeverity,
    #[serde(default)]
    pub selector: BTreeMap<String, Value>,
    #[serde(default)]
    pub applies_to: Option<PathFilters>,
    pub instruction: String,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub remediation: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySuppression {
    pub rule_id: String,
    pub paths: PathFilters,
    pub reason: String,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub config: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewPublicationMode {
    #[default]
    Inline,
    File,
    General,
    LocalOnly,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublishConfig {
    #[serde(default)]
    pub default_mode: Option<ReviewPublicationMode>,
    #[serde(default)]
    pub require_manual_submit: Option<bool>,
    #[serde(default)]
    pub allow_general_comments: Option<bool>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoConfigValidationMessage {
    pub path: String,
    pub message: String,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoadedPolicyPack {
    pub id: String,
    pub name: Option<String>,
    pub path: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoReviewConfigLoadResult {
    pub repo_path: String,
    pub config_path: String,
    pub exists: bool,
    pub config: Option<RepoReviewConfig>,
    pub selected_profile: Option<String>,
    pub loaded_policy_packs: Vec<LoadedPolicyPack>,
    pub warnings: Vec<RepoConfigValidationMessage>,
    pub errors: Vec<RepoConfigValidationMessage>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoConfigMigrationAction {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub content_changes: Vec<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoConfigMigrationResult {
    pub repo_path: String,
    pub dry_run: bool,
    pub actions: Vec<RepoConfigMigrationAction>,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
struct PolicyPackConfig {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    review: Option<ReviewConfig>,
    #[serde(default)]
    policy: Option<PolicyConfig>,
    #[serde(default)]
    profiles: BTreeMap<String, ReviewProfileConfig>,
    #[serde(default)]
    analyzers: BTreeMap<String, AnalyzerConfig>,
}

fn deserialize_version<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(value) => Ok(value),
        Value::Number(value) => Ok(value.to_string()),
        _ => Err(serde::de::Error::custom(
            "version must be a string or number",
        )),
    }
}

pub fn load_from_repo_path(repo_path: &Path) -> Result<RepoReviewConfigLoadResult, String> {
    load_from_repo_path_with_profile(repo_path, None)
}

pub fn load_from_repo_path_with_profile(
    repo_path: &Path,
    profile_override: Option<&str>,
) -> Result<RepoReviewConfigLoadResult, String> {
    if !repo_path.is_dir() {
        return Err(format!(
            "Repository path does not exist or is not a directory: {}",
            repo_path.display()
        ));
    }

    let Some(source) = discover_repo_config_source(repo_path)? else {
        let config_path = repo_path.join(CONFIG_FILE);
        return Ok(RepoReviewConfigLoadResult {
            repo_path: repo_path.display().to_string(),
            config_path: config_path.display().to_string(),
            exists: false,
            config: None,
            selected_profile: None,
            loaded_policy_packs: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        });
    };

    if is_config_dir_source(&source) {
        return load_from_config_dir(repo_path, &source, profile_override)?.ok_or_else(|| {
            format!(
                "Repository config directory disappeared: {}",
                source.display()
            )
        });
    }

    let contents = fs::read_to_string(&source)
        .map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
    Ok(load_from_str(
        repo_path,
        &source,
        &contents,
        profile_override,
    ))
}

fn is_config_dir_source(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(CONFIG_DIR) | Some(LEGACY_CONFIG_DIR)
    )
}

fn render_default_repo_config_contents() -> String {
    format!("version: {SUPPORTED_VERSION}\nreview:\n  mode: balanced\n")
}

pub(crate) fn discover_repo_config_source(path: &Path) -> Result<Option<PathBuf>, String> {
    discover_repo_config_source_inner(path)
}

fn discover_repo_config_source_inner(repo_path: &Path) -> Result<Option<PathBuf>, String> {
    let canonical_file = repo_path.join(CONFIG_FILE);
    let canonical_dir = repo_path.join(CONFIG_DIR);
    let legacy_file = repo_path.join(LEGACY_CONFIG_FILE);
    let legacy_dir = repo_path.join(LEGACY_CONFIG_DIR);
    if canonical_file.exists() && canonical_dir.exists() {
        return Err(format!(
            "Norn found both canonical repository config roots {} and {}. Keep exactly one; Norn will not choose between them implicitly.",
            canonical_file.display(),
            canonical_dir.display()
        ));
    }
    if legacy_file.exists() && legacy_dir.exists() {
        return Err(format!(
            "Norn found both legacy repository config roots {} and {}. Migrate or remove one before loading config; Norn will not choose between them implicitly.",
            legacy_file.display(),
            legacy_dir.display()
        ));
    }
    let canonical = canonical_file
        .exists()
        .then_some(canonical_file)
        .or_else(|| canonical_dir.exists().then_some(canonical_dir));
    let legacy = legacy_file
        .exists()
        .then_some(legacy_file)
        .or_else(|| legacy_dir.exists().then_some(legacy_dir));

    match (canonical, legacy) {
        (Some(canonical), Some(legacy)) => Err(format!(
            "Norn found both canonical repository config {} and legacy config {}. Migrate or remove the legacy source; Norn will not merge them implicitly.",
            canonical.display(),
            legacy.display()
        )),
        (Some(path), None) | (None, Some(path)) => Ok(Some(path)),
        (None, None) => Ok(None),
    }
}

pub(crate) fn default_init_action_if_needed(
    repo_path: &Path,
) -> Result<Option<RepoConfigMigrationAction>, String> {
    let config_source = discover_repo_config_source_inner(repo_path)?;
    if config_source.is_some() {
        return Ok(None);
    }

    let default_target = repo_path.join(DEFAULT_REPO_INIT_CONFIG_FILE);
    if default_target.exists() {
        return Ok(None);
    }

    Ok(Some(RepoConfigMigrationAction {
        source: String::from("<norn-init-template>"),
        target: default_target.display().to_string(),
        kind: "file".to_string(),
        content_changes: vec![
            "Create a default .norn.yaml from repository evidence and stable defaults.".to_string(),
        ],
    }))
}

pub(crate) fn write_default_repo_config_if_missing(repo_path: &Path) -> Result<bool, String> {
    if !repo_path.is_dir() {
        return Err(format!(
            "Repository path does not exist or is not a directory: {}",
            repo_path.display()
        ));
    }

    let target = repo_path.join(DEFAULT_REPO_INIT_CONFIG_FILE);
    if target.exists() {
        return Ok(false);
    }

    let parent = target.parent().ok_or_else(|| {
        format!(
            "Default repo config target has no parent: {}",
            target.display()
        )
    })?;
    let contents = render_default_repo_config_contents();
    let mut temporary = tempfile::Builder::new()
        .prefix(".norn-repo-init-")
        .tempfile_in(parent)
        .map_err(|error| {
            format!(
                "Failed to stage default repository config at {}: {error}",
                target.display()
            )
        })?;
    temporary.write_all(contents.as_bytes()).map_err(|error| {
        format!(
            "Failed to stage default repository config at {}: {error}",
            target.display()
        )
    })?;
    set_staged_file_permissions(temporary.as_file(), None, &target).map_err(|error| {
        format!(
            "Failed to stage default repository config at {}: {error}",
            target.display()
        )
    })?;
    temporary.as_file().sync_all().map_err(|error| {
        format!(
            "Failed to persist default repository config at {}: {error}",
            target.display()
        )
    })?;
    match temporary.persist_noclobber(&target) {
        Ok(_) => Ok(true),
        Err(error) => {
            if target.exists() {
                return Ok(false);
            }
            Err(format!(
                "Failed to write default repository config at {}: {}",
                target.display(),
                error.error
            ))
        }
    }
}

fn load_from_config_dir(
    repo_path: &Path,
    config_dir: &Path,
    profile_override: Option<&str>,
) -> Result<Option<RepoReviewConfigLoadResult>, String> {
    let Some(config) = synthesize_config_dir(repo_path, config_dir)? else {
        return Ok(None);
    };
    let contents = serde_yaml::to_string(&config).map_err(|error| {
        format!(
            "Failed to synthesize {} config: {error}",
            config_dir.display()
        )
    })?;
    Ok(Some(load_from_str(
        repo_path,
        config_dir,
        &contents,
        profile_override,
    )))
}

fn synthesize_config_dir(
    repo_path: &Path,
    config_dir: &Path,
) -> Result<Option<RepoReviewConfig>, String> {
    if !config_dir.is_dir() {
        return Ok(None);
    }

    let mut config = RepoReviewConfig {
        version: SUPPORTED_VERSION.to_string(),
        ..RepoReviewConfig::default()
    };
    if let Some(prompt) = load_config_dir_prompt(config_dir)? {
        config.review = Some(ReviewConfig {
            prompt: Some(PromptConfig {
                replace: Some(prompt),
                ..PromptConfig::default()
            }),
            ..ReviewConfig::default()
        });
    }
    let packs = discover_config_dir_policy_packs(repo_path, config_dir)?;
    if !packs.is_empty() {
        config.policy = Some(PolicyConfig {
            packs,
            ..PolicyConfig::default()
        });
    }
    Ok(Some(config))
}

fn load_config_dir_prompt(config_dir: &Path) -> Result<Option<String>, String> {
    for file_name in [
        "system-prompt.md",
        "review-prompt.md",
        "review.md",
        "prompt.md",
    ] {
        let path = config_dir.join(file_name);
        if path.is_file() {
            let prompt = fs::read_to_string(&path)
                .map_err(|error| format!("Failed to read {}: {error}", path.display()))?
                .trim()
                .to_string();
            if !prompt.is_empty() {
                return Ok(Some(prompt));
            }
        }
    }
    Ok(None)
}

fn discover_config_dir_policy_packs(
    repo_path: &Path,
    config_dir: &Path,
) -> Result<Vec<String>, String> {
    let packs_dir = config_dir.join("packs");
    if !packs_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut packs = Vec::new();
    for entry in fs::read_dir(&packs_dir)
        .map_err(|error| format!("Failed to read {}: {error}", packs_dir.display()))?
    {
        let entry = entry.map_err(|error| format!("Failed to inspect policy pack: {error}"))?;
        let path = entry.path();
        if resolve_pack_manifest_path(repo_path, &path.to_string_lossy()).is_some() {
            packs.push(path.to_string_lossy().to_string());
        }
    }
    packs.sort();
    Ok(packs)
}

fn load_from_str(
    repo_path: &Path,
    config_path: &Path,
    contents: &str,
    profile_override: Option<&str>,
) -> RepoReviewConfigLoadResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    let value = match serde_yaml::from_str::<Value>(contents) {
        Ok(value) => value,
        Err(error) => {
            errors.push(message(
                config_path,
                format!("Failed to parse YAML: {error}"),
            ));
            return result(
                repo_path,
                config_path,
                true,
                None,
                None,
                Vec::new(),
                warnings,
                errors,
            );
        }
    };

    warnings.extend(unknown_field_warnings(config_path, &value));
    errors.extend(forbidden_field_errors(config_path, &value));

    let mut loaded_policy_packs = Vec::new();
    let mut selected_profile = None;
    let config = match serde_yaml::from_value::<RepoReviewConfig>(value) {
        Ok(mut config) => {
            let mut loaded_pack_paths = BTreeSet::new();
            loaded_policy_packs.extend(load_policy_packs(
                repo_path,
                config_path,
                &mut config,
                &mut warnings,
                &mut errors,
                &mut loaded_pack_paths,
            ));
            selected_profile =
                apply_review_profile(config_path, &mut config, profile_override, &mut warnings);
            loaded_policy_packs.extend(load_policy_packs(
                repo_path,
                config_path,
                &mut config,
                &mut warnings,
                &mut errors,
                &mut loaded_pack_paths,
            ));
            if let Some(profile_id) = selected_profile.as_deref() {
                apply_profile_analyzer_requirements(
                    config_path,
                    &mut config,
                    profile_id,
                    &mut errors,
                );
            }
            validate_config(config_path, &config, &mut errors);
            Some(config)
        }
        Err(error) => {
            errors.push(message(
                config_path,
                format!("Invalid repo config shape: {error}"),
            ));
            None
        }
    };

    result(
        repo_path,
        config_path,
        true,
        config,
        selected_profile,
        loaded_policy_packs,
        warnings,
        errors,
    )
}

fn validate_config(
    config_path: &Path,
    config: &RepoReviewConfig,
    errors: &mut Vec<RepoConfigValidationMessage>,
) {
    if config.version.trim().is_empty() {
        errors.push(message(config_path, "version is required"));
    } else if config.version != SUPPORTED_VERSION {
        errors.push(message(
            config_path,
            format!(
                "Unsupported Norn repository config version {}. Supported version is {SUPPORTED_VERSION}.",
                config.version
            ),
        ));
    }

    for (id, analyzer) in &config.analyzers {
        if analyzer.enabled && analyzer.command.as_deref().unwrap_or("").trim().is_empty() {
            errors.push(message(
                config_path,
                format!("Analyzer `{id}` is enabled but has no command."),
            ));
        }
    }
}

pub(crate) fn validate_resolved_config(config: &RepoReviewConfig) -> Result<(), String> {
    let mut errors = Vec::new();
    validate_config(Path::new("<resolved-policy>"), config, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

pub(crate) fn validate_external_config_layer(value: &Value) -> Result<(), String> {
    let path = Path::new("<organization-policy>");
    let mut messages = unknown_field_warnings(path, value);
    messages.extend(forbidden_field_errors(path, value));
    if messages.is_empty() {
        Ok(())
    } else {
        Err(messages
            .into_iter()
            .map(|message| message.message)
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

pub(crate) fn load_repository_policy_layer(
    repo_path: &Path,
) -> Result<(Option<JsonValue>, Vec<RepoConfigValidationMessage>), String> {
    let Some(config_path) = discover_repo_config_source(repo_path)? else {
        return Ok((None, Vec::new()));
    };
    if !is_config_dir_source(&config_path) {
        let contents = fs::read_to_string(&config_path)
            .map_err(|error| format!("Failed to read {}: {error}", config_path.display()))?;
        let standalone = load_from_str(repo_path, &config_path, &contents, None);
        if !standalone.errors.is_empty() {
            return Err(standalone
                .errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("\n"));
        }
        let yaml = serde_yaml::from_str::<Value>(&contents)
            .map_err(|error| format!("Failed to parse {}: {error}", config_path.display()))?;
        let warnings = unknown_field_warnings(&config_path, &yaml);
        let secret_errors = forbidden_field_errors(&config_path, &yaml);
        if !secret_errors.is_empty() {
            return Err(secret_errors
                .into_iter()
                .map(|message| message.message)
                .collect::<Vec<_>>()
                .join("\n"));
        }
        return serde_json::to_value(yaml)
            .map(|layer| (Some(layer), warnings))
            .map_err(|error| format!("Failed to normalize {}: {error}", config_path.display()));
    }

    synthesize_config_dir(repo_path, &config_path)?
        .map(|config| {
            serde_json::to_value(config)
                .map(compact_policy_layer)
                .map_err(|error| {
                    format!(
                        "Failed to normalize {} config: {error}",
                        config_path.display()
                    )
                })
        })
        .transpose()
        .map(|layer| (layer, Vec::new()))
}

pub(crate) fn load_local_policy_layer(repo_path: &Path) -> Result<Option<JsonValue>, String> {
    let canonical = repo_path.join(LOCAL_CONFIG_FILE);
    let legacy = repo_path.join(LEGACY_LOCAL_CONFIG_FILE);
    if canonical.exists() && legacy.exists() {
        return Err(format!(
            "Norn found both canonical local override {} and legacy override {}. Migrate or remove the legacy source; Norn will not merge them implicitly.",
            canonical.display(),
            legacy.display()
        ));
    }
    let path = if canonical.exists() {
        canonical
    } else {
        legacy
    };
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Failed to inspect local policy override {}: {error}",
                path.display()
            ))
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "{} must be a regular file and cannot be a symbolic link.",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(LOCAL_CONFIG_FILE)
        ));
    }
    if !metadata.is_file() {
        return Ok(None);
    }
    validate_local_policy_override_state(repo_path, &path)?;
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    let yaml = serde_yaml::from_str::<Value>(&contents)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;
    let mut messages = unknown_field_warnings(&path, &yaml);
    messages.extend(forbidden_field_errors(&path, &yaml));
    if !messages.is_empty() {
        return Err(messages
            .into_iter()
            .map(|message| message.message)
            .collect::<Vec<_>>()
            .join("\n"));
    }
    serde_json::to_value(yaml)
        .map(Some)
        .map_err(|error| format!("Failed to normalize {}: {error}", path.display()))
}

fn validate_local_policy_override_state(repo_path: &Path, path: &Path) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Local policy override has an invalid file name.".to_string())?;
    let tracked =
        run_local_policy_git(repo_path, &["ls-files", "--error-unmatch", "--", file_name])
            .map_err(|error| format!("Failed to inspect local policy tracking state: {error}"))?;
    if tracked.status.success() {
        return Err(format!(
            "{file_name} is tracked by Git and cannot be used as a local policy override."
        ));
    }
    if tracked.status.code() != Some(1) {
        return Err(format!(
            "Could not determine whether {file_name} is tracked: {}",
            String::from_utf8_lossy(&tracked.stderr).trim()
        ));
    }

    let ignored = run_local_policy_git(repo_path, &["check-ignore", "--quiet", "--", file_name])
        .map_err(|error| format!("Failed to inspect local policy ignore state: {error}"))?;
    if ignored.status.success() {
        return Ok(());
    }
    if ignored.status.code() == Some(1) {
        return Err(format!(
            "{file_name} must be untracked and covered by a Git ignore rule before it can be used."
        ));
    }
    Err(format!(
        "Could not determine whether {file_name} is ignored: {}",
        String::from_utf8_lossy(&ignored.stderr).trim()
    ))
}

fn run_local_policy_git(repo_path: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    #[cfg(target_os = "macos")]
    {
        let command = std::iter::once("git".to_string())
            .chain(std::iter::once("-C".to_string()))
            .chain(std::iter::once(shell_quote(
                repo_path.to_string_lossy().as_ref(),
            )))
            .chain(args.iter().map(|arg| shell_quote(arg)))
            .collect::<Vec<_>>()
            .join(" ");
        return Command::new("/bin/zsh").arg("-lc").arg(command).output();
    }
    #[cfg(not(target_os = "macos"))]
    {
        Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(args)
            .output()
    }
}

pub fn migrate_repository_config(
    repo_path: &Path,
    dry_run: bool,
) -> Result<RepoConfigMigrationResult, String> {
    if !repo_path.is_dir() {
        return Err(format!(
            "Repository path does not exist or is not a directory: {}",
            repo_path.display()
        ));
    }

    let mappings = [
        (LEGACY_CONFIG_FILE, CONFIG_FILE, false),
        (LEGACY_CONFIG_DIR, CONFIG_DIR, true),
        (LEGACY_LOCAL_CONFIG_FILE, LOCAL_CONFIG_FILE, false),
    ];
    let mut actions = Vec::new();
    let legacy_root_present = [LEGACY_CONFIG_FILE, LEGACY_CONFIG_DIR]
        .iter()
        .any(|name| fs::symlink_metadata(repo_path.join(name)).is_ok());
    let canonical_root_present = [CONFIG_FILE, CONFIG_DIR]
        .iter()
        .any(|name| fs::symlink_metadata(repo_path.join(name)).is_ok());
    if legacy_root_present && canonical_root_present {
        return Err(
            "Cannot migrate a legacy repository config root while a canonical .norn.yaml or .norn root already exists. Norn never overwrites or creates a second canonical repository config root; keep exactly one root, then retry."
                .to_string(),
        );
    }
    if repo_path.join(LEGACY_CONFIG_FILE).exists() && repo_path.join(LEGACY_CONFIG_DIR).exists() {
        return Err(format!(
            "Cannot migrate both {} and {} because they are ambiguous repository config roots. Keep one legacy root, then retry.",
            repo_path.join(LEGACY_CONFIG_FILE).display(),
            repo_path.join(LEGACY_CONFIG_DIR).display()
        ));
    }
    let legacy_local = repo_path.join(LEGACY_LOCAL_CONFIG_FILE);
    let canonical_local = repo_path.join(LOCAL_CONFIG_FILE);
    if legacy_local.is_file() && !canonical_local.exists() {
        validate_local_policy_override_state(repo_path, &legacy_local)?;
        if !local_policy_path_is_ignored(repo_path, LOCAL_CONFIG_FILE)? {
            let gitignore = repo_path.join(".gitignore");
            if gitignore.exists() && !gitignore.is_file() {
                return Err(format!(
                    "Cannot add the canonical local-override ignore rule because {} is not a regular file.",
                    gitignore.display()
                ));
            }
            actions.push(RepoConfigMigrationAction {
                source: gitignore.display().to_string(),
                target: gitignore.display().to_string(),
                kind: "edit".to_string(),
                content_changes: vec![format!(
                    "Add /{LOCAL_CONFIG_FILE} so the migrated local override remains ignored."
                )],
            });
        }
    }
    for (legacy_name, canonical_name, expected_directory) in mappings {
        let source = repo_path.join(legacy_name);
        let metadata = match fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "Failed to inspect migration source {}: {error}",
                    source.display()
                ))
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Cannot migrate symbolic link {}. Replace it with a regular file or directory first.",
                source.display()
            ));
        }
        let target = repo_path.join(canonical_name);
        match fs::symlink_metadata(&target) {
            Ok(_) => {
                return Err(format!(
                    "Cannot migrate {} because {} already exists. Norn never overwrites a canonical repository config target.",
                    source.display(),
                    target.display()
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Failed to inspect migration target {}: {error}",
                    target.display()
                ))
            }
        }
        let is_dir = metadata.is_dir();
        if !is_dir && !metadata.is_file() {
            return Err(format!(
                "Cannot migrate {} because it is not a regular file or directory.",
                source.display()
            ));
        }
        if is_dir != expected_directory {
            return Err(format!(
                "Cannot migrate {} because it must be a regular {}.",
                source.display(),
                if expected_directory {
                    "directory"
                } else {
                    "file"
                }
            ));
        }
        let content_changes = if migration_has_content_changes(&source)? {
            vec![
                "Rewrite legacy .lachesi repository-config paths to their .norn equivalents."
                    .to_string(),
            ]
        } else {
            Vec::new()
        };
        actions.push(RepoConfigMigrationAction {
            source: source.display().to_string(),
            target: target.display().to_string(),
            kind: if is_dir { "directory" } else { "file" }.to_string(),
            content_changes,
        });
    }

    if !dry_run {
        for action in &actions {
            let source = Path::new(&action.source);
            let target = Path::new(&action.target);
            if action.kind == "edit" {
                ensure_canonical_local_ignore(repo_path)?;
            } else if action.kind == "directory" {
                migrate_config_directory(source, target)?;
            } else {
                migrate_config_file(source, target)?;
            }
        }
    }

    Ok(RepoConfigMigrationResult {
        repo_path: repo_path.display().to_string(),
        dry_run,
        actions,
    })
}

fn local_policy_path_is_ignored(repo_path: &Path, file_name: &str) -> Result<bool, String> {
    let ignored = run_local_policy_git(
        repo_path,
        &["check-ignore", "--quiet", "--no-index", "--", file_name],
    )
    .map_err(|error| format!("Failed to inspect canonical local policy ignore state: {error}"))?;
    match ignored.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "Could not determine whether {file_name} is ignored: {}",
            String::from_utf8_lossy(&ignored.stderr).trim()
        )),
    }
}

fn ensure_canonical_local_ignore(repo_path: &Path) -> Result<(), String> {
    let path = repo_path.join(".gitignore");
    let existing_permissions = match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "Cannot update symbolic link {}. Replace it with a regular file first.",
                path.display()
            ));
        }
        Ok(metadata) if metadata.is_file() => Some(metadata.permissions()),
        Ok(_) => {
            return Err(format!("{} must be a regular file.", path.display()));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("Failed to inspect {}: {error}", path.display())),
    };
    let original = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(format!("Failed to read {}: {error}", path.display())),
    };
    let text = std::str::from_utf8(&original)
        .map_err(|error| format!("{} is not valid UTF-8: {error}", path.display()))?;
    let mut updated = text.to_string();
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&format!("/{LOCAL_CONFIG_FILE}\n"));

    let mut temporary = tempfile::Builder::new()
        .prefix(".norn-gitignore-migration-")
        .tempfile_in(repo_path)
        .map_err(|error| format!("Failed to stage {}: {error}", path.display()))?;
    temporary
        .write_all(updated.as_bytes())
        .map_err(|error| format!("Failed to stage {}: {error}", path.display()))?;
    set_staged_file_permissions(temporary.as_file(), existing_permissions, &path)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("Failed to sync {}: {error}", path.display()))?;
    if path.exists() {
        temporary
            .persist(&path)
            .map_err(|error| format!("Failed to update {}: {}", path.display(), error.error))?;
    } else {
        temporary.persist_noclobber(&path).map_err(|error| {
            format!(
                "Failed to create {} without overwriting it: {}",
                path.display(),
                error.error
            )
        })?;
    }
    if local_policy_path_is_ignored(repo_path, LOCAL_CONFIG_FILE)? {
        Ok(())
    } else {
        Err(format!(
            "{} is still not ignored after updating {}; fix the Git ignore rules before retrying.",
            LOCAL_CONFIG_FILE,
            path.display()
        ))
    }
}

fn migration_has_content_changes(path: &Path) -> Result<bool, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Failed to inspect {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Cannot migrate symbolic link {}. Replace it with a regular file or directory first.",
            path.display()
        ));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|error| format!("Failed to inspect {}: {error}", path.display()))?
        {
            let entry =
                entry.map_err(|error| format!("Failed to inspect migration source: {error}"))?;
            if migration_has_content_changes(&entry.path())? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if !metadata.is_file() {
        return Err(format!(
            "Cannot migrate unsupported filesystem entry {}.",
            path.display()
        ));
    }
    if !is_yaml_path(path) {
        return Ok(false);
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    Ok(transform_migration_bytes(path, &bytes)? != bytes)
}

fn is_yaml_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("yaml" | "yml")
    )
}

fn transform_migration_bytes(path: &Path, bytes: &[u8]) -> Result<Vec<u8>, String> {
    if !is_yaml_path(path) {
        return Ok(bytes.to_vec());
    }
    serde_yaml::from_slice::<Value>(bytes)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;
    let source = std::str::from_utf8(bytes)
        .map_err(|error| format!("Failed to decode {} as UTF-8: {error}", path.display()))?;
    Ok(rewrite_known_yaml_path_text(source).into_bytes())
}

fn rewrite_known_yaml_path_text(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut path_sequence_indent = None;

    for line_with_ending in source.split_inclusive('\n') {
        let (line, ending) = line_with_ending
            .strip_suffix('\n')
            .map_or((line_with_ending, ""), |line| (line, "\n"));
        let content = line.trim_start_matches([' ', '\t']);
        let indent = line.len() - content.len();
        let significant = !content.is_empty() && !content.starts_with('#');
        if significant && path_sequence_indent.is_some_and(|active| indent <= active) {
            path_sequence_indent = None;
        }

        let field = yaml_mapping_field(content);
        let rewritten = if let Some((field_name, value_is_empty)) = field {
            if is_migrated_path_field(field_name) {
                path_sequence_indent = value_is_empty.then_some(indent);
                rewrite_path_tokens_preserving_comment(line)
            } else if path_sequence_indent.is_some_and(|active| indent > active)
                && yaml_sequence_scalar(content)
            {
                rewrite_path_tokens_preserving_comment(line)
            } else {
                line.to_string()
            }
        } else if path_sequence_indent.is_some_and(|active| indent > active)
            && yaml_sequence_scalar(content)
        {
            rewrite_path_tokens_preserving_comment(line)
        } else {
            line.to_string()
        };
        output.push_str(&rewritten);
        output.push_str(ending);
    }

    output
}

fn yaml_mapping_field(content: &str) -> Option<(&str, bool)> {
    let content = content.strip_prefix("- ").unwrap_or(content);
    let code = &content[..yaml_comment_start(content).unwrap_or(content.len())];
    let colon = code.find(':')?;
    let field = code[..colon]
        .trim()
        .trim_matches(|character| matches!(character, '\'' | '"'));
    if field.is_empty() {
        return None;
    }
    Some((field, code[colon + 1..].trim().is_empty()))
}

fn is_migrated_path_field(field: &str) -> bool {
    matches!(
        field,
        "path" | "packs" | "policyPacks" | "include" | "exclude"
    )
}

fn yaml_sequence_scalar(content: &str) -> bool {
    let Some(value) = content.strip_prefix('-') else {
        return false;
    };
    let code = &value[..yaml_comment_start(value).unwrap_or(value.len())];
    !code.contains(':')
}

fn rewrite_path_tokens_preserving_comment(line: &str) -> String {
    let comment = yaml_comment_start(line).unwrap_or(line.len());
    let (code, suffix) = line.split_at(comment);
    let rewritten = code
        .replace(LEGACY_LOCAL_CONFIG_FILE, LOCAL_CONFIG_FILE)
        .replace(LEGACY_CONFIG_FILE, CONFIG_FILE)
        .replace(".lachesi/", ".norn/");
    format!(
        "{}{suffix}",
        replace_exact_legacy_directory_tokens(&rewritten)
    )
}

fn replace_exact_legacy_directory_tokens(text: &str) -> String {
    const LEGACY: &str = ".lachesi";
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    for (offset, _) in text.match_indices(LEGACY) {
        let end = offset + LEGACY.len();
        let before = text[..offset].chars().next_back();
        let after = text[end..].chars().next();
        let starts_at_boundary = before.is_none_or(|character| {
            character.is_whitespace()
                || matches!(
                    character,
                    '/' | '\\' | '"' | '\'' | '[' | '{' | '(' | ':' | ','
                )
        });
        let ends_at_boundary = after.is_none_or(|character| {
            character.is_whitespace()
                || matches!(character, '/' | '\\' | '"' | '\'' | ']' | '}' | ')' | ',')
        });
        if starts_at_boundary && ends_at_boundary {
            output.push_str(&text[cursor..offset]);
            output.push_str(".norn");
            cursor = end;
        }
    }
    output.push_str(&text[cursor..]);
    output
}

fn yaml_comment_start(text: &str) -> Option<usize> {
    let mut single_quoted = false;
    let mut double_quoted = false;
    let mut escaped = false;
    for (index, character) in text.char_indices() {
        if double_quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                double_quoted = false;
            }
            continue;
        }
        if single_quoted {
            if character == '\'' {
                single_quoted = false;
            }
            continue;
        }
        match character {
            '"' => double_quoted = true,
            '\'' => single_quoted = true,
            '#' if text[..index]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace) =>
            {
                return Some(index)
            }
            _ => {}
        }
    }
    None
}

fn migrate_config_file(source: &Path, target: &Path) -> Result<(), String> {
    let source_permissions = fs::symlink_metadata(source)
        .map_err(|error| format!("Failed to inspect {}: {error}", source.display()))?
        .permissions();
    let bytes = fs::read(source)
        .map_err(|error| format!("Failed to read {}: {error}", source.display()))?;
    let transformed = transform_migration_bytes(source, &bytes)?;
    let parent = target
        .parent()
        .ok_or_else(|| format!("Migration target has no parent: {}", target.display()))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".norn-config-migration-")
        .tempfile_in(parent)
        .map_err(|error| format!("Failed to stage repository config migration: {error}"))?;
    temporary
        .write_all(&transformed)
        .map_err(|error| format!("Failed to stage repository config migration: {error}"))?;
    set_staged_file_permissions(temporary.as_file(), Some(source_permissions), target)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("Failed to sync repository config migration: {error}"))?;
    temporary.persist_noclobber(target).map_err(|error| {
        format!(
            "Failed to publish migrated repository config at {} without overwriting it: {}",
            target.display(),
            error.error
        )
    })?;
    fs::remove_file(source).map_err(|error| {
        format!(
            "Migrated repository config to {}, but could not remove legacy source {}: {error}. Remove the legacy source before loading config again.",
            target.display(),
            source.display()
        )
    })
}

fn set_staged_file_permissions(
    file: &fs::File,
    source_permissions: Option<fs::Permissions>,
    target: &Path,
) -> Result<(), String> {
    let permissions = match source_permissions {
        Some(permissions) => permissions,
        None => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::Permissions::from_mode(0o644)
            }
            #[cfg(not(unix))]
            {
                let mut permissions = file
                    .metadata()
                    .map_err(|error| format!("Failed to inspect staged file: {error}"))?
                    .permissions();
                permissions.set_readonly(false);
                permissions
            }
        }
    };
    file.set_permissions(permissions).map_err(|error| {
        format!(
            "Failed to preserve permissions for {}: {error}",
            target.display()
        )
    })
}

fn migrate_config_directory(source: &Path, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("Migration target has no parent: {}", target.display()))?;
    let staged = tempfile::Builder::new()
        .prefix(".norn-config-migration-")
        .tempdir_in(parent)
        .map_err(|error| format!("Failed to stage repository config migration: {error}"))?;
    copy_config_directory(source, staged.path())?;
    let staged_path = staged.keep();
    if let Err(error) = crate::runtime_identity::rename_directory_noclobber(&staged_path, target) {
        let _ = fs::remove_dir_all(&staged_path);
        return Err(format!(
            "Failed to publish migrated repository config at {} without overwriting it: {error}",
            target.display()
        ));
    }
    fs::remove_dir_all(source).map_err(|error| {
        format!(
            "Migrated repository config to {}, but could not remove legacy source {}: {error}. Remove the legacy source before loading config again.",
            target.display(),
            source.display()
        )
    })
}

fn copy_config_directory(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source)
        .map_err(|error| format!("Failed to read {}: {error}", source.display()))?
    {
        let entry =
            entry.map_err(|error| format!("Failed to inspect migration source: {error}"))?;
        let source_path = entry.path();
        let target_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)
            .map_err(|error| format!("Failed to inspect {}: {error}", source_path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Cannot migrate symbolic link {}. Replace it with a regular file or directory first.",
                source_path.display()
            ));
        }
        if metadata.is_dir() {
            fs::create_dir(&target_path)
                .map_err(|error| format!("Failed to create {}: {error}", target_path.display()))?;
            copy_config_directory(&source_path, &target_path)?;
        } else if metadata.is_file() {
            let bytes = fs::read(&source_path)
                .map_err(|error| format!("Failed to read {}: {error}", source_path.display()))?;
            let transformed = transform_migration_bytes(&source_path, &bytes)?;
            fs::write(&target_path, transformed)
                .map_err(|error| format!("Failed to write {}: {error}", target_path.display()))?;
            fs::set_permissions(&target_path, metadata.permissions()).map_err(|error| {
                format!(
                    "Failed to preserve permissions for {}: {error}",
                    target_path.display()
                )
            })?;
        } else {
            return Err(format!(
                "Cannot migrate unsupported filesystem entry {}.",
                source_path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn finalize_resolved_config(
    repo_path: &Path,
    config: &RepoReviewConfig,
    profile_override: Option<&str>,
) -> Result<RepoReviewConfigLoadResult, String> {
    let contents = serde_yaml::to_string(config)
        .map_err(|error| format!("Failed to serialize resolved policy: {error}"))?;
    Ok(load_from_str(
        repo_path,
        Path::new("<resolved-policy>"),
        &contents,
        profile_override,
    ))
}

fn compact_policy_layer(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(object) => JsonValue::Object(
            object
                .into_iter()
                .filter_map(|(key, value)| {
                    let value = compact_policy_layer(value);
                    let empty = match &value {
                        JsonValue::Null => true,
                        JsonValue::Array(items) => items.is_empty(),
                        JsonValue::Object(object) => object.is_empty(),
                        _ => false,
                    };
                    (!empty).then_some((key, value))
                })
                .collect(),
        ),
        JsonValue::Array(items) => {
            JsonValue::Array(items.into_iter().map(compact_policy_layer).collect())
        }
        value => value,
    }
}

fn apply_review_profile(
    config_path: &Path,
    config: &mut RepoReviewConfig,
    profile_override: Option<&str>,
    warnings: &mut Vec<RepoConfigValidationMessage>,
) -> Option<String> {
    let requested_profile = profile_override
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            config
                .review
                .as_ref()
                .and_then(|review| review.profile.as_deref())
                .map(str::trim)
                .filter(|profile| !profile.is_empty())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            config
                .profiles
                .contains_key("default")
                .then(|| "default".to_string())
        });

    let Some(profile_id) = requested_profile else {
        return None;
    };
    let Some(profile) = config.profiles.get(&profile_id).cloned() else {
        warnings.push(message(
            config_path,
            format!("Review profile `{profile_id}` was not found; using base review config."),
        ));
        return None;
    };

    let review = config.review.get_or_insert_with(ReviewConfig::default);
    review.profile = Some(profile_id.clone());
    if let Some(mode) = profile.mode {
        review.mode = Some(mode);
    }
    if let Some(min_severity) = profile.min_severity {
        review
            .findings
            .get_or_insert_with(FindingConfig::default)
            .min_severity = Some(min_severity);
    }
    merge_prompt_config(&mut review.prompt, profile.prompt);

    if !profile.policy_packs.is_empty() {
        config
            .policy
            .get_or_insert_with(PolicyConfig::default)
            .packs
            .extend(profile.policy_packs);
    }

    Some(profile_id)
}

fn apply_profile_analyzer_requirements(
    config_path: &Path,
    config: &mut RepoReviewConfig,
    profile_id: &str,
    errors: &mut Vec<RepoConfigValidationMessage>,
) {
    let Some(profile) = config.profiles.get(profile_id) else {
        return;
    };
    let requirements = profile.analyzers.clone();
    for (id, requirement) in requirements {
        match requirement {
            ProfileAnalyzerRequirement::Required => {
                if let Some(analyzer) = config.analyzers.get_mut(&id) {
                    analyzer.enabled = true;
                    analyzer.required = true;
                } else {
                    errors.push(message(
                        config_path,
                        format!(
                            "Review profile `{profile_id}` requires analyzer `{id}`, but no analyzer config is available."
                        ),
                    ));
                }
            }
            ProfileAnalyzerRequirement::Disabled => {
                if let Some(analyzer) = config.analyzers.get_mut(&id) {
                    analyzer.enabled = false;
                }
            }
            ProfileAnalyzerRequirement::Optional => {}
        }
    }
}

fn load_policy_packs(
    repo_path: &Path,
    config_path: &Path,
    config: &mut RepoReviewConfig,
    warnings: &mut Vec<RepoConfigValidationMessage>,
    errors: &mut Vec<RepoConfigValidationMessage>,
    loaded_pack_paths: &mut BTreeSet<String>,
) -> Vec<LoadedPolicyPack> {
    let Some(policy) = config.policy.as_ref() else {
        return Vec::new();
    };

    let mut pack_refs = policy.packs.clone();
    pack_refs.extend(
        policy
            .sources
            .iter()
            .filter(|source| source.source_type == "pack")
            .map(|source| source.path.clone()),
    );

    let mut loaded = Vec::new();
    for pack_ref in pack_refs {
        let resolved_path = resolve_pack_manifest_path(repo_path, &pack_ref);
        let Some(pack_path) = resolved_path else {
            let missing_key = format!("missing:{pack_ref}");
            if !loaded_pack_paths.insert(missing_key) {
                continue;
            }
            warnings.push(message(
                config_path,
                format!("Policy pack `{pack_ref}` was not found."),
            ));
            continue;
        };
        let pack_path_key = pack_path.display().to_string();
        if !loaded_pack_paths.insert(pack_path_key.clone()) {
            continue;
        }

        let value = match fs::read_to_string(&pack_path)
            .map_err(|error| {
                format!(
                    "Failed to read policy pack `{}`: {error}",
                    pack_path.display()
                )
            })
            .and_then(|contents| {
                serde_yaml::from_str::<Value>(&contents).map_err(|error| {
                    format!(
                        "Failed to parse policy pack `{}`: {error}",
                        pack_path.display()
                    )
                })
            }) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(message(config_path, error));
                continue;
            }
        };

        let secret_errors = forbidden_field_errors(&pack_path, &value);
        if !secret_errors.is_empty() {
            errors.extend(secret_errors);
            continue;
        }

        let pack = match serde_yaml::from_value::<PolicyPackConfig>(value) {
            Ok(pack) => pack,
            Err(error) => {
                warnings.push(message(
                    &pack_path,
                    format!("Invalid policy pack shape: {error}"),
                ));
                continue;
            }
        };

        let pack_id = pack
            .id
            .clone()
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| default_policy_pack_id(&pack_path));
        let pack_name = pack.name.clone();

        merge_policy_pack(config, pack, warnings, &pack_path);
        loaded.push(LoadedPolicyPack {
            id: pack_id,
            name: pack_name,
            path: pack_path_key,
        });
    }

    loaded
}

fn default_policy_pack_id(pack_path: &Path) -> String {
    let file_stem = pack_path.file_stem().and_then(|name| name.to_str());
    if matches!(file_stem, Some("pack" | "lachesi-pack" | ".lachesi-pack")) {
        return pack_path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("policy-pack")
            .to_string();
    }

    file_stem.unwrap_or("policy-pack").to_string()
}

fn resolve_pack_manifest_path(repo_path: &Path, pack_ref: &str) -> Option<PathBuf> {
    let raw_path = Path::new(pack_ref);
    let path = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        repo_path.join(raw_path)
    };

    if path.is_file() {
        return Some(path);
    }
    if !path.is_dir() {
        return None;
    }

    ["pack.yaml", "lachesi-pack.yaml", ".lachesi-pack.yaml"]
        .iter()
        .map(|file| path.join(file))
        .find(|candidate| candidate.is_file())
}

fn merge_policy_pack(
    config: &mut RepoReviewConfig,
    pack: PolicyPackConfig,
    warnings: &mut Vec<RepoConfigValidationMessage>,
    pack_path: &Path,
) {
    if let Some(pack_review) = pack.review {
        merge_review_config(&mut config.review, pack_review);
    }

    if let Some(mut pack_policy) = pack.policy {
        if !pack_policy.packs.is_empty() {
            warnings.push(message(
                pack_path,
                "Nested policy packs are not loaded from inside a policy pack.",
            ));
            pack_policy.packs.clear();
        }
        let target = config.policy.get_or_insert_with(PolicyConfig::default);
        target.sources.extend(pack_policy.sources);
        target.rules.extend(pack_policy.rules);
        target.path_rules.extend(pack_policy.path_rules);
        target.ast_rules.extend(pack_policy.ast_rules);
        target.suppressions.extend(pack_policy.suppressions);
    }

    for (id, profile) in pack.profiles {
        config.profiles.entry(id).or_insert(profile);
    }

    for (id, analyzer) in pack.analyzers {
        config.analyzers.entry(id).or_insert(analyzer);
    }
}

fn merge_review_config(target: &mut Option<ReviewConfig>, pack_review: ReviewConfig) {
    let target = target.get_or_insert_with(ReviewConfig::default);
    if target.mode.is_none() {
        target.mode = pack_review.mode;
    }
    merge_prompt_config(&mut target.prompt, pack_review.prompt);
    if target.findings.is_none() {
        target.findings = pack_review.findings;
    }
}

fn merge_prompt_config(target: &mut Option<PromptConfig>, pack_prompt: Option<PromptConfig>) {
    let Some(pack_prompt) = pack_prompt else {
        return;
    };
    let target = target.get_or_insert_with(PromptConfig::default);
    if target.replace.is_none() {
        target.replace = pack_prompt.replace;
    }
    if let Some(pack_extend) = pack_prompt.extend {
        target.extend = match target.extend.take() {
            Some(existing) if !existing.trim().is_empty() => {
                Some(format!("{pack_extend}\n\n{existing}"))
            }
            _ => Some(pack_extend),
        }
    }
}

fn result(
    repo_path: &Path,
    config_path: &Path,
    exists: bool,
    config: Option<RepoReviewConfig>,
    selected_profile: Option<String>,
    loaded_policy_packs: Vec<LoadedPolicyPack>,
    warnings: Vec<RepoConfigValidationMessage>,
    errors: Vec<RepoConfigValidationMessage>,
) -> RepoReviewConfigLoadResult {
    RepoReviewConfigLoadResult {
        repo_path: repo_path.display().to_string(),
        config_path: config_path.display().to_string(),
        exists,
        config,
        selected_profile,
        loaded_policy_packs,
        warnings,
        errors,
    }
}

fn message(path: &Path, message: impl Into<String>) -> RepoConfigValidationMessage {
    RepoConfigValidationMessage {
        path: path.display().to_string(),
        message: message.into(),
    }
}

fn unknown_field_warnings(config_path: &Path, value: &Value) -> Vec<RepoConfigValidationMessage> {
    let mut warnings = Vec::new();
    collect_unknown_fields(config_path, value, "$", None, &mut warnings);
    warnings
}

fn collect_unknown_fields(
    config_path: &Path,
    value: &Value,
    path: &str,
    context: Option<&str>,
    warnings: &mut Vec<RepoConfigValidationMessage>,
) {
    if context == Some("opaque") {
        return;
    }
    let mapping = match value {
        Value::Mapping(mapping) => mapping,
        Value::Sequence(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_unknown_fields(
                    config_path,
                    item,
                    &format!("{path}[{index}]"),
                    context,
                    warnings,
                );
            }
            return;
        }
        _ => return,
    };

    for (key, child) in mapping {
        let Some(key) = key.as_str() else {
            continue;
        };
        let child_path = format!("{path}.{key}");
        if let Some(known) = known_keys(context, key) {
            if !known.contains(&key) && !key.starts_with("x-") {
                warnings.push(message(
                    config_path,
                    format!("Unknown repo config field `{child_path}`."),
                ));
            }
        }

        let next_context = next_context(context, key);
        if next_context == Some("analyzerMap") {
            collect_analyzer_fields(config_path, child, &child_path, warnings);
        } else if next_context == Some("profileMap") {
            collect_profile_fields(config_path, child, &child_path, warnings);
        } else if next_context == Some("profileAnalyzerMap") {
            // Analyzer requirement ids are user-defined keys.
        } else {
            collect_unknown_fields(config_path, child, &child_path, next_context, warnings);
        }
    }
}

fn collect_profile_fields(
    config_path: &Path,
    value: &Value,
    path: &str,
    warnings: &mut Vec<RepoConfigValidationMessage>,
) {
    let Value::Mapping(mapping) = value else {
        return;
    };
    for (key, child) in mapping {
        let Some(profile_id) = key.as_str() else {
            continue;
        };
        let profile_path = format!("{path}.{profile_id}");
        collect_unknown_fields(config_path, child, &profile_path, Some("profile"), warnings);
    }
}

fn collect_analyzer_fields(
    config_path: &Path,
    value: &Value,
    path: &str,
    warnings: &mut Vec<RepoConfigValidationMessage>,
) {
    let Value::Mapping(mapping) = value else {
        return;
    };
    for (key, child) in mapping {
        let Some(analyzer_id) = key.as_str() else {
            continue;
        };
        let analyzer_path = format!("{path}.{analyzer_id}");
        collect_unknown_fields(
            config_path,
            child,
            &analyzer_path,
            Some("analyzer"),
            warnings,
        );
    }
}

fn known_keys(context: Option<&str>, key: &str) -> Option<&'static [&'static str]> {
    match context {
        None => Some(&[
            "version",
            "review",
            "profiles",
            "paths",
            "policy",
            "analyzers",
            "publish",
        ]),
        Some("review") => Some(&["profile", "mode", "prompt", "findings"]),
        Some("profile") => Some(&["mode", "minSeverity", "prompt", "policyPacks", "analyzers"]),
        Some("prompt") => Some(&["extend", "replace"]),
        Some("findings") => Some(&["minSeverity", "requireAnchors"]),
        Some("paths") | Some("appliesTo") => Some(&["include", "exclude"]),
        Some("policy") => Some(&[
            "packs",
            "sources",
            "rules",
            "pathRules",
            "astRules",
            "suppressions",
        ]),
        Some("policySource") => Some(&["type", "path"]),
        Some("rule") => Some(&[
            "id",
            "source",
            "severity",
            "confidence",
            "appliesTo",
            "instruction",
            "rationale",
            "remediation",
            "enforcement",
        ]),
        Some("pathRule") => Some(&[
            "id",
            "severity",
            "paths",
            "instruction",
            "rationale",
            "remediation",
        ]),
        Some("astRule") => Some(&[
            "id",
            "language",
            "severity",
            "selector",
            "appliesTo",
            "instruction",
            "rationale",
            "remediation",
        ]),
        Some("selector") => Some(&["kind", "callee", "argumentContains"]),
        Some("suppression") => Some(&["ruleId", "paths", "reason", "expiresAt"]),
        Some("analyzer") => Some(&["enabled", "command", "timeoutSeconds", "required", "config"]),
        Some("publish") => Some(&["defaultMode", "requireManualSubmit", "allowGeneralComments"]),
        Some("analyzerMap") => {
            let _ = key;
            None
        }
        Some("profileMap") | Some("profileAnalyzerMap") => {
            let _ = key;
            None
        }
        _ => None,
    }
}

fn next_context(context: Option<&str>, key: &str) -> Option<&'static str> {
    match (context, key) {
        (None, "review") => Some("review"),
        (None, "profiles") => Some("profileMap"),
        (None, "paths") => Some("paths"),
        (None, "policy") => Some("policy"),
        (None, "analyzers") => Some("analyzerMap"),
        (None, "publish") => Some("publish"),
        (Some("review"), "prompt") => Some("prompt"),
        (Some("review"), "findings") => Some("findings"),
        (Some("profile"), "prompt") => Some("prompt"),
        (Some("profile"), "analyzers") => Some("profileAnalyzerMap"),
        (Some("analyzer"), "config") => Some("opaque"),
        (Some("policy"), "sources") => Some("policySource"),
        (Some("policy"), "rules") => Some("rule"),
        (Some("policy"), "pathRules") => Some("pathRule"),
        (Some("policy"), "astRules") => Some("astRule"),
        (Some("policy"), "suppressions") => Some("suppression"),
        (Some("rule"), "appliesTo") | (Some("astRule"), "appliesTo") => Some("appliesTo"),
        (Some("pathRule"), "paths") | (Some("suppression"), "paths") => Some("paths"),
        (Some("astRule"), "selector") => Some("selector"),
        _ => None,
    }
}

fn forbidden_field_errors(config_path: &Path, value: &Value) -> Vec<RepoConfigValidationMessage> {
    let mut errors = Vec::new();
    collect_forbidden_fields(config_path, value, "$", &mut errors);
    errors
}

fn collect_forbidden_fields(
    config_path: &Path,
    value: &Value,
    path: &str,
    errors: &mut Vec<RepoConfigValidationMessage>,
) {
    match value {
        Value::Mapping(mapping) => {
            for (key, child) in mapping {
                let Some(key) = key.as_str() else {
                    continue;
                };
                let child_path = format!("{path}.{key}");
                let normalized = key
                    .chars()
                    .filter(|ch| ch.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_lowercase();
                if matches!(
                    normalized.as_str(),
                    "credential"
                        | "credentials"
                        | "token"
                        | "apitoken"
                        | "password"
                        | "secret"
                        | "username"
                ) {
                    errors.push(message(
                        config_path,
                        format!(
                            "Repo config field `{child_path}` looks like a credential. Store secrets in the keychain or environment instead."
                        ),
                    ));
                }
                collect_forbidden_fields(config_path, child, &child_path, errors);
            }
        }
        Value::Sequence(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_forbidden_fields(config_path, child, &format!("{path}[{index}]"), errors);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_init_action_if_needed, finalize_resolved_config, load_from_repo_path,
        load_from_str, load_local_policy_layer, load_repository_policy_layer,
        migrate_repository_config, validate_external_config_layer,
        write_default_repo_config_if_missing, RepoReviewConfig, RepoReviewConfigLoadResult,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_REPO_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_repo() -> PathBuf {
        let nonce = TEMP_REPO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "lachesi-repo-config-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp repo");
        path
    }

    fn load_test_config(repo: &std::path::Path, contents: &str) -> RepoReviewConfigLoadResult {
        load_from_str(repo, &repo.join(".norn.yaml"), contents, None)
    }

    fn load_test_config_with_profile(
        repo: &std::path::Path,
        contents: &str,
        profile: &str,
    ) -> RepoReviewConfigLoadResult {
        load_from_str(repo, &repo.join(".norn.yaml"), contents, Some(profile))
    }

    #[test]
    fn missing_config_is_valid_empty_result() {
        let repo = temp_repo();
        let result = load_from_repo_path(&repo).expect("load result");
        assert!(!result.exists);
        assert!(result.config.is_none());
        assert!(result.warnings.is_empty());
        assert!(result.errors.is_empty());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn default_init_action_is_suggested_for_fresh_repository() {
        let repo = temp_repo();
        let action = default_init_action_if_needed(&repo).expect("proposal");
        assert!(action.is_some());
        let action = action.expect("fresh repo action");
        assert_eq!(action.kind, "file");
        assert!(action.source == "<norn-init-template>");
        assert!(action.target.ends_with(".norn.yaml"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn write_default_repo_config_is_created_once() {
        let repo = temp_repo();
        assert!(!repo.join(".norn.yaml").exists());

        assert!(write_default_repo_config_if_missing(&repo).expect("first write"));
        assert!(repo.join(".norn.yaml").exists());
        let contents = fs::read_to_string(repo.join(".norn.yaml")).expect("written config");
        assert!(contents.contains("version: 0.1"));
        assert!(contents.contains("review:\n  mode: balanced"));

        assert!(!write_default_repo_config_if_missing(&repo).expect("second write"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn canonical_config_wins_for_fresh_repositories() {
        let repo = temp_repo();
        fs::write(repo.join(".norn.yaml"), "version: 0.1\n").expect("canonical config");

        let result = load_from_repo_path(&repo).expect("canonical load");

        assert!(result.exists);
        assert_eq!(
            result.config_path,
            repo.join(".norn.yaml").to_string_lossy()
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn legacy_config_remains_a_fallback() {
        let repo = temp_repo();
        fs::write(repo.join(".lachesi.yaml"), "version: 0.1\n").expect("legacy config");

        let result = load_from_repo_path(&repo).expect("legacy fallback");

        assert!(result.exists);
        assert_eq!(
            result.config_path,
            repo.join(".lachesi.yaml").to_string_lossy()
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn every_canonical_and_legacy_root_combination_is_rejected() {
        for (canonical_name, legacy_name) in [
            (".norn.yaml", ".lachesi.yaml"),
            (".norn.yaml", ".lachesi"),
            (".norn", ".lachesi.yaml"),
            (".norn", ".lachesi"),
        ] {
            let repo = temp_repo();
            for name in [canonical_name, legacy_name] {
                let path = repo.join(name);
                if name.ends_with(".yaml") {
                    fs::write(path, "version: 0.1\n").expect("config file");
                } else {
                    fs::create_dir(path).expect("config directory");
                }
            }

            let error = load_from_repo_path(&repo).expect_err("mixed namespaces must fail");
            assert!(error.contains("will not merge them implicitly"), "{error}");
            let _ = fs::remove_dir_all(repo);
        }
    }

    #[test]
    fn file_and_directory_roots_in_the_same_namespace_are_rejected() {
        for (file_name, directory_name) in [(".norn.yaml", ".norn"), (".lachesi.yaml", ".lachesi")]
        {
            let repo = temp_repo();
            fs::write(repo.join(file_name), "version: 0.1\n").expect("config file");
            fs::create_dir(repo.join(directory_name)).expect("config directory");

            let error = load_from_repo_path(&repo).expect_err("ambiguous roots must fail");

            assert!(error.contains("will not choose between them implicitly"));
            let _ = fs::remove_dir_all(repo);
        }
    }

    #[test]
    fn parses_valid_minimal_config() {
        let repo = temp_repo();
        let path = repo.join(".lachesi.yaml");
        fs::write(
            &path,
            r#"
version: 0.1
review:
  mode: balanced
publish:
  requireManualSubmit: true
"#,
        )
        .expect("write config");

        let result = load_from_repo_path(&repo).expect("load result");
        assert!(result.exists);
        assert!(result.errors.is_empty());
        assert_eq!(
            result.config.as_ref().map(|config| config.version.as_str()),
            Some("0.1")
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn unknown_fields_warn_without_blocking() {
        let repo = temp_repo();
        let result = load_test_config(
            &repo,
            r#"
version: 0.1
x-experiment: true
review:
  mode: fast
  surprise: true
"#,
        );

        assert!(result.errors.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("$.review.surprise"));
    }

    #[test]
    fn unsupported_version_is_blocking_error() {
        let repo = temp_repo();
        let result = load_test_config(
            &repo,
            r#"
version: 2.0
"#,
        );

        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0]
            .message
            .contains("Unsupported Norn repository config version"));
    }

    #[test]
    fn credential_fields_are_blocking_errors() {
        let repo = temp_repo();
        let result = load_test_config(
            &repo,
            r#"
version: 0.1
token: abc123
"#,
        );

        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].message.contains("looks like a credential"));
    }

    #[test]
    fn raw_repository_policy_layer_rejects_credentials() {
        let repo = temp_repo();
        fs::write(
            repo.join(".lachesi.yaml"),
            r#"
version: 0.1
provider:
  apiToken: should-not-be-here
  password: neither-should-this
"#,
        )
        .expect("write config");

        let error = load_repository_policy_layer(&repo)
            .expect_err("raw organization merge layer must reject credentials");
        assert!(error.contains("$.provider.apiToken"));
        assert!(error.contains("$.provider.password"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn raw_repository_policy_layer_preserves_unknown_field_warnings() {
        let repo = temp_repo();
        fs::write(
            repo.join(".lachesi.yaml"),
            r#"
version: 0.1
review:
  mode: strict
  surprise: true
"#,
        )
        .expect("write config");

        let (_, warnings) =
            load_repository_policy_layer(&repo).expect("raw organization merge layer");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("$.review.surprise"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn raw_repository_policy_layer_rejects_invalid_standalone_config() {
        let repo = temp_repo();
        fs::write(repo.join(".lachesi.yaml"), "review:\n  mode: strict\n")
            .expect("missing version config");

        let error = load_repository_policy_layer(&repo)
            .expect_err("missing standalone config version must fail");
        assert!(error.contains("missing field `version`"));

        fs::write(
            repo.join(".lachesi.yaml"),
            "version: 0.1\nanalyzers:\n  check:\n    enabled: true\n",
        )
        .expect("invalid analyzer config");
        let error =
            load_repository_policy_layer(&repo).expect_err("invalid standalone analyzer must fail");
        assert!(error.contains("enabled but has no command"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn local_policy_layer_must_be_untracked_and_ignored() {
        let ignored_repo = temp_repo();
        assert!(std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(&ignored_repo)
            .status()
            .expect("git init")
            .success());
        fs::write(ignored_repo.join(".gitignore"), ".lachesi.local.yaml\n")
            .expect("ignore fixture");
        fs::write(
            ignored_repo.join(".lachesi.local.yaml"),
            "review:\n  mode: strict\n",
        )
        .expect("local policy fixture");
        assert!(load_local_policy_layer(&ignored_repo)
            .expect("ignored local policy")
            .is_some());

        let unignored_repo = temp_repo();
        assert!(std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(&unignored_repo)
            .status()
            .expect("git init")
            .success());
        fs::write(
            unignored_repo.join(".lachesi.local.yaml"),
            "review:\n  mode: strict\n",
        )
        .expect("local policy fixture");
        assert!(load_local_policy_layer(&unignored_repo)
            .expect_err("unignored local policy")
            .contains("covered by a Git ignore rule"));

        let tracked_repo = temp_repo();
        assert!(std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(&tracked_repo)
            .status()
            .expect("git init")
            .success());
        fs::write(
            tracked_repo.join(".lachesi.local.yaml"),
            "review:\n  mode: strict\n",
        )
        .expect("local policy fixture");
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(&tracked_repo)
            .args(["add", ".lachesi.local.yaml"])
            .status()
            .expect("git add")
            .success());
        assert!(load_local_policy_layer(&tracked_repo)
            .expect_err("tracked local policy")
            .contains("tracked by Git"));

        let _ = fs::remove_dir_all(ignored_repo);
        let _ = fs::remove_dir_all(unignored_repo);
        let _ = fs::remove_dir_all(tracked_repo);
    }

    #[test]
    fn canonical_local_policy_layer_is_untracked_ignored_and_preferred() {
        let repo = temp_repo();
        assert!(std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(&repo)
            .status()
            .expect("git init")
            .success());
        fs::write(repo.join(".gitignore"), ".norn.local.yaml\n").expect("ignore fixture");
        fs::write(repo.join(".norn.local.yaml"), "review:\n  mode: strict\n")
            .expect("canonical local policy");

        assert!(load_local_policy_layer(&repo)
            .expect("canonical local policy")
            .is_some());

        fs::write(repo.join(".lachesi.local.yaml"), "review:\n  mode: fast\n")
            .expect("legacy local policy");
        let error = load_local_policy_layer(&repo).expect_err("mixed local overrides must fail");
        assert!(error.contains("will not merge them implicitly"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn repository_config_migration_previews_rewrites_and_never_overwrites() {
        let repo = temp_repo();
        fs::write(
            repo.join(".lachesi.yaml"),
            "version: 0.1\npolicy:\n  packs:\n    - \".lachesi/packs/team\" # keep .lachesi/comment\n  policyPacks: [\".lachesi\", \"./.lachesi\", \".lachesi-pack.yaml\"]\nrules:\n  - paths:\n      include:\n        - ./.lachesi.yaml\n      exclude:\n        - config/.lachesi.local.yaml\n",
        )
        .expect("legacy root config");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                repo.join(".lachesi.yaml"),
                fs::Permissions::from_mode(0o640),
            )
            .expect("legacy config permissions");
        }
        let preview = migrate_repository_config(&repo, true).expect("migration preview");
        assert!(preview.dry_run);
        assert_eq!(preview.actions.len(), 1);
        assert!(preview
            .actions
            .iter()
            .all(|action| !action.content_changes.is_empty()));
        assert!(repo.join(".lachesi.yaml").exists());
        assert!(!repo.join(".norn.yaml").exists());

        let migrated = migrate_repository_config(&repo, false).expect("migration execution");
        assert!(!migrated.dry_run);
        assert!(!repo.join(".lachesi.yaml").exists());
        assert!(fs::read_to_string(repo.join(".norn.yaml"))
            .expect("canonical root config")
            .contains(".norn/packs/team"));
        let canonical =
            fs::read_to_string(repo.join(".norn.yaml")).expect("canonical root config paths");
        assert!(canonical.contains("./.norn.yaml"));
        assert!(canonical.contains("config/.norn.local.yaml"));
        assert!(canonical.contains("    - \".norn/packs/team\" # keep .lachesi/comment"));
        assert!(canonical.contains("policyPacks: [\".norn\", \"./.norn\", \".lachesi-pack.yaml\"]"));
        assert!(!canonical.contains("./.lachesi.yaml"));
        assert!(!canonical.contains("config/.lachesi.local.yaml"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(repo.join(".norn.yaml"))
                    .expect("canonical config metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o640
            );
        }
        assert!(migrate_repository_config(&repo, false)
            .expect("idempotent migration")
            .actions
            .is_empty());

        let canonical_before =
            fs::read_to_string(repo.join(".norn.yaml")).expect("canonical config before conflict");
        fs::write(repo.join(".lachesi.yaml"), "version: 0.1\n").expect("legacy conflict");
        let error = migrate_repository_config(&repo, false)
            .expect_err("canonical targets must never be overwritten");
        assert!(error.contains("never overwrites"));
        assert_eq!(
            fs::read_to_string(repo.join(".norn.yaml")).expect("preserved canonical config"),
            canonical_before
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn repository_config_directory_migration_is_no_clobber_and_rewrites_yaml() {
        let repo = temp_repo();
        fs::create_dir_all(repo.join(".lachesi/packs/team")).expect("legacy pack directory");
        fs::write(
            repo.join(".lachesi/packs/team/pack.yaml"),
            "id: team\nreview:\n  prompt:\n    extend: Keep the prose example .lachesi/examples unchanged.\npolicy:\n  packs:\n    - .lachesi/packs/base\n",
        )
        .expect("legacy pack");

        migrate_repository_config(&repo, false).expect("directory migration");

        assert!(!repo.join(".lachesi").exists());
        assert!(fs::read_to_string(repo.join(".norn/packs/team/pack.yaml"))
            .expect("canonical pack")
            .contains(".norn/packs/base"));
        assert!(fs::read_to_string(repo.join(".norn/packs/team/pack.yaml"))
            .expect("canonical pack")
            .contains("prose example .lachesi/examples unchanged"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn repository_config_directory_migration_stages_before_atomic_publication() {
        let repo = temp_repo();
        fs::create_dir_all(repo.join(".lachesi/packs/team")).expect("legacy pack directory");
        fs::write(repo.join(".lachesi/packs/team/pack.yaml"), "not: [valid")
            .expect("invalid legacy pack");

        let error = migrate_repository_config(&repo, false)
            .expect_err("invalid staged config must not publish a target");

        assert!(error.contains("Failed to parse"));
        assert!(repo.join(".lachesi").exists());
        assert!(!repo.join(".norn").exists());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn atomic_directory_publication_never_replaces_a_concurrent_target() {
        let repo = temp_repo();
        let staged = repo.join("staged");
        let target = repo.join("target");
        fs::create_dir(&staged).expect("staged directory");
        fs::write(staged.join("new.txt"), "new").expect("staged content");
        fs::create_dir(&target).expect("concurrent target");
        fs::write(target.join("existing.txt"), "existing").expect("target content");

        let error = crate::runtime_identity::rename_directory_noclobber(&staged, &target)
            .expect_err("concurrent target must not be replaced");

        assert!(matches!(
            error.kind(),
            std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::Other
        ));
        assert!(staged.join("new.txt").exists());
        assert_eq!(
            fs::read_to_string(target.join("existing.txt")).expect("preserved target"),
            "existing"
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn repository_config_migration_rejects_ambiguous_legacy_roots() {
        let repo = temp_repo();
        fs::write(repo.join(".lachesi.yaml"), "version: 0.1\n").expect("legacy file");
        fs::create_dir(repo.join(".lachesi")).expect("legacy directory");

        let error = migrate_repository_config(&repo, true)
            .expect_err("ambiguous legacy roots must not be migrated");

        assert!(error.contains("ambiguous repository config roots"));
        assert!(!repo.join(".norn.yaml").exists());
        assert!(!repo.join(".norn").exists());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn repository_config_migration_rejects_mixed_canonical_and_legacy_roots() {
        for (canonical, legacy) in [(".norn", ".lachesi.yaml"), (".norn.yaml", ".lachesi")] {
            let repo = temp_repo();
            if canonical.ends_with(".yaml") {
                fs::write(repo.join(canonical), "version: 0.1\n").expect("canonical file");
            } else {
                fs::create_dir(repo.join(canonical)).expect("canonical directory");
            }
            if legacy.ends_with(".yaml") {
                fs::write(repo.join(legacy), "version: 0.1\n").expect("legacy file");
            } else {
                fs::create_dir(repo.join(legacy)).expect("legacy directory");
            }

            let error = migrate_repository_config(&repo, false)
                .expect_err("mixed canonical and legacy roots must not be migrated");

            assert!(error.contains("canonical .norn.yaml or .norn root already exists"));
            assert!(repo.join(canonical).exists());
            assert!(repo.join(legacy).exists());
            let _ = fs::remove_dir_all(repo);
        }
    }

    #[cfg(unix)]
    #[test]
    fn repository_config_migration_rejects_symlink_sources() {
        use std::os::unix::fs::symlink;

        let repo = temp_repo();
        let outside = repo.with_extension("secret.yaml");
        fs::write(&outside, "token: must-not-be-copied\n").expect("outside file");
        symlink(&outside, repo.join(".lachesi.yaml")).expect("legacy symlink");

        let error = migrate_repository_config(&repo, false)
            .expect_err("symlinked migration source must fail");

        assert!(error.contains("Cannot migrate symbolic link"));
        assert!(!repo.join(".norn.yaml").exists());
        let _ = fs::remove_dir_all(repo);
        let _ = fs::remove_file(outside);
    }

    #[cfg(unix)]
    #[test]
    fn repository_config_migration_dry_run_rejects_nested_symlinks_without_reading_them() {
        use std::os::unix::fs::symlink;

        let repo = temp_repo();
        let outside = repo.with_extension("outside.yaml");
        fs::create_dir(repo.join(".lachesi")).expect("legacy config directory");
        fs::write(&outside, b"\xff\xfe\xfd").expect("outside non-UTF-8 file");
        symlink(&outside, repo.join(".lachesi/policy.yaml")).expect("nested legacy symlink");

        let error = migrate_repository_config(&repo, true)
            .expect_err("dry-run must reject nested symlinks before reading them");

        assert!(error.contains("Cannot migrate symbolic link"));
        assert!(!repo.join(".norn").exists());
        let _ = fs::remove_dir_all(repo);
        let _ = fs::remove_file(outside);
    }

    #[test]
    fn repository_config_migration_rejects_wrong_source_kinds() {
        for legacy_name in [".lachesi.yaml", ".lachesi.local.yaml"] {
            let repo = temp_repo();
            fs::create_dir(repo.join(legacy_name)).expect("invalid legacy directory");

            let error = migrate_repository_config(&repo, false)
                .expect_err("file-shaped legacy source must reject directories");

            assert!(error.contains("must be a regular file"));
            assert!(repo.join(legacy_name).is_dir());
            assert!(!repo.join(legacy_name.replace(".lachesi", ".norn")).exists());
            let _ = fs::remove_dir_all(repo);
        }
    }

    #[test]
    fn repository_config_migration_keeps_local_override_ignored() {
        let repo = temp_repo();
        assert!(std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(&repo)
            .status()
            .expect("git init")
            .success());
        fs::write(repo.join(".gitignore"), "/.lachesi.local.yaml\n").expect("legacy ignore rule");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(repo.join(".gitignore"), fs::Permissions::from_mode(0o664))
                .expect("gitignore permissions");
        }
        fs::write(
            repo.join(".lachesi.local.yaml"),
            "review:\n  mode: strict\n",
        )
        .expect("legacy local override");

        let preview = migrate_repository_config(&repo, true).expect("migration preview");
        assert!(preview.actions.iter().any(|action| action.kind == "edit"));
        assert!(!fs::read_to_string(repo.join(".gitignore"))
            .expect("unchanged ignore file")
            .contains(".norn.local.yaml"));

        migrate_repository_config(&repo, false).expect("migration execution");
        assert!(!repo.join(".lachesi.local.yaml").exists());
        assert!(repo.join(".norn.local.yaml").exists());
        assert!(fs::read_to_string(repo.join(".gitignore"))
            .expect("updated ignore file")
            .contains("/.norn.local.yaml"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(repo.join(".gitignore"))
                    .expect("updated gitignore metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o664
            );
        }
        assert!(load_local_policy_layer(&repo)
            .expect("migrated local override remains valid")
            .is_some());
        let _ = fs::remove_dir_all(repo);
    }

    #[cfg(unix)]
    #[test]
    fn local_policy_layer_rejects_symbolic_links() {
        use std::os::unix::fs::symlink;

        let repo = temp_repo();
        assert!(std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(&repo)
            .status()
            .expect("git init")
            .success());
        fs::write(repo.join(".gitignore"), ".lachesi.local.yaml\n").expect("ignore fixture");
        fs::write(
            repo.join("tracked-policy.yaml"),
            "review:\n  mode: strict\n",
        )
        .expect("target fixture");
        symlink(
            repo.join("tracked-policy.yaml"),
            repo.join(".lachesi.local.yaml"),
        )
        .expect("local policy symlink fixture");

        assert!(load_local_policy_layer(&repo)
            .expect_err("symlinked local policy")
            .contains("cannot be a symbolic link"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn external_policy_layer_rejects_unknown_fields_inside_arrays() {
        let layer = serde_yaml::from_str(
            r#"
policy:
  rules:
    - id: signed-rule
      severity: high
      instruction: Enforce the rule.
      enforcemnt: analyzer
"#,
        )
        .expect("policy layer fixture");

        let error = validate_external_config_layer(&layer)
            .expect_err("unknown signed rule field must be rejected");
        assert!(error.contains("$.policy.rules[0].enforcemnt"));
    }

    #[test]
    fn external_policy_layer_accepts_free_form_analyzer_config() {
        let layer = serde_yaml::from_str(
            r#"
analyzers:
  custom:
    enabled: true
    command: custom-check
    config:
      threshold: 10
      nested:
        mode: strict
"#,
        )
        .expect("external layer fixture");

        validate_external_config_layer(&layer).expect("free-form analyzer config");
    }

    #[test]
    fn enabled_analyzer_requires_command() {
        let repo = temp_repo();
        let result = load_test_config(
            &repo,
            r#"
version: 0.1
analyzers:
  tsc:
    enabled: true
"#,
        );

        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0]
            .message
            .contains("Analyzer `tsc` is enabled but has no command"));
    }

    #[test]
    fn applies_default_review_profile_when_present() {
        let repo = temp_repo();
        let result = load_test_config(
            &repo,
            r#"
version: 0.1
profiles:
  default:
    mode: strict
    minSeverity: medium
    prompt:
      extend: Default profile prompt.
    analyzers:
      tsc: required
analyzers:
  tsc:
    enabled: false
    command: "pnpm typecheck"
"#,
        );

        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
        assert_eq!(result.selected_profile.as_deref(), Some("default"));
        let config = result.config.expect("config");
        let review = config.review.expect("review");
        assert_eq!(review.profile.as_deref(), Some("default"));
        assert_eq!(review.mode, Some(super::ReviewMode::Strict));
        assert_eq!(
            review.findings.and_then(|findings| findings.min_severity),
            Some(super::ReviewSeverity::Medium)
        );
        assert_eq!(
            review.prompt.and_then(|prompt| prompt.extend),
            Some("Default profile prompt.".to_string())
        );
        assert_eq!(
            config.analyzers.get("tsc").map(|analyzer| analyzer.enabled),
            Some(true)
        );
        assert_eq!(
            config
                .analyzers
                .get("tsc")
                .map(|analyzer| analyzer.required),
            Some(true)
        );
    }

    #[test]
    fn missing_required_profile_analyzer_is_a_config_error() {
        let repo = temp_repo();
        let result = load_test_config(
            &repo,
            r#"
version: 0.1
profiles:
  default:
    analyzers:
      missing-check: required
"#,
        );

        assert!(result.warnings.is_empty());
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0]
            .message
            .contains("requires analyzer `missing-check`"));
    }

    #[test]
    fn required_profile_analyzer_can_come_from_profile_policy_pack() {
        let repo = temp_repo();
        let pack_dir = repo.join("packs/profile-checks");
        fs::create_dir_all(&pack_dir).expect("create profile pack");
        fs::write(
            pack_dir.join("pack.yaml"),
            r#"
id: profile-checks
analyzers:
  pack-check:
    enabled: false
    command: "cargo check"
"#,
        )
        .expect("write profile pack");
        let result = load_test_config(
            &repo,
            r#"
version: 0.1
profiles:
  default:
    policyPacks:
      - ./packs/profile-checks
    analyzers:
      pack-check: required
"#,
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let config = result.config.expect("config");
        let analyzer = config.analyzers.get("pack-check").expect("pack analyzer");
        assert!(analyzer.enabled);
        assert!(analyzer.required);
    }

    #[test]
    fn applies_explicit_review_profile_override() {
        let repo = temp_repo();
        let result = load_test_config_with_profile(
            &repo,
            r#"
version: 0.1
review:
  profile: fast-profile
profiles:
  fast-profile:
    mode: fast
  strict-profile:
    mode: strict
    policyPacks:
      - ./packs/strict
"#,
            "strict-profile",
        );

        assert!(result.errors.is_empty());
        assert!(result.warnings.iter().any(|warning| warning
            .message
            .contains("Policy pack `./packs/strict` was not found")));
        assert_eq!(result.selected_profile.as_deref(), Some("strict-profile"));
        let config = result.config.expect("config");
        assert_eq!(
            config.review.and_then(|review| review.mode),
            Some(super::ReviewMode::Strict)
        );
    }

    #[test]
    fn missing_review_profile_warns_and_keeps_base_config() {
        let repo = temp_repo();
        let result = load_test_config(
            &repo,
            r#"
version: 0.1
review:
  profile: missing-profile
  mode: fast
"#,
        );

        assert!(result.errors.is_empty());
        assert_eq!(result.selected_profile, None);
        assert!(result.warnings[0]
            .message
            .contains("Review profile `missing-profile` was not found"));
        assert_eq!(
            result.config.unwrap().review.and_then(|review| review.mode),
            Some(super::ReviewMode::Fast)
        );
    }

    #[test]
    fn loads_policy_pack_from_local_directory() {
        let repo = temp_repo();
        let pack_dir = repo.join("lachesi-policies/agentic-code");
        fs::create_dir_all(&pack_dir).expect("create pack dir");
        fs::write(
            pack_dir.join("pack.yaml"),
            r#"
id: agentic-code
name: Agentic Code
review:
  prompt:
    extend: Pack prompt.
policy:
  rules:
    - id: agentic.large-refactor
      severity: high
      instruction: Large generated refactors must include verification evidence.
  pathRules:
    - id: agentic.generated-tests
      severity: medium
      paths:
        include:
          - "src/**"
      instruction: Generated code should preserve local test patterns.
analyzers:
  tsc:
    enabled: true
    command: "pnpm typecheck"
"#,
        )
        .expect("write pack");

        let result = load_test_config(
            &repo,
            r#"
version: 0.1
review:
  prompt:
    extend: Repo prompt.
policy:
  packs:
    - ./lachesi-policies/agentic-code
"#,
        );

        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
        assert_eq!(result.loaded_policy_packs.len(), 1);
        assert_eq!(result.loaded_policy_packs[0].id, "agentic-code");
        assert_eq!(
            result.loaded_policy_packs[0].name.as_deref(),
            Some("Agentic Code")
        );

        let config = result.config.expect("config");
        let prompt = config
            .review
            .as_ref()
            .and_then(|review| review.prompt.as_ref())
            .and_then(|prompt| prompt.extend.as_deref())
            .expect("prompt");
        assert_eq!(prompt, "Pack prompt.\n\nRepo prompt.");

        let policy = config.policy.expect("policy");
        assert_eq!(policy.rules.len(), 1);
        assert_eq!(policy.path_rules.len(), 1);
        assert_eq!(
            config
                .analyzers
                .get("tsc")
                .and_then(|analyzer| analyzer.command.as_deref()),
            Some("pnpm typecheck")
        );
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn loads_policy_pack_from_policy_source() {
        let repo = temp_repo();
        let pack_dir = repo.join(".lachesi/packs/react-saas");
        fs::create_dir_all(&pack_dir).expect("create pack dir");
        fs::write(
            pack_dir.join("pack.yaml"),
            r#"
id: react-saas
policy:
  rules:
    - id: react.empty-state
      severity: medium
      instruction: Async UI should keep loading, empty, and error states explicit.
"#,
        )
        .expect("write pack");

        let result = load_test_config(
            &repo,
            r#"
version: 0.1
policy:
  sources:
    - type: pack
      path: .lachesi/packs/react-saas
"#,
        );

        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
        assert_eq!(result.loaded_policy_packs[0].id, "react-saas");
        assert_eq!(result.config.unwrap().policy.unwrap().rules.len(), 1);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn loads_implicit_lachesi_folder_prompt_and_policy_packs() {
        let repo = temp_repo();
        let lachesi_dir = repo.join(".lachesi");
        let pack_dir = lachesi_dir.join("packs/team-rules");
        fs::create_dir_all(&pack_dir).expect("create pack dir");
        fs::write(
            lachesi_dir.join("system-prompt.md"),
            "Repository system prompt.",
        )
        .expect("write prompt");
        fs::write(
            pack_dir.join("pack.yaml"),
            r#"
id: team-rules
review:
  prompt:
    extend: Pack prompt.
policy:
  rules:
    - id: team.boundary
      severity: high
      instruction: Keep provider calls behind native services.
"#,
        )
        .expect("write pack");

        let result = load_from_repo_path(&repo).expect("load result");

        assert!(result.exists);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        assert_eq!(result.config_path, lachesi_dir.to_string_lossy());
        assert_eq!(result.loaded_policy_packs.len(), 1);
        assert_eq!(result.loaded_policy_packs[0].id, "team-rules");

        let config = result.config.expect("config");
        let prompt = config
            .review
            .as_ref()
            .and_then(|review| review.prompt.as_ref())
            .and_then(|prompt| prompt.replace.as_deref())
            .expect("prompt");
        assert_eq!(prompt, "Repository system prompt.");
        let policy_prompt = config
            .review
            .as_ref()
            .and_then(|review| review.prompt.as_ref())
            .and_then(|prompt| prompt.extend.as_deref())
            .expect("policy prompt");
        assert_eq!(policy_prompt, "Pack prompt.");
        assert_eq!(config.policy.expect("policy").rules.len(), 1);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn raw_implicit_policy_layer_is_finalized_once() {
        let repo = temp_repo();
        let pack_dir = repo.join(".lachesi/packs/team-rules");
        fs::create_dir_all(&pack_dir).expect("create pack dir");
        fs::write(
            pack_dir.join("pack.yaml"),
            r#"
id: team-rules
review:
  prompt:
    extend: Pack prompt.
policy:
  rules:
    - id: team.boundary
      severity: high
      instruction: Keep provider calls behind native services.
"#,
        )
        .expect("write pack");

        let (layer, warnings) =
            load_repository_policy_layer(&repo).expect("raw repository policy layer");
        assert!(warnings.is_empty());
        let config = serde_json::from_value::<RepoReviewConfig>(layer.expect("implicit layer"))
            .expect("implicit config");
        let finalized =
            finalize_resolved_config(&repo, &config, None).expect("finalized implicit config");

        assert!(finalized.errors.is_empty(), "{:?}", finalized.errors);
        assert_eq!(finalized.loaded_policy_packs.len(), 1);
        let config = finalized.config.expect("final config");
        assert_eq!(
            config
                .review
                .and_then(|review| review.prompt)
                .and_then(|prompt| prompt.extend),
            Some("Pack prompt.".to_string())
        );
        assert_eq!(config.policy.expect("policy").rules.len(), 1);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn loads_checked_in_agentic_code_policy_pack() {
        let repo = temp_repo();
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root");
        let pack_dir = repo_root.join("examples/policy-packs/agentic-code");
        assert!(pack_dir.join("pack.yaml").is_file());

        let result = load_test_config(
            &repo,
            &format!(
                r#"
version: 0.1
review:
  profile: agentic-balanced
policy:
  packs:
    - {}
"#,
                pack_dir.display()
            ),
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        assert_eq!(result.selected_profile.as_deref(), Some("agentic-balanced"));
        assert_eq!(result.loaded_policy_packs.len(), 1);
        assert_eq!(result.loaded_policy_packs[0].id, "agentic-code");

        let config = result.config.expect("config");
        let policy = config.policy.expect("policy");
        let declaration_count =
            policy.rules.len() + policy.path_rules.len() + policy.ast_rules.len();
        assert!((15..=25).contains(&declaration_count));
        assert!(config.profiles.contains_key("agentic-fast"));
        assert!(config.profiles.contains_key("agentic-balanced"));
        assert!(config.profiles.contains_key("agentic-strict"));
        assert!(config.analyzers.contains_key("typecheck"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn missing_policy_pack_warns_without_blocking() {
        let repo = temp_repo();
        let result = load_test_config(
            &repo,
            r#"
version: 0.1
policy:
  packs:
    - ./missing-pack
"#,
        );

        assert!(result.errors.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("was not found"));
        assert!(result.loaded_policy_packs.is_empty());
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn policy_pack_secret_fields_are_blocking_errors() {
        let repo = temp_repo();
        let pack_dir = repo.join("packs/unsafe");
        fs::create_dir_all(&pack_dir).expect("create pack dir");
        fs::write(
            pack_dir.join("pack.yaml"),
            r#"
id: unsafe
token: should-not-be-here
"#,
        )
        .expect("write pack");

        let result = load_test_config(
            &repo,
            r#"
version: 0.1
policy:
  packs:
    - packs/unsafe
"#,
        );

        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].message.contains("looks like a credential"));
        assert!(result.loaded_policy_packs.is_empty());
        let _ = fs::remove_dir_all(repo);
    }
}
