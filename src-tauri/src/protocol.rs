//! Versioned typed stdio protocol between the OpenTUI client and the Rust
//! backend (epic #294, issue #297).
//!
//! Framing is newline-delimited UTF-8 JSON: standard output carries protocol
//! messages only and standard error carries diagnostics. Every frame is a
//! single JSON object with no embedded newline. Unknown fields, unknown methods,
//! invalid enums, and oversized or malformed frames are rejected. The concrete
//! contract lives in `protocol/norn-protocol.schema.json`; the TypeScript types
//! are generated from it under `src/protocol/generated/`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The only supported protocol version for this contract.
pub const PROTOCOL_VERSION: u32 = 1;
/// Maximum accepted frame size in bytes (defensive bound on a single line).
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Structured protocol error carried in error responses and state events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtocolError {
    pub code: ProtocolErrorCode,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}

impl ProtocolError {
    pub fn new(code: ProtocolErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
        }
    }

    /// The stable machine code used across the Rust and TypeScript sides.
    pub fn code_str(&self) -> &'static str {
        self.code.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProtocolErrorCode {
    InvalidRequest,
    UnsupportedVersion,
    UnknownMethod,
    InvalidParams,
    Cancelled,
    NotFound,
    TargetStale,
    Conflict,
    Internal,
}

impl ProtocolErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalidRequest",
            Self::UnsupportedVersion => "unsupportedVersion",
            Self::UnknownMethod => "unknownMethod",
            Self::InvalidParams => "invalidParams",
            Self::Cancelled => "cancelled",
            Self::NotFound => "notFound",
            Self::TargetStale => "targetStale",
            Self::Conflict => "conflict",
            Self::Internal => "internal",
        }
    }
}

/// Identity of the review target a request or event applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TargetIdentity {
    Provider {
        provider: ProviderKind,
        workspace: String,
        repo: String,
        #[serde(rename = "prId", default, skip_serializing_if = "Option::is_none")]
        pr_id: Option<u32>,
        #[serde(rename = "runId", default, skip_serializing_if = "Option::is_none")]
        run_id: Option<String>,
    },
    Local {
        #[serde(rename = "localSnapshotSha256")]
        local_snapshot_sha256: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Github,
    Bitbucket,
}

/// Parameters for `review.reply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewReplyParams {
    pub target: TargetIdentity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewCommitPreviewParams {
    pub target: TargetIdentity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsSaveParams {
    pub expected_version: String,
    pub settings: crate::services::settings::WorkspaceSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataPreviewParams {
    pub path: String,
    pub provider: Option<crate::config::ReviewProvider>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataImportParams {
    pub path: String,
    pub provider: Option<crate::config::ReviewProvider>,
    pub input_version: String,
    pub destination_version: String,
    pub confirm: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataExportParams {
    pub path: String,
}

/// A single-use approval from `review.fix.preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewFixParams {
    pub target: TargetIdentity,
    pub approval_token: String,
}

/// Parameters for `review.jobs`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewJobsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

/// Parameters for `review.job` and `review.job.cancel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewJobParams {
    pub id: String,
}

/// Parameters for `review.metrics.effectiveness`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewEffectivenessParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_ms: Option<i64>,
}

/// Parameters for `file.preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilePreviewParams {
    pub target: TargetIdentity,
    pub path: String,
    pub side: Option<String>,
    pub offset: Option<u32>,
    pub max_bytes: Option<u32>,
    pub content_sha256: Option<String>,
}

/// Parameters for `diff.file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiffFileParams {
    pub target: TargetIdentity,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_lines: Option<u32>,
}

/// Parameters for `review.start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewStartParams {
    pub target: TargetIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<crate::services::provider_snapshot::SnapshotRef>,
}

/// Parameters for the control methods (`repository.status`, `operation.status`,
/// `operation.cancel`, `shutdown`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationControlParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Parameters for methods that take no arguments; rejects unknown fields so the
/// schema and the Rust types agree on what a valid request looks like.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyParams {}

/// Parameters carrying only a review target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetParams {
    pub target: TargetIdentity,
}

/// A first-slice request. The `method` discriminates the typed parameters, so
/// an unknown method or a malformed target is rejected at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum Request {
    #[serde(rename = "provider.snapshot")]
    ProviderSnapshot {
        id: String,
        params: crate::services::bitbucket::workspace_prs::DetailParams,
    },
    #[serde(rename = "provider.snapshot.browser")]
    ProviderSnapshotBrowser {
        id: String,
        params: crate::services::provider_snapshot::BrowserParams,
    },
    #[serde(rename = "provider.snapshot.image")]
    ProviderSnapshotImage {
        id: String,
        params: crate::services::provider_snapshot::ImageParams,
    },
    #[serde(rename = "provider.snapshot.files")]
    ProviderSnapshotFiles {
        id: String,
        params: crate::services::provider_snapshot::FilesParams,
    },
    #[serde(rename = "provider.snapshot.diff")]
    ProviderSnapshotDiff {
        id: String,
        params: crate::services::provider_snapshot::DiffParams,
    },
    #[serde(rename = "provider.prs")]
    ProviderPrs {
        id: String,
        params: crate::services::bitbucket::workspace_prs::ListParams,
    },
    #[serde(rename = "provider.approval.preview")]
    ProviderApprovalPreview {
        id: String,
        params: crate::services::bitbucket::workspace_prs::DetailParams,
    },
    #[serde(rename = "provider.approval.confirm")]
    ProviderApprovalConfirm {
        id: String,
        params: crate::services::provider_approval::ConfirmParams,
    },
    #[serde(rename = "provider.approval.status")]
    ProviderApprovalStatus {
        id: String,
        params: crate::services::bitbucket::workspace_prs::DetailParams,
    },
    #[serde(rename = "provider.approval.reconcile")]
    ProviderApprovalReconcile {
        id: String,
        params: crate::services::bitbucket::workspace_prs::DetailParams,
    },
    #[serde(rename = "provider.browser")]
    ProviderBrowser {
        id: String,
        params: crate::services::bitbucket::workspace_prs::DetailParams,
    },
    #[serde(rename = "provider.pr.detail")]
    ProviderPrDetail {
        id: String,
        params: crate::services::bitbucket::workspace_prs::DetailParams,
    },
    #[serde(rename = "provider.user")]
    ProviderUser {
        id: String,
        params: crate::services::bitbucket::workspace_prs::UserParams,
    },
    #[serde(rename = "publication.prepare")]
    PublicationPrepare {
        id: String,
        params: crate::services::draft_publication::PrepareParams,
    },
    #[serde(rename = "publication.apply")]
    PublicationApply {
        id: String,
        params: crate::services::draft_publication::ApplyParams,
    },
    #[serde(rename = "publication.cancel")]
    PublicationCancel {
        id: String,
        params: crate::services::draft_publication::CancelParams,
    },
    #[serde(rename = "publication.status")]
    PublicationStatus {
        id: String,
        params: crate::services::draft_publication::StatusParams,
    },
    #[serde(rename = "publication.reconcile")]
    PublicationReconcile {
        id: String,
        params: crate::services::draft_publication::ReconcileParams,
    },
    #[serde(rename = "comments.list")]
    CommentsList {
        id: String,
        params: crate::services::draft_publication::CommentsParams,
    },
    #[serde(rename = "draft.prepare")]
    DraftPrepare {
        id: String,
        params: crate::services::draft_staging::PrepareParams,
    },
    #[serde(rename = "draft.stage")]
    DraftStage {
        id: String,
        params: crate::services::draft_staging::StageParams,
    },
    #[serde(rename = "workspace.read")]
    WorkspaceRead {
        id: String,
        params: crate::services::workspace_data::RecordReadParams,
    },
    #[serde(rename = "workspace.mutate")]
    WorkspaceMutate {
        id: String,
        params: crate::services::workspace_data::RecordMutateParams,
    },
    #[serde(rename = "data.preview")]
    DataPreview {
        id: String,
        params: DataPreviewParams,
    },
    #[serde(rename = "data.import")]
    DataImport {
        id: String,
        params: DataImportParams,
    },
    #[serde(rename = "data.export")]
    DataExport {
        id: String,
        params: DataExportParams,
    },
    #[serde(rename = "auth.status")]
    AuthStatus {
        id: String,
        #[serde(default)]
        params: EmptyParams,
    },
    #[serde(rename = "auth.set")]
    AuthSet {
        id: String,
        params: crate::services::workspace_auth::AuthSetParams,
    },
    #[serde(rename = "auth.clear")]
    AuthClear {
        id: String,
        params: crate::services::workspace_auth::AuthProviderParams,
    },
    #[serde(rename = "settings.get")]
    SettingsGet {
        id: String,
        #[serde(default)]
        params: EmptyParams,
    },
    #[serde(rename = "workspace.readiness")]
    WorkspaceReadiness {
        id: String,
        #[serde(default)]
        params: EmptyParams,
    },
    #[serde(rename = "settings.save")]
    SettingsSave {
        id: String,
        params: Box<SettingsSaveParams>,
    },
    #[serde(rename = "repository.status")]
    RepositoryStatus {
        id: String,
        #[serde(default)]
        params: OperationControlParams,
    },
    #[serde(rename = "diff.file")]
    DiffFile { id: String, params: DiffFileParams },
    #[serde(rename = "file.preview")]
    FilePreview {
        id: String,
        params: FilePreviewParams,
    },
    #[serde(rename = "review.files")]
    ReviewFiles { id: String, params: TargetParams },
    #[serde(rename = "review.findings")]
    ReviewFindings { id: String, params: TargetParams },
    #[serde(rename = "local.review.profiles")]
    LocalReviewProfiles {
        id: String,
        params: crate::services::local_snapshot::LookupParams,
    },
    #[serde(rename = "local.review.start")]
    LocalReviewStart {
        id: String,
        params: crate::services::local_reviews::StartParams,
    },
    #[serde(rename = "local.review.history")]
    LocalReviewHistory {
        id: String,
        params: crate::services::local_reviews::HistoryParams,
    },
    #[serde(rename = "local.review.findings")]
    LocalReviewFindings {
        id: String,
        params: crate::services::local_reviews::FindingsParams,
    },
    #[serde(rename = "local.review.output")]
    LocalReviewOutput {
        id: String,
        params: crate::services::local_reviews::QueryParams,
    },
    #[serde(rename = "local.review.threads")]
    LocalReviewThreads {
        id: String,
        params: crate::services::local_reviews::QueryParams,
    },
    #[serde(rename = "repository.explore.info")]
    RepositoryExploreInfo {
        id: String,
        params: crate::services::repository_reader::InfoParams,
    },
    #[serde(rename = "repository.explore.files")]
    RepositoryExploreFiles {
        id: String,
        params: crate::services::repository_reader::FilesParams,
    },
    #[serde(rename = "repository.explore.content")]
    RepositoryExploreContent {
        id: String,
        params: crate::services::repository_reader::ContentParams,
    },
    #[serde(rename = "repository.explore.diff")]
    RepositoryExploreDiff {
        id: String,
        params: crate::services::repository_reader::DiffParams,
    },
    #[serde(rename = "repository.explore.blame")]
    RepositoryExploreBlame {
        id: String,
        params: crate::services::repository_reader::BlameParams,
    },
    #[serde(rename = "repository.explore.find")]
    RepositoryExploreFind {
        id: String,
        params: crate::services::repository_reader::FindParams,
    },
    #[serde(rename = "repository.editor.prepare")]
    RepositoryEditorPrepare {
        id: String,
        params: crate::services::repository_editor::PrepareParams,
    },
    #[serde(rename = "repository.editor.open")]
    RepositoryEditorOpen {
        id: String,
        params: crate::services::repository_editor::OpenParams,
    },
    #[serde(rename = "repository.editor.status")]
    RepositoryEditorStatus {
        id: String,
        params: crate::services::repository_editor::StatusParams,
    },
    #[serde(rename = "repository.editor.cancel")]
    RepositoryEditorCancel {
        id: String,
        params: crate::services::repository_editor::StatusParams,
    },
    #[serde(rename = "repository.operation.list")]
    RepositoryOperationList {
        id: String,
        params: crate::services::repository_operations::ListParams,
    },
    #[serde(rename = "repository.operation.preview")]
    RepositoryOperationPreview {
        id: String,
        params: crate::services::repository_operations::PreviewParams,
    },
    #[serde(rename = "repository.operation.apply")]
    RepositoryOperationApply {
        id: String,
        params: crate::services::repository_operations::ApplyParams,
    },
    #[serde(rename = "repository.operation.status")]
    RepositoryOperationStatus {
        id: String,
        params: crate::services::repository_operations::JobParams,
    },
    #[serde(rename = "repository.operation.cancel")]
    RepositoryOperationCancel {
        id: String,
        params: crate::services::repository_operations::JobParams,
    },
    #[serde(rename = "local.options")]
    LocalOptions {
        id: String,
        params: crate::services::local_snapshot::OptionsParams,
    },
    #[serde(rename = "local.snapshot")]
    LocalSnapshot {
        id: String,
        params: crate::services::local_snapshot::CaptureParams,
    },
    #[serde(rename = "local.snapshot.cancel")]
    LocalSnapshotCancel {
        id: String,
        params: crate::services::local_snapshot::CancelParams,
    },
    #[serde(rename = "local.snapshot.info")]
    LocalSnapshotInfo {
        id: String,
        params: crate::services::local_snapshot::LookupParams,
    },
    #[serde(rename = "local.snapshot.files")]
    LocalSnapshotFiles {
        id: String,
        params: crate::services::local_snapshot::FilesParams,
    },
    #[serde(rename = "local.snapshot.diff")]
    LocalSnapshotDiff {
        id: String,
        params: crate::services::local_snapshot::DiffParams,
    },
    #[serde(rename = "local.snapshot.image")]
    LocalSnapshotImage {
        id: String,
        params: crate::services::local_snapshot::ImageParams,
    },
    #[serde(rename = "local.snapshot.browser")]
    LocalSnapshotBrowser {
        id: String,
        params: crate::services::local_snapshot::BrowserParams,
    },
    #[serde(rename = "local.snapshot.list")]
    LocalSnapshotList {
        id: String,
        params: crate::services::local_snapshot::ListParams,
    },
    #[serde(rename = "local.snapshot.recheck")]
    LocalSnapshotRecheck {
        id: String,
        params: crate::services::local_snapshot::RecheckParams,
    },
    #[serde(rename = "review.profiles")]
    ReviewProfiles {
        id: String,
        params: crate::services::review_profiles::Params,
    },
    #[serde(rename = "review.metrics.effectiveness")]
    ReviewEffectiveness {
        id: String,
        #[serde(default)]
        params: ReviewEffectivenessParams,
    },
    #[serde(rename = "review.metrics.closed")]
    ReviewClosedMetrics {
        id: String,
        #[serde(default)]
        params: EmptyParams,
    },
    #[serde(rename = "review.job")]
    ReviewJob { id: String, params: ReviewJobParams },
    #[serde(rename = "review.job.cancel")]
    ReviewJobCancel { id: String, params: ReviewJobParams },
    #[serde(rename = "review.jobs")]
    ReviewJobs {
        id: String,
        #[serde(default)]
        params: ReviewJobsParams,
    },
    #[serde(rename = "review.history")]
    ReviewHistory { id: String, params: TargetParams },
    #[serde(rename = "review.threads")]
    ReviewThreads { id: String, params: TargetParams },
    #[serde(rename = "review.output")]
    ReviewOutput { id: String, params: TargetParams },
    #[serde(rename = "review.targets")]
    ReviewTargets {
        id: String,
        #[serde(default)]
        params: EmptyParams,
    },
    #[serde(rename = "workspace.scopes")]
    WorkspaceScopes {
        id: String,
        params: crate::services::workspace_data::ScopeReadParams,
    },
    #[serde(rename = "browser.open")]
    BrowserOpen { id: String, params: TargetParams },
    #[serde(rename = "review.start")]
    ReviewStart {
        id: String,
        params: ReviewStartParams,
    },
    #[serde(rename = "review.commit.preview")]
    ReviewCommitPreview {
        id: String,
        params: ReviewCommitPreviewParams,
    },
    #[serde(rename = "review.commit")]
    ReviewCommit { id: String, params: ReviewFixParams },
    #[serde(rename = "review.push.preview")]
    ReviewPushPreview { id: String, params: TargetParams },
    #[serde(rename = "review.push")]
    ReviewPush { id: String, params: ReviewFixParams },
    #[serde(rename = "review.sync.preview")]
    ReviewSyncPreview { id: String, params: TargetParams },
    #[serde(rename = "review.sync")]
    ReviewSync { id: String, params: ReviewFixParams },
    #[serde(rename = "review.conflicts.preview")]
    ReviewConflictsPreview { id: String, params: TargetParams },
    #[serde(rename = "review.conflicts")]
    ReviewConflicts { id: String, params: ReviewFixParams },
    #[serde(rename = "review.fix.state")]
    ReviewFixState { id: String, params: TargetParams },
    #[serde(rename = "review.fix.preview")]
    ReviewFixPreview { id: String, params: TargetParams },
    #[serde(rename = "review.fix")]
    ReviewFix { id: String, params: ReviewFixParams },
    #[serde(rename = "review.reply")]
    ReviewReply {
        id: String,
        params: ReviewReplyParams,
    },
    #[serde(rename = "operation.status")]
    OperationStatus {
        id: String,
        #[serde(default)]
        params: OperationControlParams,
    },
    #[serde(rename = "operation.cancel")]
    OperationCancel {
        id: String,
        #[serde(default)]
        params: OperationControlParams,
    },
    #[serde(rename = "shutdown")]
    Shutdown {
        id: String,
        #[serde(default)]
        params: OperationControlParams,
    },
}

impl Request {
    pub fn id(&self) -> &str {
        match self {
            Self::PublicationPrepare { id, .. }
            | Self::PublicationApply { id, .. }
            | Self::PublicationCancel { id, .. }
            | Self::PublicationStatus { id, .. }
            | Self::PublicationReconcile { id, .. }
            | Self::CommentsList { id, .. }
            | Self::DraftPrepare { id, .. }
            | Self::DraftStage { id, .. }
            | Self::WorkspaceRead { id, .. }
            | Self::WorkspaceMutate { id, .. }
            | Self::DataPreview { id, .. }
            | Self::DataImport { id, .. }
            | Self::DataExport { id, .. }
            | Self::AuthStatus { id, .. }
            | Self::AuthSet { id, .. }
            | Self::AuthClear { id, .. }
            | Self::SettingsGet { id, .. }
            | Self::WorkspaceReadiness { id, .. }
            | Self::SettingsSave { id, .. }
            | Self::RepositoryStatus { id, .. }
            | Self::DiffFile { id, .. }
            | Self::FilePreview { id, .. }
            | Self::ReviewFiles { id, .. }
            | Self::ReviewFindings { id, .. }
            | Self::ReviewTargets { id, .. }
            | Self::ProviderSnapshot { id, .. }
            | Self::ProviderSnapshotBrowser { id, .. }
            | Self::ProviderSnapshotImage { id, .. }
            | Self::ProviderSnapshotFiles { id, .. }
            | Self::ProviderSnapshotDiff { id, .. }
            | Self::ProviderPrs { id, .. }
            | Self::ProviderApprovalPreview { id, .. }
            | Self::ProviderApprovalConfirm { id, .. }
            | Self::ProviderApprovalStatus { id, .. }
            | Self::ProviderApprovalReconcile { id, .. }
            | Self::ProviderBrowser { id, .. }
            | Self::ProviderPrDetail { id, .. }
            | Self::ProviderUser { id, .. }
            | Self::WorkspaceScopes { id, .. }
            | Self::LocalReviewProfiles { id, .. }
            | Self::LocalReviewStart { id, .. }
            | Self::LocalReviewHistory { id, .. }
            | Self::LocalReviewFindings { id, .. }
            | Self::LocalReviewOutput { id, .. }
            | Self::LocalReviewThreads { id, .. }
            | Self::RepositoryExploreInfo { id, .. }
            | Self::RepositoryExploreFiles { id, .. }
            | Self::RepositoryExploreContent { id, .. }
            | Self::RepositoryExploreDiff { id, .. }
            | Self::RepositoryExploreBlame { id, .. }
            | Self::RepositoryExploreFind { id, .. }
            | Self::RepositoryEditorPrepare { id, .. }
            | Self::RepositoryEditorOpen { id, .. }
            | Self::RepositoryEditorStatus { id, .. }
            | Self::RepositoryEditorCancel { id, .. }
            | Self::RepositoryOperationList { id, .. }
            | Self::RepositoryOperationPreview { id, .. }
            | Self::RepositoryOperationApply { id, .. }
            | Self::RepositoryOperationStatus { id, .. }
            | Self::RepositoryOperationCancel { id, .. }
            | Self::LocalOptions { id, .. }
            | Self::LocalSnapshot { id, .. }
            | Self::LocalSnapshotCancel { id, .. }
            | Self::LocalSnapshotInfo { id, .. }
            | Self::LocalSnapshotFiles { id, .. }
            | Self::LocalSnapshotImage { id, .. }
            | Self::LocalSnapshotBrowser { id, .. }
            | Self::LocalSnapshotList { id, .. }
            | Self::LocalSnapshotDiff { id, .. }
            | Self::LocalSnapshotRecheck { id, .. }
            | Self::ReviewProfiles { id, .. }
            | Self::ReviewHistory { id, .. }
            | Self::ReviewJobs { id, .. }
            | Self::ReviewJob { id, .. }
            | Self::ReviewJobCancel { id, .. }
            | Self::ReviewClosedMetrics { id, .. }
            | Self::ReviewEffectiveness { id, .. }
            | Self::ReviewThreads { id, .. }
            | Self::ReviewOutput { id, .. }
            | Self::BrowserOpen { id, .. }
            | Self::ReviewStart { id, .. }
            | Self::ReviewCommitPreview { id, .. }
            | Self::ReviewCommit { id, .. }
            | Self::ReviewPushPreview { id, .. }
            | Self::ReviewPush { id, .. }
            | Self::ReviewSyncPreview { id, .. }
            | Self::ReviewSync { id, .. }
            | Self::ReviewConflictsPreview { id, .. }
            | Self::ReviewConflicts { id, .. }
            | Self::ReviewFixState { id, .. }
            | Self::ReviewFixPreview { id, .. }
            | Self::ReviewFix { id, .. }
            | Self::ReviewReply { id, .. }
            | Self::OperationStatus { id, .. }
            | Self::OperationCancel { id, .. }
            | Self::Shutdown { id, .. } => id,
        }
    }

    pub fn method(&self) -> &'static str {
        match self {
            Self::PublicationPrepare { .. } => "publication.prepare",
            Self::PublicationApply { .. } => "publication.apply",
            Self::PublicationCancel { .. } => "publication.cancel",
            Self::PublicationStatus { .. } => "publication.status",
            Self::PublicationReconcile { .. } => "publication.reconcile",
            Self::CommentsList { .. } => "comments.list",
            Self::DraftPrepare { .. } => "draft.prepare",
            Self::DraftStage { .. } => "draft.stage",
            Self::WorkspaceRead { .. } => "workspace.read",
            Self::WorkspaceMutate { .. } => "workspace.mutate",
            Self::DataPreview { .. } => "data.preview",
            Self::DataImport { .. } => "data.import",
            Self::DataExport { .. } => "data.export",
            Self::AuthStatus { .. } => "auth.status",
            Self::AuthSet { .. } => "auth.set",
            Self::AuthClear { .. } => "auth.clear",
            Self::SettingsGet { .. } => "settings.get",
            Self::WorkspaceReadiness { .. } => "workspace.readiness",
            Self::SettingsSave { .. } => "settings.save",
            Self::RepositoryStatus { .. } => "repository.status",
            Self::DiffFile { .. } => "diff.file",
            Self::FilePreview { .. } => "file.preview",
            Self::ReviewFiles { .. } => "review.files",
            Self::ReviewFindings { .. } => "review.findings",
            Self::ReviewTargets { .. } => "review.targets",
            Self::ProviderSnapshot { .. } => "provider.snapshot",
            Self::ProviderSnapshotBrowser { .. } => "provider.snapshot.browser",
            Self::ProviderSnapshotImage { .. } => "provider.snapshot.image",
            Self::ProviderSnapshotFiles { .. } => "provider.snapshot.files",
            Self::ProviderSnapshotDiff { .. } => "provider.snapshot.diff",
            Self::ProviderPrs { .. } => "provider.prs",
            Self::ProviderApprovalPreview { .. } => "provider.approval.preview",
            Self::ProviderApprovalConfirm { .. } => "provider.approval.confirm",
            Self::ProviderApprovalStatus { .. } => "provider.approval.status",
            Self::ProviderApprovalReconcile { .. } => "provider.approval.reconcile",
            Self::ProviderBrowser { .. } => "provider.browser",
            Self::ProviderPrDetail { .. } => "provider.pr.detail",
            Self::ProviderUser { .. } => "provider.user",
            Self::WorkspaceScopes { .. } => "workspace.scopes",
            Self::LocalReviewProfiles { .. } => "local.review.profiles",
            Self::LocalReviewStart { .. } => "local.review.start",
            Self::LocalReviewHistory { .. } => "local.review.history",
            Self::LocalReviewFindings { .. } => "local.review.findings",
            Self::LocalReviewOutput { .. } => "local.review.output",
            Self::LocalReviewThreads { .. } => "local.review.threads",
            Self::RepositoryExploreInfo { .. } => "repository.explore.info",
            Self::RepositoryExploreFiles { .. } => "repository.explore.files",
            Self::RepositoryExploreContent { .. } => "repository.explore.content",
            Self::RepositoryExploreDiff { .. } => "repository.explore.diff",
            Self::RepositoryExploreBlame { .. } => "repository.explore.blame",
            Self::RepositoryExploreFind { .. } => "repository.explore.find",
            Self::RepositoryEditorPrepare { .. } => "repository.editor.prepare",
            Self::RepositoryEditorOpen { .. } => "repository.editor.open",
            Self::RepositoryEditorStatus { .. } => "repository.editor.status",
            Self::RepositoryEditorCancel { .. } => "repository.editor.cancel",
            Self::RepositoryOperationList { .. } => "repository.operation.list",
            Self::RepositoryOperationPreview { .. } => "repository.operation.preview",
            Self::RepositoryOperationApply { .. } => "repository.operation.apply",
            Self::RepositoryOperationStatus { .. } => "repository.operation.status",
            Self::RepositoryOperationCancel { .. } => "repository.operation.cancel",
            Self::LocalOptions { .. } => "local.options",
            Self::LocalSnapshot { .. } => "local.snapshot",
            Self::LocalSnapshotCancel { .. } => "local.snapshot.cancel",
            Self::LocalSnapshotInfo { .. } => "local.snapshot.info",
            Self::LocalSnapshotFiles { .. } => "local.snapshot.files",
            Self::LocalSnapshotImage { .. } => "local.snapshot.image",
            Self::LocalSnapshotBrowser { .. } => "local.snapshot.browser",
            Self::LocalSnapshotList { .. } => "local.snapshot.list",
            Self::LocalSnapshotDiff { .. } => "local.snapshot.diff",
            Self::LocalSnapshotRecheck { .. } => "local.snapshot.recheck",
            Self::ReviewProfiles { .. } => "review.profiles",
            Self::ReviewHistory { .. } => "review.history",
            Self::ReviewJobs { .. } => "review.jobs",
            Self::ReviewJob { .. } => "review.job",
            Self::ReviewJobCancel { .. } => "review.job.cancel",
            Self::ReviewClosedMetrics { .. } => "review.metrics.closed",
            Self::ReviewEffectiveness { .. } => "review.metrics.effectiveness",
            Self::ReviewThreads { .. } => "review.threads",
            Self::ReviewOutput { .. } => "review.output",
            Self::BrowserOpen { .. } => "browser.open",
            Self::ReviewStart { .. } => "review.start",
            Self::ReviewReply { .. } => "review.reply",
            Self::ReviewCommitPreview { .. } => "review.commit.preview",
            Self::ReviewCommit { .. } => "review.commit",
            Self::ReviewPushPreview { .. } => "review.push.preview",
            Self::ReviewPush { .. } => "review.push",
            Self::ReviewSyncPreview { .. } => "review.sync.preview",
            Self::ReviewSync { .. } => "review.sync",
            Self::ReviewConflictsPreview { .. } => "review.conflicts.preview",
            Self::ReviewConflicts { .. } => "review.conflicts",
            Self::ReviewFixState { .. } => "review.fix.state",
            Self::ReviewFixPreview { .. } => "review.fix.preview",
            Self::ReviewFix { .. } => "review.fix",
            Self::OperationStatus { .. } => "operation.status",
            Self::OperationCancel { .. } => "operation.cancel",
            Self::Shutdown { .. } => "shutdown",
        }
    }

    /// Enforce the schema rules the envelope types do not express: a non-empty
    /// id, a valid target, a non-empty path, and the `contextLines` range.
    fn validate(&self) -> Result<(), ProtocolError> {
        if self.id().is_empty() {
            return Err(invalid_request("request id must not be empty"));
        }
        match self {
            Self::PublicationPrepare { params, .. } => {
                crate::services::draft_publication::validate_scope(&params.scope)
                    .map_err(|message| invalid_request(&message))?
            }
            Self::PublicationApply { params, .. } => {
                crate::services::draft_publication::validate_scope(&params.scope)
                    .map_err(|message| invalid_request(&message))?
            }
            Self::PublicationCancel { params, .. } => {
                crate::services::draft_publication::validate_scope(&params.scope)
                    .map_err(|message| invalid_request(&message))?
            }
            Self::PublicationStatus { params, .. } => {
                crate::services::draft_publication::validate_scope(&params.scope)
                    .map_err(|message| invalid_request(&message))?
            }
            Self::PublicationReconcile { params, .. } => {
                crate::services::draft_publication::validate_scope(&params.scope)
                    .map_err(|message| invalid_request(&message))?
            }
            Self::CommentsList { params, .. } => {
                crate::services::draft_publication::validate_scope(&params.scope)
                    .map_err(|message| invalid_request(&message))?
            }
            Self::DraftPrepare { params, .. } => crate::services::draft_staging::validate(params)?,
            Self::DraftStage { params, .. } => {
                crate::services::draft_staging::validate(
                    &crate::services::draft_staging::PrepareParams {
                        target: params.target.clone(),
                        finding_id: params.finding_id.clone(),
                        publication_mode: params.publication_mode,
                    },
                )?;
                if [&params.expected_version, &params.preview_hash]
                    .iter()
                    .any(|value| {
                        value.len() != 64
                            || !value
                                .bytes()
                                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                    })
                {
                    return Err(invalid_request("Invalid draft preview or record version."));
                }
            }
            Self::WorkspaceRead { params, .. } => {
                if params.key.is_empty()
                    || params.key.len() > 1024
                    || params.offset > 10_000
                    || params.limit == 0
                    || params.limit > 100
                    || params.record_version.as_deref().is_some_and(|value| {
                        value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
                    || (params.offset > 0 && params.record_version.is_none())
                {
                    return Err(invalid_request("Invalid user-data page parameters."));
                }
            }
            Self::ProviderSnapshot { params, .. } => params.validate().map_err(invalid_request)?,
            Self::ProviderSnapshotBrowser { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::ProviderSnapshotImage { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::ProviderSnapshotFiles { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::ProviderSnapshotDiff { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::ProviderPrs { params, .. } => params.validate().map_err(invalid_request)?,
            Self::ProviderApprovalConfirm { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::ProviderApprovalPreview { params, .. }
            | Self::ProviderApprovalStatus { params, .. }
            | Self::ProviderApprovalReconcile { params, .. }
            | Self::ProviderBrowser { params, .. }
            | Self::ProviderPrDetail { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::ProviderUser { params, .. } => {
                params.repository.validate().map_err(invalid_request)?
            }
            Self::WorkspaceScopes { params, .. } => {
                params.validate().map_err(invalid_request)?;
            }
            Self::WorkspaceMutate { params, .. } => {
                if params.key.is_empty()
                    || params.key.len() > 1024
                    || params.expected_version.len() != 64
                    || !params
                        .expected_version
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit())
                {
                    return Err(invalid_request("Invalid user-data mutation parameters."));
                }
            }
            Self::DataPreview { params, .. } => validate_data_path(&params.path)?,
            Self::DataExport { params, .. } => validate_data_path(&params.path)?,
            Self::DataImport { params, .. } => {
                validate_data_path(&params.path)?;
                if !params.confirm
                    || [&params.input_version, &params.destination_version]
                        .iter()
                        .any(|value| {
                            value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
                        })
                {
                    return Err(invalid_request(
                        "Explicit import confirmation and valid preview versions are required.",
                    ));
                }
            }
            Self::SettingsSave { params, .. } => {
                if params.expected_version.len() != 64
                    || !params
                        .expected_version
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit())
                {
                    return Err(invalid_request("invalid settings version"));
                }
                params.settings.validate().map_err(invalid_request)?;
            }
            Self::SettingsGet { .. }
            | Self::WorkspaceReadiness { .. }
            | Self::AuthStatus { .. }
            | Self::AuthClear { .. } => {}
            Self::AuthSet { params, .. } => params.validate().map_err(invalid_request)?,
            Self::DiffFile { params, .. } => {
                params.target.validate()?;
                if params.path.is_empty() {
                    return Err(invalid_request("`diff.file` path must not be empty"));
                }
                if params.context_lines.is_some_and(|lines| lines > 100) {
                    return Err(invalid_request("`diff.file` contextLines must be <= 100"));
                }
            }
            Self::FilePreview { params, .. } => {
                params.target.validate()?;
                validate_data_path(&params.path)?;
                if params
                    .side
                    .as_deref()
                    .is_some_and(|side| !matches!(side, "old" | "new"))
                    || params.offset.is_some_and(|offset| offset > 2 * 1024 * 1024)
                    || params
                        .max_bytes
                        .is_some_and(|size| size == 0 || size > 192 * 1024)
                    || params.content_sha256.as_ref().is_some_and(|value| {
                        value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit())
                    })
                    || (params.offset.unwrap_or(0) > 0 && params.content_sha256.is_none())
                {
                    return Err(invalid_request(
                        "Invalid image side, chunk range or content version.",
                    ));
                }
            }
            Self::ReviewStart { params, .. } => {
                params.target.validate()?;
                if let Some(snapshot) = &params.snapshot {
                    snapshot
                        .validate()
                        .map_err(|_| invalid_request("Invalid review snapshot."))?;
                    let TargetIdentity::Provider {
                        provider,
                        workspace,
                        repo,
                        pr_id: Some(pr_id),
                        run_id: None,
                    } = &params.target
                    else {
                        return Err(invalid_request("A provider preview target without a saved run is required for snapshot review."));
                    };
                    let snapshot_provider = match snapshot.repository.provider {
                        crate::config::ReviewProvider::Github => ProviderKind::Github,
                        crate::config::ReviewProvider::Bitbucket => ProviderKind::Bitbucket,
                    };
                    if snapshot_provider != *provider
                        || snapshot.repository.workspace != *workspace
                        || snapshot.repository.repo != *repo
                        || snapshot.pr_id != *pr_id
                    {
                        return Err(invalid_request(
                            "The review snapshot belongs to a different target.",
                        ));
                    }
                }
                if params.profile.as_ref().is_some_and(|profile| {
                    profile.is_empty()
                        || profile.chars().count() > 128
                        || profile.starts_with(' ')
                        || profile.ends_with(' ')
                        || profile.chars().any(char::is_control)
                }) {
                    return Err(invalid_request("A profile name between 1 and 128 characters without control characters or surrounding spaces is required."));
                }
            }
            Self::ReviewCommitPreview { params, .. } => {
                params.target.validate()?;
                if params.message.trim().is_empty() || params.message.chars().count() > 4096 {
                    return Err(invalid_request(
                        "a nonempty commit message up to 4096 characters is required",
                    ));
                }
            }
            Self::ReviewCommit { params, .. }
            | Self::ReviewPush { params, .. }
            | Self::ReviewSync { params, .. }
            | Self::ReviewConflicts { params, .. }
            | Self::ReviewFix { params, .. } => {
                params.target.validate()?;
                if params.approval_token.len() != 64
                    || !params.approval_token.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(invalid_request("invalid fix approval token"));
                }
            }
            Self::ReviewReply { params, .. } => {
                params.target.validate()?;
                if params.message.trim().is_empty() {
                    return Err(invalid_request("`review.reply` message must not be empty"));
                }
            }
            Self::LocalReviewProfiles { params, .. } => {
                params.snapshot.validate().map_err(invalid_request)?
            }
            Self::LocalReviewStart { params, .. } => params.validate().map_err(invalid_request)?,
            Self::LocalReviewHistory { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::LocalReviewFindings { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::LocalReviewOutput { params, .. } => params.validate().map_err(invalid_request)?,
            Self::LocalReviewThreads { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryExploreInfo { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryExploreFiles { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryExploreContent { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryExploreDiff { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryExploreBlame { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryExploreFind { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryEditorPrepare { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryEditorOpen { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryEditorStatus { params, .. }
            | Self::RepositoryEditorCancel { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryOperationList { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryOperationPreview { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryOperationApply { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryOperationStatus { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::RepositoryOperationCancel { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::LocalOptions { params, .. } => params.validate().map_err(invalid_request)?,
            Self::LocalSnapshot { params, .. } => params.validate().map_err(invalid_request)?,
            Self::LocalSnapshotCancel { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::LocalSnapshotInfo { params, .. } => params.validate().map_err(invalid_request)?,
            Self::LocalSnapshotFiles { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::LocalSnapshotImage { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::LocalSnapshotBrowser { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::LocalSnapshotList { params, .. } => params.validate().map_err(invalid_request)?,
            Self::LocalSnapshotDiff { params, .. } => params.validate().map_err(invalid_request)?,
            Self::LocalSnapshotRecheck { params, .. } => {
                params.validate().map_err(invalid_request)?
            }
            Self::ReviewProfiles { params, .. } => params.validate()?,
            Self::ReviewSyncPreview { params, .. }
            | Self::ReviewConflictsPreview { params, .. }
            | Self::ReviewPushPreview { params, .. }
            | Self::ReviewFixState { params, .. }
            | Self::ReviewFixPreview { params, .. }
            | Self::ReviewFiles { params, .. }
            | Self::ReviewFindings { params, .. }
            | Self::ReviewHistory { params, .. }
            | Self::ReviewThreads { params, .. }
            | Self::ReviewOutput { params, .. }
            | Self::BrowserOpen { params, .. } => params.target.validate()?,
            Self::RepositoryStatus { .. }
            | Self::ReviewTargets { .. }
            | Self::ReviewJobs { .. }
            | Self::ReviewJob { .. }
            | Self::ReviewJobCancel { .. }
            | Self::ReviewClosedMetrics { .. }
            | Self::ReviewEffectiveness { .. }
            | Self::OperationStatus { .. }
            | Self::OperationCancel { .. }
            | Self::Shutdown { .. } => {}
        }
        Ok(())
    }
}

impl TargetIdentity {
    fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Provider {
                workspace,
                repo,
                run_id,
                ..
            } => {
                if workspace.is_empty() || repo.is_empty() {
                    return Err(invalid_request(
                        "provider target workspace and repo must not be empty",
                    ));
                }
                if run_id.as_deref() == Some("") {
                    return Err(invalid_request("provider target `runId` must not be empty"));
                }
                Ok(())
            }
            Self::Local {
                local_snapshot_sha256,
            } => {
                if local_snapshot_sha256.len() != 64 {
                    return Err(invalid_request(
                        "local target `localSnapshotSha256` must be 64 characters",
                    ));
                }
                Ok(())
            }
        }
    }
}

impl Response {
    /// A success carries exactly a result; a failure carries exactly an error.
    fn validate(&self) -> Result<(), ProtocolError> {
        if self.id.is_empty() {
            return Err(invalid_request("response id must not be empty"));
        }
        match (self.ok, self.result.is_some(), self.error.is_some()) {
            (true, true, false) | (false, false, true) => Ok(()),
            _ => Err(invalid_request(
                "response must carry a result when ok and an error otherwise",
            )),
        }
    }
}

fn invalid_request(message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidRequest, message)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hello {
    pub protocol_version: u32,
    pub client: ClientInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ready {
    pub protocol_version: u32,
    pub server_version: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    pub id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ProtocolError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationEventFrame {
    pub operation_id: String,
    pub sequence: u64,
    pub target: TargetIdentity,
    pub event: OperationEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OperationEvent {
    Progress {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        completed: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
    },
    Log {
        level: LogLevel,
        message: String,
    },
    State {
        state: OperationState,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<ProtocolError>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationState {
    Accepted,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

/// A request the client sends to the backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
    Hello(Hello),
    Request(Request),
}

/// A message the backend sends to the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    Ready(Ready),
    Response(Response),
    Event(OperationEventFrame),
}

fn frame_too_large() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::InvalidRequest,
        format!("frame exceeds {MAX_FRAME_BYTES} bytes"),
    )
}

fn decode_json<T: serde::de::DeserializeOwned>(line: &[u8]) -> Result<T, ProtocolError> {
    if line.len() > MAX_FRAME_BYTES {
        return Err(frame_too_large());
    }
    let text = std::str::from_utf8(line).map_err(|_| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidRequest,
            "frame is not valid UTF-8",
        )
    })?;
    let trimmed = text.trim_end_matches(['\n', '\r']);
    if trimmed.is_empty() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidRequest,
            "empty frame",
        ));
    }
    serde_json::from_str(trimmed).map_err(|_| {
        // Schema errors may contain the rejected value. Never echo a request
        // value that could be a credential into response diagnostics.
        ProtocolError::new(
            ProtocolErrorCode::InvalidRequest,
            "malformed protocol frame",
        )
    })
}

/// Decode one client frame, enforcing framing limits and the envelope shape.
pub fn decode_client_line(line: &[u8]) -> Result<ClientMessage, ProtocolError> {
    let message: ClientMessage = decode_json(line)?;
    match &message {
        ClientMessage::Hello(hello) => validate_handshake(hello)?,
        ClientMessage::Request(request) => request.validate()?,
    }
    Ok(message)
}

/// Decode one server frame, enforcing framing limits and the envelope shape.
pub fn decode_server_line(line: &[u8]) -> Result<ServerMessage, ProtocolError> {
    let message: ServerMessage = decode_json(line)?;
    match &message {
        ServerMessage::Ready(ready) => {
            if ready.protocol_version != PROTOCOL_VERSION {
                return Err(ProtocolError::new(
                    ProtocolErrorCode::UnsupportedVersion,
                    format!(
                        "unsupported protocol version {}; this build speaks {PROTOCOL_VERSION}",
                        ready.protocol_version
                    ),
                ));
            }
        }
        ServerMessage::Response(response) => response.validate()?,
        ServerMessage::Event(frame) => {
            if frame.operation_id.is_empty() {
                return Err(invalid_request("event operationId must not be empty"));
            }
        }
    }
    Ok(message)
}

/// Encode a message as a single protocol frame (one line, no embedded newline).
pub fn encode_line<T: Serialize>(message: &T) -> Result<String, ProtocolError> {
    let mut encoded = serde_json::to_string(message).map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::Internal,
            format!("failed to encode frame: {error}"),
        )
    })?;
    if encoded.len() + 1 > MAX_FRAME_BYTES {
        return Err(frame_too_large());
    }
    encoded.push('\n');
    Ok(encoded)
}

/// Reject an incompatible handshake before any review action is enabled.
pub fn validate_handshake(hello: &Hello) -> Result<(), ProtocolError> {
    if hello.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::new(
            ProtocolErrorCode::UnsupportedVersion,
            format!(
                "unsupported protocol version {}; this build speaks {PROTOCOL_VERSION}",
                hello.protocol_version
            ),
        ));
    }
    Ok(())
}

/// Tracks the highest observed event sequence per operation. Duplicate or
/// out-of-order event sequences are rejected so the client never renders a
/// stale snapshot; the caller resynchronizes with an `operation.status`
/// request when it detects a gap.
#[derive(Debug, Default)]
pub struct SequenceGuard {
    highest: HashMap<String, u64>,
}

impl SequenceGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe an event sequence. Returns the previous highest sequence, or an
    /// error when the sequence is a duplicate or moves backwards.
    pub fn observe(
        &mut self,
        operation_id: &str,
        sequence: u64,
    ) -> Result<Option<u64>, ProtocolError> {
        match self.highest.get(operation_id).copied() {
            Some(previous) if sequence <= previous => Err(ProtocolError::new(
                ProtocolErrorCode::Conflict,
                format!(
                    "out-of-order event sequence {sequence} for operation {operation_id}; expected greater than {previous}"
                ),
            )),
            previous => {
                self.highest.insert(operation_id.to_string(), sequence);
                Ok(previous)
            }
        }
    }

    /// Highest sequence observed so far for an operation, if any.
    pub fn highest(&self, operation_id: &str) -> Option<u64> {
        self.highest.get(operation_id).copied()
    }
}

fn validate_data_path(path: &str) -> Result<(), ProtocolError> {
    if path.is_empty() || path.chars().count() > 4096 || path.chars().any(char::is_control) {
        Err(invalid_request(
            "User-data path must contain 1–4096 printable characters.",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../protocol/fixtures")
    }

    fn read_fixture(relative: &str) -> Vec<u8> {
        std::fs::read(fixtures_dir().join(relative))
            .unwrap_or_else(|error| panic!("failed to read fixture {relative}: {error}"))
    }

    #[test]
    fn credential_shape_errors_and_debug_never_echo_the_secret() {
        let invalid = br#"{"type":"request","id":"auth","method":"auth.set","params":{"provider":"fixture_sensitive_value","token":"fixture_sensitive_value"}}"#;
        let error = decode_client_line(invalid).unwrap_err();
        assert!(!error.message.contains("fixture_sensitive_value"));
        let valid = br#"{"type":"request","id":"auth","method":"auth.set","params":{"provider":"github","token":"fixture_sensitive_value"}}"#;
        let request = decode_client_line(valid).unwrap();
        assert!(!format!("{request:?}").contains("fixture_sensitive_value"));
    }

    #[test]
    fn valid_fixtures_decode() {
        for fixture in [
            "valid/hello.json",
            "valid/request-repository-explore-info.json",
            "valid/response-repository-explore-info.json",
            "valid/request-repository-explore-files.json",
            "valid/response-repository-explore-files.json",
            "valid/request-repository-explore-content.json",
            "valid/response-repository-explore-content.json",
            "valid/request-repository-explore-diff.json",
            "valid/response-repository-explore-diff.json",
            "valid/request-repository-explore-blame.json",
            "valid/response-repository-explore-blame.json",
            "valid/response-repository-operation-list.json",
            "valid/response-repository-operation-preview.json",
            "valid/response-repository-operation-apply.json",
            "valid/response-repository-operation-status.json",
            "valid/request-repository-operation-list.json",
            "valid/request-repository-operation-preview.json",
            "valid/request-repository-operation-apply.json",
            "valid/request-repository-operation-status.json",
            "valid/request-repository-operation-cancel.json",
            "valid/request-repository-editor-prepare.json",
            "valid/response-repository-editor-prepare.json",
            "valid/request-repository-editor-open.json",
            "valid/response-repository-editor-open.json",
            "valid/request-repository-editor-status.json",
            "valid/response-repository-editor-status.json",
            "valid/request-repository-editor-cancel.json",
            "valid/response-repository-editor-cancel.json",
            "valid/request-repository-explore-find.json",
            "valid/response-repository-explore-find.json",
            "valid/ready.json",
            "valid/request-diff-file.json",
            "valid/request-review-start.json",
            "valid/request-review-start-snapshot.json",
            "valid/request-operation-cancel.json",
            "valid/request-provider-snapshot.json",
            "valid/request-provider-snapshot-files.json",
            "valid/request-provider-snapshot-diff.json",
            "valid/request-provider-snapshot-image.json",
            "valid/request-provider-snapshot-browser.json",
            "valid/response-provider-snapshot-image.json",
            "valid/response-provider-snapshot-browser.json",
            "valid/response-provider-snapshot.json",
            "valid/response-provider-snapshot-files.json",
            "valid/response-provider-snapshot-diff.json",
            "valid/request-provider-prs.json",
            "valid/request-local-review-profiles.json",
            "valid/response-local-review-profiles.json",
            "valid/request-local-review-start.json",
            "valid/response-local-review-start.json",
            "valid/request-local-review-history.json",
            "valid/response-local-review-history.json",
            "valid/request-local-review-findings.json",
            "valid/response-local-review-findings.json",
            "valid/request-local-review-output.json",
            "valid/response-local-review-output.json",
            "valid/request-local-review-threads.json",
            "valid/response-local-review-threads.json",
            "valid/request-local-options.json",
            "valid/response-local-options.json",
            "valid/request-local-snapshot.json",
            "valid/response-local-snapshot.json",
            "valid/request-local-cancel.json",
            "valid/response-local-cancel.json",
            "valid/request-local-info.json",
            "valid/response-local-info.json",
            "valid/request-local-files.json",
            "valid/response-local-files.json",
            "valid/request-local-image.json",
            "valid/response-local-image.json",
            "valid/request-local-browser.json",
            "valid/response-local-browser.json",
            "valid/request-local-list.json",
            "valid/response-local-list.json",
            "valid/request-local-diff.json",
            "valid/response-local-diff.json",
            "valid/request-local-recheck.json",
            "valid/response-local-recheck.json",
            "valid/request-review-profiles.json",
            "valid/response-review-profiles.json",
            "valid/request-provider-browser.json",
            "valid/request-provider-approval-preview.json",
            "valid/request-provider-approval-confirm.json",
            "valid/request-provider-approval-status.json",
            "valid/request-provider-approval-reconcile.json",
            "valid/response-provider-approval-preview.json",
            "valid/response-provider-approval-uncertain.json",
            "valid/response-provider-browser.json",
            "valid/request-provider-pr-detail.json",
            "valid/request-provider-pr-uuid.json",
            "valid/response-provider-prs.json",
            "valid/response-provider-user.json",
            "valid/request-workspace-read.json",
            "valid/request-workspace-mutate.json",
            "valid/response-workspace-page.json",
            "valid/response-ok.json",
            "valid/response-error.json",
            "valid/event-progress.json",
            "valid/event-state.json",
        ] {
            let bytes = read_fixture(fixture);
            let client = decode_client_line(&bytes);
            let server = decode_server_line(&bytes);
            assert!(
                client.is_ok() || server.is_ok(),
                "fixture {fixture} should decode: client={client:?} server={server:?}"
            );
        }
    }

    #[test]
    fn invalid_fixtures_are_rejected() {
        for fixture in [
            "invalid/unknown-method.json",
            "invalid/request-repository-explore-unsafe-path.json",
            "invalid/request-repository-explore-unknown-root.json",
            "invalid/request-repository-explore-oversize-page.json",
            "invalid/request-repository-explore-unknown-search.json",
            "invalid/request-repository-operation-root.json",
            "invalid/request-repository-operation-false-confirm.json",
            "invalid/request-repository-operation-missing-version.json",
            "invalid/request-repository-operation-unversioned-page.json",
            "invalid/request-repository-operation-oversized-page.json",
            "invalid/request-repository-operation-ref-kind.json",
            "invalid/request-repository-operation-ref-option.json",
            "invalid/request-repository-operation-ref-traversal.json",
            "invalid/request-repository-operation-remote-url.json",
            "invalid/request-repository-operation-pid.json",
            "invalid/request-repository-editor-program.json",
            "invalid/request-repository-editor-unsafe-path.json",
            "invalid/request-repository-editor-zero-line.json",
            "invalid/request-repository-editor-unknown-root.json",
            "invalid/request-repository-editor-guess-token.json",
            "invalid/request-repository-editor-pid.json",
            "invalid/request-repository-explore-unpinned.json",
            "invalid/error-code.json",
            "invalid/missing-id.json",
            "invalid/unknown-field.json",
            "invalid/version-mismatch.json",
            "invalid/bad-target.json",
            "invalid/request-provider-snapshot-local.json",
            "invalid/request-review-snapshot-local.json",
            "invalid/request-review-snapshot-repository.json",
            "invalid/request-review-snapshot-saved.json",
            "invalid/request-provider-snapshot-id.json",
            "invalid/request-provider-snapshot-offset.json",
            "invalid/request-provider-snapshot-image-offset.json",
            "invalid/request-provider-snapshot-image-chunk.json",
            "invalid/request-provider-snapshot-image-hash.json",
            "invalid/request-provider-snapshot-image-missing-hash.json",
            "invalid/request-provider-pr-unsafe-owner.json",
            "invalid/request-provider-pr-local-id.json",
            "invalid/request-local-review-profile-path.json",
            "invalid/request-local-review-path.json",
            "invalid/request-local-review-run.json",
            "invalid/request-local-review-page.json",
            "invalid/request-local-review-profile.json",
            "invalid/request-local-path.json",
            "invalid/request-local-layers.json",
            "invalid/request-local-nonce.json",
            "invalid/request-local-branch-layers.json",
            "invalid/request-local-image-path.json",
            "invalid/request-local-image-continuation.json",
            "invalid/request-local-image-layer.json",
            "invalid/request-local-browser-layer.json",
            "invalid/request-local-browser-missing-layer.json",
            "invalid/request-local-list-limit.json",
            "invalid/request-local-list-version.json",
            "invalid/request-local-list-missing-version.json",
            "invalid/request-local-diff-path.json",
            "invalid/request-review-profiles-path.json",
            "invalid/request-review-profiles-owner.json",
            "invalid/request-review-profiles-local.json",
            "invalid/request-provider-browser-local-id.json",
            "invalid/request-provider-browser-url.json",
            "invalid/request-provider-approval-token.json",
            "invalid/request-provider-approval-scope.json",
            "invalid/request-provider-approval-assertion.json",
            "invalid/request-workspace-unpinned-page.json",
            "invalid/request-workspace-empty-version.json",
        ] {
            let bytes = read_fixture(fixture);
            let client = decode_client_line(&bytes);
            let server = decode_server_line(&bytes);
            assert!(
                client.is_err() && server.is_err(),
                "fixture {fixture} should be rejected by both sides"
            );
        }
    }

    #[test]
    fn version_mismatch_is_reported_as_unsupported_version() {
        let bytes = read_fixture("invalid/version-mismatch.json");
        let error = decode_client_line(&bytes).expect_err("version mismatch should fail");
        assert_eq!(error.code, ProtocolErrorCode::UnsupportedVersion);
    }

    #[test]
    fn framing_rejects_oversized_and_empty_frames() {
        let oversized = vec![b'a'; MAX_FRAME_BYTES + 1];
        assert_eq!(
            decode_client_line(&oversized).unwrap_err().code,
            ProtocolErrorCode::InvalidRequest
        );
        assert!(decode_client_line(b"\n").is_err());
        assert!(decode_client_line(b"").is_err());
    }

    #[test]
    fn encoding_escapes_newlines_into_one_frame() {
        let frame = ServerMessage::Event(OperationEventFrame {
            operation_id: "op-1".to_string(),
            sequence: 1,
            target: TargetIdentity::Local {
                local_snapshot_sha256: "a".repeat(64),
            },
            event: OperationEvent::Progress {
                message: "line one\nline two ✓".to_string(),
                completed: Some(1),
                total: Some(3),
            },
        });
        let encoded = encode_line(&frame).expect("encode");
        assert_eq!(encoded.matches('\n').count(), 1);
        assert!(encoded.ends_with('\n'));
        let decoded = decode_server_line(encoded.as_bytes()).expect("round trip");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn sequence_guard_rejects_duplicates_and_backwards_progress() {
        let mut guard = SequenceGuard::new();
        assert_eq!(guard.observe("op-1", 1).expect("first"), None);
        assert_eq!(guard.observe("op-1", 2).expect("second"), Some(1));
        let duplicate = guard.observe("op-1", 2).expect_err("duplicate");
        assert_eq!(duplicate.code, ProtocolErrorCode::Conflict);
        let backwards = guard.observe("op-1", 0).expect_err("backwards");
        assert_eq!(backwards.code, ProtocolErrorCode::Conflict);
        // Different operation identities are tracked independently.
        assert_eq!(guard.observe("op-2", 1).expect("other op"), None);
        assert_eq!(guard.highest("op-1"), Some(2));
    }

    #[test]
    fn target_identity_round_trips_provider_and_local() {
        for value in [
            serde_json::json!({"kind":"provider","provider":"bitbucket","workspace":"ws","repo":"r","prId":7}),
            serde_json::json!({"kind":"local","localSnapshotSha256":"b".repeat(64)}),
        ] {
            let target: TargetIdentity = serde_json::from_value(value).expect("target");
            let encoded = serde_json::to_value(&target).expect("encode");
            let decoded: TargetIdentity = serde_json::from_value(encoded).expect("decode");
            assert_eq!(decoded, target);
        }
    }
}
