use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::Value;

const CONFIG_FILE: &str = ".lachesi.yaml";
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

    let config_path = repo_path.join(CONFIG_FILE);
    if !config_path.exists() {
        if let Some(result) = load_from_lachesi_dir(repo_path, profile_override)? {
            return Ok(result);
        }
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
    }

    let contents = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read {}: {e}", config_path.display()))?;
    Ok(load_from_str(
        repo_path,
        &config_path,
        &contents,
        profile_override,
    ))
}

fn load_from_lachesi_dir(
    repo_path: &Path,
    profile_override: Option<&str>,
) -> Result<Option<RepoReviewConfigLoadResult>, String> {
    let lachesi_dir = repo_path.join(".lachesi");
    if !lachesi_dir.is_dir() {
        return Ok(None);
    }

    let mut config = RepoReviewConfig {
        version: SUPPORTED_VERSION.to_string(),
        ..RepoReviewConfig::default()
    };

    if let Some(prompt) = load_lachesi_dir_prompt(&lachesi_dir)? {
        config.review = Some(ReviewConfig {
            prompt: Some(PromptConfig {
                replace: Some(prompt),
                ..PromptConfig::default()
            }),
            ..ReviewConfig::default()
        });
    }

    let packs = discover_lachesi_dir_policy_packs(repo_path)?;
    if !packs.is_empty() {
        config.policy = Some(PolicyConfig {
            packs,
            ..PolicyConfig::default()
        });
    }

    let contents = serde_yaml::to_string(&config)
        .map_err(|error| format!("Failed to synthesize .lachesi config: {error}"))?;
    Ok(Some(load_from_str(
        repo_path,
        &lachesi_dir,
        &contents,
        profile_override,
    )))
}

fn load_lachesi_dir_prompt(lachesi_dir: &Path) -> Result<Option<String>, String> {
    for file_name in [
        "system-prompt.md",
        "review-prompt.md",
        "review.md",
        "prompt.md",
    ] {
        let path = lachesi_dir.join(file_name);
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

fn discover_lachesi_dir_policy_packs(repo_path: &Path) -> Result<Vec<String>, String> {
    let packs_dir = repo_path.join(".lachesi/packs");
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
                "Unsupported .lachesi.yaml version {}. Supported version is {SUPPORTED_VERSION}.",
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

    for (id, requirement) in profile.analyzers {
        match requirement {
            ProfileAnalyzerRequirement::Required => {
                if let Some(analyzer) = config.analyzers.get_mut(&id) {
                    analyzer.enabled = true;
                    analyzer.required = true;
                } else {
                    warnings.push(message(
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

    Some(profile_id)
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

    config.profiles.extend(pack.profiles);

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
    let Value::Mapping(mapping) = value else {
        return;
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
    use super::{load_from_repo_path, load_from_str, RepoReviewConfigLoadResult};
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
        load_from_str(repo, &repo.join(".lachesi.yaml"), contents, None)
    }

    fn load_test_config_with_profile(
        repo: &std::path::Path,
        contents: &str,
        profile: &str,
    ) -> RepoReviewConfigLoadResult {
        load_from_str(repo, &repo.join(".lachesi.yaml"), contents, Some(profile))
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
            .contains("Unsupported .lachesi.yaml version"));
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
