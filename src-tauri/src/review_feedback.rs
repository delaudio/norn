//! Provider-neutral, append-only reviewer feedback for structured findings.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::review_event::PullRequestReviewEventProvider;

const MAX_IDENTIFIER_BYTES: usize = 512;
const MAX_REASON_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewFindingFeedbackIdentity {
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repo: String,
    pub pr_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewFindingFeedbackTarget {
    pub identity: ReviewFindingFeedbackIdentity,
    pub review_run_id: String,
    pub finding_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFindingFeedbackAction {
    Accepted,
    Dismissed,
    FalsePositive,
    Fixed,
    Reopened,
}

impl ReviewFindingFeedbackAction {
    pub const ALL: [Self; 5] = [
        Self::Accepted,
        Self::Dismissed,
        Self::FalsePositive,
        Self::Fixed,
        Self::Reopened,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Dismissed => "dismissed",
            Self::FalsePositive => "false_positive",
            Self::Fixed => "fixed",
            Self::Reopened => "reopened",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "accepted" => Some(Self::Accepted),
            "dismissed" => Some(Self::Dismissed),
            "false_positive" => Some(Self::FalsePositive),
            "fixed" => Some(Self::Fixed),
            "reopened" => Some(Self::Reopened),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFindingDisposition {
    Open,
    Accepted,
    Dismissed,
    FalsePositive,
    Fixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewFindingFeedbackEvent {
    /// Tenant-unique delivery key used to make retries idempotent.
    pub event_id: String,
    pub identity: ReviewFindingFeedbackIdentity,
    pub review_run_id: String,
    pub finding_fingerprint: String,
    pub action: ReviewFindingFeedbackAction,
    /// Milliseconds since Unix epoch, represented as a decimal string.
    pub occurred_at: String,
    pub actor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFindingFeedbackState {
    pub disposition: ReviewFindingDisposition,
    pub latest_event: Option<ReviewFindingFeedbackEvent>,
    pub events: Vec<ReviewFindingFeedbackEvent>,
}

impl ReviewFindingFeedbackEvent {
    pub fn target(&self) -> ReviewFindingFeedbackTarget {
        ReviewFindingFeedbackTarget {
            identity: self.identity.clone(),
            review_run_id: self.review_run_id.clone(),
            finding_fingerprint: self.finding_fingerprint.clone(),
        }
    }

    pub fn validate(&self) -> Result<(), ReviewFindingFeedbackValidationError> {
        validate_identifier("eventId", &self.event_id)?;
        self.target().validate()?;
        validate_identifier("actorId", &self.actor_id)?;
        parse_timestamp(&self.occurred_at)?;
        if let Some(reason) = &self.reason {
            if reason.trim().is_empty() {
                return Err(ReviewFindingFeedbackValidationError::EmptyReason);
            }
            if reason.len() > MAX_REASON_BYTES {
                return Err(ReviewFindingFeedbackValidationError::FieldTooLong("reason"));
            }
        }
        Ok(())
    }
}

impl ReviewFindingFeedbackTarget {
    pub fn validate(&self) -> Result<(), ReviewFindingFeedbackValidationError> {
        validate_identity(&self.identity)?;
        validate_identifier("reviewRunId", &self.review_run_id)?;
        validate_identifier("findingFingerprint", &self.finding_fingerprint)?;
        Ok(())
    }
}

pub fn derive_finding_feedback_state(
    mut events: Vec<ReviewFindingFeedbackEvent>,
) -> Result<ReviewFindingFeedbackState, ReviewFindingFeedbackValidationError> {
    for event in &events {
        event.validate()?;
    }
    if let Some(first) = events.first() {
        if events.iter().any(|event| {
            event.identity != first.identity
                || event.review_run_id != first.review_run_id
                || event.finding_fingerprint != first.finding_fingerprint
        }) {
            return Err(ReviewFindingFeedbackValidationError::MixedFindingTargets);
        }
    }
    events.sort_by(|left, right| {
        timestamp_value(left)
            .cmp(&timestamp_value(right))
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    let latest_event = events.last().cloned();
    let disposition = latest_event
        .as_ref()
        .map(|event| match event.action {
            ReviewFindingFeedbackAction::Accepted => ReviewFindingDisposition::Accepted,
            ReviewFindingFeedbackAction::Dismissed => ReviewFindingDisposition::Dismissed,
            ReviewFindingFeedbackAction::FalsePositive => ReviewFindingDisposition::FalsePositive,
            ReviewFindingFeedbackAction::Fixed => ReviewFindingDisposition::Fixed,
            ReviewFindingFeedbackAction::Reopened => ReviewFindingDisposition::Open,
        })
        .unwrap_or(ReviewFindingDisposition::Open);
    Ok(ReviewFindingFeedbackState {
        disposition,
        latest_event,
        events,
    })
}

fn validate_identity(
    identity: &ReviewFindingFeedbackIdentity,
) -> Result<(), ReviewFindingFeedbackValidationError> {
    validate_identifier("tenantId", &identity.tenant_id)?;
    validate_identifier("workspace", &identity.workspace)?;
    validate_identifier("repo", &identity.repo)?;
    if identity.pr_id == 0 {
        return Err(ReviewFindingFeedbackValidationError::InvalidPullRequestId);
    }
    Ok(())
}

fn validate_identifier(
    field: &'static str,
    value: &str,
) -> Result<(), ReviewFindingFeedbackValidationError> {
    if value.trim().is_empty() {
        return Err(ReviewFindingFeedbackValidationError::MissingField(field));
    }
    if value.len() > MAX_IDENTIFIER_BYTES {
        return Err(ReviewFindingFeedbackValidationError::FieldTooLong(field));
    }
    Ok(())
}

fn parse_timestamp(value: &str) -> Result<i64, ReviewFindingFeedbackValidationError> {
    let timestamp = value
        .parse::<i64>()
        .map_err(|_| ReviewFindingFeedbackValidationError::InvalidTimestamp)?;
    if timestamp < 0 {
        return Err(ReviewFindingFeedbackValidationError::InvalidTimestamp);
    }
    Ok(timestamp)
}

fn timestamp_value(event: &ReviewFindingFeedbackEvent) -> i64 {
    event
        .occurred_at
        .parse()
        .expect("validated feedback timestamps must parse")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewFindingFeedbackValidationError {
    MissingField(&'static str),
    FieldTooLong(&'static str),
    InvalidPullRequestId,
    InvalidTimestamp,
    EmptyReason,
    MixedFindingTargets,
}

impl fmt::Display for ReviewFindingFeedbackValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => write!(formatter, "`{field}` must not be empty"),
            Self::FieldTooLong(field) => write!(formatter, "`{field}` exceeds the supported size"),
            Self::InvalidPullRequestId => formatter.write_str("`prId` must be a positive integer"),
            Self::InvalidTimestamp => formatter
                .write_str("`occurredAt` must be a non-negative Unix timestamp in milliseconds"),
            Self::EmptyReason => formatter.write_str("`reason` must not be empty when provided"),
            Self::MixedFindingTargets => {
                formatter.write_str("feedback events must target the same finding")
            }
        }
    }
}

impl std::error::Error for ReviewFindingFeedbackValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        event_id: &str,
        action: ReviewFindingFeedbackAction,
        occurred_at: &str,
    ) -> ReviewFindingFeedbackEvent {
        ReviewFindingFeedbackEvent {
            event_id: event_id.to_string(),
            identity: ReviewFindingFeedbackIdentity {
                tenant_id: "tenant-acme".to_string(),
                provider: PullRequestReviewEventProvider::Github,
                workspace: "acme".to_string(),
                repo: "payments".to_string(),
                pr_id: 42,
            },
            review_run_id: "run-1".to_string(),
            finding_fingerprint: "finding-abc".to_string(),
            action,
            occurred_at: occurred_at.to_string(),
            actor_id: "reviewer-1".to_string(),
            reason: None,
        }
    }

    #[test]
    fn every_feedback_action_maps_to_a_disposition() {
        let expected = [
            ReviewFindingDisposition::Accepted,
            ReviewFindingDisposition::Dismissed,
            ReviewFindingDisposition::FalsePositive,
            ReviewFindingDisposition::Fixed,
            ReviewFindingDisposition::Open,
        ];
        for (action, disposition) in ReviewFindingFeedbackAction::ALL.into_iter().zip(expected) {
            let state = derive_finding_feedback_state(vec![event("event-1", action, "1000")])
                .expect("state");
            assert_eq!(state.disposition, disposition);
        }
    }

    #[test]
    fn event_time_then_event_id_deterministically_orders_conflicts() {
        let state = derive_finding_feedback_state(vec![
            event("event-z", ReviewFindingFeedbackAction::Accepted, "1000"),
            event("event-a", ReviewFindingFeedbackAction::Fixed, "1000"),
            event(
                "event-middle",
                ReviewFindingFeedbackAction::Reopened,
                "2000",
            ),
        ])
        .expect("state");

        assert_eq!(state.disposition, ReviewFindingDisposition::Open);
        assert_eq!(
            state
                .latest_event
                .as_ref()
                .map(|event| event.event_id.as_str()),
            Some("event-middle")
        );
        assert_eq!(state.events[0].event_id, "event-a");
        assert_eq!(state.events[1].event_id, "event-z");
    }

    #[test]
    fn validation_rejects_mixed_targets_and_invalid_metadata() {
        let mut mixed = event("event-2", ReviewFindingFeedbackAction::Accepted, "1000");
        mixed.finding_fingerprint = "another-finding".to_string();
        assert_eq!(
            derive_finding_feedback_state(vec![
                event("event-1", ReviewFindingFeedbackAction::Accepted, "1000"),
                mixed
            ]),
            Err(ReviewFindingFeedbackValidationError::MixedFindingTargets)
        );

        let mut invalid = event("event-1", ReviewFindingFeedbackAction::Accepted, "-1");
        assert_eq!(
            invalid.validate(),
            Err(ReviewFindingFeedbackValidationError::InvalidTimestamp)
        );
        invalid.occurred_at = "1000".to_string();
        invalid.reason = Some(" ".to_string());
        assert_eq!(
            invalid.validate(),
            Err(ReviewFindingFeedbackValidationError::EmptyReason)
        );
    }
}
