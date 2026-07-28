//! Provider-neutral identity, role, and authorization boundary for team operations.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::administrative_audit::{
    validate_identifier, validate_timestamp, AdministrativeAuditAction, AdministrativeAuditActor,
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

    const fn supports_repository_scope(self) -> bool {
        self.requires_repository() || matches!(self, Self::ReadMetrics | Self::Unknown)
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

/// Principal produced by a trusted authentication adapter, never by request decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamActor {
    id: String,
    kind: TeamActorKind,
    organization_id: String,
    team_ids: Vec<String>,
    role: TeamRole,
}

impl TeamActor {
    pub fn from_authenticated_claims(
        id: String,
        kind: TeamActorKind,
        organization_id: String,
        team_ids: Vec<String>,
        role: TeamRole,
    ) -> Result<Self, TeamAuthorizationError> {
        let actor = Self {
            id,
            kind,
            organization_id,
            team_ids,
            role,
        };
        validate_actor(&actor)?;
        Ok(actor)
    }

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

/// Authorizes untrusted operation input against a separately authenticated principal.
pub fn authorize_team_operation(
    authenticated_actor: &TeamActor,
    request: &TeamAuthorizationRequest,
    audit_sink: &dyn TeamAuthorizationAuditSink,
) -> Result<TeamAuthorizationDecision, TeamAuthorizationError> {
    validate_request(authenticated_actor, request)?;
    let permission = request.operation.required_permission();
    let denial = denied_reason(authenticated_actor, request, permission);
    let Some(reason) = denial else {
        return Ok(TeamAuthorizationDecision::Allowed { permission });
    };

    let event = authorization_denied_audit_event(authenticated_actor, request, permission, reason)
        .prepare_for_storage()
        .map_err(|_| TeamAuthorizationError::AuditFailed)?;
    audit_sink
        .record_denied(&event)
        .map_err(|_| TeamAuthorizationError::AuditFailed)?;
    Ok(TeamAuthorizationDecision::Denied { permission, reason })
}

fn validate_request(
    authenticated_actor: &TeamActor,
    request: &TeamAuthorizationRequest,
) -> Result<(), TeamAuthorizationError> {
    validate_actor(authenticated_actor)?;
    for (field, value) in [
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
    validate_timestamp(&request.audit.occurred_at_ms.to_string())
        .map_err(|_| TeamAuthorizationError::InvalidRequest("audit.occurredAtMs"))?;
    validate_identifier(
        "audit.attemptId",
        &format!("authorization-denied:{}", request.audit.attempt_id),
    )
    .map_err(|_| TeamAuthorizationError::InvalidRequest("audit.attemptId"))?;
    if !request.operation.supports_repository_scope()
        && (request.team.is_some() || request.repository.is_some())
    {
        return Err(TeamAuthorizationError::InvalidRequest("scope"));
    }
    if request.operation == TeamOperation::ReadMetrics
        && request.team.is_some() != request.repository.is_some()
    {
        return Err(TeamAuthorizationError::InvalidRequest("scope"));
    }
    if authenticated_actor.kind == TeamActorKind::Local
        && request.organization != TeamOrganization::local()
    {
        return Err(TeamAuthorizationError::InvalidRequest("actor.kind"));
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
    Ok(())
}

fn validate_actor(actor: &TeamActor) -> Result<(), TeamAuthorizationError> {
    for (field, value) in [
        ("actor.id", actor.id.as_str()),
        ("actor.organizationId", actor.organization_id.as_str()),
    ] {
        validate_identifier(field, value)
            .map_err(|_| TeamAuthorizationError::InvalidRequest(field))?;
    }
    for team_id in &actor.team_ids {
        validate_identifier("actor.teamIds", team_id)
            .map_err(|_| TeamAuthorizationError::InvalidRequest("actor.teamIds"))?;
    }
    let valid = match actor.kind {
        TeamActorKind::User => {
            actor.id.starts_with("user:") && actor.role != TeamRole::ServiceAccount
        }
        TeamActorKind::ServiceAccount => {
            actor.id.starts_with("service:")
                && matches!(actor.role, TeamRole::ServiceAccount | TeamRole::Unknown)
        }
        TeamActorKind::Local => actor == &TeamActor::local_single_user(),
    };
    if valid {
        Ok(())
    } else {
        Err(TeamAuthorizationError::InvalidRequest("actor.kind"))
    }
}

fn denied_reason(
    authenticated_actor: &TeamActor,
    request: &TeamAuthorizationRequest,
    permission: TeamPermission,
) -> Option<TeamAuthorizationDeniedReason> {
    if authenticated_actor.organization_id != request.organization.id {
        return Some(TeamAuthorizationDeniedReason::OrganizationMismatch);
    }
    if request.operation.requires_repository()
        && (request.team.is_none() || request.repository.is_none())
    {
        return Some(TeamAuthorizationDeniedReason::RepositoryScopeRequired);
    }
    if let Some(team) = &request.team {
        if team.organization_id != request.organization.id {
            return Some(TeamAuthorizationDeniedReason::OrganizationMismatch);
        }
        if authenticated_actor.role != TeamRole::Admin
            && !authenticated_actor.team_ids.contains(&team.id)
        {
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
    if authenticated_actor.role == TeamRole::Unknown {
        return Some(TeamAuthorizationDeniedReason::UnknownRole);
    }
    if permission == TeamPermission::Unknown {
        return Some(TeamAuthorizationDeniedReason::UnknownOperation);
    }
    (!authenticated_actor.role.allows(permission))
        .then_some(TeamAuthorizationDeniedReason::PermissionDenied)
}

fn authorization_denied_audit_event(
    authenticated_actor: &TeamActor,
    request: &TeamAuthorizationRequest,
    permission: TeamPermission,
    reason: TeamAuthorizationDeniedReason,
) -> AdministrativeAuditEvent {
    AdministrativeAuditEvent {
        schema_version: AdministrativeAuditSchemaVersion::V2,
        delivery_id: format!("authorization-denied:{}", request.audit.attempt_id),
        tenant_id: authenticated_actor.organization_id.clone(),
        occurred_at: request.audit.occurred_at_ms.to_string(),
        actor: AdministrativeAuditActor {
            kind: match authenticated_actor.kind {
                TeamActorKind::User => AdministrativeAuditActorKind::User,
                TeamActorKind::ServiceAccount => AdministrativeAuditActorKind::Service,
                TeamActorKind::Local => AdministrativeAuditActorKind::System,
            },
            id: authenticated_actor.id.clone(),
        },
        repository: audited_repository(authenticated_actor, request),
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

fn audited_repository(
    authenticated_actor: &TeamActor,
    request: &TeamAuthorizationRequest,
) -> Option<AdministrativeAuditRepositoryScope> {
    let repository = request.repository.as_ref()?;
    let team = request.team.as_ref()?;
    (request.operation.supports_repository_scope()
        && request.organization.id == authenticated_actor.organization_id
        && repository.organization_id == authenticated_actor.organization_id
        && team.organization_id == authenticated_actor.organization_id
        && repository.team_id == team.id)
        .then(|| AdministrativeAuditRepositoryScope {
            provider: repository.provider,
            workspace: repository.workspace.clone(),
            repo: repository.repo.clone(),
            pr_id: None,
        })
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
    use std::ops::{Deref, DerefMut};

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

    struct AuthorizationCase {
        actor: TeamActor,
        request: TeamAuthorizationRequest,
    }

    impl Deref for AuthorizationCase {
        type Target = TeamAuthorizationRequest;

        fn deref(&self) -> &Self::Target {
            &self.request
        }
    }

    impl DerefMut for AuthorizationCase {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.request
        }
    }

    fn authorize_case(
        case: &AuthorizationCase,
        sink: &dyn TeamAuthorizationAuditSink,
    ) -> Result<TeamAuthorizationDecision, TeamAuthorizationError> {
        authorize_team_operation(&case.actor, &case.request, sink)
    }

    fn request(role: TeamRole, operation: TeamOperation) -> AuthorizationCase {
        let repository_required =
            operation.requires_repository() || operation == TeamOperation::ReadMetrics;
        AuthorizationCase {
            actor: actor(role),
            request: TeamAuthorizationRequest {
                schema_version: TeamAuthorizationSchemaVersion::V1,
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
                let decision = authorize_case(&request(role, operation), &sink).expect("decision");
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
            authorize_case(&request(unknown_role, TeamOperation::ReadMetrics), &sink)
                .expect("unknown role decision");
        assert!(matches!(
            role_decision,
            TeamAuthorizationDecision::Denied {
                reason: TeamAuthorizationDeniedReason::UnknownRole,
                ..
            }
        ));

        let operation_decision =
            authorize_case(&request(TeamRole::Admin, unknown_operation), &sink)
                .expect("unknown operation decision");
        assert!(matches!(
            operation_decision,
            TeamAuthorizationDecision::Denied {
                reason: TeamAuthorizationDeniedReason::UnknownOperation,
                ..
            }
        ));

        let mut scoped_unknown = request(TeamRole::Admin, TeamOperation::TriggerReview);
        scoped_unknown.operation = unknown_operation;
        let scoped_sink = MemoryAuditSink::default();
        let scoped_decision = authorize_case(&scoped_unknown, &scoped_sink)
            .expect("scoped unknown operation decision");
        assert!(matches!(
            scoped_decision,
            TeamAuthorizationDecision::Denied {
                reason: TeamAuthorizationDeniedReason::UnknownOperation,
                ..
            }
        ));
        let scoped_events = scoped_sink.events.borrow();
        assert_eq!(scoped_events.len(), 1);
        assert!(scoped_events[0].repository.is_some());

        let mut service_request = request(TeamRole::ServiceAccount, TeamOperation::TriggerReview);
        service_request.actor.role = TeamRole::Unknown;
        let service_sink = MemoryAuditSink::default();
        let service_decision = authorize_case(&service_request, &service_sink)
            .expect("unknown service-account role decision");
        assert!(matches!(
            service_decision,
            TeamAuthorizationDecision::Denied {
                reason: TeamAuthorizationDeniedReason::UnknownRole,
                ..
            }
        ));
        assert_eq!(service_sink.events.borrow().len(), 1);
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
            let decision = authorize_case(&request, &sink).expect("scope decision");
            assert!(matches!(
                decision,
                TeamAuthorizationDecision::Denied { reason, .. } if reason == expected_reason
            ));
            assert_eq!(sink.events.borrow().len(), 1);
        }
    }

    #[test]
    fn scope_mismatches_precede_unknown_role_and_operation_denials() {
        let mut unknown_role = request(TeamRole::Unknown, TeamOperation::TriggerReview);
        unknown_role
            .repository
            .as_mut()
            .expect("repository")
            .organization_id = "tenant-other".to_string();

        let mut unknown_operation = request(TeamRole::Admin, TeamOperation::TriggerReview);
        unknown_operation.operation = TeamOperation::Unknown;
        unknown_operation
            .repository
            .as_mut()
            .expect("repository")
            .team_id = "team-other".to_string();

        for (request, expected_reason) in [
            (
                unknown_role,
                TeamAuthorizationDeniedReason::OrganizationMismatch,
            ),
            (
                unknown_operation,
                TeamAuthorizationDeniedReason::TeamMismatch,
            ),
        ] {
            let sink = MemoryAuditSink::default();
            let decision = authorize_case(&request, &sink).expect("scope decision");
            assert!(matches!(
                decision,
                TeamAuthorizationDecision::Denied { reason, .. } if reason == expected_reason
            ));
            let events = sink.events.borrow();
            assert_eq!(events.len(), 1);
            assert!(events[0].repository.is_none());
        }
    }

    #[test]
    fn repository_operations_fail_closed_without_repository_scope() {
        let mut missing_repository = request(TeamRole::Admin, TeamOperation::PublishReview);
        missing_repository.repository = None;
        missing_repository.team = None;
        let sink = MemoryAuditSink::default();

        let decision = authorize_case(&missing_repository, &sink).expect("scope decision");

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

        let mut missing_team = request(TeamRole::Admin, TeamOperation::PublishReview);
        missing_team.team = None;
        let missing_team_sink = MemoryAuditSink::default();
        assert!(matches!(
            authorize_case(&missing_team, &missing_team_sink).expect("missing team decision"),
            TeamAuthorizationDecision::Denied {
                reason: TeamAuthorizationDeniedReason::RepositoryScopeRequired,
                ..
            }
        ));
    }

    #[test]
    fn organization_mismatch_is_audited_only_in_the_authenticated_tenant() {
        let mut request = request(TeamRole::Member, TeamOperation::TriggerReview);
        request.organization.id = "tenant-other".to_string();
        request.team.as_mut().expect("team").organization_id = "tenant-other".to_string();
        let repository = request.repository.as_mut().expect("repository");
        repository.organization_id = "tenant-other".to_string();
        let sink = MemoryAuditSink::default();

        let decision = authorize_case(&request, &sink).expect("organization mismatch");

        assert!(matches!(
            decision,
            TeamAuthorizationDecision::Denied {
                reason: TeamAuthorizationDeniedReason::OrganizationMismatch,
                ..
            }
        ));
        let event = &sink.events.borrow()[0];
        assert_eq!(event.tenant_id, "tenant-acme");
        assert!(event.repository.is_none());
    }

    #[test]
    fn organization_operations_reject_repository_scope_and_omit_it_from_denial_audit() {
        let mut invalid = request(TeamRole::Admin, TeamOperation::AdministerPolicy);
        invalid.team = Some(TeamIdentity {
            id: "team-payments".to_string(),
            organization_id: "tenant-acme".to_string(),
        });
        invalid.repository = Some(TeamRepositoryScope {
            organization_id: "tenant-acme".to_string(),
            team_id: "team-payments".to_string(),
            provider: PullRequestReviewEventProvider::Github,
            workspace: "acme".to_string(),
            repo: "payments".to_string(),
        });
        let invalid_sink = MemoryAuditSink::default();
        assert_eq!(
            authorize_case(&invalid, &invalid_sink),
            Err(TeamAuthorizationError::InvalidRequest("scope"))
        );
        assert!(invalid_sink.events.borrow().is_empty());

        let sink = MemoryAuditSink::default();
        let decision = authorize_case(
            &request(TeamRole::Member, TeamOperation::AdministerPolicy),
            &sink,
        )
        .expect("organization denial");
        assert!(matches!(
            decision,
            TeamAuthorizationDecision::Denied {
                reason: TeamAuthorizationDeniedReason::PermissionDenied,
                ..
            }
        ));
        let event = &sink.events.borrow()[0];
        assert!(event.repository.is_none());
        assert!(serde_json::to_value(event)
            .expect("serialize organization denial")
            .get("repository")
            .is_none());
    }

    #[test]
    fn organization_metrics_need_no_repository_scope() {
        let mut request = request(TeamRole::Viewer, TeamOperation::ReadMetrics);
        request.team = None;
        request.repository = None;
        let sink = MemoryAuditSink::default();

        assert!(matches!(
            authorize_case(&request, &sink).expect("organization metrics decision"),
            TeamAuthorizationDecision::Allowed {
                permission: TeamPermission::ReadMetrics
            }
        ));
        assert!(sink.events.borrow().is_empty());

        request.team = Some(TeamIdentity {
            id: "team-payments".to_string(),
            organization_id: "tenant-acme".to_string(),
        });
        assert_eq!(
            authorize_case(&request, &sink),
            Err(TeamAuthorizationError::InvalidRequest("scope"))
        );
    }

    #[test]
    fn denied_actions_are_redacted_before_reaching_the_audit_sink() {
        let mut request = request(TeamRole::Viewer, TeamOperation::PublishReview);
        request.audit.correlation_id = "Bearer super-secret-token".to_string();
        let sink = MemoryAuditSink::default();

        let decision = authorize_case(&request, &sink).expect("denied decision");

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

        let error = authorize_case(
            &request(TeamRole::Viewer, TeamOperation::PublishReview),
            &sink,
        )
        .expect_err("audit failure");

        assert_eq!(error, TeamAuthorizationError::AuditFailed);
        assert!(!error.to_string().contains("audit-secret"));
    }

    #[test]
    fn invalid_audit_timestamp_is_rejected_before_authorization() {
        let mut request = request(TeamRole::Admin, TeamOperation::ReadMetrics);
        request.audit.occurred_at_ms = u64::MAX;
        let sink = MemoryAuditSink::default();

        assert_eq!(
            authorize_case(&request, &sink),
            Err(TeamAuthorizationError::InvalidRequest("audit.occurredAtMs"))
        );
        assert!(sink.events.borrow().is_empty());
    }

    #[test]
    fn derived_audit_delivery_id_is_validated_before_authorization() {
        let mut request = request(TeamRole::Admin, TeamOperation::ReadMetrics);
        request.audit.attempt_id = "a".repeat(512);
        let sink = MemoryAuditSink::default();

        assert_eq!(
            authorize_case(&request, &sink),
            Err(TeamAuthorizationError::InvalidRequest("audit.attemptId"))
        );
        assert!(sink.events.borrow().is_empty());
    }

    #[test]
    fn authenticated_actor_constructor_validates_claim_shape() {
        assert!(TeamActor::from_authenticated_claims(
            "user:reviewer-1".to_string(),
            TeamActorKind::User,
            "tenant-acme".to_string(),
            vec!["team-payments".to_string()],
            TeamRole::Member,
        )
        .is_ok());
        assert_eq!(
            TeamActor::from_authenticated_claims(
                "user:reviewer-1".to_string(),
                TeamActorKind::ServiceAccount,
                "tenant-acme".to_string(),
                vec!["team-payments".to_string()],
                TeamRole::Admin,
            ),
            Err(TeamAuthorizationError::InvalidRequest("actor.kind"))
        );
    }

    #[test]
    fn local_single_user_mode_needs_no_identity_provider() {
        for operation in TeamOperation::ALL {
            let repository_required =
                operation.requires_repository() || operation == TeamOperation::ReadMetrics;
            let actor = TeamActor::local_single_user();
            let request = TeamAuthorizationRequest {
                schema_version: TeamAuthorizationSchemaVersion::V1,
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
                authorize_team_operation(&actor, &request, &sink).expect("local decision"),
                TeamAuthorizationDecision::Allowed { .. }
            ));
            assert!(sink.events.borrow().is_empty());
        }
    }

    #[test]
    fn v1_request_shape_is_strict_and_provider_neutral() {
        let request = request(TeamRole::Member, TeamOperation::TriggerReview);
        let value = serde_json::to_value(&request.request).expect("serialize request");
        assert_eq!(value["schemaVersion"], "v1");
        assert!(value.get("actor").is_none());
        assert!(value.get("role").is_none());
        assert!(value.get("teamIds").is_none());
        assert_eq!(value["operation"], "trigger_review");
        assert_eq!(value["repository"]["provider"], "github");
        assert!(value.get("issuer").is_none());
        assert!(value.get("accessToken").is_none());

        let mut unknown = value;
        unknown["accessToken"] = json!("secret");
        assert!(serde_json::from_value::<TeamAuthorizationRequest>(unknown).is_err());
    }
}
