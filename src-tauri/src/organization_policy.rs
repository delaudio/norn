//! Signed organization policy resolution for shared and local review runtimes.
//!
//! Resolution is opt-in. Callers that do not configure an organization source
//! continue to use the repository loader unchanged. When configured, layers
//! are applied in this fixed order:
//!
//! 1. built-in defaults
//! 2. signed organization defaults
//! 3. repository-owned policy
//! 4. local, non-committed overrides
//! 5. signed organization enforcement

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::administrative_audit::{
    AdministrativeAuditAction, AdministrativeAuditActor, AdministrativeAuditActorKind,
    AdministrativeAuditEvent, AdministrativeAuditOutcome, AdministrativeAuditRepositoryScope,
    AdministrativeAuditSchemaVersion, AdministrativeAuditTarget, AdministrativeAuditTargetKind,
};
use crate::repo_config::RepoReviewConfig;
use crate::review_event::PullRequestReviewEventProvider;
use crate::review_storage;

const MAX_BUNDLE_BYTES: usize = 1024 * 1024;
const MAX_CACHED_ENVELOPE_BYTES: usize = MAX_BUNDLE_BYTES + 16 * 1024;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const ED25519_PUBLIC_KEY_BYTES: usize = 32;
const ED25519_SIGNATURE_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationPolicySchemaVersion {
    #[serde(rename = "v1")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationPolicySignatureAlgorithm {
    Ed25519,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrganizationPolicyBundle {
    pub schema_version: OrganizationPolicySchemaVersion,
    pub tenant_id: String,
    pub source_id: String,
    /// Monotonically increasing version for one tenant and source.
    pub version: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    /// Lowest-precedence organization layer.
    pub defaults: Value,
    /// Highest-precedence organization layer.
    pub enforced: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrganizationPolicyIntegrity {
    pub algorithm: OrganizationPolicySignatureAlgorithm,
    pub key_id: String,
    /// Standard base64-encoded Ed25519 signature over the canonical bundle JSON.
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedOrganizationPolicyBundle {
    pub bundle: OrganizationPolicyBundle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity: Option<OrganizationPolicyIntegrity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationPolicySourceRequest {
    pub tenant_id: String,
    pub source_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizationPolicySourceErrorKind {
    Unavailable,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationPolicySourceError {
    pub kind: OrganizationPolicySourceErrorKind,
    pub message: String,
}

impl OrganizationPolicySourceError {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: OrganizationPolicySourceErrorKind::Unavailable,
            message: message.into(),
        }
    }

    pub fn rejected(message: impl Into<String>) -> Self {
        Self {
            kind: OrganizationPolicySourceErrorKind::Rejected,
            message: message.into(),
        }
    }
}

pub trait OrganizationPolicySource {
    fn fetch(
        &self,
        request: &OrganizationPolicySourceRequest,
    ) -> Result<SignedOrganizationPolicyBundle, OrganizationPolicySourceError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationPolicyRequirement {
    Mandatory,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum OrganizationPolicyUnavailableBehavior {
    FailClosed,
    UseVerifiedCache { max_staleness_ms: u64 },
    ContinueWithoutOrganizationPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationPolicySourceConfig {
    pub tenant_id: String,
    pub source_id: String,
    pub requirement: OrganizationPolicyRequirement,
    pub unavailable_behavior: OrganizationPolicyUnavailableBehavior,
    /// Trusted Ed25519 public keys indexed by key id, encoded as standard base64.
    pub trusted_keys: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfiguredOrganizationPolicyFile {
    tenant_id: String,
    source_id: String,
    requirement: OrganizationPolicyRequirement,
    unavailable_behavior: OrganizationPolicyUnavailableBehavior,
    trusted_keys: BTreeMap<String, String>,
    bundle_path: PathBuf,
}

struct FileOrganizationPolicySource {
    path: PathBuf,
}

impl OrganizationPolicySource for FileOrganizationPolicySource {
    fn fetch(
        &self,
        _request: &OrganizationPolicySourceRequest,
    ) -> Result<SignedOrganizationPolicyBundle, OrganizationPolicySourceError> {
        let file = fs::File::open(&self.path).map_err(|error| {
            OrganizationPolicySourceError::unavailable(format!(
                "Could not read signed bundle {}: {error}",
                self.path.display()
            ))
        })?;
        let mut contents = Vec::with_capacity(MAX_CACHED_ENVELOPE_BYTES.min(64 * 1024));
        file.take((MAX_CACHED_ENVELOPE_BYTES + 1) as u64)
            .read_to_end(&mut contents)
            .map_err(|error| {
                OrganizationPolicySourceError::unavailable(format!(
                    "Could not read signed bundle {}: {error}",
                    self.path.display()
                ))
            })?;
        if contents.len() > MAX_CACHED_ENVELOPE_BYTES {
            return Err(OrganizationPolicySourceError::rejected(format!(
                "Signed organization policy bundle exceeds the {MAX_CACHED_ENVELOPE_BYTES}-byte file limit."
            )));
        }
        serde_json::from_slice(&contents).map_err(|error| {
            OrganizationPolicySourceError::rejected(format!(
                "Signed organization policy bundle is invalid JSON: {error}"
            ))
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedOrganizationPolicyBundle {
    pub envelope: SignedOrganizationPolicyBundle,
    pub digest: String,
    pub verified_at_ms: u64,
}

pub trait OrganizationPolicyCache {
    fn load(
        &self,
        tenant_id: &str,
        source_id: &str,
    ) -> Result<Option<CachedOrganizationPolicyBundle>, String>;

    fn store(&self, bundle: &CachedOrganizationPolicyBundle) -> Result<(), String>;

    fn remove(&self, tenant_id: &str, source_id: &str) -> Result<(), String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SqliteOrganizationPolicyCache;

impl OrganizationPolicyCache for SqliteOrganizationPolicyCache {
    fn load(
        &self,
        tenant_id: &str,
        source_id: &str,
    ) -> Result<Option<CachedOrganizationPolicyBundle>, String> {
        review_storage::load_organization_policy_bundle(tenant_id, source_id)?.map_or(
            Ok(None),
            |stored| {
                if stored.envelope_json.len() > MAX_CACHED_ENVELOPE_BYTES {
                    return Err("Cached organization policy bundle is too large.".to_string());
                }
                let envelope = serde_json::from_str(&stored.envelope_json)
                    .map_err(|_| "Cached organization policy bundle is invalid.".to_string())?;
                Ok(Some(CachedOrganizationPolicyBundle {
                    envelope,
                    digest: stored.digest,
                    verified_at_ms: stored.verified_at_ms,
                }))
            },
        )
    }

    fn store(&self, bundle: &CachedOrganizationPolicyBundle) -> Result<(), String> {
        let envelope_json = serde_json::to_string(&bundle.envelope)
            .map_err(|error| format!("Could not serialize organization policy cache: {error}"))?;
        review_storage::store_organization_policy_bundle(
            &bundle.envelope.bundle.tenant_id,
            &bundle.envelope.bundle.source_id,
            bundle.envelope.bundle.version,
            &bundle.digest,
            bundle.verified_at_ms,
            bundle.envelope.bundle.expires_at_ms,
            &envelope_json,
        )
    }

    fn remove(&self, tenant_id: &str, source_id: &str) -> Result<(), String> {
        review_storage::delete_organization_policy_bundle(tenant_id, source_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedPolicySourceVersion {
    pub tenant_id: String,
    pub source_id: String,
    pub version: u64,
    pub digest: String,
    pub key_id: String,
    pub from_cache: bool,
}

pub trait OrganizationPolicyAuditSink {
    fn record(
        &self,
        source: &ResolvedPolicySourceVersion,
        resolved_at_ms: u64,
    ) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdministrativePolicyAuditContext {
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repository: String,
    pub pull_request_id: Option<u64>,
    pub actor_kind: AdministrativeAuditActorKind,
    pub actor_id: String,
    pub correlation_id: String,
}

pub struct SqliteOrganizationPolicyAuditSink {
    pub context: AdministrativePolicyAuditContext,
}

impl OrganizationPolicyAuditSink for SqliteOrganizationPolicyAuditSink {
    fn record(
        &self,
        source: &ResolvedPolicySourceVersion,
        resolved_at_ms: u64,
    ) -> Result<(), String> {
        let event = policy_resolution_audit_event(&self.context, source, resolved_at_ms);
        review_storage::append_administrative_audit_event(&event).map(|_| ())
    }
}

fn policy_resolution_audit_event(
    context: &AdministrativePolicyAuditContext,
    source: &ResolvedPolicySourceVersion,
    resolved_at_ms: u64,
) -> AdministrativeAuditEvent {
    let delivery_hash = Sha256::digest(
        format!(
            "{}:{}:{}:{}:{}",
            source.tenant_id,
            source.source_id,
            source.version,
            context.correlation_id,
            resolved_at_ms
        )
        .as_bytes(),
    );
    AdministrativeAuditEvent {
        schema_version: AdministrativeAuditSchemaVersion::V1,
        delivery_id: format!("policy-resolution:{}", hex::encode(&delivery_hash[..16])),
        tenant_id: source.tenant_id.clone(),
        occurred_at: resolved_at_ms.to_string(),
        actor: AdministrativeAuditActor {
            kind: context.actor_kind,
            id: context.actor_id.clone(),
        },
        repository: AdministrativeAuditRepositoryScope {
            provider: context.provider,
            workspace: context.workspace.clone(),
            repo: context.repository.clone(),
            pr_id: context.pull_request_id,
        },
        action: AdministrativeAuditAction::PolicyResolved,
        target: AdministrativeAuditTarget {
            kind: AdministrativeAuditTargetKind::Policy,
            id: format!("policy:{}:v{}", source.source_id, source.version),
        },
        outcome: AdministrativeAuditOutcome::Succeeded,
        correlation_id: context.correlation_id.clone(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrganizationPolicyResolutionInput {
    pub built_in: RepoReviewConfig,
    /// Parsed JSON object representing repository-owned `.lachesi` policy.
    pub repository: Option<Value>,
    /// Parsed JSON object representing non-committed local overrides.
    pub local_overrides: Option<Value>,
    pub now_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedOrganizationPolicy {
    pub config: RepoReviewConfig,
    pub sources: Vec<ResolvedPolicySourceVersion>,
    pub required_analyzers: Vec<String>,
    pub selected_profile: Option<String>,
    pub loaded_policy_packs: Vec<crate::repo_config::LoadedPolicyPack>,
    pub warnings: Vec<crate::repo_config::RepoConfigValidationMessage>,
    enforced_layer: Option<Value>,
    pending_cache_bundle: Option<CachedOrganizationPolicyBundle>,
}

pub fn organization_policy_is_configured() -> bool {
    std::env::var_os("LACHESI_ORGANIZATION_POLICY_CONFIG").is_some()
}

pub fn resolve_configured_organization_policy(
    repo_path: &Path,
    profile_override: Option<&str>,
    audit: Option<&dyn OrganizationPolicyAuditSink>,
    now_ms: u64,
) -> Result<Option<ResolvedOrganizationPolicy>, OrganizationPolicyResolutionError> {
    let Some(config_path) = std::env::var_os("LACHESI_ORGANIZATION_POLICY_CONFIG") else {
        return Ok(None);
    };
    let config_path = validate_organization_policy_path(
        repo_path,
        PathBuf::from(config_path).as_path(),
        "LACHESI_ORGANIZATION_POLICY_CONFIG",
    )?;
    let contents = fs::read_to_string(&config_path).map_err(|error| {
        OrganizationPolicyResolutionError::InvalidConfiguration(format!(
            "Could not read organization policy config {}: {error}",
            config_path.display()
        ))
    })?;
    let configured: ConfiguredOrganizationPolicyFile =
        serde_json::from_str(&contents).map_err(|error| {
            OrganizationPolicyResolutionError::InvalidConfiguration(format!(
                "Organization policy config {} is invalid: {error}",
                config_path.display()
            ))
        })?;
    let bundle_path =
        resolve_organization_policy_bundle_path(repo_path, &config_path, &configured.bundle_path)?;
    let source_config = OrganizationPolicySourceConfig {
        tenant_id: configured.tenant_id,
        source_id: configured.source_id,
        requirement: configured.requirement,
        unavailable_behavior: configured.unavailable_behavior,
        trusted_keys: configured.trusted_keys,
    };
    let (repository, repository_warnings) =
        crate::repo_config::load_repository_policy_layer(repo_path)
            .map_err(OrganizationPolicyResolutionError::InvalidLayer)?;
    let local_overrides = crate::repo_config::load_local_policy_layer(repo_path)
        .map_err(OrganizationPolicyResolutionError::InvalidLayer)?;
    let mut resolved = resolve_organization_policy(
        &source_config,
        &FileOrganizationPolicySource { path: bundle_path },
        &SqliteOrganizationPolicyCache,
        None,
        OrganizationPolicyResolutionInput {
            built_in: RepoReviewConfig {
                version: "0.1".to_string(),
                ..RepoReviewConfig::default()
            },
            repository,
            local_overrides,
            now_ms,
        },
    )?;
    resolved.warnings = repository_warnings;
    finalize_resolved_organization_policy(repo_path, &mut resolved, profile_override)?;
    store_pending_cache_bundle(&mut resolved, &SqliteOrganizationPolicyCache)?;
    if let Some(audit) = audit {
        for source in &resolved.sources {
            audit
                .record(source, now_ms)
                .map_err(OrganizationPolicyResolutionError::Audit)?;
        }
    }
    Ok(Some(resolved))
}

fn resolve_organization_policy_bundle_path(
    repo_path: &Path,
    config_path: &Path,
    configured_bundle_path: &Path,
) -> Result<PathBuf, OrganizationPolicyResolutionError> {
    let bundle_path = if configured_bundle_path.is_absolute() {
        configured_bundle_path.to_path_buf()
    } else {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(configured_bundle_path)
    };
    validate_organization_policy_bundle_path(repo_path, &bundle_path)
}

fn validate_organization_policy_bundle_path(
    repo_path: &Path,
    bundle_path: &Path,
) -> Result<PathBuf, OrganizationPolicyResolutionError> {
    if !bundle_path.is_absolute() {
        return Err(OrganizationPolicyResolutionError::InvalidConfiguration(
            "Organization policy bundle must resolve to an absolute path outside the reviewed repository."
                .to_string(),
        ));
    }
    let bundle_path = normalize_absolute_path(bundle_path);
    let repository_input = if repo_path.is_absolute() {
        normalize_absolute_path(repo_path)
    } else {
        normalize_absolute_path(
            &std::env::current_dir()
                .map_err(|error| {
                    OrganizationPolicyResolutionError::InvalidConfiguration(format!(
                        "Could not resolve the current directory: {error}"
                    ))
                })?
                .join(repo_path),
        )
    };
    let repository = repo_path.canonicalize().map_err(|error| {
        OrganizationPolicyResolutionError::InvalidConfiguration(format!(
            "Could not resolve reviewed repository {}: {error}",
            repo_path.display()
        ))
    })?;
    let existing_ancestor = bundle_path
        .ancestors()
        .find(|ancestor| ancestor.exists())
        .ok_or_else(|| {
            OrganizationPolicyResolutionError::InvalidConfiguration(format!(
                "Could not resolve an existing ancestor of organization policy bundle {}.",
                bundle_path.display()
            ))
        })?;
    let canonical_ancestor = existing_ancestor.canonicalize().map_err(|error| {
        OrganizationPolicyResolutionError::InvalidConfiguration(format!(
            "Could not resolve organization policy bundle ancestor {}: {error}",
            existing_ancestor.display()
        ))
    })?;
    if bundle_path.starts_with(&repository_input)
        || bundle_path.starts_with(&repository)
        || canonical_ancestor.starts_with(&repository)
    {
        return Err(OrganizationPolicyResolutionError::InvalidConfiguration(
            "Organization policy bundle must be stored outside the reviewed repository."
                .to_string(),
        ));
    }
    if bundle_path.exists() {
        bundle_path.canonicalize().map_err(|error| {
            OrganizationPolicyResolutionError::InvalidConfiguration(format!(
                "Could not resolve organization policy bundle {}: {error}",
                bundle_path.display()
            ))
        })
    } else {
        Ok(bundle_path)
    }
}

fn normalize_absolute_path(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn finalize_resolved_organization_policy(
    repo_path: &Path,
    resolved: &mut ResolvedOrganizationPolicy,
    profile_override: Option<&str>,
) -> Result<(), OrganizationPolicyResolutionError> {
    let enforced_profile_override = enforced_profile_override(resolved.enforced_layer.as_ref());
    if let Some(Some(profile_id)) = enforced_profile_override {
        let profile_is_signed = resolved
            .enforced_layer
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|layer| layer.get("profiles"))
            .and_then(Value::as_object)
            .is_some_and(|profiles| profiles.contains_key(profile_id));
        if !profile_is_signed {
            return Err(OrganizationPolicyResolutionError::InvalidLayer(format!(
                "Enforced organization review profile `{profile_id}` must be defined in the signed enforced layer."
            )));
        }
    }
    replace_enforced_owned_definitions(&mut resolved.config, resolved.enforced_layer.as_ref())?;
    let enforced_profile = enforced_profile_override.unwrap_or(profile_override);
    let finalized =
        crate::repo_config::finalize_resolved_config(repo_path, &resolved.config, enforced_profile)
            .map_err(OrganizationPolicyResolutionError::InvalidLayer)?;
    if !finalized.errors.is_empty() {
        return Err(OrganizationPolicyResolutionError::InvalidLayer(
            finalized
                .errors
                .into_iter()
                .map(|error| format!("{}: {}", error.path, error.message))
                .collect::<Vec<_>>()
                .join("\n"),
        ));
    }
    if let Some(Some(expected_profile)) = enforced_profile_override {
        if finalized.selected_profile.as_deref() != Some(expected_profile) {
            return Err(OrganizationPolicyResolutionError::InvalidLayer(format!(
                "Enforced organization review profile `{expected_profile}` was not found."
            )));
        }
    }
    let finalized_config = finalized.config.ok_or_else(|| {
        OrganizationPolicyResolutionError::InvalidLayer(
            "Resolved organization policy produced no review configuration.".to_string(),
        )
    })?;
    resolved.selected_profile = finalized.selected_profile;
    resolved.loaded_policy_packs = finalized.loaded_policy_packs;
    resolved.warnings.extend(finalized.warnings);
    resolved.config = reapply_enforced_layer(finalized_config, resolved.enforced_layer.as_ref())?;
    resolved.required_analyzers =
        enforced_required_analyzer_ids(&resolved.config, resolved.enforced_layer.as_ref());
    Ok(())
}

fn store_pending_cache_bundle(
    resolved: &mut ResolvedOrganizationPolicy,
    cache: &dyn OrganizationPolicyCache,
) -> Result<(), OrganizationPolicyResolutionError> {
    if let Some(bundle) = resolved.pending_cache_bundle.take() {
        cache
            .store(&bundle)
            .map_err(OrganizationPolicyResolutionError::Cache)?;
    }
    Ok(())
}

fn enforced_required_analyzer_ids(
    config: &RepoReviewConfig,
    enforced: Option<&Value>,
) -> Vec<String> {
    let Some(enforced_analyzers) = enforced
        .and_then(Value::as_object)
        .and_then(|layer| layer.get("analyzers"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    enforced_analyzers
        .keys()
        .filter(|id| {
            config.analyzers.get(*id).is_some_and(|analyzer| {
                analyzer.enabled
                    && analyzer.required
                    && analyzer
                        .command
                        .as_deref()
                        .is_some_and(|command| !command.trim().is_empty())
            })
        })
        .cloned()
        .collect()
}

fn replace_enforced_owned_definitions(
    config: &mut RepoReviewConfig,
    enforced: Option<&Value>,
) -> Result<(), OrganizationPolicyResolutionError> {
    let Some(enforced) = enforced.and_then(Value::as_object) else {
        return Ok(());
    };
    if let Some(profiles) = enforced.get("profiles").and_then(Value::as_object) {
        for (id, value) in profiles {
            let profile =
                serde_json::from_value::<crate::repo_config::ReviewProfileConfig>(value.clone())
                    .map_err(|error| {
                        OrganizationPolicyResolutionError::InvalidLayer(format!(
                            "Invalid signed enforced profile `{id}`: {error}"
                        ))
                    })?;
            config.profiles.insert(id.clone(), profile);
        }
    }
    if let Some(analyzers) = enforced.get("analyzers").and_then(Value::as_object) {
        for (id, value) in analyzers {
            let analyzer =
                serde_json::from_value::<crate::repo_config::AnalyzerConfig>(value.clone())
                    .map_err(|error| {
                        OrganizationPolicyResolutionError::InvalidLayer(format!(
                            "Invalid signed enforced analyzer `{id}`: {error}"
                        ))
                    })?;
            config.analyzers.insert(id.clone(), analyzer);
        }
    }
    Ok(())
}

fn enforced_profile_override(enforced: Option<&Value>) -> Option<Option<&str>> {
    enforced?
        .as_object()?
        .get("review")?
        .as_object()?
        .get("profile")
        .map(Value::as_str)
}

fn validate_organization_policy_path(
    repo_path: &Path,
    policy_path: &Path,
    label: &str,
) -> Result<PathBuf, OrganizationPolicyResolutionError> {
    if !policy_path.is_absolute() {
        return Err(OrganizationPolicyResolutionError::InvalidConfiguration(
            format!("{label} must be an absolute path outside the reviewed repository."),
        ));
    }
    let repository_input = if repo_path.is_absolute() {
        repo_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                OrganizationPolicyResolutionError::InvalidConfiguration(format!(
                    "Could not resolve the current directory: {error}"
                ))
            })?
            .join(repo_path)
    };
    let repository = repo_path.canonicalize().map_err(|error| {
        OrganizationPolicyResolutionError::InvalidConfiguration(format!(
            "Could not resolve reviewed repository {}: {error}",
            repo_path.display()
        ))
    })?;
    let policy = policy_path.canonicalize().map_err(|error| {
        OrganizationPolicyResolutionError::InvalidConfiguration(format!(
            "Could not resolve {label} {}: {error}",
            policy_path.display()
        ))
    })?;
    if policy_path.starts_with(&repository_input)
        || policy_path.starts_with(&repository)
        || policy.starts_with(&repository)
    {
        return Err(OrganizationPolicyResolutionError::InvalidConfiguration(
            format!("{label} must be stored outside the reviewed repository."),
        ));
    }
    Ok(policy)
}

fn reapply_enforced_layer(
    config: RepoReviewConfig,
    enforced: Option<&Value>,
) -> Result<RepoReviewConfig, OrganizationPolicyResolutionError> {
    let Some(enforced) = enforced else {
        return Ok(config);
    };
    let mut value = serde_json::to_value(config).map_err(|error| {
        OrganizationPolicyResolutionError::InvalidLayer(format!(
            "Could not serialize finalized repository policy: {error}"
        ))
    })?;
    merge_json(&mut value, enforced);
    let config = serde_json::from_value(value).map_err(|error| {
        OrganizationPolicyResolutionError::InvalidLayer(format!(
            "Enforced organization policy is invalid after profile resolution: {error}"
        ))
    })?;
    crate::repo_config::validate_resolved_config(&config)
        .map_err(OrganizationPolicyResolutionError::InvalidLayer)?;
    Ok(config)
}

pub fn review_policy_prompt_appendix(config: &RepoReviewConfig) -> Option<String> {
    if config.review.is_none() && config.paths.is_none() && config.policy.is_none() {
        return None;
    }
    let review = config.review.as_ref().map(|review| {
        serde_json::json!({
            "profile": &review.profile,
            "mode": &review.mode,
            "findings": &review.findings,
        })
    });
    serde_yaml::to_string(&serde_json::json!({
        "review": review,
        "paths": &config.paths,
        "policy": &config.policy,
    }))
    .ok()
    .map(|policy| format!("## Resolved review policy\n\n```yaml\n{policy}```"))
}

pub fn execution_policy_prompt_appendix(
    config: &RepoReviewConfig,
    _existing_payload: &str,
) -> Option<String> {
    let prompt = config.review.as_ref()?.prompt.as_ref()?;
    let replacement = prompt
        .replace
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let extension = prompt
        .extend
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if replacement.is_none() && extension.is_none() {
        return None;
    }

    let mut sections = vec!["## Authoritative resolved review instructions".to_string()];
    if let Some(replacement) = replacement {
        sections.push(
            "The following organization policy replaces any earlier review instructions:"
                .to_string(),
        );
        sections.push(replacement.to_string());
    }
    if let Some(extension) = extension {
        sections.push("The following organization policy instructions also apply:".to_string());
        sections.push(extension.to_string());
    }
    Some(sections.join("\n\n"))
}

pub fn resolve_organization_policy(
    source_config: &OrganizationPolicySourceConfig,
    source: &dyn OrganizationPolicySource,
    cache: &dyn OrganizationPolicyCache,
    audit: Option<&dyn OrganizationPolicyAuditSink>,
    input: OrganizationPolicyResolutionInput,
) -> Result<ResolvedOrganizationPolicy, OrganizationPolicyResolutionError> {
    validate_source_config(source_config)?;
    validate_layer("repository", input.repository.as_ref())?;
    validate_layer("local overrides", input.local_overrides.as_ref())?;

    let request = OrganizationPolicySourceRequest {
        tenant_id: source_config.tenant_id.clone(),
        source_id: source_config.source_id.clone(),
    };
    let (resolved_bundle, from_cache) = match source.fetch(&request) {
        Ok(envelope) => {
            let verified = verify_bundle(source_config, envelope, input.now_ms, false)?;
            let cached = match cache.load(&request.tenant_id, &request.source_id) {
                Ok(Some(cached)) => match verify_cached_for_rollback(source_config, cached) {
                    Ok(cached) => Some(cached),
                    Err(_) => {
                        cache
                            .remove(&request.tenant_id, &request.source_id)
                            .map_err(OrganizationPolicyResolutionError::Cache)?;
                        None
                    }
                },
                Ok(None) => None,
                Err(_) => {
                    cache
                        .remove(&request.tenant_id, &request.source_id)
                        .map_err(OrganizationPolicyResolutionError::Cache)?;
                    None
                }
            };
            reject_version_rollback(cached.as_ref(), &verified)?;
            (Some(verified), false)
        }
        Err(error) if error.kind == OrganizationPolicySourceErrorKind::Unavailable => {
            let cached = match source_config.unavailable_behavior {
                OrganizationPolicyUnavailableBehavior::UseVerifiedCache { .. } => cache
                    .load(&request.tenant_id, &request.source_id)
                    .map_err(OrganizationPolicyResolutionError::Cache)?,
                _ => None,
            };
            let fallback =
                resolve_unavailable(source_config, cached, input.now_ms, &error.message)?;
            let from_cache = fallback.is_some();
            (fallback, from_cache)
        }
        Err(error) => {
            return Err(OrganizationPolicyResolutionError::SourceRejected(
                error.message,
            ));
        }
    };

    let mut config = serde_json::to_value(&input.built_in).map_err(|error| {
        OrganizationPolicyResolutionError::InvalidLayer(format!(
            "Could not serialize built-in policy: {error}"
        ))
    })?;
    let mut sources = Vec::new();
    let mut enforced_layer = None;
    if let Some(verified) = resolved_bundle.as_ref() {
        merge_json(&mut config, &verified.envelope.bundle.defaults);
        if let Some(repository) = input.repository.as_ref() {
            merge_json(&mut config, repository);
        }
        if let Some(local) = input.local_overrides.as_ref() {
            merge_json(&mut config, local);
        }
        merge_json(&mut config, &verified.envelope.bundle.enforced);
        enforced_layer = Some(verified.envelope.bundle.enforced.clone());

        let integrity = verified
            .envelope
            .integrity
            .as_ref()
            .expect("verified bundles have integrity");
        let metadata = ResolvedPolicySourceVersion {
            tenant_id: verified.envelope.bundle.tenant_id.clone(),
            source_id: verified.envelope.bundle.source_id.clone(),
            version: verified.envelope.bundle.version,
            digest: verified.digest.clone(),
            key_id: integrity.key_id.clone(),
            from_cache,
        };
        sources.push(metadata);
    } else {
        if let Some(repository) = input.repository.as_ref() {
            merge_json(&mut config, repository);
        }
        if let Some(local) = input.local_overrides.as_ref() {
            merge_json(&mut config, local);
        }
    }

    let config = serde_json::from_value::<RepoReviewConfig>(config).map_err(|error| {
        OrganizationPolicyResolutionError::InvalidLayer(format!(
            "Resolved policy does not match the repository config schema: {error}"
        ))
    })?;
    crate::repo_config::validate_resolved_config(&config)
        .map_err(OrganizationPolicyResolutionError::InvalidLayer)?;
    if let Some(audit) = audit {
        for source in &sources {
            audit
                .record(source, input.now_ms)
                .map_err(OrganizationPolicyResolutionError::Audit)?;
        }
    }

    Ok(ResolvedOrganizationPolicy {
        config,
        sources,
        required_analyzers: Vec::new(),
        selected_profile: None,
        loaded_policy_packs: Vec::new(),
        warnings: Vec::new(),
        enforced_layer,
        pending_cache_bundle: resolved_bundle.filter(|_| !from_cache),
    })
}

fn resolve_unavailable(
    source_config: &OrganizationPolicySourceConfig,
    cached: Option<CachedOrganizationPolicyBundle>,
    now_ms: u64,
    source_message: &str,
) -> Result<Option<CachedOrganizationPolicyBundle>, OrganizationPolicyResolutionError> {
    match source_config.unavailable_behavior {
        OrganizationPolicyUnavailableBehavior::FailClosed => Err(
            OrganizationPolicyResolutionError::Unavailable(format!(
                "Organization policy source `{}` is unavailable: {source_message}. Configure bounded verified-cache fallback explicitly to permit offline review.",
                source_config.source_id
            )),
        ),
        OrganizationPolicyUnavailableBehavior::ContinueWithoutOrganizationPolicy => {
            if source_config.requirement == OrganizationPolicyRequirement::Mandatory {
                return Err(OrganizationPolicyResolutionError::InvalidConfiguration(
                    "Mandatory organization policy cannot continue without a verified bundle."
                        .to_string(),
                ));
            }
            Ok(None)
        }
        OrganizationPolicyUnavailableBehavior::UseVerifiedCache { max_staleness_ms } => {
            let cached = cached.ok_or_else(|| {
                OrganizationPolicyResolutionError::Unavailable(format!(
                    "Organization policy source `{}` is unavailable and no verified cached bundle exists.",
                    source_config.source_id
                ))
            })?;
            let age = now_ms.checked_sub(cached.verified_at_ms).ok_or_else(|| {
                OrganizationPolicyResolutionError::StaleCache(
                    "Cached organization policy has a verification time in the future.".to_string(),
                )
            })?;
            if age > max_staleness_ms {
                return Err(OrganizationPolicyResolutionError::StaleCache(format!(
                    "Cached organization policy `{}` is {} ms old, exceeding the configured {} ms offline limit.",
                    source_config.source_id, age, max_staleness_ms
                )));
            }
            let reverified = verify_bundle(source_config, cached.envelope.clone(), now_ms, true)?;
            if reverified.digest != cached.digest {
                return Err(OrganizationPolicyResolutionError::Cache(
                    "Cached organization policy digest does not match its signed content."
                        .to_string(),
                ));
            }
            Ok(Some(cached))
        }
    }
}

fn validate_source_config(
    config: &OrganizationPolicySourceConfig,
) -> Result<(), OrganizationPolicyResolutionError> {
    for (field, value) in [
        ("tenantId", config.tenant_id.as_str()),
        ("sourceId", config.source_id.as_str()),
    ] {
        if value.trim().is_empty()
            || value.len() > 256
            || !value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._:@-".contains(character))
        {
            return Err(OrganizationPolicyResolutionError::InvalidConfiguration(
                format!("`{field}` must be a non-empty safe identifier."),
            ));
        }
    }
    if config.trusted_keys.is_empty() {
        return Err(OrganizationPolicyResolutionError::InvalidConfiguration(
            "At least one trusted organization policy signing key is required.".to_string(),
        ));
    }
    if config.requirement == OrganizationPolicyRequirement::Mandatory
        && config.unavailable_behavior
            == OrganizationPolicyUnavailableBehavior::ContinueWithoutOrganizationPolicy
    {
        return Err(OrganizationPolicyResolutionError::InvalidConfiguration(
            "Mandatory organization policy cannot continue without a verified bundle.".to_string(),
        ));
    }
    Ok(())
}

fn validate_layer(
    name: &str,
    layer: Option<&Value>,
) -> Result<(), OrganizationPolicyResolutionError> {
    let Some(layer) = layer else {
        return Ok(());
    };
    if !layer.is_object() {
        return Err(OrganizationPolicyResolutionError::InvalidLayer(format!(
            "{name} policy must be a JSON object."
        )));
    }
    Ok(())
}

fn verify_bundle(
    config: &OrganizationPolicySourceConfig,
    envelope: SignedOrganizationPolicyBundle,
    now_ms: u64,
    from_cache: bool,
) -> Result<CachedOrganizationPolicyBundle, OrganizationPolicyResolutionError> {
    if envelope.bundle.tenant_id != config.tenant_id
        || envelope.bundle.source_id != config.source_id
    {
        return Err(OrganizationPolicyResolutionError::IdentityMismatch);
    }
    if envelope.bundle.version == 0 || envelope.bundle.version > MAX_SAFE_JSON_INTEGER {
        return Err(OrganizationPolicyResolutionError::InvalidBundle(format!(
            "Organization policy version must be between 1 and {MAX_SAFE_JSON_INTEGER}."
        )));
    }
    if envelope.bundle.issued_at_ms > now_ms {
        return Err(OrganizationPolicyResolutionError::InvalidBundle(
            "Organization policy is not valid yet.".to_string(),
        ));
    }
    if envelope.bundle.expires_at_ms <= now_ms
        || envelope.bundle.expires_at_ms <= envelope.bundle.issued_at_ms
    {
        return Err(OrganizationPolicyResolutionError::Expired);
    }
    let canonical = canonical_bundle_bytes(&envelope.bundle)?;

    let integrity = envelope
        .integrity
        .as_ref()
        .ok_or(OrganizationPolicyResolutionError::Unsigned)?;
    let public_key = config
        .trusted_keys
        .get(&integrity.key_id)
        .ok_or_else(|| OrganizationPolicyResolutionError::UntrustedKey(integrity.key_id.clone()))?;
    let public_key = STANDARD.decode(public_key).map_err(|_| {
        OrganizationPolicyResolutionError::InvalidConfiguration(format!(
            "Trusted key `{}` is not valid base64.",
            integrity.key_id
        ))
    })?;
    let public_key: [u8; ED25519_PUBLIC_KEY_BYTES] = public_key.try_into().map_err(|_| {
        OrganizationPolicyResolutionError::InvalidConfiguration(format!(
            "Trusted key `{}` must decode to {ED25519_PUBLIC_KEY_BYTES} bytes.",
            integrity.key_id
        ))
    })?;
    let signature = STANDARD
        .decode(&integrity.signature)
        .map_err(|_| OrganizationPolicyResolutionError::InvalidSignature)?;
    let signature: [u8; ED25519_SIGNATURE_BYTES] = signature
        .try_into()
        .map_err(|_| OrganizationPolicyResolutionError::InvalidSignature)?;
    VerifyingKey::from_bytes(&public_key)
        .map_err(|_| {
            OrganizationPolicyResolutionError::InvalidConfiguration(format!(
                "Trusted key `{}` is not a valid Ed25519 public key.",
                integrity.key_id
            ))
        })?
        .verify(&canonical, &Signature::from_bytes(&signature))
        .map_err(|_| OrganizationPolicyResolutionError::InvalidSignature)?;
    validate_layer("organization defaults", Some(&envelope.bundle.defaults))?;
    validate_layer("organization enforcement", Some(&envelope.bundle.enforced))?;
    reject_organization_version_override(&envelope.bundle.defaults)?;
    reject_organization_version_override(&envelope.bundle.enforced)?;
    reject_repository_policy_indirection(&envelope.bundle.defaults)?;
    reject_repository_policy_indirection(&envelope.bundle.enforced)?;
    reject_signed_layer_secrets(&envelope.bundle.defaults, "$.defaults")?;
    reject_signed_layer_secrets(&envelope.bundle.enforced, "$.enforced")?;
    validate_organization_layer_schema(&envelope.bundle.defaults, "defaults")?;
    validate_organization_layer_schema(&envelope.bundle.enforced, "enforced")?;
    let digest = hex::encode(Sha256::digest(&canonical));

    Ok(CachedOrganizationPolicyBundle {
        envelope,
        digest,
        verified_at_ms: if from_cache { 0 } else { now_ms },
    })
}

fn canonical_bundle_bytes(
    bundle: &OrganizationPolicyBundle,
) -> Result<Vec<u8>, OrganizationPolicyResolutionError> {
    let bytes = serde_json::to_vec(bundle).map_err(|error| {
        OrganizationPolicyResolutionError::InvalidBundle(format!(
            "Could not serialize organization policy: {error}"
        ))
    })?;
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(OrganizationPolicyResolutionError::InvalidBundle(format!(
            "Organization policy exceeds the {MAX_BUNDLE_BYTES}-byte limit."
        )));
    }
    Ok(bytes)
}

fn reject_organization_version_override(
    layer: &Value,
) -> Result<(), OrganizationPolicyResolutionError> {
    if layer
        .as_object()
        .is_some_and(|object| object.contains_key("version"))
    {
        return Err(OrganizationPolicyResolutionError::InvalidBundle(
            "Organization policy layers must not override the repository config schema version."
                .to_string(),
        ));
    }
    Ok(())
}

fn reject_repository_policy_indirection(
    layer: &Value,
) -> Result<(), OrganizationPolicyResolutionError> {
    if let Some(policy) = layer
        .as_object()
        .and_then(|object| object.get("policy"))
        .and_then(Value::as_object)
    {
        let forbidden = ["packs", "sources"]
            .into_iter()
            .filter(|field| policy.contains_key(*field))
            .collect::<Vec<_>>();
        if !forbidden.is_empty() {
            return Err(OrganizationPolicyResolutionError::InvalidBundle(format!(
                "Signed organization policy cannot reference repository-controlled policy {}. Embed rules directly in the signed bundle.",
                forbidden.join(" or ")
            )));
        }
    }
    if let Some((profile_id, _)) = layer
        .as_object()
        .and_then(|object| object.get("profiles"))
        .and_then(Value::as_object)
        .and_then(|profiles| {
            profiles.iter().find(|(_, profile)| {
                profile
                    .as_object()
                    .is_some_and(|profile| profile.contains_key("policyPacks"))
            })
        })
    {
        return Err(OrganizationPolicyResolutionError::InvalidBundle(format!(
            "Signed organization profile `{profile_id}` cannot reference repository-controlled policy packs. Embed rules directly in the signed bundle."
        )));
    }
    Ok(())
}

fn reject_signed_layer_secrets(
    value: &Value,
    path: &str,
) -> Result<(), OrganizationPolicyResolutionError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{key}");
                let normalized = key
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_lowercase();
                if is_credential_shaped_key(&normalized) {
                    return Err(OrganizationPolicyResolutionError::InvalidBundle(
                        format!(
                            "Signed organization policy field `{child_path}` looks like a credential. Store secrets in the keychain or environment instead."
                        ),
                    ));
                }
                reject_signed_layer_secrets(child, &child_path)?;
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                reject_signed_layer_secrets(child, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_credential_shaped_key(normalized: &str) -> bool {
    [
        "credential",
        "credentials",
        "token",
        "password",
        "secret",
        "username",
    ]
    .into_iter()
    .any(|marker| normalized.contains(marker))
}

fn validate_organization_layer_schema(
    value: &Value,
    layer: &str,
) -> Result<(), OrganizationPolicyResolutionError> {
    let yaml = serde_yaml::to_value(value).map_err(|error| {
        OrganizationPolicyResolutionError::InvalidBundle(format!(
            "Could not validate organization policy {layer}: {error}"
        ))
    })?;
    crate::repo_config::validate_external_config_layer(&yaml).map_err(|error| {
        OrganizationPolicyResolutionError::InvalidBundle(format!(
            "Invalid organization policy {layer}: {error}"
        ))
    })?;
    let mut candidate = serde_json::to_value(RepoReviewConfig {
        version: "0.1".to_string(),
        ..RepoReviewConfig::default()
    })
    .expect("repository config serialization is infallible");
    merge_json(&mut candidate, value);
    let candidate = serde_json::from_value(candidate).map_err(|error| {
        OrganizationPolicyResolutionError::InvalidBundle(format!(
            "Invalid organization policy {layer} shape: {error}"
        ))
    })?;
    crate::repo_config::validate_resolved_config(&candidate).map_err(|error| {
        OrganizationPolicyResolutionError::InvalidBundle(format!(
            "Invalid organization policy {layer}: {error}"
        ))
    })
}

fn reject_version_rollback(
    cached: Option<&CachedOrganizationPolicyBundle>,
    candidate: &CachedOrganizationPolicyBundle,
) -> Result<(), OrganizationPolicyResolutionError> {
    let Some(cached) = cached else {
        return Ok(());
    };
    let cached_version = cached.envelope.bundle.version;
    let candidate_version = candidate.envelope.bundle.version;
    if candidate_version < cached_version {
        return Err(OrganizationPolicyResolutionError::Rollback {
            cached: cached_version,
            received: candidate_version,
        });
    }
    if candidate_version == cached_version && candidate.digest != cached.digest {
        return Err(OrganizationPolicyResolutionError::VersionConflict(
            candidate_version,
        ));
    }
    Ok(())
}

fn verify_cached_for_rollback(
    config: &OrganizationPolicySourceConfig,
    cached: CachedOrganizationPolicyBundle,
) -> Result<CachedOrganizationPolicyBundle, OrganizationPolicyResolutionError> {
    let verification_time = cached.envelope.bundle.issued_at_ms;
    let mut verified = verify_bundle(config, cached.envelope, verification_time, true)?;
    if verified.digest != cached.digest {
        return Err(OrganizationPolicyResolutionError::Cache(
            "Cached organization policy digest does not match its signed content.".to_string(),
        ));
    }
    verified.verified_at_ms = cached.verified_at_ms;
    Ok(verified)
}

fn merge_json(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(key) {
                    Some(existing) if existing.is_object() && value.is_object() => {
                        merge_json(existing, value);
                    }
                    _ => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, overlay) => *base = overlay.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrganizationPolicyResolutionError {
    InvalidConfiguration(String),
    SourceRejected(String),
    Unavailable(String),
    Unsigned,
    UntrustedKey(String),
    InvalidSignature,
    IdentityMismatch,
    Expired,
    InvalidBundle(String),
    InvalidLayer(String),
    StaleCache(String),
    Rollback { cached: u64, received: u64 },
    VersionConflict(u64),
    Cache(String),
    Audit(String),
}

impl fmt::Display for OrganizationPolicyResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message)
            | Self::SourceRejected(message)
            | Self::Unavailable(message)
            | Self::InvalidBundle(message)
            | Self::InvalidLayer(message)
            | Self::StaleCache(message)
            | Self::Cache(message)
            | Self::Audit(message) => formatter.write_str(message),
            Self::Unsigned => formatter.write_str(
                "Organization policy bundle is unsigned; a trusted Ed25519 signature is required.",
            ),
            Self::UntrustedKey(key_id) => write!(
                formatter,
                "Organization policy was signed by untrusted key `{key_id}`."
            ),
            Self::InvalidSignature => formatter.write_str(
                "Organization policy signature verification failed; the bundle was not applied.",
            ),
            Self::IdentityMismatch => formatter.write_str(
                "Organization policy tenant or source does not match the requested identity.",
            ),
            Self::Expired => formatter.write_str(
                "Organization policy bundle has expired; fetch a current signed version.",
            ),
            Self::Rollback { cached, received } => write!(
                formatter,
                "Organization policy rollback rejected: cached version is {cached}, received version is {received}."
            ),
            Self::VersionConflict(version) => write!(
                formatter,
                "Organization policy version {version} has different signed content than the cached bundle."
            ),
        }
    }
}

impl std::error::Error for OrganizationPolicyResolutionError {}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;

    use super::*;

    const NOW: u64 = 1_800_000_000_000;

    struct StaticSource(Result<SignedOrganizationPolicyBundle, OrganizationPolicySourceError>);

    impl OrganizationPolicySource for StaticSource {
        fn fetch(
            &self,
            _request: &OrganizationPolicySourceRequest,
        ) -> Result<SignedOrganizationPolicyBundle, OrganizationPolicySourceError> {
            self.0.clone()
        }
    }

    #[derive(Default)]
    struct MemoryCache(RefCell<Option<CachedOrganizationPolicyBundle>>);

    impl OrganizationPolicyCache for MemoryCache {
        fn load(
            &self,
            _tenant_id: &str,
            _source_id: &str,
        ) -> Result<Option<CachedOrganizationPolicyBundle>, String> {
            Ok(self.0.borrow().clone())
        }

        fn store(&self, bundle: &CachedOrganizationPolicyBundle) -> Result<(), String> {
            *self.0.borrow_mut() = Some(bundle.clone());
            Ok(())
        }

        fn remove(&self, _tenant_id: &str, _source_id: &str) -> Result<(), String> {
            *self.0.borrow_mut() = None;
            Ok(())
        }
    }

    struct CorruptCache;

    impl OrganizationPolicyCache for CorruptCache {
        fn load(
            &self,
            _tenant_id: &str,
            _source_id: &str,
        ) -> Result<Option<CachedOrganizationPolicyBundle>, String> {
            Err("corrupt cached envelope".to_string())
        }

        fn store(&self, _bundle: &CachedOrganizationPolicyBundle) -> Result<(), String> {
            Ok(())
        }

        fn remove(&self, _tenant_id: &str, _source_id: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct MemoryAudit(RefCell<Vec<ResolvedPolicySourceVersion>>);

    impl OrganizationPolicyAuditSink for MemoryAudit {
        fn record(
            &self,
            source: &ResolvedPolicySourceVersion,
            _resolved_at_ms: u64,
        ) -> Result<(), String> {
            self.0.borrow_mut().push(source.clone());
            Ok(())
        }
    }

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7_u8; 32])
    }

    fn config(
        requirement: OrganizationPolicyRequirement,
        unavailable_behavior: OrganizationPolicyUnavailableBehavior,
    ) -> OrganizationPolicySourceConfig {
        let key = signing_key();
        OrganizationPolicySourceConfig {
            tenant_id: "tenant-acme".to_string(),
            source_id: "engineering".to_string(),
            requirement,
            unavailable_behavior,
            trusted_keys: BTreeMap::from([(
                "root-2026".to_string(),
                STANDARD.encode(key.verifying_key().to_bytes()),
            )]),
        }
    }

    fn signed_bundle(
        version: u64,
        defaults: Value,
        enforced: Value,
    ) -> SignedOrganizationPolicyBundle {
        let bundle = OrganizationPolicyBundle {
            schema_version: OrganizationPolicySchemaVersion::V1,
            tenant_id: "tenant-acme".to_string(),
            source_id: "engineering".to_string(),
            version,
            issued_at_ms: NOW - 1_000,
            expires_at_ms: NOW + 100_000,
            defaults,
            enforced,
        };
        let signature = signing_key().sign(&canonical_bundle_bytes(&bundle).expect("canonical"));
        SignedOrganizationPolicyBundle {
            bundle,
            integrity: Some(OrganizationPolicyIntegrity {
                algorithm: OrganizationPolicySignatureAlgorithm::Ed25519,
                key_id: "root-2026".to_string(),
                signature: STANDARD.encode(signature.to_bytes()),
            }),
        }
    }

    fn input() -> OrganizationPolicyResolutionInput {
        OrganizationPolicyResolutionInput {
            built_in: RepoReviewConfig {
                version: "0.1".to_string(),
                ..RepoReviewConfig::default()
            },
            repository: None,
            local_overrides: None,
            now_ms: NOW,
        }
    }

    #[test]
    fn organization_policy_config_path_must_be_absolute_and_outside_repo() {
        let repo = tempfile::tempdir().expect("repo temp dir");
        let outside = tempfile::tempdir().expect("outside temp dir");
        let inside_config = repo.path().join("organization-policy.json");
        let outside_config = outside.path().join("organization-policy.json");
        fs::write(&inside_config, "{}").expect("inside config fixture");
        fs::write(&outside_config, "{}").expect("outside config fixture");

        let relative = validate_organization_policy_path(
            repo.path(),
            Path::new("organization-policy.json"),
            "LACHESI_ORGANIZATION_POLICY_CONFIG",
        )
        .expect_err("relative trust config must be rejected");
        assert!(matches!(
            relative,
            OrganizationPolicyResolutionError::InvalidConfiguration(_)
        ));

        let inside = validate_organization_policy_path(
            repo.path(),
            &inside_config,
            "LACHESI_ORGANIZATION_POLICY_CONFIG",
        )
        .expect_err("repo-controlled trust config must be rejected");
        assert!(inside.to_string().contains("outside"));

        let resolved = validate_organization_policy_path(
            repo.path(),
            &outside_config,
            "LACHESI_ORGANIZATION_POLICY_CONFIG",
        )
        .expect("outside trust config");
        assert_eq!(
            resolved,
            outside_config.canonicalize().expect("canonical fixture")
        );
    }

    #[cfg(unix)]
    #[test]
    fn organization_policy_config_path_rejects_symlink_into_repo() {
        use std::os::unix::fs::symlink;

        let repo = tempfile::tempdir().expect("repo temp dir");
        let outside = tempfile::tempdir().expect("outside temp dir");
        let inside_config = repo.path().join("organization-policy.json");
        let config_link = outside.path().join("organization-policy.json");
        fs::write(&inside_config, "{}").expect("inside config fixture");
        symlink(&inside_config, &config_link).expect("config symlink fixture");

        let error = validate_organization_policy_path(
            repo.path(),
            &config_link,
            "LACHESI_ORGANIZATION_POLICY_CONFIG",
        )
        .expect_err("symlink into repo must be rejected");
        assert!(error.to_string().contains("outside"));
    }

    #[test]
    fn organization_policy_bundle_path_must_resolve_outside_repo() {
        let root = tempfile::tempdir().expect("root temp dir");
        let repo = root.path().join("repo");
        let admin = root.path().join("admin");
        fs::create_dir_all(&repo).expect("repo fixture");
        fs::create_dir_all(&admin).expect("admin fixture");
        let config_path = admin.join("organization-policy.json");
        let repo_bundle = repo.join("organization-policy.bundle.json");
        let admin_bundle = admin.join("organization-policy.bundle.json");
        fs::write(&config_path, "{}").expect("config fixture");
        fs::write(&repo_bundle, "{}").expect("repo bundle fixture");
        fs::write(&admin_bundle, "{}").expect("admin bundle fixture");

        resolve_organization_policy_bundle_path(&repo, &config_path, &repo_bundle)
            .expect_err("absolute repo bundle must be rejected");
        resolve_organization_policy_bundle_path(
            &repo,
            &config_path,
            Path::new("../repo/organization-policy.bundle.json"),
        )
        .expect_err("relative repo bundle must be rejected");
        assert_eq!(
            resolve_organization_policy_bundle_path(
                &repo,
                &config_path,
                Path::new("organization-policy.bundle.json"),
            )
            .expect("admin bundle"),
            admin_bundle.canonicalize().expect("canonical bundle")
        );

        let missing_admin_bundle = admin.join("temporarily-unavailable.bundle.json");
        assert_eq!(
            resolve_organization_policy_bundle_path(
                &repo,
                &config_path,
                Path::new("temporarily-unavailable.bundle.json"),
            )
            .expect("missing admin bundle can fall back to cache"),
            missing_admin_bundle
        );
        resolve_organization_policy_bundle_path(
            &repo,
            &config_path,
            &repo.join("temporarily-unavailable.bundle.json"),
        )
        .expect_err("missing repo-controlled bundle must be rejected");
    }

    #[test]
    fn file_source_rejects_oversized_bundle_before_json_parsing() {
        let directory = tempfile::tempdir().expect("bundle temp dir");
        let path = directory.path().join("organization-policy.bundle.json");
        fs::write(&path, vec![b' '; MAX_CACHED_ENVELOPE_BYTES + 1])
            .expect("oversized bundle fixture");
        let source = FileOrganizationPolicySource { path };

        let error = source
            .fetch(&OrganizationPolicySourceRequest {
                tenant_id: "tenant-acme".to_string(),
                source_id: "engineering".to_string(),
            })
            .expect_err("oversized bundle must be rejected");
        assert_eq!(error.kind, OrganizationPolicySourceErrorKind::Rejected);
        assert!(error.message.contains("file limit"));
    }

    #[test]
    fn precedence_is_defaults_then_repo_then_local_then_enforced() {
        let source = StaticSource(Ok(signed_bundle(
            3,
            json!({"review": {"mode": "fast", "findings": {"requireAnchors": false}}}),
            json!({"review": {"mode": "strict"}}),
        )));
        let mut input = input();
        input.repository = Some(json!({
            "review": {"mode": "balanced", "findings": {"requireAnchors": true}}
        }));
        input.local_overrides = Some(json!({
            "review": {"mode": "fast"},
            "paths": {"include": ["src/**"]}
        }));
        let audit = MemoryAudit::default();

        let resolved = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &source,
            &MemoryCache::default(),
            Some(&audit),
            input,
        )
        .expect("resolved policy");

        let review = resolved.config.review.expect("review");
        assert_eq!(review.mode, Some(crate::repo_config::ReviewMode::Strict));
        assert_eq!(
            review
                .findings
                .and_then(|findings| findings.require_anchors),
            Some(true)
        );
        assert_eq!(
            resolved.config.paths.expect("paths").include,
            vec!["src/**"]
        );
        assert_eq!(resolved.sources[0].version, 3);
        assert!(!resolved.sources[0].from_cache);
        assert_eq!(audit.0.borrow().as_slice(), resolved.sources.as_slice());
    }

    #[test]
    fn enforced_layer_wins_after_repository_profile_finalization() {
        let config = RepoReviewConfig {
            version: "0.1".to_string(),
            review: Some(crate::repo_config::ReviewConfig {
                mode: Some(crate::repo_config::ReviewMode::Fast),
                ..crate::repo_config::ReviewConfig::default()
            }),
            ..RepoReviewConfig::default()
        };
        let resolved = reapply_enforced_layer(
            config,
            Some(&json!({
                "review": {"mode": "strict"},
                "analyzers": {
                    "security": {
                        "enabled": true,
                        "required": true,
                        "command": "security-check"
                    }
                }
            })),
        )
        .expect("reapply enforcement");

        assert_eq!(
            resolved.review.and_then(|review| review.mode),
            Some(crate::repo_config::ReviewMode::Strict)
        );
        assert!(resolved.analyzers["security"].required);
    }

    #[test]
    fn enforced_profile_is_expanded_instead_of_caller_override() {
        let repo = tempfile::tempdir().expect("repo temp dir");
        let mut resolved = ResolvedOrganizationPolicy {
            config: RepoReviewConfig {
                version: "0.1".to_string(),
                review: Some(crate::repo_config::ReviewConfig {
                    profile: Some("strict".to_string()),
                    ..crate::repo_config::ReviewConfig::default()
                }),
                profiles: BTreeMap::from([
                    (
                        "fast".to_string(),
                        crate::repo_config::ReviewProfileConfig {
                            mode: Some(crate::repo_config::ReviewMode::Fast),
                            ..crate::repo_config::ReviewProfileConfig::default()
                        },
                    ),
                    (
                        "strict".to_string(),
                        crate::repo_config::ReviewProfileConfig {
                            mode: Some(crate::repo_config::ReviewMode::Fast),
                            policy_packs: vec![".lachesi/packs/repository".to_string()],
                            analyzers: BTreeMap::from([(
                                "security".to_string(),
                                crate::repo_config::ProfileAnalyzerRequirement::Required,
                            )]),
                            ..crate::repo_config::ReviewProfileConfig::default()
                        },
                    ),
                ]),
                analyzers: BTreeMap::from([(
                    "security".to_string(),
                    crate::repo_config::AnalyzerConfig {
                        command: Some("repository-check".to_string()),
                        ..crate::repo_config::AnalyzerConfig::default()
                    },
                )]),
                ..RepoReviewConfig::default()
            },
            sources: Vec::new(),
            required_analyzers: Vec::new(),
            selected_profile: None,
            loaded_policy_packs: Vec::new(),
            warnings: Vec::new(),
            enforced_layer: Some(json!({
                "review": {"profile": "strict"},
                "profiles": {
                    "strict": {
                        "mode": "strict",
                        "analyzers": {"security": "required"}
                    }
                },
                "analyzers": {
                    "security": {
                        "enabled": true,
                        "required": true,
                        "command": "organization-check"
                    }
                }
            })),
            pending_cache_bundle: None,
        };

        finalize_resolved_organization_policy(repo.path(), &mut resolved, Some("fast"))
            .expect("enforced profile finalization");

        let review = resolved.config.review.expect("review config");
        assert_eq!(review.profile.as_deref(), Some("strict"));
        assert_eq!(review.mode, Some(crate::repo_config::ReviewMode::Strict));
        assert!(resolved.config.analyzers["security"].enabled);
        assert!(resolved.config.analyzers["security"].required);
        assert_eq!(
            resolved.config.analyzers["security"].command.as_deref(),
            Some("organization-check")
        );
        assert_eq!(resolved.required_analyzers, vec!["security"]);
        assert!(resolved
            .config
            .policy
            .as_ref()
            .is_none_or(|policy| policy.packs.is_empty()));
    }

    #[test]
    fn missing_enforced_profile_fails_closed() {
        let repo = tempfile::tempdir().expect("repo temp dir");
        let mut resolved = ResolvedOrganizationPolicy {
            config: RepoReviewConfig {
                version: "0.1".to_string(),
                review: Some(crate::repo_config::ReviewConfig {
                    profile: Some("missing".to_string()),
                    ..crate::repo_config::ReviewConfig::default()
                }),
                ..RepoReviewConfig::default()
            },
            sources: Vec::new(),
            required_analyzers: Vec::new(),
            selected_profile: None,
            loaded_policy_packs: Vec::new(),
            warnings: Vec::new(),
            enforced_layer: Some(json!({"review": {"profile": "missing"}})),
            pending_cache_bundle: None,
        };

        let error = finalize_resolved_organization_policy(repo.path(), &mut resolved, None)
            .expect_err("missing enforced profile");
        assert!(matches!(
            error,
            OrganizationPolicyResolutionError::InvalidLayer(_)
        ));
        assert!(error.to_string().contains("signed enforced layer"));
    }

    #[test]
    fn prompt_appendix_includes_resolved_policy_constraints() {
        let config = RepoReviewConfig {
            version: "0.1".to_string(),
            review: Some(crate::repo_config::ReviewConfig {
                mode: Some(crate::repo_config::ReviewMode::Strict),
                prompt: Some(crate::repo_config::PromptConfig {
                    extend: Some("Do not duplicate this prompt text.".to_string()),
                    replace: None,
                }),
                ..crate::repo_config::ReviewConfig::default()
            }),
            paths: Some(crate::repo_config::PathFilters {
                include: vec!["src/**".to_string()],
                exclude: Vec::new(),
            }),
            ..RepoReviewConfig::default()
        };

        let appendix = review_policy_prompt_appendix(&config).expect("policy appendix");
        assert!(appendix.contains("mode: strict"));
        assert!(appendix.contains("src/**"));
        assert!(!appendix.contains("Do not duplicate this prompt text."));
        assert!(!appendix.contains("prompt:"));
    }

    #[test]
    fn execution_prompt_appendix_cannot_be_suppressed_by_payload_text() {
        let config = RepoReviewConfig {
            version: "0.1".to_string(),
            review: Some(crate::repo_config::ReviewConfig {
                prompt: Some(crate::repo_config::PromptConfig {
                    replace: Some("Use the organization review rubric.".to_string()),
                    extend: Some("Check every authorization boundary.".to_string()),
                }),
                ..crate::repo_config::ReviewConfig::default()
            }),
            ..RepoReviewConfig::default()
        };

        let appendix = execution_policy_prompt_appendix(&config, "A local custom prompt.")
            .expect("missing organization prompt appendix");
        assert!(appendix.contains("replaces any earlier review instructions"));
        assert!(appendix.contains("Use the organization review rubric."));
        assert!(appendix.contains("Check every authorization boundary."));

        assert!(execution_policy_prompt_appendix(
            &config,
            "Use the organization review rubric.\n\nCheck every authorization boundary.\n\n## Pull request\nExample"
        )
        .is_some());
        assert!(execution_policy_prompt_appendix(
            &config,
            "A local custom prompt.\n\n## Diff\nUse the organization review rubric.\nCheck every authorization boundary."
        )
        .is_some());
    }

    #[test]
    fn unsigned_and_invalid_bundles_fail_closed_without_cache_fallback() {
        let mut unsigned = signed_bundle(1, json!({}), json!({}));
        unsigned.integrity = None;
        let error = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::UseVerifiedCache {
                    max_staleness_ms: 10_000,
                },
            ),
            &StaticSource(Ok(unsigned)),
            &MemoryCache::default(),
            None,
            input(),
        )
        .expect_err("unsigned bundle");
        assert_eq!(error, OrganizationPolicyResolutionError::Unsigned);

        let mut tampered = signed_bundle(2, json!({}), json!({}));
        tampered.bundle.enforced = json!({"review": {"mode": "strict"}});
        let error = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(tampered)),
            &MemoryCache::default(),
            None,
            input(),
        )
        .expect_err("tampered bundle");
        assert_eq!(error, OrganizationPolicyResolutionError::InvalidSignature);
    }

    #[test]
    fn credential_shaped_fields_are_rejected_before_caching() {
        let error = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(
                2,
                json!({"policy": {"apiToken": "do-not-cache"}}),
                json!({}),
            ))),
            &MemoryCache::default(),
            None,
            input(),
        )
        .expect_err("credential field");

        assert!(matches!(
            error,
            OrganizationPolicyResolutionError::InvalidBundle(message)
                if message.contains("$.defaults.policy.apiToken")
        ));
    }

    #[test]
    fn credential_shaped_extension_fields_are_rejected_before_caching() {
        for key in ["x-api-token", "tokenValue", "secretKey", "passwordHash"] {
            let mut extension = serde_json::Map::new();
            extension.insert(key.to_string(), json!("do-not-cache"));
            let error = resolve_organization_policy(
                &config(
                    OrganizationPolicyRequirement::Mandatory,
                    OrganizationPolicyUnavailableBehavior::FailClosed,
                ),
                &StaticSource(Ok(signed_bundle(
                    2,
                    json!({
                        "analyzers": {
                            "custom": {
                                "enabled": false,
                                "config": extension
                            }
                        }
                    }),
                    json!({}),
                ))),
                &MemoryCache::default(),
                None,
                input(),
            )
            .expect_err("credential extension field");

            assert!(matches!(
                error,
                OrganizationPolicyResolutionError::InvalidBundle(message)
                    if message.contains(key)
            ));
        }
    }

    #[test]
    fn unknown_organization_fields_fail_closed() {
        let error = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(
                2,
                json!({"review": {"strictnes": "high"}}),
                json!({}),
            ))),
            &MemoryCache::default(),
            None,
            input(),
        )
        .expect_err("unknown field");

        assert!(matches!(
            error,
            OrganizationPolicyResolutionError::InvalidBundle(message)
                if message.contains("$.review.strictnes")
        ));
    }

    #[test]
    fn signed_layers_cannot_delegate_to_repository_policy_files() {
        for policy in [
            json!({"packs": [".lachesi/packs/security"]}),
            json!({"sources": [{"type": "adr", "path": "docs/adr"}]}),
        ] {
            let error = resolve_organization_policy(
                &config(
                    OrganizationPolicyRequirement::Mandatory,
                    OrganizationPolicyUnavailableBehavior::FailClosed,
                ),
                &StaticSource(Ok(signed_bundle(2, json!({"policy": policy}), json!({})))),
                &MemoryCache::default(),
                None,
                input(),
            )
            .expect_err("repository indirection");

            assert!(matches!(
                error,
                OrganizationPolicyResolutionError::InvalidBundle(message)
                    if message.contains("repository-controlled")
            ));
        }

        let error = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(
                2,
                json!({
                    "profiles": {
                        "strict": {"policyPacks": [".lachesi/packs/security"]}
                    }
                }),
                json!({}),
            ))),
            &MemoryCache::default(),
            None,
            input(),
        )
        .expect_err("profile repository indirection");
        assert!(matches!(
            error,
            OrganizationPolicyResolutionError::InvalidBundle(message)
                if message.contains("profile `strict`")
        ));
    }

    #[test]
    fn invalid_organization_value_shapes_fail_before_caching() {
        let cache = MemoryCache::default();
        let error = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(
                2,
                json!({"review": {"mode": "maximum"}}),
                json!({}),
            ))),
            &cache,
            None,
            input(),
        )
        .expect_err("invalid enum");

        assert!(matches!(
            error,
            OrganizationPolicyResolutionError::InvalidBundle(_)
        ));
        assert!(cache.0.borrow().is_none());
    }

    #[test]
    fn bounded_cache_fallback_rejects_stale_entries() {
        let envelope = signed_bundle(4, json!({}), json!({}));
        let canonical = canonical_bundle_bytes(&envelope.bundle).expect("canonical");
        let cache = MemoryCache(RefCell::new(Some(CachedOrganizationPolicyBundle {
            envelope,
            digest: hex::encode(Sha256::digest(canonical)),
            verified_at_ms: NOW - 10_001,
        })));
        let error = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::UseVerifiedCache {
                    max_staleness_ms: 10_000,
                },
            ),
            &StaticSource(Err(OrganizationPolicySourceError::unavailable(
                "network timeout",
            ))),
            &cache,
            None,
            input(),
        )
        .expect_err("stale cache");
        assert!(matches!(
            error,
            OrganizationPolicyResolutionError::StaleCache(_)
        ));
    }

    #[test]
    fn corrupt_cache_does_not_block_a_valid_live_bundle() {
        let resolved = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(9, json!({}), json!({})))),
            &CorruptCache,
            None,
            input(),
        )
        .expect("live bundle");

        assert_eq!(resolved.sources[0].version, 9);
        assert!(!resolved.sources[0].from_cache);
    }

    #[test]
    fn invalid_high_version_cache_is_repaired_by_valid_live_bundle() {
        let mut corrupt_envelope = signed_bundle(99, json!({}), json!({}));
        corrupt_envelope
            .integrity
            .as_mut()
            .expect("integrity")
            .signature = STANDARD.encode([0_u8; ED25519_SIGNATURE_BYTES]);
        let cache = MemoryCache(RefCell::new(Some(CachedOrganizationPolicyBundle {
            envelope: corrupt_envelope,
            digest: "f".repeat(64),
            verified_at_ms: NOW - 100,
        })));

        let mut resolved = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(9, json!({}), json!({})))),
            &cache,
            None,
            input(),
        )
        .expect("valid live bundle repairs cache");

        assert_eq!(resolved.sources[0].version, 9);
        let repo = tempfile::tempdir().expect("repo temp dir");
        finalize_resolved_organization_policy(repo.path(), &mut resolved, None)
            .expect("finalize live bundle");
        store_pending_cache_bundle(&mut resolved, &cache).expect("store finalized bundle");
        assert_eq!(
            cache
                .0
                .borrow()
                .as_ref()
                .map(|cached| cached.envelope.bundle.version),
            Some(9)
        );
    }

    #[test]
    fn live_bundle_is_not_cached_before_finalization_succeeds() {
        let cache = MemoryCache::default();
        let mut resolved = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(
                9,
                json!({}),
                json!({"review": {"profile": "missing"}}),
            ))),
            &cache,
            None,
            input(),
        )
        .expect("verified live bundle");

        let repo = tempfile::tempdir().expect("repo temp dir");
        finalize_resolved_organization_policy(repo.path(), &mut resolved, None)
            .expect_err("missing signed enforced profile");
        assert!(cache.0.borrow().is_none());
    }

    #[test]
    fn verified_cache_is_used_only_when_explicitly_configured() {
        let envelope = signed_bundle(4, json!({"review": {"mode": "fast"}}), json!({}));
        let canonical = canonical_bundle_bytes(&envelope.bundle).expect("canonical");
        let cache = MemoryCache(RefCell::new(Some(CachedOrganizationPolicyBundle {
            envelope,
            digest: hex::encode(Sha256::digest(canonical)),
            verified_at_ms: NOW - 100,
        })));
        let source = StaticSource(Err(OrganizationPolicySourceError::unavailable(
            "network timeout",
        )));

        let failed = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Optional,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &source,
            &cache,
            None,
            input(),
        )
        .expect_err("implicit fallback");
        assert!(matches!(
            failed,
            OrganizationPolicyResolutionError::Unavailable(_)
        ));

        let resolved = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Optional,
                OrganizationPolicyUnavailableBehavior::UseVerifiedCache {
                    max_staleness_ms: 1_000,
                },
            ),
            &source,
            &cache,
            None,
            input(),
        )
        .expect("explicit cache fallback");
        assert!(resolved.sources[0].from_cache);
    }

    #[test]
    fn lower_versions_and_same_version_mutations_are_rejected() {
        let cached_envelope = signed_bundle(8, json!({}), json!({}));
        let canonical = canonical_bundle_bytes(&cached_envelope.bundle).expect("canonical");
        let cache = MemoryCache(RefCell::new(Some(CachedOrganizationPolicyBundle {
            envelope: cached_envelope,
            digest: hex::encode(Sha256::digest(canonical)),
            verified_at_ms: NOW - 100,
        })));

        let rollback = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(7, json!({}), json!({})))),
            &cache,
            None,
            input(),
        )
        .expect_err("rollback");
        assert_eq!(
            rollback,
            OrganizationPolicyResolutionError::Rollback {
                cached: 8,
                received: 7
            }
        );

        let conflict = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(
                8,
                json!({"review": {"mode": "fast"}}),
                json!({}),
            ))),
            &cache,
            None,
            input(),
        )
        .expect_err("version conflict");
        assert_eq!(
            conflict,
            OrganizationPolicyResolutionError::VersionConflict(8)
        );
    }

    #[test]
    fn versions_must_fit_exactly_in_json_consumers() {
        let error = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(
                MAX_SAFE_JSON_INTEGER + 1,
                json!({}),
                json!({}),
            ))),
            &MemoryCache::default(),
            None,
            input(),
        )
        .expect_err("unsafe JSON integer");

        assert!(matches!(
            error,
            OrganizationPolicyResolutionError::InvalidBundle(message)
                if message.contains("9007199254740991")
        ));
    }

    #[test]
    fn optional_local_fallback_requires_explicit_configuration() {
        let source = StaticSource(Err(OrganizationPolicySourceError::unavailable(
            "not configured",
        )));
        let mut input = input();
        input.repository = Some(json!({"review": {"mode": "strict"}}));
        let resolved = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Optional,
                OrganizationPolicyUnavailableBehavior::ContinueWithoutOrganizationPolicy,
            ),
            &source,
            &MemoryCache::default(),
            None,
            input,
        )
        .expect("explicit local fallback");
        assert!(resolved.sources.is_empty());
        assert_eq!(
            resolved.config.review.and_then(|review| review.mode),
            Some(crate::repo_config::ReviewMode::Strict)
        );
    }

    #[test]
    fn audit_event_records_the_resolved_source_version() {
        let metadata = ResolvedPolicySourceVersion {
            tenant_id: "tenant-acme".to_string(),
            source_id: "engineering".to_string(),
            version: 12,
            digest: "a".repeat(64),
            key_id: "root-2026".to_string(),
            from_cache: false,
        };
        let event = policy_resolution_audit_event(
            &AdministrativePolicyAuditContext {
                provider: PullRequestReviewEventProvider::Github,
                workspace: "acme".to_string(),
                repository: "payments".to_string(),
                pull_request_id: Some(42),
                actor_kind: AdministrativeAuditActorKind::Service,
                actor_id: "service:review-worker".to_string(),
                correlation_id: "correlation:job-42".to_string(),
            },
            &metadata,
            NOW,
        );

        assert_eq!(event.action, AdministrativeAuditAction::PolicyResolved);
        assert_eq!(event.target.id, "policy:engineering:v12");
        event.prepare_for_storage().expect("valid audit event");

        let next_turn = policy_resolution_audit_event(
            &AdministrativePolicyAuditContext {
                provider: PullRequestReviewEventProvider::Github,
                workspace: "acme".to_string(),
                repository: "payments".to_string(),
                pull_request_id: Some(42),
                actor_kind: AdministrativeAuditActorKind::Service,
                actor_id: "service:review-worker".to_string(),
                correlation_id: "correlation:job-42".to_string(),
            },
            &metadata,
            NOW + 1,
        );
        assert_ne!(event.delivery_id, next_turn.delivery_id);
    }

    #[test]
    fn invalid_resolved_config_is_not_audited_as_successful() {
        let audit = MemoryAudit::default();
        let cache = MemoryCache::default();
        let error = resolve_organization_policy(
            &config(
                OrganizationPolicyRequirement::Mandatory,
                OrganizationPolicyUnavailableBehavior::FailClosed,
            ),
            &StaticSource(Ok(signed_bundle(
                5,
                json!({
                    "analyzers": {
                        "required-check": {"enabled": true, "command": "true"}
                    }
                }),
                json!({"analyzers": {"required-check": {"command": null}}}),
            ))),
            &cache,
            Some(&audit),
            input(),
        )
        .expect_err("invalid resolved config");

        assert!(matches!(
            error,
            OrganizationPolicyResolutionError::InvalidLayer(_)
        ));
        assert!(audit.0.borrow().is_empty());
        assert!(cache.0.borrow().is_none());
    }
}
