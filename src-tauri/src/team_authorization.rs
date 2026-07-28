//! Provider-neutral identity, role, and authorization boundary for team operations.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::administrative_audit::{
    validate_identifier, AdministrativeAuditAction, AdministrativeAuditActor,
    AdministrativeAuditActorKind, AdministrativeAuditEvent, AdministrativeAuditOutcome,
    AdministrativeAuditRepositoryScope, AdministrativeAuditSchemaVersion,
    AdministrativeAuditTarget, AdministrativeAuditTargetKind,
};
use crate::review_event::PullRequestReviewEventProvider;
use crate::review_storage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamAuthorizationSchemaVersion {
    #[serde(rename = "v1")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamActorKind {
    User,
    ServiceAccount,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamRole {
    Admin,
    Member,
    Viewer,
    ServiceAccount,
    #[serde(other)]
    Unknown,
}

impl TeamRole {
    pub const ALL: [Self; 4] = [
        Self::Admin,
        Self::Member,
        Self::Viewer,
        Self::ServiceAccount,
    ];

    pub const fn allows(self, permission: TeamPermission) -> bool {
        match self {
            Self::Admin => permission.is_known(),
            Self::Member => matches!(
                permission,
                TeamPermission::TriggerReview
                    | TeamPermission::RecordFindingFeedback
                    | TeamPermission::PublishReview
                    | TeamPermission::ReadMetrics
            ),
            Self::Viewer => matches!(permission, TeamPermission::ReadMetrics),
            Self::ServiceAccount => matches!(
                permission,
                TeamPermission::TriggerReview | TeamPermission::PublishReview
            ),
            Self::Unknown => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamPermission {
    ManagePolicy,
    EnrollRepository,
    TriggerReview,
    RecordFindingFeedback,
    PublishReview,
    ReadMetrics,
    ExportAudit,
    #[serde(other)]
    Unknown,
}

impl TeamPermission {
    pub const ALL: [Self; 7] = [
        Self::ManagePolicy,
        Self::EnrollRepository,
        Self::TriggerReview,
        Self::RecordFindingFeedback,
        Self::PublishReview,
        Self::ReadMetrics,
        Self::ExportAudit,
    ];

    const fn is_known(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamOperation {
    AdministerPolicy,
    EnrollRepository,
    TriggerReview,
    RecordFindingFeedback,
    PublishReview,
    ReadMetrics,
    ExportAudit,
    #[serde(other)]
    Unknown,
}

impl TeamOperation {
    pub const ALL: [Self; 7] = [
        Self::AdministerPolicy,
        Self::EnrollRepository,
        Self::TriggerReview,
        Self::RecordFindingFeedback,
        Self::PublishReview,
        Self::ReadMetrics,
        Self::ExportAudit,
    ];

    pub const fn required_permission(self) -> TeamPermission {
        match self {
            Self::AdministerPolicy => TeamPermission::ManagePolicy,
            Self::EnrollRepository => TeamPermission::EnrollRepository,
            Self::TriggerReview => TeamPermission::TriggerReview,
            Self::RecordFindingFeedback => TeamPermission::RecordFindingFeedback,
            Self::PublishReview => TeamPermission::PublishReview,
            Self::ReadMetrics => TeamPermission::ReadMetrics,
            Self::ExportAudit => TeamPermission::ExportAudit,
            Self::Unknown => TeamPermission::Unknown,
        }
    }

    const fn requires_repository(self) -> bool {
        matches!(
            self,
            Self::EnrollRepository
                | Self::TriggerReview
                | Self::RecordFindingFeedback
                | Self::PublishReview
        )
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::AdministerPolicy => "administer-policy",
            Self::EnrollRepository => "enroll-repository",
            Self::TriggerReview => "trigger-review",
            Self::RecordFindingFeedback => "record-finding-feedback",
            Self::PublishReview => "publish-review",
            Self::ReadMetrics => "read-metrics",
            Self::ExportAudit => "export-audit",
            Self::Unknown => "unknown-operation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamOrganization {
    pub id: String,
}

impl TeamOrganization {
    pub fn local() -> Self {
        Self {
            id: "local".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamIdentity {
    pub id: String,
    pub organization_id: String,
}

impl TeamIdentity {
    pub fn local() -> Self {
        Self {
            id: "local".to_string(),
            organization_id: "local".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamActor {
    pub id: String,
    pub kind: TeamActorKind,
    pub organization_id: String,
    pub team_ids: Vec<String>,
    pub role: TeamRole,
}

impl TeamActor {
    pub fn local_single_user() -> Self {
        Self {
            id: "system:local-single-user".to_string(),
            kind: TeamActorKind::Local,
            organization_id: "local".to_string(),
            team_ids: vec!["local".to_string()],
            role: TeamRole::Admin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamRepositoryScope {
    pub organization_id: String,
    pub team_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repo: String,
}

impl TeamRepositoryScope {
    pub fn local(provider: PullRequestReviewEventProvider, workspace: &str, repo: &str) -> Self {
        Self {
            organization_id: "local".to_string(),
            team_id: "local".to_string(),
            provider,
            workspace: workspace.to_string(),
            repo: repo.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamAuthorizationAuditContext {
    pub attempt_id: String,
    pub occurred_at_ms: u64,
    pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamAuthorizationRequest {
    pub schema_version: TeamAuthorizationSchemaVersion,
    pub actor: TeamActor,
    pub organization: TeamOrganization,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<TeamIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<TeamRepositoryScope>,
    pub operation: TeamOperation,
    pub audit: TeamAuthorizationAuditContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamAuthorizationDeniedReason {
    UnknownRole,
    UnknownOperation,
    PermissionDenied,
    OrganizationMismatch,
    TeamMismatch,
    TeamMembershipRequired,
    RepositoryScopeRequired,
}

impl TeamAuthorizationDeniedReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownRole => "unknown-role",
            Self::UnknownOperation => "unknown-operation",
            Self::PermissionDenied => "permission-denied",
            Self::OrganizationMismatch => "organization-mismatch",
            Self::TeamMismatch => "team-mismatch",
            Self::TeamMembershipRequired => "team-membership-required",
            Self::RepositoryScopeRequired => "repository-scope-required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum TeamAuthorizationDecision {
    Allowed {
        permission: TeamPermission,
    },
    Denied {
        permission: TeamPermission,
        reason: TeamAuthorizationDeniedReason,
    },
}

pub trait TeamAuthorizationAuditSink {
    fn record_denied(&self, event: &AdministrativeAuditEvent) -> Result<(), String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SqliteTeamAuthorizationAuditSink;

impl TeamAuthorizationAuditSink for SqliteTeamAuthorizationAuditSink {
    fn record_denied(&self, event: &AdministrativeAuditEvent) -> Result<(), String> {
        review_storage::append_administrative_audit_event(event).map(|_| ())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamAuthorizationError {
    InvalidRequest(&'static str),
    AuditFailed,
}

impl fmt::Display for TeamAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(field) => {
                write!(formatter, "`{field}` is not a valid authorization field")
            }
            Self::AuditFailed => {
                formatter.write_str("Denied authorization could not be recorded safely")
            }
        }
    }
}

impl std::error::Error for TeamAuthorizationError {}

pub fn authorize_team_operation(
    request: &TeamAuthorizationRequest,
    audit_sink: &dyn TeamAuthorizationAuditSink,
) -> Result<TeamAuthorizationDecision, TeamAuthorizationError> {
    validate_request(request)?;
    let permission = request.operation.required_permission();
    let denial = denied_reason(request, permission);
    let Some(reason) = denial else {
        return Ok(TeamAuthorizationDecision::Allowed { permission });
    };

    let event = authorization_denied_audit_event(request, permission, reason)
        .prepare_for_storage()
        .map_err(|_| TeamAuthorizationError::AuditFailed)?;
    audit_sink
        .record_denied(&event)
        .map_err(|_| TeamAuthorizationError::AuditFailed)?;
    Ok(TeamAuthorizationDecision::Denied { permission, reason })
}

fn validate_request(request: &TeamAuthorizationRequest) -> Result<(), TeamAuthorizationError> {
    for (field, value) in [
        ("actor.id", request.actor.id.as_str()),
        (
            "actor.organizationId",
            request.actor.organization_id.as_str(),
        ),
        ("organization.id", request.organization.id.as_str()),
        ("audit.attemptId", request.audit.attempt_id.as_str()),
    ] {
        validate_identifier(field, value)
            .map_err(|_| TeamAuthorizationError::InvalidRequest(field))?;
    }
    if request.audit.correlation_id.trim().is_empty() {
        return Err(TeamAuthorizationError::InvalidRequest(
            "audit.correlationId",
        ));
    }
    for team_id in &request.actor.team_ids {
        validate_identifier("actor.teamIds", team_id)
            .map_err(|_| TeamAuthorizationError::InvalidRequest("actor.teamIds"))?;
    }
    if let Some(team) = &request.team {
        for (field, value) in [
            ("team.id", team.id.as_str()),
            ("team.organizationId", team.organization_id.as_str()),
        ] {
            validate_identifier(field, value)
                .map_err(|_| TeamAuthorizationError::InvalidRequest(field))?;
        }
    }
    if let Some(repository) = &request.repository {
        for (field, value) in [
            (
                "repository.organizationId",
                repository.organization_id.as_str(),
            ),
            ("repository.teamId", repository.team_id.as_str()),
            ("repository.workspace", repository.workspace.as_str()),
            ("repository.repo", repository.repo.as_str()),
        ] {
            validate_identifier(field, value)
                .map_err(|_| TeamAuthorizationError::InvalidRequest(field))?;
        }
    }
    validate_actor_kind(request)
}

fn validate_actor_kind(request: &TeamAuthorizationRequest) -> Result<(), TeamAuthorizationError> {
    let valid = match request.actor.kind {
        TeamActorKind::User => {
            request.actor.id.starts_with("user:") && request.actor.role != TeamRole::ServiceAccount
        }
        TeamActorKind::ServiceAccount => {
            request.actor.id.starts_with("service:")
                && request.actor.role == TeamRole::ServiceAccount
        }
        TeamActorKind::Local => {
            request.actor == TeamActor::local_single_user()
                && request.organization == TeamOrganization::local()
        }
    };
    if valid {
        Ok(())
    } else {
        Err(TeamAuthorizationError::InvalidRequest("actor.kind"))
    }
}

fn denied_reason(
    request: &TeamAuthorizationRequest,
    permission: TeamPermission,
) -> Option<TeamAuthorizationDeniedReason> {
    if request.actor.organization_id != request.organization.id {
        return Some(TeamAuthorizationDeniedReason::OrganizationMismatch);
    }
    if request.actor.role == TeamRole::Unknown {
        return Some(TeamAuthorizationDeniedReason::UnknownRole);
    }
    if permission == TeamPermission::Unknown {
        return Some(TeamAuthorizationDeniedReason::UnknownOperation);
    }
    if request.operation.requires_repository() && request.repository.is_none() {
        return Some(TeamAuthorizationDeniedReason::RepositoryScopeRequired);
    }
    if let Some(team) = &request.team {
        if team.organization_id != request.organization.id {
            return Some(TeamAuthorizationDeniedReason::OrganizationMismatch);
        }
        if request.actor.role != TeamRole::Admin && !request.actor.team_ids.contains(&team.id) {
            return Some(TeamAuthorizationDeniedReason::TeamMembershipRequired);
        }
    }
    if let Some(repository) = &request.repository {
        if repository.organization_id != request.organization.id {
            return Some(TeamAuthorizationDeniedReason::OrganizationMismatch);
        }
        let Some(team) = &request.team else {
            return Some(TeamAuthorizationDeniedReason::TeamMismatch);
        };
        if repository.team_id != team.id {
            return Some(TeamAuthorizationDeniedReason::TeamMismatch);
        }
    }
    (!request.actor.role.allows(permission))
        .then_some(TeamAuthorizationDeniedReason::PermissionDenied)
}

fn authorization_denied_audit_event(
    request: &TeamAuthorizationRequest,
    permission: TeamPermission,
    reason: TeamAuthorizationDeniedReason,
) -> AdministrativeAuditEvent {
    AdministrativeAuditEvent {
        schema_version: AdministrativeAuditSchemaVersion::V1,
        delivery_id: format!("authorization-denied:{}", request.audit.attempt_id),
        tenant_id: request.organization.id.clone(),
        occurred_at: request.audit.occurred_at_ms.to_string(),
        actor: AdministrativeAuditActor {
            kind: match request.actor.kind {
                TeamActorKind::User => AdministrativeAuditActorKind::User,
                TeamActorKind::ServiceAccount => AdministrativeAuditActorKind::Service,
                TeamActorKind::Local => AdministrativeAuditActorKind::System,
            },
            id: request.actor.id.clone(),
        },
        repository: request.repository.as_ref().map(|repository| {
            AdministrativeAuditRepositoryScope {
                provider: repository.provider,
                workspace: repository.workspace.clone(),
                repo: repository.repo.clone(),
                pr_id: None,
            }
        }),
        action: AdministrativeAuditAction::AuthorizationDenied,
        target: AdministrativeAuditTarget {
            kind: AdministrativeAuditTargetKind::AuthorizationRequest,
            id: format!(
                "authorization:{}:{}:{}",
                request.operation.as_str(),
                permission_name(permission),
                reason.as_str()
            ),
        },
        outcome: AdministrativeAuditOutcome::Denied,
        correlation_id: request.audit.correlation_id.clone(),
    }
}

const fn permission_name(permission: TeamPermission) -> &'static str {
    match permission {
        TeamPermission::ManagePolicy => "manage-policy",
        TeamPermission::EnrollRepository => "enroll-repository",
        TeamPermission::TriggerReview => "trigger-review",
        TeamPermission::RecordFindingFeedback => "record-finding-feedback",
        TeamPermission::PublishReview => "publish-review",
        TeamPermission::ReadMetrics => "read-metrics",
        TeamPermission::ExportAudit => "export-audit",
        TeamPermission::Unknown => "unknown-permission",
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use serde_json::json;

    use super::*;

    #[derive(Default)]
    struct MemoryAuditSink {
        events: RefCell<Vec<AdministrativeAuditEvent>>,
        fail: bool,
    }

    impl TeamAuthorizationAuditSink for MemoryAuditSink {
        fn record_denied(&self, event: &AdministrativeAuditEvent) -> Result<(), String> {
            if self.fail {
                return Err("Bearer audit-secret".to_string());
            }
            self.events.borrow_mut().push(event.clone());
            Ok(())
        }
    }

    fn actor(role: TeamRole) -> TeamActor {
        match role {
            TeamRole::ServiceAccount => TeamActor {
                id: "service:review-worker".to_string(),
                kind: TeamActorKind::ServiceAccount,
                organization_id: "tenant-acme".to_string(),
                team_ids: vec!["team-payments".to_string()],
                role,
            },
            _ => TeamActor {
                id: "user:reviewer-1".to_string(),
                kind: TeamActorKind::User,
                organization_id: "tenant-acme".to_string(),
                team_ids: vec!["team-payments".to_string()],
                role,
            },
        }
    }

    fn request(role: TeamRole, operation: TeamOperation) -> TeamAuthorizationRequest {
        let repository_required =
            operation.requires_repository() || operation == TeamOperation::ReadMetrics;
        TeamAuthorizationRequest {
            schema_version: TeamAuthorizationSchemaVersion::V1,
            actor: actor(role),
            organization: TeamOrganization {
                id: "tenant-acme".to_string(),
            },
            team: repository_required.then(|| TeamIdentity {
                id: "team-payments".to_string(),
                organization_id: "tenant-acme".to_string(),
            }),
            repository: repository_required.then(|| TeamRepositoryScope {
                organization_id: "tenant-acme".to_string(),
                team_id: "team-payments".to_string(),
                provider: PullRequestReviewEventProvider::Github,
                workspace: "acme".to_string(),
                repo: "payments".to_string(),
            }),
            operation,
            audit: TeamAuthorizationAuditContext {
                attempt_id: "attempt-1".to_string(),
                occurred_at_ms: 1_000,
                correlation_id: "correlation:1".to_string(),
            },
        }
    }

    fn expected(role: TeamRole, operation: TeamOperation) -> bool {
        match role {
            TeamRole::Admin => true,
            TeamRole::Member => matches!(
                operation,
                TeamOperation::TriggerReview
                    | TeamOperation::RecordFindingFeedback
                    | TeamOperation::PublishReview
                    | TeamOperation::ReadMetrics
            ),
            TeamRole::Viewer => matches!(operation, TeamOperation::ReadMetrics),
            TeamRole::ServiceAccount => matches!(
                operation,
                TeamOperation::TriggerReview | TeamOperation::PublishReview
            ),
            TeamRole::Unknown => false,
        }
    }

    #[test]
    fn every_role_operation_combination_matches_the_explicit_permission_matrix() {
        let mut combinations = 0;
        for role in TeamRole::ALL {
            for operation in TeamOperation::ALL {
                combinations += 1;
                let sink = MemoryAuditSink::default();
                let decision =
                    authorize_team_operation(&request(role, operation), &sink).expect("decision");
                assert_eq!(
                    matches!(decision, TeamAuthorizationDecision::Allowed { .. }),
                    expected(role, operation),
                    "{role:?} / {operation:?}"
                );
                assert_eq!(
                    sink.events.borrow().len(),
                    usize::from(!expected(role, operation))
                );
            }
        }
        assert_eq!(combinations, TeamRole::ALL.len() * TeamOperation::ALL.len());
        assert_eq!(combinations, 28);
    }

    #[test]
    fn every_operation_maps_to_one_known_permission() {
        let permissions = TeamOperation::ALL.map(TeamOperation::required_permission);
        assert_eq!(permissions, TeamPermission::ALL);
        assert!(!TeamRole::Admin.allows(TeamPermission::Unknown));
    }

    #[test]
    fn unknown_roles_operations_and_permissions_default_to_deny() {
        let unknown_role: TeamRole =
            serde_json::from_value(json!("owner")).expect("forward-compatible role");
        let unknown_operation: TeamOperation =
            serde_json::from_value(json!("delete_tenant")).expect("forward-compatible operation");
        let unknown_permission: TeamPermission =
            serde_json::from_value(json!("manage_billing")).expect("forward-compatible permission");
        assert_eq!(unknown_role, TeamRole::Unknown);
        assert_eq!(unknown_operation, TeamOperation::Unknown);
        assert_eq!(unknown_permission, TeamPermission::Unknown);
        assert!(!TeamRole::Admin.allows(unknown_permission));

        let sink = MemoryAuditSink::default();
        let role_decision =
            authorize_team_operation(&request(unknown_role, TeamOperation::ReadMetrics), &sink)
                .expect("unknown role decision");
        assert!(matches!(
            role_decision,
            TeamAuthorizationDecision::Denied {
                reason: TeamAuthorizationDeniedReason::UnknownRole,
                ..
            }
        ));

        let operation_decision =
            authorize_team_operation(&request(TeamRole::Admin, unknown_operation), &sink)
                .expect("unknown operation decision");
        assert!(matches!(
            operation_decision,
            TeamAuthorizationDecision::Denied {
                reason: TeamAuthorizationDeniedReason::UnknownOperation,
                ..
            }
        ));
    }

    #[test]
    fn organization_team_and_repository_scope_must_agree() {
        let cases = [
            (
                {
                    let mut value = request(TeamRole::Member, TeamOperation::TriggerReview);
                    value.organization.id = "tenant-other".to_string();
                    value
                },
                TeamAuthorizationDeniedReason::OrganizationMismatch,
            ),
            (
                {
                    let mut value = request(TeamRole::Member, TeamOperation::TriggerReview);
                    value
                        .repository
                        .as_mut()
                        .expect("repository")
                        .organization_id = "tenant-other".to_string();
                    value
                },
                TeamAuthorizationDeniedReason::OrganizationMismatch,
            ),
            (
                {
                    let mut value = request(TeamRole::Member, TeamOperation::TriggerReview);
                    value.repository.as_mut().expect("repository").team_id =
                        "team-other".to_string();
                    value
                },
                TeamAuthorizationDeniedReason::TeamMismatch,
            ),
            (
                {
                    let mut value = request(TeamRole::Member, TeamOperation::TriggerReview);
                    value.actor.team_ids.clear();
                    value
                },
                TeamAuthorizationDeniedReason::TeamMembershipRequired,
            ),
        ];

        for (request, expected_reason) in cases {
            let sink = MemoryAuditSink::default();
            let decision = authorize_team_operation(&request, &sink).expect("scope decision");
            assert!(matches!(
                decision,
                TeamAuthorizationDecision::Denied { reason, .. } if reason == expected_reason
            ));
            assert_eq!(sink.events.borrow().len(), 1);
        }
    }

    #[test]
    fn repository_operations_fail_closed_without_repository_scope() {
        let mut request = request(TeamRole::Admin, TeamOperation::PublishReview);
        request.repository = None;
        request.team = None;
        let sink = MemoryAuditSink::default();

        let decision = authorize_team_operation(&request, &sink).expect("scope decision");

        assert!(matches!(
            decision,
            TeamAuthorizationDecision::Denied {
                reason: TeamAuthorizationDeniedReason::RepositoryScopeRequired,
                ..
            }
        ));
        let events = sink.events.borrow();
        assert!(events[0].repository.is_none());
        assert!(serde_json::to_value(&events[0])
            .expect("serialize organization audit")
            .get("repository")
            .is_none());
    }

    #[test]
    fn denied_actions_are_redacted_before_reaching_the_audit_sink() {
        let mut request = request(TeamRole::Viewer, TeamOperation::PublishReview);
        request.audit.correlation_id = "Bearer super-secret-token".to_string();
        let sink = MemoryAuditSink::default();

        let decision = authorize_team_operation(&request, &sink).expect("denied decision");

        assert!(matches!(decision, TeamAuthorizationDecision::Denied { .. }));
        let event = &sink.events.borrow()[0];
        let json = serde_json::to_string(event).expect("serialize event");
        assert_eq!(event.action, AdministrativeAuditAction::AuthorizationDenied);
        assert_eq!(event.outcome, AdministrativeAuditOutcome::Denied);
        assert_eq!(
            event.correlation_id,
            crate::administrative_audit::REDACTED_AUDIT_VALUE
        );
        assert!(!json.contains("super-secret-token"));
        assert!(!json.contains("Bearer"));
    }

    #[test]
    fn audit_failure_never_turns_a_denial_into_an_allow() {
        let sink = MemoryAuditSink {
            events: RefCell::default(),
            fail: true,
        };

        let error = authorize_team_operation(
            &request(TeamRole::Viewer, TeamOperation::PublishReview),
            &sink,
        )
        .expect_err("audit failure");

        assert_eq!(error, TeamAuthorizationError::AuditFailed);
        assert!(!error.to_string().contains("audit-secret"));
    }

    #[test]
    fn local_single_user_mode_needs_no_identity_provider() {
        for operation in TeamOperation::ALL {
            let repository_required =
                operation.requires_repository() || operation == TeamOperation::ReadMetrics;
            let request = TeamAuthorizationRequest {
                schema_version: TeamAuthorizationSchemaVersion::V1,
                actor: TeamActor::local_single_user(),
                organization: TeamOrganization::local(),
                team: repository_required.then(TeamIdentity::local),
                repository: repository_required.then(|| {
                    TeamRepositoryScope::local(
                        PullRequestReviewEventProvider::Github,
                        "local",
                        "repository",
                    )
                }),
                operation,
                audit: TeamAuthorizationAuditContext {
                    attempt_id: format!("local-{operation:?}").to_ascii_lowercase(),
                    occurred_at_ms: 1_000,
                    correlation_id: "correlation:local".to_string(),
                },
            };
            let sink = MemoryAuditSink::default();
            assert!(matches!(
                authorize_team_operation(&request, &sink).expect("local decision"),
                TeamAuthorizationDecision::Allowed { .. }
            ));
            assert!(sink.events.borrow().is_empty());
        }
    }

    #[test]
    fn v1_request_shape_is_strict_and_provider_neutral() {
        let request = request(TeamRole::Member, TeamOperation::TriggerReview);
        let value = serde_json::to_value(&request).expect("serialize request");
        assert_eq!(value["schemaVersion"], "v1");
        assert_eq!(value["actor"]["role"], "member");
        assert_eq!(value["operation"], "trigger_review");
        assert_eq!(value["repository"]["provider"], "github");
        assert!(value.get("issuer").is_none());
        assert!(value.get("accessToken").is_none());

        let mut unknown = value;
        unknown["accessToken"] = json!("secret");
        assert!(serde_json::from_value::<TeamAuthorizationRequest>(unknown).is_err());
    }
}
