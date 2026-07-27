//! Provider-neutral pull-request events consumed by shared review orchestration.
//!
//! Provider adapters own webhook authentication and payload parsing. They emit
//! this contract only after assigning a tenant and normalizing provider data.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PullRequestReviewEventSchemaVersion {
    #[serde(rename = "v1")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestReviewEventKind {
    Opened,
    Reopened,
    Synchronized,
    ReadyForReview,
    Closed,
}

impl PullRequestReviewEventKind {
    pub const ALL: [Self; 5] = [
        Self::Opened,
        Self::Reopened,
        Self::Synchronized,
        Self::ReadyForReview,
        Self::Closed,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestReviewEventProvider {
    Github,
    Bitbucket,
}

impl PullRequestReviewEventProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Bitbucket => "bitbucket",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestClosedOutcome {
    Merged,
    ClosedWithoutMerge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PullRequestRevision {
    pub ref_name: String,
    pub sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PullRequestEventActor {
    pub id: String,
    pub login: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PullRequestReviewEvent {
    pub schema_version: PullRequestReviewEventSchemaVersion,
    pub kind: PullRequestReviewEventKind,
    pub provider: PullRequestReviewEventProvider,
    pub tenant_id: String,
    /// GitHub organization or Bitbucket workspace.
    pub workspace: String,
    pub repository: String,
    pub pull_request_id: u64,
    pub base: PullRequestRevision,
    pub head: PullRequestRevision,
    pub draft: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_outcome: Option<PullRequestClosedOutcome>,
    pub actor: PullRequestEventActor,
    pub delivery_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestEventDeliveryKey {
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub delivery_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestReviewKey {
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub kind: PullRequestReviewEventKind,
    pub workspace: String,
    pub repository: String,
    pub pull_request_id: u64,
    pub head_sha: String,
    pub closed_outcome: Option<PullRequestClosedOutcome>,
}

impl PullRequestReviewEvent {
    pub fn validate(&self) -> Result<(), PullRequestReviewEventValidationError> {
        require_value("tenantId", &self.tenant_id)?;
        require_value("workspace", &self.workspace)?;
        require_value("repository", &self.repository)?;
        if self.pull_request_id == 0 {
            return Err(PullRequestReviewEventValidationError::InvalidPullRequestId);
        }
        require_value("base.refName", &self.base.ref_name)?;
        validate_sha("base.sha", &self.base.sha)?;
        require_value("head.refName", &self.head.ref_name)?;
        validate_sha("head.sha", &self.head.sha)?;
        match (self.kind, self.closed_outcome) {
            (PullRequestReviewEventKind::Closed, None) => {
                return Err(PullRequestReviewEventValidationError::MissingField(
                    "closedOutcome",
                ));
            }
            (PullRequestReviewEventKind::Closed, Some(_)) | (_, None) => {}
            (_, Some(_)) => {
                return Err(PullRequestReviewEventValidationError::UnexpectedClosedOutcome);
            }
        }
        require_value("actor.id", &self.actor.id)?;
        require_value("actor.login", &self.actor.login)?;
        require_value("deliveryId", &self.delivery_id)?;
        Ok(())
    }

    pub fn delivery_key(&self) -> PullRequestEventDeliveryKey {
        PullRequestEventDeliveryKey {
            tenant_id: self.tenant_id.clone(),
            provider: self.provider,
            delivery_id: self.delivery_id.clone(),
        }
    }

    pub fn review_key(&self) -> PullRequestReviewKey {
        PullRequestReviewKey {
            tenant_id: self.tenant_id.clone(),
            provider: self.provider,
            kind: self.kind,
            workspace: self.workspace.clone(),
            repository: self.repository.clone(),
            pull_request_id: self.pull_request_id,
            head_sha: self.head.sha.clone(),
            closed_outcome: self.closed_outcome,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullRequestReviewEventValidationError {
    MissingField(&'static str),
    InvalidPullRequestId,
    InvalidCommitSha(&'static str),
    UnexpectedClosedOutcome,
}

impl fmt::Display for PullRequestReviewEventValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => write!(formatter, "`{field}` must not be empty"),
            Self::InvalidPullRequestId => {
                formatter.write_str("`pullRequestId` must be a positive integer")
            }
            Self::InvalidCommitSha(field) => {
                write!(formatter, "`{field}` must be a full hexadecimal commit SHA")
            }
            Self::UnexpectedClosedOutcome => {
                formatter.write_str("`closedOutcome` is only valid for closed events")
            }
        }
    }
}

impl std::error::Error for PullRequestReviewEventValidationError {}

fn require_value(
    field: &'static str,
    value: &str,
) -> Result<(), PullRequestReviewEventValidationError> {
    if value.trim().is_empty() {
        Err(PullRequestReviewEventValidationError::MissingField(field))
    } else {
        Ok(())
    }
}

fn validate_sha(
    field: &'static str,
    value: &str,
) -> Result<(), PullRequestReviewEventValidationError> {
    if matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(PullRequestReviewEventValidationError::InvalidCommitSha(
            field,
        ))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        PullRequestClosedOutcome, PullRequestEventActor, PullRequestReviewEvent,
        PullRequestReviewEventKind, PullRequestReviewEventProvider,
        PullRequestReviewEventSchemaVersion, PullRequestReviewEventValidationError,
        PullRequestRevision,
    };

    const BASE_SHA: &str = "1111111111111111111111111111111111111111";
    const HEAD_SHA: &str = "2222222222222222222222222222222222222222";

    fn event(kind: PullRequestReviewEventKind) -> PullRequestReviewEvent {
        PullRequestReviewEvent {
            schema_version: PullRequestReviewEventSchemaVersion::V1,
            kind,
            provider: PullRequestReviewEventProvider::Github,
            tenant_id: "tenant-acme".to_string(),
            workspace: "acme".to_string(),
            repository: "payments".to_string(),
            pull_request_id: 42,
            base: PullRequestRevision {
                ref_name: "main".to_string(),
                sha: BASE_SHA.to_string(),
            },
            head: PullRequestRevision {
                ref_name: "feature/retry".to_string(),
                sha: HEAD_SHA.to_string(),
            },
            draft: false,
            closed_outcome: (kind == PullRequestReviewEventKind::Closed)
                .then_some(PullRequestClosedOutcome::Merged),
            actor: PullRequestEventActor {
                id: "user-7".to_string(),
                login: "octocat".to_string(),
                display_name: Some("Octo Cat".to_string()),
            },
            delivery_id: "delivery-123".to_string(),
        }
    }

    #[test]
    fn json_round_trip_covers_every_supported_event_kind() {
        for (index, kind) in PullRequestReviewEventKind::ALL.into_iter().enumerate() {
            let mut expected = event(kind);
            if index % 2 == 1 {
                expected.provider = PullRequestReviewEventProvider::Bitbucket;
            }

            let encoded = serde_json::to_string(&expected).expect("serialize review event");
            let decoded: PullRequestReviewEvent =
                serde_json::from_str(&encoded).expect("deserialize review event");

            assert_eq!(decoded, expected);
            decoded.validate().expect("round-tripped event is valid");
        }
    }

    #[test]
    fn v1_json_shape_is_stable_and_contains_no_secret_fields() {
        let value = serde_json::to_value(event(PullRequestReviewEventKind::Opened))
            .expect("serialize review event");

        assert_eq!(
            value,
            json!({
                "schemaVersion": "v1",
                "kind": "opened",
                "provider": "github",
                "tenantId": "tenant-acme",
                "workspace": "acme",
                "repository": "payments",
                "pullRequestId": 42,
                "base": {
                    "refName": "main",
                    "sha": BASE_SHA
                },
                "head": {
                    "refName": "feature/retry",
                    "sha": HEAD_SHA
                },
                "draft": false,
                "actor": {
                    "id": "user-7",
                    "login": "octocat",
                    "displayName": "Octo Cat"
                },
                "deliveryId": "delivery-123"
            })
        );
        let rendered = value.to_string().to_ascii_lowercase();
        assert!(!rendered.contains("credential"));
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains("token"));
        assert!(!rendered.contains("webhook"));
    }

    #[test]
    fn unknown_event_kind_is_rejected_without_panicking() {
        let mut value =
            serde_json::to_value(event(PullRequestReviewEventKind::Opened)).expect("serialize");
        value["kind"] = json!("labeled");

        let error = serde_json::from_value::<PullRequestReviewEvent>(value)
            .expect_err("unknown event kind must fail");

        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn unknown_fields_are_rejected_instead_of_carrying_webhook_secrets() {
        let mut value =
            serde_json::to_value(event(PullRequestReviewEventKind::Opened)).expect("serialize");
        value["webhookSecret"] = json!("must-not-cross-the-contract");

        let error = serde_json::from_value::<PullRequestReviewEvent>(value)
            .expect_err("unknown fields must fail");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn delivery_and_review_idempotency_keys_cover_distinct_replay_cases() {
        let event = event(PullRequestReviewEventKind::Synchronized);
        let delivery_key = event.delivery_key();
        let review_key = event.review_key();

        assert_eq!(delivery_key.delivery_id, "delivery-123");
        assert_eq!(delivery_key.tenant_id, "tenant-acme");
        assert_eq!(review_key.head_sha, HEAD_SHA);
        assert_eq!(review_key.tenant_id, "tenant-acme");
        assert_eq!(review_key.kind, PullRequestReviewEventKind::Synchronized);
        assert_eq!(review_key.pull_request_id, 42);

        let mut redelivery = event.clone();
        redelivery.delivery_id = "delivery-456".to_string();
        assert_ne!(redelivery.delivery_key(), delivery_key);
        assert_eq!(redelivery.review_key(), review_key);
    }

    #[test]
    fn closed_events_preserve_the_terminal_outcome() {
        let mut closed = event(PullRequestReviewEventKind::Closed);
        closed.closed_outcome = Some(PullRequestClosedOutcome::ClosedWithoutMerge);

        let value = serde_json::to_value(&closed).expect("serialize closed event");
        assert_eq!(value["closedOutcome"], "closed_without_merge");

        let decoded: PullRequestReviewEvent =
            serde_json::from_value(value).expect("deserialize closed event");
        assert_eq!(
            decoded.closed_outcome,
            Some(PullRequestClosedOutcome::ClosedWithoutMerge)
        );
        decoded.validate().expect("closed event is valid");
    }

    #[test]
    fn validation_rejects_missing_identity_invalid_sha_and_invalid_closed_outcome() {
        let mut missing_tenant = event(PullRequestReviewEventKind::Opened);
        missing_tenant.tenant_id = " ".to_string();
        assert_eq!(
            missing_tenant.validate(),
            Err(PullRequestReviewEventValidationError::MissingField(
                "tenantId"
            ))
        );

        let mut invalid_sha = event(PullRequestReviewEventKind::Opened);
        invalid_sha.head.sha = "not-a-sha".to_string();
        assert_eq!(
            invalid_sha.validate(),
            Err(PullRequestReviewEventValidationError::InvalidCommitSha(
                "head.sha"
            ))
        );

        let mut missing_closed_outcome = event(PullRequestReviewEventKind::Closed);
        missing_closed_outcome.closed_outcome = None;
        assert_eq!(
            missing_closed_outcome.validate(),
            Err(PullRequestReviewEventValidationError::MissingField(
                "closedOutcome"
            ))
        );

        let mut unexpected_closed_outcome = event(PullRequestReviewEventKind::Opened);
        unexpected_closed_outcome.closed_outcome = Some(PullRequestClosedOutcome::Merged);
        assert_eq!(
            unexpected_closed_outcome.validate(),
            Err(PullRequestReviewEventValidationError::UnexpectedClosedOutcome)
        );
    }
}
