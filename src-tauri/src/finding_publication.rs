//! Explicit, provider-neutral publication of structured review findings.
//!
//! Provider adapters implement [`ProviderInlineCommentApi`]. The publisher
//! validates anchors, fences writes to the reviewed head, and uses a stable
//! hidden marker to make retries idempotent without rerunning review work.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::review_event::PullRequestReviewEventProvider;
use crate::review_storage;

const MAX_IDENTIFIER_BYTES: usize = 512;
const MAX_PATH_BYTES: usize = 4096;
const MAX_TITLE_BYTES: usize = 1024;
const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_SUGGESTED_FIX_BYTES: usize = 64 * 1024;
const MAX_PROVIDER_MARKDOWN_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingPublicationSchemaVersion {
    #[serde(rename = "v1")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl FindingSeverity {
    const fn label(self) -> &'static str {
        match self {
            Self::Critical => "Critical",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
            Self::Info => "Info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingAnchorSide {
    Old,
    New,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingLineRange {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub side: FindingAnchorSide,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingPublicationRequest {
    pub schema_version: FindingPublicationSchemaVersion,
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repository: String,
    pub pull_request_id: u64,
    /// Full immutable head SHA used by the review that produced the finding.
    pub head_sha: String,
    /// Full immutable destination/base SHA used by the same review snapshot.
    pub base_sha: String,
    pub finding_fingerprint: String,
    pub anchor: FindingLineRange,
    pub title: String,
    pub body: String,
    pub severity: FindingSeverity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedCommentIdentity {
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repository: String,
    pub pull_request_id: u64,
    pub comment_id: String,
    pub finding_marker: String,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub side: FindingAnchorSide,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCommentIdentity {
    pub comment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPullRequestRevision {
    pub head_sha: String,
    pub base_sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPublicationTarget {
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repository: String,
    pub pull_request_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInlineCommentPayload {
    pub head_sha: String,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub side: FindingAnchorSide,
    pub markdown: String,
}

pub trait ProviderInlineCommentApi {
    /// Returns the provider's current pull-request head and base commit IDs.
    fn current_revision(
        &self,
        target: &ProviderPublicationTarget,
    ) -> Result<ProviderPullRequestRevision, ProviderPublicationApiError>;

    /// Finds an existing non-deleted inline comment containing `marker`.
    fn find_inline_comment(
        &self,
        target: &ProviderPublicationTarget,
        marker: &str,
        expected: &ProviderInlineCommentPayload,
    ) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError>;

    /// Finds any non-deleted comment containing `marker`, regardless of its
    /// provider-returned anchor. This is used only to remove failed writes
    /// before retrying, never to declare a publication successful.
    fn find_comment_by_marker(
        &self,
        target: &ProviderPublicationTarget,
        marker: &str,
    ) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError>;

    /// Creates an inline comment. Implementations must never fall back to a
    /// file-level or top-level comment when the anchor is rejected.
    fn create_inline_comment(
        &self,
        target: &ProviderPublicationTarget,
        payload: &ProviderInlineCommentPayload,
    ) -> Result<ProviderCommentIdentity, ProviderPublicationApiError>;

    /// Removes a comment created by the current attempt when post-write
    /// fencing proves that it must not be published.
    fn delete_comment(
        &self,
        target: &ProviderPublicationTarget,
        identity: &ProviderCommentIdentity,
    ) -> Result<(), ProviderPublicationApiError>;
}

impl<T> ProviderInlineCommentApi for &T
where
    T: ProviderInlineCommentApi + ?Sized,
{
    fn current_revision(
        &self,
        target: &ProviderPublicationTarget,
    ) -> Result<ProviderPullRequestRevision, ProviderPublicationApiError> {
        (**self).current_revision(target)
    }

    fn find_inline_comment(
        &self,
        target: &ProviderPublicationTarget,
        marker: &str,
        expected: &ProviderInlineCommentPayload,
    ) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError> {
        (**self).find_inline_comment(target, marker, expected)
    }

    fn find_comment_by_marker(
        &self,
        target: &ProviderPublicationTarget,
        marker: &str,
    ) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError> {
        (**self).find_comment_by_marker(target, marker)
    }

    fn create_inline_comment(
        &self,
        target: &ProviderPublicationTarget,
        payload: &ProviderInlineCommentPayload,
    ) -> Result<ProviderCommentIdentity, ProviderPublicationApiError> {
        (**self).create_inline_comment(target, payload)
    }

    fn delete_comment(
        &self,
        target: &ProviderPublicationTarget,
        identity: &ProviderCommentIdentity,
    ) -> Result<(), ProviderPublicationApiError> {
        (**self).delete_comment(target, identity)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderPublicationApiErrorKind {
    InvalidAnchor,
    PermissionDenied,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPublicationApiError {
    pub kind: ProviderPublicationApiErrorKind,
    pub message: String,
}

impl ProviderPublicationApiError {
    pub fn invalid_anchor(message: impl Into<String>) -> Self {
        Self {
            kind: ProviderPublicationApiErrorKind::InvalidAnchor,
            message: message.into(),
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: ProviderPublicationApiErrorKind::Unavailable,
            message: message.into(),
        }
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self {
            kind: ProviderPublicationApiErrorKind::PermissionDenied,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingPublicationLease {
    pub marker: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingPublicationReservation {
    Acquired(FindingPublicationLease),
    InProgress,
    Published(ProviderCommentIdentity),
}

pub trait FindingPublicationStore {
    /// Atomically reserves a marker, returns its durable result, or reports
    /// another live publisher. Expired reservations may be reclaimed.
    fn reserve(
        &self,
        request: &FindingPublicationRequest,
        marker: &str,
    ) -> Result<FindingPublicationReservation, String>;

    /// Persists the provider identity only for the current fenced lease.
    fn complete(
        &self,
        lease: &FindingPublicationLease,
        identity: &ProviderCommentIdentity,
    ) -> Result<(), String>;

    /// Releases a failed attempt only when the caller still owns the lease.
    fn release(&self, lease: &FindingPublicationLease) -> Result<(), String>;

    /// Confirms that an exact marker and provider comment were durably
    /// published for the requested target and finding lineage.
    fn owns_published_comment(
        &self,
        marker: &str,
        identity: &ProviderCommentIdentity,
        target: &ProviderPublicationTarget,
        finding_fingerprint: &str,
    ) -> Result<bool, String> {
        let _ = (marker, identity, target, finding_fingerprint);
        Ok(false)
    }
}

impl<T> FindingPublicationStore for &T
where
    T: FindingPublicationStore + ?Sized,
{
    fn reserve(
        &self,
        request: &FindingPublicationRequest,
        marker: &str,
    ) -> Result<FindingPublicationReservation, String> {
        (**self).reserve(request, marker)
    }

    fn complete(
        &self,
        lease: &FindingPublicationLease,
        identity: &ProviderCommentIdentity,
    ) -> Result<(), String> {
        (**self).complete(lease, identity)
    }

    fn release(&self, lease: &FindingPublicationLease) -> Result<(), String> {
        (**self).release(lease)
    }

    fn owns_published_comment(
        &self,
        marker: &str,
        identity: &ProviderCommentIdentity,
        target: &ProviderPublicationTarget,
        finding_fingerprint: &str,
    ) -> Result<bool, String> {
        (**self).owns_published_comment(marker, identity, target, finding_fingerprint)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SqliteFindingPublicationStore;

impl FindingPublicationStore for SqliteFindingPublicationStore {
    fn reserve(
        &self,
        request: &FindingPublicationRequest,
        marker: &str,
    ) -> Result<FindingPublicationReservation, String> {
        review_storage::reserve_finding_publication(request, marker, &next_lease_token())
    }

    fn complete(
        &self,
        lease: &FindingPublicationLease,
        identity: &ProviderCommentIdentity,
    ) -> Result<(), String> {
        review_storage::complete_finding_publication(lease, identity)
    }

    fn release(&self, lease: &FindingPublicationLease) -> Result<(), String> {
        review_storage::release_finding_publication(lease)
    }

    fn owns_published_comment(
        &self,
        marker: &str,
        identity: &ProviderCommentIdentity,
        target: &ProviderPublicationTarget,
        finding_fingerprint: &str,
    ) -> Result<bool, String> {
        review_storage::finding_publication_owns_comment(
            marker,
            identity,
            target,
            finding_fingerprint,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingPublicationErrorCode {
    InvalidRequest,
    AnchorRejected,
    OutdatedAnchor,
    PublicationInProgress,
    PermissionDenied,
    ProviderUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingPublicationError {
    pub code: FindingPublicationErrorCode,
    pub retryable: bool,
    pub message: String,
}

impl fmt::Display for FindingPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for FindingPublicationError {}

#[derive(Debug)]
pub struct FindingPublisher<A, S> {
    api: A,
    store: S,
}

impl<A, S> FindingPublisher<A, S>
where
    A: ProviderInlineCommentApi,
    S: FindingPublicationStore,
{
    pub const fn new(api: A, store: S) -> Self {
        Self { api, store }
    }

    /// Publishes only when explicitly called by the user or service
    /// coordinator. Validation and lookup happen before any provider write.
    pub fn publish(
        &self,
        request: &FindingPublicationRequest,
    ) -> Result<PublishedCommentIdentity, FindingPublicationError> {
        request.validate().map_err(invalid_request)?;
        let target = request.target();
        let marker = finding_marker(request);
        let legacy_marker = legacy_marker(&marker);
        let legacy_lease = match self
            .store
            .reserve(request, &legacy_marker)
            .map_err(publication_state_error)?
        {
            FindingPublicationReservation::Published(existing) => {
                return Ok(request.published_identity(existing.comment_id, legacy_marker));
            }
            FindingPublicationReservation::InProgress => {
                return Err(FindingPublicationError {
                    code: FindingPublicationErrorCode::PublicationInProgress,
                    retryable: true,
                    message: "This finding is already being published.".to_string(),
                });
            }
            FindingPublicationReservation::Acquired(lease) => lease,
        };
        let reservation = self.store.reserve(request, &marker);
        let lease = match reservation {
            Err(error) => {
                let _ = self.store.release(&legacy_lease);
                return Err(publication_state_error(error));
            }
            Ok(FindingPublicationReservation::Published(existing)) => {
                let _ = self.store.release(&legacy_lease);
                return Ok(request.published_identity(existing.comment_id, marker));
            }
            Ok(FindingPublicationReservation::InProgress) => {
                let _ = self.store.release(&legacy_lease);
                return Err(FindingPublicationError {
                    code: FindingPublicationErrorCode::PublicationInProgress,
                    retryable: true,
                    message: "This finding is already being published.".to_string(),
                });
            }
            Ok(FindingPublicationReservation::Acquired(lease)) => lease,
        };

        let result = self.publish_reserved(request, &target, &marker, &legacy_marker, &lease);
        let _ = self.store.release(&legacy_lease);
        if result.is_err() {
            let _ = self.store.release(&lease);
        }
        result
    }

    fn publish_reserved(
        &self,
        request: &FindingPublicationRequest,
        target: &ProviderPublicationTarget,
        marker: &str,
        legacy_marker: &str,
        lease: &FindingPublicationLease,
    ) -> Result<PublishedCommentIdentity, FindingPublicationError> {
        let payload = request.provider_payload(marker);
        let legacy_payload = request.provider_payload(legacy_marker);
        let current_revision = self
            .api
            .current_revision(target)
            .map_err(publication_api_error)?;
        if !request.matches_revision(&current_revision) {
            for candidate in [marker, legacy_marker] {
                if let Some(stale) = self
                    .api
                    .find_comment_by_marker(target, candidate)
                    .map_err(publication_api_error)?
                {
                    self.api
                        .delete_comment(target, &stale)
                        .map_err(publication_api_error)?;
                }
            }
            return Err(FindingPublicationError {
                code: FindingPublicationErrorCode::OutdatedAnchor,
                retryable: false,
                message: "The pull request changed after this finding was produced; review the current diff before publishing.".to_string(),
            });
        }
        let published = if let Some(existing) = self
            .api
            .find_inline_comment(target, marker, &payload)
            .map_err(publication_api_error)?
        {
            existing
        } else if let Some(existing) = self
            .api
            .find_inline_comment(target, legacy_marker, &legacy_payload)
            .map_err(publication_api_error)?
        {
            existing
        } else {
            for candidate in [marker, legacy_marker] {
                if let Some(orphan) = self
                    .api
                    .find_comment_by_marker(target, candidate)
                    .map_err(publication_api_error)?
                {
                    self.api
                        .delete_comment(target, &orphan)
                        .map_err(publication_api_error)?;
                }
            }
            self.api
                .create_inline_comment(target, &payload)
                .map_err(publication_api_error)?
        };
        let current_revision = self
            .api
            .current_revision(target)
            .map_err(publication_api_error)?;
        if !request.matches_revision(&current_revision) {
            self.api
                .delete_comment(target, &published)
                .map_err(publication_api_error)?;
            return Err(FindingPublicationError {
                code: FindingPublicationErrorCode::OutdatedAnchor,
                retryable: false,
                message: "The pull request changed while this finding was being published; the stale comment was removed.".to_string(),
            });
        }
        self.store
            .complete(lease, &published)
            .map_err(publication_state_error)?;
        Ok(request.published_identity(published.comment_id, marker.to_string()))
    }
}

impl FindingPublicationRequest {
    pub fn validate(&self) -> Result<(), String> {
        validate_identifier("tenantId", &self.tenant_id)?;
        validate_identifier("workspace", &self.workspace)?;
        validate_identifier("repository", &self.repository)?;
        if self.pull_request_id == 0 {
            return Err("`pullRequestId` must be a positive integer".to_string());
        }
        validate_sha("headSha", &self.head_sha)?;
        validate_sha("baseSha", &self.base_sha)?;
        validate_identifier("findingFingerprint", &self.finding_fingerprint)?;
        validate_path(&self.anchor.path)?;
        if self.anchor.start_line == 0 || self.anchor.end_line == 0 {
            return Err("finding line numbers must be positive".to_string());
        }
        if self.anchor.start_line > self.anchor.end_line {
            return Err("finding start line must not exceed its end line".to_string());
        }
        validate_text("title", &self.title, MAX_TITLE_BYTES)?;
        validate_text("body", &self.body, MAX_BODY_BYTES)?;
        if let Some(suggested_fix) = &self.suggested_fix {
            validate_text("suggestedFix", suggested_fix, MAX_SUGGESTED_FIX_BYTES)?;
        }
        let rendered = render_finding_markdown(self, &finding_marker(self));
        if rendered.len() > MAX_PROVIDER_MARKDOWN_BYTES {
            return Err("rendered finding markdown is too long for provider comments".to_string());
        }
        Ok(())
    }

    pub(crate) fn target(&self) -> ProviderPublicationTarget {
        ProviderPublicationTarget {
            tenant_id: self.tenant_id.clone(),
            provider: self.provider,
            workspace: self.workspace.clone(),
            repository: self.repository.clone(),
            pull_request_id: self.pull_request_id,
        }
    }

    pub(crate) fn provider_payload(&self, marker: &str) -> ProviderInlineCommentPayload {
        ProviderInlineCommentPayload {
            head_sha: self.head_sha.clone(),
            path: self.anchor.path.clone(),
            start_line: self.anchor.start_line,
            end_line: self.anchor.end_line,
            side: self.anchor.side,
            markdown: render_finding_markdown(self, marker),
        }
    }

    pub(crate) fn matches_revision(&self, revision: &ProviderPullRequestRevision) -> bool {
        revision.head_sha.eq_ignore_ascii_case(&self.head_sha)
            && revision.base_sha.eq_ignore_ascii_case(&self.base_sha)
    }

    fn published_identity(
        &self,
        comment_id: String,
        finding_marker: String,
    ) -> PublishedCommentIdentity {
        PublishedCommentIdentity {
            tenant_id: self.tenant_id.clone(),
            provider: self.provider,
            workspace: self.workspace.clone(),
            repository: self.repository.clone(),
            pull_request_id: self.pull_request_id,
            comment_id,
            finding_marker,
            path: self.anchor.path.clone(),
            start_line: self.anchor.start_line,
            end_line: self.anchor.end_line,
            side: self.anchor.side,
        }
    }
}

pub fn finding_marker(request: &FindingPublicationRequest) -> String {
    let pull_request_id = request.pull_request_id.to_string();
    let head_sha = request.head_sha.to_ascii_lowercase();
    let base_sha = request.base_sha.to_ascii_lowercase();
    let workspace = request.workspace.to_ascii_lowercase();
    let repository = request.repository.to_ascii_lowercase();
    let start_line = request.anchor.start_line.to_string();
    let end_line = request.anchor.end_line.to_string();
    let anchor_side = match request.anchor.side {
        FindingAnchorSide::Old => "old",
        FindingAnchorSide::New => "new",
    };
    let suggested_fix_presence = if request.suggested_fix.is_some() {
        "present"
    } else {
        "absent"
    };
    let mut hasher = Sha256::new();
    for part in [
        request.tenant_id.as_str(),
        request.provider.as_str(),
        workspace.as_str(),
        repository.as_str(),
        pull_request_id.as_str(),
        base_sha.as_str(),
        head_sha.as_str(),
        request.finding_fingerprint.as_str(),
        request.anchor.path.as_str(),
        start_line.as_str(),
        end_line.as_str(),
        anchor_side,
        request.title.as_str(),
        request.body.as_str(),
        request.severity.label(),
        suggested_fix_presence,
        request.suggested_fix.as_deref().unwrap_or(""),
    ] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    format!("<!-- norn:finding:{digest:x} -->")
}

fn legacy_marker(canonical_marker: &str) -> String {
    canonical_marker.replacen("<!-- norn:", "<!-- lachesi:", 1)
}

pub fn finding_lineage_marker(request: &FindingPublicationRequest) -> String {
    finding_lineage_marker_for(
        &request.tenant_id,
        request.provider,
        &request.workspace,
        &request.repository,
        request.pull_request_id,
        &request.finding_fingerprint,
    )
}

pub(crate) fn finding_lineage_marker_for(
    tenant_id: &str,
    provider: PullRequestReviewEventProvider,
    workspace: &str,
    repository: &str,
    pull_request_id: u64,
    finding_fingerprint: &str,
) -> String {
    let pull_request_id = pull_request_id.to_string();
    let workspace = workspace.to_ascii_lowercase();
    let repository = repository.to_ascii_lowercase();
    let mut hasher = Sha256::new();
    for part in [
        tenant_id,
        provider.as_str(),
        workspace.as_str(),
        repository.as_str(),
        pull_request_id.as_str(),
        finding_fingerprint,
    ] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    format!("<!-- norn:finding-lineage:{digest:x} -->")
}

pub fn dry_run_publication_identity(
    request: &FindingPublicationRequest,
) -> Result<PublishedCommentIdentity, FindingPublicationError> {
    request.validate().map_err(invalid_request)?;
    Ok(request.published_identity("dry-run".to_string(), finding_marker(request)))
}

pub(crate) fn render_finding_markdown(request: &FindingPublicationRequest, marker: &str) -> String {
    let mut markdown = format!(
        "**{}**\n\n{}\n\nSeverity: **{}**",
        request.title,
        request.body,
        request.severity.label()
    );
    if let Some(suggested_fix) = &request.suggested_fix {
        let fence = suggestion_fence(suggested_fix);
        markdown.push_str("\n\nSuggested fix:\n\n");
        markdown.push_str(&fence);
        markdown.push_str("suggestion\n");
        markdown.push_str(suggested_fix);
        if !suggested_fix.ends_with('\n') {
            markdown.push('\n');
        }
        markdown.push_str(&fence);
    }
    markdown.push_str("\n\n");
    markdown.push_str(&finding_lineage_marker(request));
    markdown.push('\n');
    markdown.push_str(marker);
    markdown
}

fn suggestion_fence(suggested_fix: &str) -> String {
    let longest_run = suggested_fix
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    "`".repeat(longest_run.saturating_add(1).max(3))
}

fn validate_identifier(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("`{field}` must not be empty"));
    }
    if value != value.trim() {
        return Err(format!("`{field}` must not contain surrounding whitespace"));
    }
    if value.len() > MAX_IDENTIFIER_BYTES {
        return Err(format!("`{field}` is too long"));
    }
    Ok(())
}

fn validate_sha(field: &str, value: &str) -> Result<(), String> {
    if matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("`{field}` must be a full hexadecimal commit SHA"))
    }
}

fn validate_path(value: &str) -> Result<(), String> {
    validate_text("anchor.path", value, MAX_PATH_BYTES)?;
    if value.starts_with('/')
        || value.split('/').any(|segment| segment == "..")
        || value.contains('\\')
        || value.bytes().any(|byte| byte == 0)
    {
        return Err("`anchor.path` must be a relative repository path".to_string());
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, maximum: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("`{field}` must not be empty"));
    }
    if value.len() > maximum {
        return Err(format!("`{field}` is too long"));
    }
    Ok(())
}

fn invalid_request(message: String) -> FindingPublicationError {
    FindingPublicationError {
        code: FindingPublicationErrorCode::InvalidRequest,
        retryable: false,
        message,
    }
}

fn publication_api_error(error: ProviderPublicationApiError) -> FindingPublicationError {
    match error.kind {
        ProviderPublicationApiErrorKind::InvalidAnchor => FindingPublicationError {
            code: FindingPublicationErrorCode::AnchorRejected,
            retryable: false,
            message: error.message,
        },
        ProviderPublicationApiErrorKind::PermissionDenied => FindingPublicationError {
            code: FindingPublicationErrorCode::PermissionDenied,
            retryable: false,
            message: error.message,
        },
        ProviderPublicationApiErrorKind::Unavailable => FindingPublicationError {
            code: FindingPublicationErrorCode::ProviderUnavailable,
            retryable: true,
            message: error.message,
        },
    }
}

fn publication_state_error(message: String) -> FindingPublicationError {
    FindingPublicationError {
        code: FindingPublicationErrorCode::ProviderUnavailable,
        retryable: true,
        message,
    }
}

fn next_lease_token() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("lease:{}:{now}:{sequence}", std::process::id())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    const BASE_SHA: &str = "1111111111111111111111111111111111111111";
    const HEAD_SHA: &str = "2222222222222222222222222222222222222222";

    #[derive(Debug, Default)]
    struct MockProviderApi {
        state: Mutex<MockProviderState>,
    }

    #[derive(Debug)]
    struct MockProviderState {
        current_head_sha: String,
        current_base_sha: String,
        comments: HashMap<String, ProviderCommentIdentity>,
        mismatched_comments: HashMap<String, ProviderCommentIdentity>,
        payloads: Vec<(PullRequestReviewEventProvider, ProviderInlineCommentPayload)>,
        fail_next_write: bool,
        fail_next_delete: bool,
        orphan_next_write: bool,
        advance_head_on_write: bool,
        advance_head_on_find: bool,
        deleted_comment_ids: Vec<String>,
    }

    impl Default for MockProviderState {
        fn default() -> Self {
            Self {
                current_head_sha: HEAD_SHA.to_string(),
                current_base_sha: BASE_SHA.to_string(),
                comments: HashMap::new(),
                mismatched_comments: HashMap::new(),
                payloads: Vec::new(),
                fail_next_write: false,
                fail_next_delete: false,
                orphan_next_write: false,
                advance_head_on_write: false,
                advance_head_on_find: false,
                deleted_comment_ids: Vec::new(),
            }
        }
    }

    impl ProviderInlineCommentApi for MockProviderApi {
        fn current_revision(
            &self,
            _target: &ProviderPublicationTarget,
        ) -> Result<ProviderPullRequestRevision, ProviderPublicationApiError> {
            let state = self.state.lock().unwrap();
            Ok(ProviderPullRequestRevision {
                head_sha: state.current_head_sha.clone(),
                base_sha: state.current_base_sha.clone(),
            })
        }

        fn find_inline_comment(
            &self,
            _target: &ProviderPublicationTarget,
            marker: &str,
            _expected: &ProviderInlineCommentPayload,
        ) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError> {
            let mut state = self.state.lock().unwrap();
            let existing = state.comments.get(marker).cloned();
            if existing.is_some() && state.advance_head_on_find {
                state.advance_head_on_find = false;
                state.current_head_sha = "3333333333333333333333333333333333333333".to_string();
            }
            Ok(existing)
        }

        fn find_comment_by_marker(
            &self,
            _target: &ProviderPublicationTarget,
            marker: &str,
        ) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError> {
            let state = self.state.lock().unwrap();
            Ok(state
                .comments
                .get(marker)
                .or_else(|| state.mismatched_comments.get(marker))
                .cloned())
        }

        fn create_inline_comment(
            &self,
            target: &ProviderPublicationTarget,
            payload: &ProviderInlineCommentPayload,
        ) -> Result<ProviderCommentIdentity, ProviderPublicationApiError> {
            let mut state = self.state.lock().unwrap();
            if state.fail_next_write {
                state.fail_next_write = false;
                return Err(ProviderPublicationApiError::unavailable(
                    "provider temporarily unavailable",
                ));
            }
            let marker = payload
                .markdown
                .lines()
                .last()
                .expect("marker is the last line")
                .to_string();
            let identity = ProviderCommentIdentity {
                comment_id: format!("comment-{}", state.payloads.len() + 1),
            };
            state.payloads.push((target.provider, payload.clone()));
            if state.orphan_next_write {
                state.orphan_next_write = false;
                state.mismatched_comments.insert(marker, identity);
                return Err(ProviderPublicationApiError::unavailable(
                    "provider created a wrong-anchor comment and cleanup failed",
                ));
            }
            state.comments.insert(marker, identity.clone());
            if state.advance_head_on_write {
                state.current_head_sha = "3333333333333333333333333333333333333333".to_string();
            }
            Ok(identity)
        }

        fn delete_comment(
            &self,
            _target: &ProviderPublicationTarget,
            identity: &ProviderCommentIdentity,
        ) -> Result<(), ProviderPublicationApiError> {
            let mut state = self.state.lock().unwrap();
            if state.fail_next_delete {
                state.fail_next_delete = false;
                return Err(ProviderPublicationApiError::unavailable(
                    "provider delete temporarily unavailable",
                ));
            }
            state.comments.retain(|_, current| current != identity);
            state
                .mismatched_comments
                .retain(|_, current| current != identity);
            state.deleted_comment_ids.push(identity.comment_id.clone());
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct MockPublicationStore {
        state: Mutex<HashMap<String, MockPublicationState>>,
    }

    #[derive(Debug, Clone)]
    enum MockPublicationState {
        Publishing(FindingPublicationLease),
        Published(ProviderCommentIdentity),
    }

    impl FindingPublicationStore for MockPublicationStore {
        fn reserve(
            &self,
            _request: &FindingPublicationRequest,
            marker: &str,
        ) -> Result<FindingPublicationReservation, String> {
            let mut state = self.state.lock().unwrap();
            match state.get(marker) {
                Some(MockPublicationState::Publishing(_)) => {
                    Ok(FindingPublicationReservation::InProgress)
                }
                Some(MockPublicationState::Published(identity)) => {
                    Ok(FindingPublicationReservation::Published(identity.clone()))
                }
                None => {
                    let lease = FindingPublicationLease {
                        marker: marker.to_string(),
                        token: format!("test-lease-{}", state.len() + 1),
                    };
                    state.insert(
                        marker.to_string(),
                        MockPublicationState::Publishing(lease.clone()),
                    );
                    Ok(FindingPublicationReservation::Acquired(lease))
                }
            }
        }

        fn complete(
            &self,
            lease: &FindingPublicationLease,
            identity: &ProviderCommentIdentity,
        ) -> Result<(), String> {
            let mut state = self.state.lock().unwrap();
            match state.get(&lease.marker) {
                Some(MockPublicationState::Publishing(current)) if current == lease => {
                    state.insert(
                        lease.marker.clone(),
                        MockPublicationState::Published(identity.clone()),
                    );
                    Ok(())
                }
                _ => Err("publication lease was fenced".to_string()),
            }
        }

        fn release(&self, lease: &FindingPublicationLease) -> Result<(), String> {
            let mut state = self.state.lock().unwrap();
            if matches!(
                state.get(&lease.marker),
                Some(MockPublicationState::Publishing(current)) if current == lease
            ) {
                state.remove(&lease.marker);
            }
            Ok(())
        }
    }

    #[derive(Debug)]
    struct RetryingCompletionStore {
        store: MockPublicationStore,
        fail_next_completion: Mutex<bool>,
    }

    impl Default for RetryingCompletionStore {
        fn default() -> Self {
            Self {
                store: MockPublicationStore::default(),
                fail_next_completion: Mutex::new(true),
            }
        }
    }

    impl FindingPublicationStore for RetryingCompletionStore {
        fn reserve(
            &self,
            request: &FindingPublicationRequest,
            marker: &str,
        ) -> Result<FindingPublicationReservation, String> {
            self.store.reserve(request, marker)
        }

        fn complete(
            &self,
            lease: &FindingPublicationLease,
            identity: &ProviderCommentIdentity,
        ) -> Result<(), String> {
            let mut fail_next = self.fail_next_completion.lock().unwrap();
            if *fail_next {
                *fail_next = false;
                return Err("publication database unavailable".to_string());
            }
            drop(fail_next);
            self.store.complete(lease, identity)
        }

        fn release(&self, lease: &FindingPublicationLease) -> Result<(), String> {
            self.store.release(lease)
        }
    }

    fn request(provider: PullRequestReviewEventProvider) -> FindingPublicationRequest {
        FindingPublicationRequest {
            schema_version: FindingPublicationSchemaVersion::V1,
            tenant_id: "tenant-acme".to_string(),
            provider,
            workspace: "acme".to_string(),
            repository: "payments".to_string(),
            pull_request_id: 42,
            head_sha: HEAD_SHA.to_string(),
            base_sha: BASE_SHA.to_string(),
            finding_fingerprint: "finding:src/lib.rs:12:null-check".to_string(),
            anchor: FindingLineRange {
                path: "src/lib.rs".to_string(),
                start_line: 12,
                end_line: 14,
                side: FindingAnchorSide::New,
            },
            title: "Guard the nullable value".to_string(),
            body: "This value can be absent on the error path.".to_string(),
            severity: FindingSeverity::High,
            suggested_fix: Some("let value = value?;".to_string()),
        }
    }

    #[test]
    fn same_contract_publishes_through_github_and_bitbucket_adapters() {
        for provider in [
            PullRequestReviewEventProvider::Github,
            PullRequestReviewEventProvider::Bitbucket,
        ] {
            let publisher =
                FindingPublisher::new(MockProviderApi::default(), MockPublicationStore::default());
            let published = publisher.publish(&request(provider)).expect("publication");

            assert_eq!(published.tenant_id, "tenant-acme");
            assert_eq!(published.provider, provider);
            assert_eq!(published.comment_id, "comment-1");
            let state = publisher.api.state.lock().unwrap();
            assert_eq!(state.payloads.len(), 1);
            assert_eq!(state.payloads[0].0, provider);
            assert_eq!(state.payloads[0].1.start_line, 12);
            assert_eq!(state.payloads[0].1.end_line, 14);
            assert!(state.payloads[0].1.markdown.contains("```suggestion"));
            assert!(state.payloads[0].1.markdown.contains("<!-- norn:finding:"));
        }
    }

    #[test]
    fn repeated_request_returns_existing_comment_without_another_write() {
        let publisher =
            FindingPublisher::new(MockProviderApi::default(), MockPublicationStore::default());
        let request = request(PullRequestReviewEventProvider::Github);

        let first = publisher.publish(&request).expect("first publication");
        let repeated = publisher.publish(&request).expect("idempotent publication");

        assert_eq!(repeated, first);
        assert_eq!(publisher.api.state.lock().unwrap().payloads.len(), 1);
    }

    #[test]
    fn direct_publish_reuses_a_legacy_publication_record() {
        let api = MockProviderApi::default();
        let store = MockPublicationStore::default();
        let request = request(PullRequestReviewEventProvider::Github);
        let legacy = legacy_marker(&finding_marker(&request));
        let identity = ProviderCommentIdentity {
            comment_id: "legacy-comment".to_string(),
        };
        store.state.lock().unwrap().insert(
            legacy.clone(),
            MockPublicationState::Published(identity.clone()),
        );
        let publisher = FindingPublisher::new(api, store);

        let published = publisher
            .publish(&request)
            .expect("legacy publication should remain idempotent");

        assert_eq!(published.comment_id, identity.comment_id);
        assert_eq!(published.finding_marker, legacy);
        assert!(publisher.api.state.lock().unwrap().payloads.is_empty());
    }

    #[test]
    fn direct_publish_backfills_a_legacy_provider_comment_without_duplication() {
        let api = MockProviderApi::default();
        let store = MockPublicationStore::default();
        let request = request(PullRequestReviewEventProvider::Github);
        let canonical = finding_marker(&request);
        let legacy = legacy_marker(&canonical);
        let identity = ProviderCommentIdentity {
            comment_id: "legacy-comment".to_string(),
        };
        api.state
            .lock()
            .unwrap()
            .comments
            .insert(legacy, identity.clone());
        let publisher = FindingPublisher::new(api, store);

        let published = publisher
            .publish(&request)
            .expect("legacy provider comment should be recovered");

        assert_eq!(published.comment_id, identity.comment_id);
        assert_eq!(published.finding_marker, canonical);
        assert!(publisher.api.state.lock().unwrap().payloads.is_empty());
        assert!(matches!(
            publisher.store.state.lock().unwrap().get(&canonical),
            Some(MockPublicationState::Published(stored)) if stored == &identity
        ));
    }

    #[test]
    fn live_reservation_prevents_a_concurrent_duplicate_write() {
        let request = request(PullRequestReviewEventProvider::Github);
        let marker = finding_marker(&request);
        let store = MockPublicationStore::default();
        let reservation = store.reserve(&request, &marker).expect("first reservation");
        assert!(matches!(
            reservation,
            FindingPublicationReservation::Acquired(_)
        ));
        let publisher = FindingPublisher::new(MockProviderApi::default(), store);

        let error = publisher
            .publish(&request)
            .expect_err("concurrent publication is fenced");

        assert_eq!(
            error.code,
            FindingPublicationErrorCode::PublicationInProgress
        );
        assert!(error.retryable);
        assert!(publisher.api.state.lock().unwrap().payloads.is_empty());
    }

    #[test]
    fn invalid_and_outdated_anchors_never_create_comments() {
        let publisher =
            FindingPublisher::new(MockProviderApi::default(), MockPublicationStore::default());
        let mut invalid = request(PullRequestReviewEventProvider::Github);
        invalid.anchor.start_line = 15;
        invalid.anchor.end_line = 14;

        let invalid_error = publisher.publish(&invalid).expect_err("invalid anchor");
        assert_eq!(
            invalid_error.code,
            FindingPublicationErrorCode::InvalidRequest
        );

        let outdated = request(PullRequestReviewEventProvider::Github);
        publisher.api.state.lock().unwrap().current_head_sha =
            "3333333333333333333333333333333333333333".to_string();
        let outdated_error = publisher.publish(&outdated).expect_err("outdated anchor");
        assert_eq!(
            outdated_error.code,
            FindingPublicationErrorCode::OutdatedAnchor
        );
        assert!(!outdated_error.retryable);
        assert!(publisher.api.state.lock().unwrap().payloads.is_empty());
    }

    #[test]
    fn base_advance_before_write_rejects_the_review_snapshot() {
        let publisher =
            FindingPublisher::new(MockProviderApi::default(), MockPublicationStore::default());
        publisher.api.state.lock().unwrap().current_base_sha =
            "4444444444444444444444444444444444444444".to_string();

        let error = publisher
            .publish(&request(PullRequestReviewEventProvider::Github))
            .expect_err("outdated base");

        assert_eq!(error.code, FindingPublicationErrorCode::OutdatedAnchor);
        assert!(!error.retryable);
        assert!(publisher.api.state.lock().unwrap().payloads.is_empty());
    }

    #[test]
    fn oversized_rendered_markdown_is_rejected_before_publication_state_changes() {
        let publisher =
            FindingPublisher::new(MockProviderApi::default(), MockPublicationStore::default());
        let mut oversized = request(PullRequestReviewEventProvider::Github);
        oversized.body = "x".repeat(MAX_PROVIDER_MARKDOWN_BYTES);
        oversized.suggested_fix = None;

        let error = publisher
            .publish(&oversized)
            .expect_err("oversized rendered markdown");

        assert_eq!(error.code, FindingPublicationErrorCode::InvalidRequest);
        assert!(error.message.contains("markdown is too long"));
        assert!(publisher.api.state.lock().unwrap().payloads.is_empty());
    }

    #[test]
    fn head_advance_during_write_deletes_the_stale_comment() {
        let publisher =
            FindingPublisher::new(MockProviderApi::default(), MockPublicationStore::default());
        publisher.api.state.lock().unwrap().advance_head_on_write = true;
        let request = request(PullRequestReviewEventProvider::Github);

        let error = publisher
            .publish(&request)
            .expect_err("post-write head change");

        assert_eq!(error.code, FindingPublicationErrorCode::OutdatedAnchor);
        let state = publisher.api.state.lock().unwrap();
        assert_eq!(state.deleted_comment_ids, vec!["comment-1"]);
        assert!(state.comments.is_empty());
    }

    #[test]
    fn head_advance_during_existing_comment_recovery_deletes_the_stale_comment() {
        let publisher =
            FindingPublisher::new(MockProviderApi::default(), MockPublicationStore::default());
        let request = request(PullRequestReviewEventProvider::Github);
        let marker = finding_marker(&request);
        {
            let mut state = publisher.api.state.lock().unwrap();
            state.comments.insert(
                marker,
                ProviderCommentIdentity {
                    comment_id: "comment-existing".to_string(),
                },
            );
            state.advance_head_on_find = true;
        }

        let error = publisher
            .publish(&request)
            .expect_err("recovered comment must be post-fenced");

        assert_eq!(error.code, FindingPublicationErrorCode::OutdatedAnchor);
        let state = publisher.api.state.lock().unwrap();
        assert_eq!(state.deleted_comment_ids, vec!["comment-existing"]);
        assert!(state.comments.is_empty());
    }

    #[test]
    fn failed_stale_comment_cleanup_is_retried_before_outdated_rejection() {
        let publisher =
            FindingPublisher::new(MockProviderApi::default(), MockPublicationStore::default());
        {
            let mut state = publisher.api.state.lock().unwrap();
            state.advance_head_on_write = true;
            state.fail_next_delete = true;
        }
        let request = request(PullRequestReviewEventProvider::Github);

        let first = publisher
            .publish(&request)
            .expect_err("first cleanup attempt fails");
        assert_eq!(first.code, FindingPublicationErrorCode::ProviderUnavailable);
        assert!(first.retryable);
        assert_eq!(publisher.api.state.lock().unwrap().comments.len(), 1);

        let retry = publisher
            .publish(&request)
            .expect_err("the stale finding remains outdated");
        assert_eq!(retry.code, FindingPublicationErrorCode::OutdatedAnchor);
        let state = publisher.api.state.lock().unwrap();
        assert_eq!(state.deleted_comment_ids, vec!["comment-1"]);
        assert!(state.comments.is_empty());
    }

    #[test]
    fn retry_removes_wrong_anchor_marker_before_creating_again() {
        let publisher =
            FindingPublisher::new(MockProviderApi::default(), MockPublicationStore::default());
        publisher.api.state.lock().unwrap().orphan_next_write = true;
        let request = request(PullRequestReviewEventProvider::Github);

        let first = publisher
            .publish(&request)
            .expect_err("provider cleanup failure");
        assert_eq!(first.code, FindingPublicationErrorCode::ProviderUnavailable);
        assert_eq!(
            publisher
                .api
                .state
                .lock()
                .unwrap()
                .mismatched_comments
                .len(),
            1
        );

        let published = publisher.publish(&request).expect("clean retry");
        assert_eq!(published.comment_id, "comment-2");
        let state = publisher.api.state.lock().unwrap();
        assert_eq!(state.deleted_comment_ids, vec!["comment-1"]);
        assert!(state.mismatched_comments.is_empty());
        assert_eq!(state.comments.len(), 1);
    }

    #[test]
    fn completion_failure_leaves_the_marked_comment_for_idempotent_retry() {
        let publisher = FindingPublisher::new(
            MockProviderApi::default(),
            RetryingCompletionStore::default(),
        );
        let request = request(PullRequestReviewEventProvider::Github);

        let error = publisher
            .publish(&request)
            .expect_err("durable completion fails");

        assert_eq!(error.code, FindingPublicationErrorCode::ProviderUnavailable);
        assert!(publisher
            .api
            .state
            .lock()
            .unwrap()
            .deleted_comment_ids
            .is_empty());

        let published = publisher.publish(&request).expect("recover marked comment");
        assert_eq!(published.comment_id, "comment-1");
        let state = publisher.api.state.lock().unwrap();
        assert_eq!(state.payloads.len(), 1);
        assert_eq!(state.comments.len(), 1);
    }

    #[test]
    fn provider_failure_is_retryable_and_does_not_mutate_the_request() {
        let publisher =
            FindingPublisher::new(MockProviderApi::default(), MockPublicationStore::default());
        let request = request(PullRequestReviewEventProvider::Bitbucket);
        let expected = request.clone();
        publisher.api.state.lock().unwrap().fail_next_write = true;

        let error = publisher.publish(&request).expect_err("first write fails");
        assert_eq!(error.code, FindingPublicationErrorCode::ProviderUnavailable);
        assert!(error.retryable);
        assert_eq!(request, expected);

        let published = publisher.publish(&request).expect("retry succeeds");
        assert_eq!(published.comment_id, "comment-1");
    }

    #[test]
    fn marker_is_stable_and_does_not_embed_the_raw_fingerprint() {
        let mut request = request(PullRequestReviewEventProvider::Github);
        request.finding_fingerprint = "sensitive/source/path:12".to_string();
        let marker = finding_marker(&request);
        assert_eq!(marker, finding_marker(&request));
        assert!(!marker.contains("sensitive/source/path"));
        assert_eq!(marker.len(), "<!-- norn:finding: -->".len() + 64);

        request.head_sha = "3333333333333333333333333333333333333333".to_string();
        assert_ne!(marker, finding_marker(&request));
        request.head_sha = HEAD_SHA.to_string();
        request.base_sha = "4444444444444444444444444444444444444444".to_string();
        assert_ne!(marker, finding_marker(&request));
        request.base_sha = BASE_SHA.to_string();
        request.tenant_id = "tenant-other".to_string();
        assert_ne!(marker, finding_marker(&request));

        request.tenant_id = "tenant-acme".to_string();
        request.head_sha = HEAD_SHA.to_ascii_uppercase();
        assert_eq!(marker, finding_marker(&request));
        request.workspace = "ACME".to_string();
        request.repository = "PAYMENTS".to_string();
        assert_eq!(marker, finding_marker(&request));

        request.anchor.start_line = 13;
        assert_ne!(marker, finding_marker(&request));
        request.anchor.start_line = 12;
        request.body = "Updated finding body.".to_string();
        assert_ne!(marker, finding_marker(&request));
    }

    #[test]
    fn lineage_marker_stays_stable_across_review_revisions() {
        let mut request = request(PullRequestReviewEventProvider::Github);
        request.finding_fingerprint = "sensitive/source/path:12".to_string();
        let lineage = finding_lineage_marker(&request);
        assert!(!lineage.contains("sensitive/source/path"));
        assert_eq!(lineage.len(), "<!-- norn:finding-lineage: -->".len() + 64);

        request.head_sha = "3333333333333333333333333333333333333333".to_string();
        request.base_sha = "4444444444444444444444444444444444444444".to_string();
        request.anchor.start_line = 99;
        request.title = "Updated message".to_string();
        assert_eq!(lineage, finding_lineage_marker(&request));

        request.finding_fingerprint = "different-fingerprint".to_string();
        assert_ne!(lineage, finding_lineage_marker(&request));
    }

    #[test]
    fn suggestion_fence_is_longer_than_embedded_backtick_runs() {
        let mut request = request(PullRequestReviewEventProvider::Github);
        request.suggested_fix = Some("before\n````\nafter".to_string());
        let markdown = render_finding_markdown(&request, "<!-- marker -->");

        assert!(markdown.contains("`````suggestion\nbefore\n````\nafter\n`````"));
        assert!(markdown.contains("<!-- norn:finding-lineage:"));
        assert!(markdown.ends_with("<!-- marker -->"));
    }

    #[test]
    fn v1_contract_round_trips_without_credentials() {
        let request = request(PullRequestReviewEventProvider::Github);
        let value = serde_json::to_value(&request).expect("serialize");
        let decoded: FindingPublicationRequest =
            serde_json::from_value(value.clone()).expect("deserialize");

        assert_eq!(decoded, request);
        assert!(value.get("token").is_none());
        assert!(value.get("credentials").is_none());
        assert_eq!(value["schemaVersion"], "v1");
    }

    #[test]
    fn invalid_revision_errors_name_the_rejected_field() {
        let mut request = request(PullRequestReviewEventProvider::Github);
        request.base_sha = "invalid".to_string();
        assert_eq!(
            request.validate().expect_err("invalid base SHA"),
            "`baseSha` must be a full hexadecimal commit SHA"
        );

        request.base_sha = BASE_SHA.to_string();
        request.head_sha = "invalid".to_string();
        assert_eq!(
            request.validate().expect_err("invalid head SHA"),
            "`headSha` must be a full hexadecimal commit SHA"
        );
    }

    #[test]
    fn dry_run_identity_is_synthetic_without_reserving_publication_state() {
        let request = request(PullRequestReviewEventProvider::Github);
        let identity = dry_run_publication_identity(&request).expect("dry-run identity");

        assert_eq!(identity.comment_id, "dry-run");
        assert_eq!(identity.finding_marker, finding_marker(&request));
    }
}
