//! Redacted, provider-neutral administrative audit event contract.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::review_event::PullRequestReviewEventProvider;

const MAX_IDENTIFIER_BYTES: usize = 512;
const MAX_REDACTABLE_VALUE_BYTES: usize = 4096;
const MAX_OCCURRED_AT_MILLIS: i64 = 4_102_444_800_000;
pub const REDACTED_AUDIT_VALUE: &str = "[redacted]";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdministrativeAuditSchemaVersion {
    #[serde(rename = "v1")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdministrativeAuditAction {
    ConfigurationChanged,
    PolicyResolved,
    AutomatedReviewTriggered,
    ReviewCancelled,
    ReviewPublished,
    CredentialReferenceChanged,
}

impl AdministrativeAuditAction {
    pub const ALL: [Self; 6] = [
        Self::ConfigurationChanged,
        Self::PolicyResolved,
        Self::AutomatedReviewTriggered,
        Self::ReviewCancelled,
        Self::ReviewPublished,
        Self::CredentialReferenceChanged,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdministrativeAuditActorKind {
    User,
    Service,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdministrativeAuditTargetKind {
    Configuration,
    Policy,
    ReviewRun,
    Publication,
    CredentialReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdministrativeAuditOutcome {
    Succeeded,
    Failed,
    Cancelled,
    Denied,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdministrativeAuditActor {
    pub kind: AdministrativeAuditActorKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdministrativeAuditRepositoryScope {
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdministrativeAuditTarget {
    pub kind: AdministrativeAuditTargetKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdministrativeAuditEvent {
    pub schema_version: AdministrativeAuditSchemaVersion,
    /// Tenant-unique delivery key used for idempotency.
    pub delivery_id: String,
    pub tenant_id: String,
    /// Milliseconds since Unix epoch, represented as a decimal string.
    pub occurred_at: String,
    pub actor: AdministrativeAuditActor,
    pub repository: AdministrativeAuditRepositoryScope,
    pub action: AdministrativeAuditAction,
    pub target: AdministrativeAuditTarget,
    pub outcome: AdministrativeAuditOutcome,
    pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdministrativeAuditAppendResult {
    Appended,
    Duplicate,
    CollectionDisabled,
}

impl AdministrativeAuditEvent {
    pub fn prepare_for_storage(
        &self,
    ) -> Result<AdministrativeAuditEvent, AdministrativeAuditValidationError> {
        self.validate_structural_fields()?;
        validate_redactable_input("actor.id", &self.actor.id)?;
        validate_redactable_input("target.id", &self.target.id)?;
        validate_redactable_input("correlationId", &self.correlation_id)?;

        let mut event = self.clone();
        event.actor.id = redact_value_v1(
            &event.actor.id,
            actor_id_is_safe_v1(event.actor.kind, &event.actor.id),
        );
        event.target.id = redact_value_v1(
            &event.target.id,
            target_id_is_safe_v1(event.target.kind, &event.target.id),
        );
        event.correlation_id = redact_value_v1(
            &event.correlation_id,
            correlation_id_is_safe_v1(&event.correlation_id),
        );
        event.validate_stored()?;
        Ok(event)
    }

    pub fn validate_stored(&self) -> Result<(), AdministrativeAuditValidationError> {
        self.validate_structural_fields()?;
        for (field, value, is_safe) in [
            (
                "actor.id",
                self.actor.id.as_str(),
                actor_id_is_safe_v1(self.actor.kind, &self.actor.id),
            ),
            (
                "target.id",
                self.target.id.as_str(),
                target_id_is_safe_v1(self.target.kind, &self.target.id),
            ),
            (
                "correlationId",
                self.correlation_id.as_str(),
                correlation_id_is_safe_v1(&self.correlation_id),
            ),
        ] {
            validate_redactable_stored(field, value)?;
            if value != REDACTED_AUDIT_VALUE && !is_safe {
                return Err(AdministrativeAuditValidationError::UnredactedSensitiveValue(field));
            }
        }
        Ok(())
    }

    fn validate_structural_fields(&self) -> Result<(), AdministrativeAuditValidationError> {
        validate_identifier("deliveryId", &self.delivery_id)?;
        validate_identifier("tenantId", &self.tenant_id)?;
        validate_timestamp(&self.occurred_at)?;
        validate_identifier("workspace", &self.repository.workspace)?;
        validate_identifier("repo", &self.repository.repo)?;
        if self.repository.pr_id == Some(0) {
            return Err(AdministrativeAuditValidationError::InvalidPullRequestId);
        }
        Ok(())
    }
}

pub(crate) fn validate_identifier(
    field: &'static str,
    value: &str,
) -> Result<(), AdministrativeAuditValidationError> {
    if value.is_empty() {
        return Err(AdministrativeAuditValidationError::MissingField(field));
    }
    if value != value.trim()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._:@-".contains(character))
        || !audit_value_is_safe_v1(value)
    {
        return Err(AdministrativeAuditValidationError::InvalidIdentifier(field));
    }
    Ok(())
}

fn validate_redactable_input(
    field: &'static str,
    value: &str,
) -> Result<(), AdministrativeAuditValidationError> {
    if value.trim().is_empty() {
        return Err(AdministrativeAuditValidationError::MissingField(field));
    }
    Ok(())
}

fn validate_redactable_stored(
    field: &'static str,
    value: &str,
) -> Result<(), AdministrativeAuditValidationError> {
    validate_redactable_input(field, value)?;
    if value.len() > MAX_REDACTABLE_VALUE_BYTES {
        return Err(AdministrativeAuditValidationError::FieldTooLong(field));
    }
    Ok(())
}

fn validate_timestamp(value: &str) -> Result<(), AdministrativeAuditValidationError> {
    let timestamp = value
        .parse::<i64>()
        .map_err(|_| AdministrativeAuditValidationError::InvalidTimestamp)?;
    if !(0..=MAX_OCCURRED_AT_MILLIS).contains(&timestamp) {
        return Err(AdministrativeAuditValidationError::InvalidTimestamp);
    }
    Ok(())
}

fn redact_value_v1(value: &str, is_safe: bool) -> String {
    if value.len() <= MAX_REDACTABLE_VALUE_BYTES && is_safe {
        value.to_string()
    } else {
        REDACTED_AUDIT_VALUE.to_string()
    }
}

fn actor_id_is_safe_v1(kind: AdministrativeAuditActorKind, value: &str) -> bool {
    let prefix = match kind {
        AdministrativeAuditActorKind::User => "user:",
        AdministrativeAuditActorKind::Service => "service:",
        AdministrativeAuditActorKind::System => "system:",
    };
    audit_value_has_prefix_v1(value, prefix)
}

fn target_id_is_safe_v1(kind: AdministrativeAuditTargetKind, value: &str) -> bool {
    let prefix = match kind {
        AdministrativeAuditTargetKind::Configuration => "config:",
        AdministrativeAuditTargetKind::Policy => "policy:",
        AdministrativeAuditTargetKind::ReviewRun => "run:",
        AdministrativeAuditTargetKind::Publication => "publication:",
        AdministrativeAuditTargetKind::CredentialReference => "credential-ref:",
    };
    audit_value_has_prefix_v1(value, prefix)
}

fn correlation_id_is_safe_v1(value: &str) -> bool {
    audit_value_has_prefix_v1(value, "correlation:")
}

fn audit_value_has_prefix_v1(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty()
            && audit_value_is_safe_v1(value)
            && audit_value_is_safe_v1(suffix)
            && !(suffix.len() >= 20
                && suffix
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric()))
    })
}

// Frozen with schema v1. Expanding redaction rules requires a new schema version.
fn audit_value_is_safe_v1(value: &str) -> bool {
    if value.len() > MAX_REDACTABLE_VALUE_BYTES {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    let sensitive_substrings = [
        "bearer ",
        "token=",
        "password=",
        "secret=",
        "access_token",
        "prompt:",
        "diff --git",
        "```",
        "-----begin ",
        "$home",
    ];
    let sensitive_prefixes = ["github_pat_", "ghp_", "xoxb-", "sk-", "akia", "asia"];
    if sensitive_substrings
        .iter()
        .any(|pattern| lower.contains(pattern))
        || sensitive_prefixes
            .iter()
            .any(|pattern| lower.starts_with(pattern))
        || lower.starts_with("file:")
        || value.starts_with('/')
        || value.starts_with("~/")
        || value.contains('\\')
        || value.contains("../")
        || value.contains("./")
        || value.chars().any(char::is_whitespace)
    {
        return false;
    }
    value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "._:@-".contains(character))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdministrativeAuditValidationError {
    MissingField(&'static str),
    InvalidIdentifier(&'static str),
    FieldTooLong(&'static str),
    InvalidPullRequestId,
    InvalidTimestamp,
    UnredactedSensitiveValue(&'static str),
}

impl fmt::Display for AdministrativeAuditValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => write!(formatter, "`{field}` must not be empty"),
            Self::InvalidIdentifier(field) => {
                write!(formatter, "`{field}` is not a valid audit identifier")
            }
            Self::FieldTooLong(field) => write!(formatter, "`{field}` exceeds the supported size"),
            Self::InvalidPullRequestId => {
                formatter.write_str("`prId` must be a positive integer when provided")
            }
            Self::InvalidTimestamp => {
                formatter.write_str("`occurredAt` is outside the supported epoch range")
            }
            Self::UnredactedSensitiveValue(field) => {
                write!(formatter, "`{field}` contains unredacted sensitive data")
            }
        }
    }
}

impl std::error::Error for AdministrativeAuditValidationError {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    pub(crate) fn audit_event() -> AdministrativeAuditEvent {
        AdministrativeAuditEvent {
            schema_version: AdministrativeAuditSchemaVersion::V1,
            delivery_id: "delivery-1".to_string(),
            tenant_id: "tenant-acme".to_string(),
            occurred_at: "1000".to_string(),
            actor: AdministrativeAuditActor {
                kind: AdministrativeAuditActorKind::User,
                id: "user:reviewer-1".to_string(),
            },
            repository: AdministrativeAuditRepositoryScope {
                provider: PullRequestReviewEventProvider::Github,
                workspace: "acme".to_string(),
                repo: "payments".to_string(),
                pr_id: Some(42),
            },
            action: AdministrativeAuditAction::ConfigurationChanged,
            target: AdministrativeAuditTarget {
                kind: AdministrativeAuditTargetKind::Configuration,
                id: "config:review-profile:strict".to_string(),
            },
            outcome: AdministrativeAuditOutcome::Succeeded,
            correlation_id: "correlation:1".to_string(),
        }
    }

    #[test]
    fn v1_json_shape_is_stable_and_strict() {
        let event = audit_event().prepare_for_storage().expect("prepared event");
        assert_eq!(
            serde_json::to_value(&event).expect("serialize"),
            json!({
                "schemaVersion": "v1",
                "deliveryId": "delivery-1",
                "tenantId": "tenant-acme",
                "occurredAt": "1000",
                "actor": {"kind": "user", "id": "user:reviewer-1"},
                "repository": {
                    "provider": "github",
                    "workspace": "acme",
                    "repo": "payments",
                    "prId": 42
                },
                "action": "configuration_changed",
                "target": {"kind": "configuration", "id": "config:review-profile:strict"},
                "outcome": "succeeded",
                "correlationId": "correlation:1"
            })
        );
        let mut unknown = serde_json::to_value(event).expect("serialize");
        unknown["sourceCode"] = json!("secret");
        assert!(serde_json::from_value::<AdministrativeAuditEvent>(unknown).is_err());
    }

    #[test]
    fn sensitive_values_are_redacted_before_serialization() {
        let mut event = audit_event();
        event.actor.id = "Bearer secret-token".to_string();
        event.target.id = "/Users/alice/private/repo".to_string();
        event.correlation_id = "prompt: review this source code".to_string();

        assert_eq!(
            event.validate_stored(),
            Err(AdministrativeAuditValidationError::UnredactedSensitiveValue("actor.id"))
        );
        let prepared = event.prepare_for_storage().expect("prepared event");
        prepared.validate_stored().expect("stored event is valid");
        let json = serde_json::to_string(&prepared).expect("serialize");

        assert_eq!(prepared.actor.id, REDACTED_AUDIT_VALUE);
        assert_eq!(prepared.target.id, REDACTED_AUDIT_VALUE);
        assert_eq!(prepared.correlation_id, REDACTED_AUDIT_VALUE);
        for sensitive in ["secret-token", "/Users/alice", "review this source"] {
            assert!(!json.contains(sensitive));
        }

        let mut invalid_identity = audit_event();
        invalid_identity.delivery_id = "ghp_secretvalue".to_string();
        assert_eq!(
            invalid_identity.prepare_for_storage(),
            Err(AdministrativeAuditValidationError::InvalidIdentifier(
                "deliveryId"
            ))
        );
        invalid_identity.delivery_id = "delivery-1".to_string();
        invalid_identity.tenant_id = "tenant-\u{0430}cme".to_string();
        assert_eq!(
            invalid_identity.prepare_for_storage(),
            Err(AdministrativeAuditValidationError::InvalidIdentifier(
                "tenantId"
            ))
        );
    }

    #[test]
    fn untyped_and_token_shaped_values_are_redacted() {
        let mut event = audit_event();
        event.actor.id = "AKIAIOSFODNN7EXAMPLE".to_string();
        event.target.id = "550e8400-e29b-41d4-a716-446655440000".to_string();
        event.correlation_id = "genericapikey1234567890".to_string();

        let prepared = event.prepare_for_storage().expect("prepared event");

        assert_eq!(prepared.actor.id, REDACTED_AUDIT_VALUE);
        assert_eq!(prepared.target.id, REDACTED_AUDIT_VALUE);
        assert_eq!(prepared.correlation_id, REDACTED_AUDIT_VALUE);
        prepared.validate_stored().expect("stored event is valid");
    }

    #[test]
    fn oversized_sensitive_values_are_redacted_before_the_stored_size_limit() {
        let mut event = audit_event();
        event.actor.id = format!("Bearer {}", "secret".repeat(1000));
        event.target.id = format!("/Users/alice/{}", "private".repeat(1000));
        event.correlation_id = format!("prompt: {}", "source".repeat(1000));

        let prepared = event.prepare_for_storage().expect("prepared event");

        assert_eq!(prepared.actor.id, REDACTED_AUDIT_VALUE);
        assert_eq!(prepared.target.id, REDACTED_AUDIT_VALUE);
        assert_eq!(prepared.correlation_id, REDACTED_AUDIT_VALUE);
        prepared.validate_stored().expect("stored event is valid");
    }

    #[test]
    fn every_action_round_trips_without_ai_provider_coupling() {
        for action in AdministrativeAuditAction::ALL {
            let mut event = audit_event();
            event.action = action;
            let json = serde_json::to_string(&event).expect("serialize");
            assert!(!json.contains("aiProvider"));
            assert_eq!(
                serde_json::from_str::<AdministrativeAuditEvent>(&json)
                    .expect("deserialize")
                    .action,
                action
            );
        }
    }
}
