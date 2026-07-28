//! Provider-neutral reconciliation of Lachesi-authored finding comments.
//!
//! Reconciliation is explicit. It matches tracked publications by finding
//! fingerprint, verifies provider ownership through hidden markers, preserves
//! comment threads by editing rather than deleting, and delegates new writes
//! to the idempotent finding publisher.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::finding_publication::{
    finding_lineage_marker_for, finding_marker, render_finding_markdown, FindingLineRange,
    FindingPublicationError, FindingPublicationErrorCode, FindingPublicationRequest,
    FindingPublicationStore, FindingPublisher, ProviderCommentIdentity, ProviderInlineCommentApi,
    ProviderPublicationApiError, ProviderPublicationApiErrorKind, ProviderPublicationTarget,
    ProviderPullRequestRevision,
};
use crate::review_event::PullRequestReviewEventProvider;

const MAX_RECONCILIATION_FINDINGS: usize = 250;
const RESOLVED_NOTICE: &str = "> This finding is no longer present in the latest Lachesi review.";
const RESOLVED_MARKER: &str = "<!-- lachesi:finding-state:resolved -->";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingReconciliationSchemaVersion {
    #[serde(rename = "v1")]
    V1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrackedFindingComment {
    pub finding_fingerprint: String,
    pub comment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingReconciliationRequest {
    pub schema_version: FindingReconciliationSchemaVersion,
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repository: String,
    pub pull_request_id: u64,
    pub base_sha: String,
    pub head_sha: String,
    #[serde(default)]
    pub tracked_comments: Vec<TrackedFindingComment>,
    #[serde(default)]
    pub current_findings: Vec<FindingPublicationRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingReconciliationStatus {
    Succeeded,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingReconciliationActionKind {
    Unchanged,
    Created,
    Updated,
    Resolved,
    Reopened,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingReconciliationActionError {
    pub code: FindingPublicationErrorCode,
    pub retryable: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingReconciliationAction {
    pub finding_fingerprint: String,
    pub kind: FindingReconciliationActionKind,
    pub previous_comment_id: Option<String>,
    pub comment_id: Option<String>,
    pub provider_mutated: bool,
    pub error: Option<FindingReconciliationActionError>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingReconciliationCounts {
    pub unchanged: u32,
    pub created: u32,
    pub updated: u32,
    pub resolved: u32,
    pub reopened: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingReconciliationSummary {
    pub schema_version: FindingReconciliationSchemaVersion,
    pub status: FindingReconciliationStatus,
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repository: String,
    pub pull_request_id: u64,
    pub base_sha: String,
    pub head_sha: String,
    pub counts: FindingReconciliationCounts,
    pub actions: Vec<FindingReconciliationAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderFindingComment {
    pub identity: ProviderCommentIdentity,
    pub markdown: String,
    pub anchor: Option<FindingLineRange>,
}

pub trait ProviderFindingReconciliationApi: ProviderInlineCommentApi {
    /// Loads only a non-deleted comment authored by the authenticated provider
    /// identity. Implementations return `None` for missing or foreign comments.
    fn get_finding_comment(
        &self,
        target: &ProviderPublicationTarget,
        identity: &ProviderCommentIdentity,
    ) -> Result<Option<ProviderFindingComment>, ProviderPublicationApiError>;

    /// Edits the original comment body in place so provider replies and thread
    /// identity remain intact.
    fn update_finding_comment(
        &self,
        target: &ProviderPublicationTarget,
        identity: &ProviderCommentIdentity,
        markdown: &str,
    ) -> Result<(), ProviderPublicationApiError>;
}

impl<T> ProviderFindingReconciliationApi for &T
where
    T: ProviderFindingReconciliationApi + ?Sized,
{
    fn get_finding_comment(
        &self,
        target: &ProviderPublicationTarget,
        identity: &ProviderCommentIdentity,
    ) -> Result<Option<ProviderFindingComment>, ProviderPublicationApiError> {
        (**self).get_finding_comment(target, identity)
    }

    fn update_finding_comment(
        &self,
        target: &ProviderPublicationTarget,
        identity: &ProviderCommentIdentity,
        markdown: &str,
    ) -> Result<(), ProviderPublicationApiError> {
        (**self).update_finding_comment(target, identity, markdown)
    }
}

#[derive(Debug)]
pub struct FindingReconciler<A, S> {
    api: A,
    store: S,
}

struct FencedUpdateFailure {
    error: FindingPublicationError,
    provider_mutated: bool,
}

impl<A, S> FindingReconciler<A, S>
where
    A: ProviderFindingReconciliationApi,
    S: FindingPublicationStore,
{
    pub const fn new(api: A, store: S) -> Self {
        Self { api, store }
    }

    pub fn reconcile(
        &self,
        request: &FindingReconciliationRequest,
    ) -> Result<FindingReconciliationSummary, FindingPublicationError> {
        request.validate()?;
        let target = request.target();
        self.ensure_current_revision(request, &target)?;
        let publisher = FindingPublisher::new(&self.api, &self.store);
        let mut current = request
            .current_findings
            .iter()
            .map(|finding| (finding.finding_fingerprint.clone(), finding))
            .collect::<BTreeMap<_, _>>();
        let tracked = request
            .tracked_comments
            .iter()
            .map(|comment| (comment.finding_fingerprint.clone(), comment))
            .collect::<BTreeMap<_, _>>();
        let mut actions = Vec::with_capacity(current.len().saturating_add(tracked.len()));

        for (fingerprint, tracked) in tracked {
            let current_finding = current.remove(&fingerprint);
            let previous_identity = ProviderCommentIdentity {
                comment_id: tracked.comment_id.clone(),
            };
            if let Err(error) = self.ensure_current_revision(request, &target) {
                actions.push(failed_action(
                    fingerprint,
                    Some(previous_identity.comment_id),
                    None,
                    false,
                    error,
                ));
                continue;
            }
            let provider_comment = match self.api.get_finding_comment(&target, &previous_identity) {
                Ok(comment) => comment,
                Err(error) => {
                    actions.push(failed_action(
                        fingerprint,
                        Some(previous_identity.comment_id),
                        None,
                        false,
                        publication_api_error(error),
                    ));
                    continue;
                }
            };

            let Some(provider_comment) = provider_comment else {
                if let Some(current_finding) = current_finding {
                    actions.push(match publisher.publish(current_finding) {
                        Ok(published) => successful_action(
                            fingerprint,
                            FindingReconciliationActionKind::Reopened,
                            Some(previous_identity.comment_id),
                            Some(published.comment_id),
                            true,
                        ),
                        Err(error) => failed_action(
                            fingerprint,
                            Some(previous_identity.comment_id),
                            None,
                            false,
                            error,
                        ),
                    });
                } else {
                    actions.push(failed_action(
                        fingerprint,
                        Some(previous_identity.comment_id.clone()),
                        None,
                        false,
                        FindingPublicationError {
                            code: FindingPublicationErrorCode::AnchorRejected,
                            retryable: false,
                            message:
                                "The tracked finding comment is missing or no longer editable."
                                    .to_string(),
                        },
                    ));
                }
                continue;
            };

            let lineage_marker = request.lineage_marker(&fingerprint);
            let exact_marker = trailing_control_lines(&provider_comment.markdown)
                .into_iter()
                .find(|line| is_exact_finding_marker(line))
                .map(str::to_string);
            let has_lineage_marker = trailing_control_lines(&provider_comment.markdown)
                .iter()
                .any(|line| line.starts_with("<!-- lachesi:finding-lineage:"));
            let lineage_marker_matches =
                comment_has_marker(&provider_comment.markdown, &lineage_marker);
            let Some(exact_marker) = exact_marker else {
                actions.push(failed_action(
                    fingerprint,
                    Some(previous_identity.comment_id),
                    None,
                    false,
                    FindingPublicationError {
                        code: FindingPublicationErrorCode::PermissionDenied,
                        retryable: false,
                        message:
                            "The tracked provider comment is not owned by this Lachesi finding."
                                .to_string(),
                    },
                ));
                continue;
            };
            if has_lineage_marker && !lineage_marker_matches {
                actions.push(failed_action(
                    fingerprint,
                    Some(previous_identity.comment_id),
                    None,
                    false,
                    FindingPublicationError {
                        code: FindingPublicationErrorCode::PermissionDenied,
                        retryable: false,
                        message:
                            "The tracked provider comment is not owned by this Lachesi finding."
                                .to_string(),
                    },
                ));
                continue;
            }

            match current_finding {
                Some(current_finding) => actions.push(self.reconcile_current_finding(
                    request,
                    &publisher,
                    fingerprint,
                    provider_comment,
                    exact_marker,
                    current_finding,
                )),
                None => actions.push(self.resolve_absent_finding(
                    request,
                    &target,
                    fingerprint,
                    tracked,
                    provider_comment,
                    exact_marker,
                )),
            }
        }

        for (fingerprint, current_finding) in current {
            if let Err(error) = self.ensure_current_revision(request, &target) {
                actions.push(failed_action(fingerprint, None, None, false, error));
                continue;
            }
            actions.push(match publisher.publish(current_finding) {
                Ok(published) => successful_action(
                    fingerprint,
                    FindingReconciliationActionKind::Created,
                    None,
                    Some(published.comment_id),
                    true,
                ),
                Err(error) => failed_action(fingerprint, None, None, false, error),
            });
        }

        Ok(request.summary(actions))
    }

    fn reconcile_current_finding(
        &self,
        request: &FindingReconciliationRequest,
        publisher: &FindingPublisher<&A, &S>,
        fingerprint: String,
        provider_comment: ProviderFindingComment,
        previous_exact_marker: String,
        current: &FindingPublicationRequest,
    ) -> FindingReconciliationAction {
        let target = request.target();
        let previous_id = provider_comment.identity.comment_id.clone();
        let exact_marker = finding_marker(current);
        let desired = render_finding_markdown(current, &exact_marker);
        let was_resolved = comment_is_resolved(&provider_comment.markdown);
        let same_anchor = provider_comment.anchor.as_ref() == Some(&current.anchor);
        let semantic_match =
            active_markdown(&provider_comment.markdown) == active_markdown(&desired);

        if same_anchor {
            let kind = if was_resolved {
                FindingReconciliationActionKind::Reopened
            } else if semantic_match {
                FindingReconciliationActionKind::Unchanged
            } else {
                FindingReconciliationActionKind::Updated
            };
            let mutated = provider_comment.markdown != desired;
            if mutated {
                if let Err(failure) = self.update_fenced(
                    request,
                    &target,
                    &provider_comment.identity,
                    &provider_comment.markdown,
                    &desired,
                ) {
                    return failed_action(
                        fingerprint,
                        Some(previous_id.clone()),
                        Some(previous_id),
                        failure.provider_mutated,
                        failure.error,
                    );
                }
            }
            return successful_action(
                fingerprint,
                kind,
                Some(previous_id.clone()),
                Some(previous_id),
                mutated,
            );
        }

        let published = match publisher.publish(current) {
            Ok(published) => published,
            Err(error) => {
                return failed_action(fingerprint, Some(previous_id), None, false, error);
            }
        };
        let resolved = resolved_markdown(
            &provider_comment.markdown,
            &request.lineage_marker(&fingerprint),
            &previous_exact_marker,
        );
        if !was_resolved && provider_comment.markdown != resolved {
            if let Err(failure) = self.update_fenced(
                request,
                &target,
                &provider_comment.identity,
                &provider_comment.markdown,
                &resolved,
            ) {
                return failed_action(
                    fingerprint,
                    Some(previous_id),
                    Some(published.comment_id),
                    true,
                    failure.error,
                );
            }
        }
        successful_action(
            fingerprint,
            if was_resolved {
                FindingReconciliationActionKind::Reopened
            } else {
                FindingReconciliationActionKind::Updated
            },
            Some(previous_id),
            Some(published.comment_id),
            true,
        )
    }

    fn resolve_absent_finding(
        &self,
        request: &FindingReconciliationRequest,
        target: &ProviderPublicationTarget,
        fingerprint: String,
        tracked: &TrackedFindingComment,
        provider_comment: ProviderFindingComment,
        previous_exact_marker: String,
    ) -> FindingReconciliationAction {
        let comment_id = provider_comment.identity.comment_id.clone();
        if comment_is_resolved(&provider_comment.markdown) {
            return successful_action(
                fingerprint,
                FindingReconciliationActionKind::Unchanged,
                Some(comment_id.clone()),
                Some(comment_id),
                false,
            );
        }
        let resolved = resolved_markdown(
            &provider_comment.markdown,
            &request.lineage_marker(&tracked.finding_fingerprint),
            &previous_exact_marker,
        );
        match self.update_fenced(
            request,
            target,
            &provider_comment.identity,
            &provider_comment.markdown,
            &resolved,
        ) {
            Ok(()) => successful_action(
                fingerprint,
                FindingReconciliationActionKind::Resolved,
                Some(comment_id.clone()),
                Some(comment_id),
                true,
            ),
            Err(failure) => failed_action(
                fingerprint,
                Some(comment_id.clone()),
                Some(comment_id),
                failure.provider_mutated,
                failure.error,
            ),
        }
    }

    fn update_fenced(
        &self,
        request: &FindingReconciliationRequest,
        target: &ProviderPublicationTarget,
        identity: &ProviderCommentIdentity,
        previous_markdown: &str,
        desired_markdown: &str,
    ) -> Result<(), FencedUpdateFailure> {
        self.ensure_current_revision(request, target)
            .map_err(|error| FencedUpdateFailure {
                error,
                provider_mutated: false,
            })?;
        self.api
            .update_finding_comment(target, identity, desired_markdown)
            .map_err(|error| {
                let provider_mutated =
                    matches!(error.kind, ProviderPublicationApiErrorKind::Unavailable);
                FencedUpdateFailure {
                    error: publication_api_error(error),
                    provider_mutated,
                }
            })?;
        if let Err(error) = self.ensure_current_revision(request, target) {
            return match self
                .api
                .update_finding_comment(target, identity, previous_markdown)
            {
                Ok(()) => Err(FencedUpdateFailure {
                    error,
                    provider_mutated: false,
                }),
                Err(rollback_error) => Err(FencedUpdateFailure {
                    error: publication_api_error(rollback_error),
                    provider_mutated: true,
                }),
            };
        }
        Ok(())
    }

    fn ensure_current_revision(
        &self,
        request: &FindingReconciliationRequest,
        target: &ProviderPublicationTarget,
    ) -> Result<(), FindingPublicationError> {
        let revision = self
            .api
            .current_revision(target)
            .map_err(publication_api_error)?;
        if request.matches_revision(&revision) {
            return Ok(());
        }
        Err(FindingPublicationError {
            code: FindingPublicationErrorCode::OutdatedAnchor,
            retryable: false,
            message: "The pull request changed after this reconciliation request was produced."
                .to_string(),
        })
    }
}

pub fn dry_run_reconciliation_summary(
    request: &FindingReconciliationRequest,
) -> Result<FindingReconciliationSummary, FindingPublicationError> {
    request.validate()?;
    let mut current = request
        .current_findings
        .iter()
        .map(|finding| (finding.finding_fingerprint.clone(), finding))
        .collect::<BTreeMap<_, _>>();
    let mut actions =
        Vec::with_capacity(request.tracked_comments.len().saturating_add(current.len()));
    for tracked in &request.tracked_comments {
        let current_finding = current.remove(&tracked.finding_fingerprint);
        actions.push(successful_action(
            tracked.finding_fingerprint.clone(),
            if current_finding.is_some() {
                FindingReconciliationActionKind::Updated
            } else {
                FindingReconciliationActionKind::Resolved
            },
            Some(tracked.comment_id.clone()),
            Some(tracked.comment_id.clone()),
            false,
        ));
    }
    for (fingerprint, finding) in current {
        let marker = finding_marker(finding);
        let digest = marker
            .strip_prefix("<!-- lachesi:finding:")
            .and_then(|value| value.strip_suffix(" -->"))
            .unwrap_or("unknown");
        actions.push(successful_action(
            fingerprint,
            FindingReconciliationActionKind::Created,
            None,
            Some(format!("dry-run-{}", &digest[..digest.len().min(16)])),
            false,
        ));
    }
    Ok(request.summary(actions))
}

impl FindingReconciliationRequest {
    fn validate(&self) -> Result<(), FindingPublicationError> {
        validate_identifier("tenantId", &self.tenant_id)?;
        validate_identifier("workspace", &self.workspace)?;
        validate_identifier("repository", &self.repository)?;
        if self.pull_request_id == 0 {
            return Err(invalid_request(
                "`pullRequestId` must be a positive integer",
            ));
        }
        validate_sha("baseSha", &self.base_sha)?;
        validate_sha("headSha", &self.head_sha)?;
        if self
            .tracked_comments
            .len()
            .saturating_add(self.current_findings.len())
            > MAX_RECONCILIATION_FINDINGS
        {
            return Err(invalid_request("reconciliation contains too many findings"));
        }

        let mut tracked = BTreeMap::new();
        for comment in &self.tracked_comments {
            validate_identifier("findingFingerprint", &comment.finding_fingerprint)?;
            validate_identifier("commentId", &comment.comment_id)?;
            if tracked
                .insert(comment.finding_fingerprint.as_str(), ())
                .is_some()
            {
                return Err(invalid_request(
                    "tracked finding fingerprints must be unique",
                ));
            }
        }

        let mut current = BTreeMap::new();
        for finding in &self.current_findings {
            finding.validate().map_err(invalid_request)?;
            if !self.matches_finding_target(finding) {
                return Err(invalid_request(
                    "current findings must belong to the reconciliation target and revision",
                ));
            }
            if current
                .insert(finding.finding_fingerprint.as_str(), ())
                .is_some()
            {
                return Err(invalid_request(
                    "current finding fingerprints must be unique",
                ));
            }
        }
        Ok(())
    }

    fn matches_finding_target(&self, finding: &FindingPublicationRequest) -> bool {
        finding.tenant_id == self.tenant_id
            && finding.provider == self.provider
            && finding.workspace.eq_ignore_ascii_case(&self.workspace)
            && finding.repository.eq_ignore_ascii_case(&self.repository)
            && finding.pull_request_id == self.pull_request_id
            && finding.base_sha.eq_ignore_ascii_case(&self.base_sha)
            && finding.head_sha.eq_ignore_ascii_case(&self.head_sha)
    }

    fn target(&self) -> ProviderPublicationTarget {
        ProviderPublicationTarget {
            tenant_id: self.tenant_id.clone(),
            provider: self.provider,
            workspace: self.workspace.clone(),
            repository: self.repository.clone(),
            pull_request_id: self.pull_request_id,
        }
    }

    fn matches_revision(&self, revision: &ProviderPullRequestRevision) -> bool {
        revision.base_sha.eq_ignore_ascii_case(&self.base_sha)
            && revision.head_sha.eq_ignore_ascii_case(&self.head_sha)
    }

    fn lineage_marker(&self, fingerprint: &str) -> String {
        finding_lineage_marker_for(
            &self.tenant_id,
            self.provider,
            &self.workspace,
            &self.repository,
            self.pull_request_id,
            fingerprint,
        )
    }

    fn summary(&self, actions: Vec<FindingReconciliationAction>) -> FindingReconciliationSummary {
        let mut counts = FindingReconciliationCounts::default();
        for action in &actions {
            match action.kind {
                FindingReconciliationActionKind::Unchanged => counts.unchanged += 1,
                FindingReconciliationActionKind::Created => counts.created += 1,
                FindingReconciliationActionKind::Updated => counts.updated += 1,
                FindingReconciliationActionKind::Resolved => counts.resolved += 1,
                FindingReconciliationActionKind::Reopened => counts.reopened += 1,
                FindingReconciliationActionKind::Failed => counts.failed += 1,
            }
        }
        FindingReconciliationSummary {
            schema_version: self.schema_version,
            status: if counts.failed == 0 {
                FindingReconciliationStatus::Succeeded
            } else {
                FindingReconciliationStatus::Partial
            },
            tenant_id: self.tenant_id.clone(),
            provider: self.provider,
            workspace: self.workspace.clone(),
            repository: self.repository.clone(),
            pull_request_id: self.pull_request_id,
            base_sha: self.base_sha.clone(),
            head_sha: self.head_sha.clone(),
            counts,
            actions,
        }
    }
}

fn successful_action(
    finding_fingerprint: String,
    kind: FindingReconciliationActionKind,
    previous_comment_id: Option<String>,
    comment_id: Option<String>,
    provider_mutated: bool,
) -> FindingReconciliationAction {
    FindingReconciliationAction {
        finding_fingerprint,
        kind,
        previous_comment_id,
        comment_id,
        provider_mutated,
        error: None,
    }
}

fn failed_action(
    finding_fingerprint: String,
    previous_comment_id: Option<String>,
    comment_id: Option<String>,
    provider_mutated: bool,
    error: FindingPublicationError,
) -> FindingReconciliationAction {
    FindingReconciliationAction {
        finding_fingerprint,
        kind: FindingReconciliationActionKind::Failed,
        previous_comment_id,
        comment_id,
        provider_mutated,
        error: Some(FindingReconciliationActionError {
            code: error.code,
            retryable: error.retryable,
            message: error.message,
        }),
    }
}

fn comment_is_resolved(markdown: &str) -> bool {
    comment_has_marker(markdown, RESOLVED_MARKER)
}

fn comment_has_marker(markdown: &str, marker: &str) -> bool {
    trailing_control_lines(markdown).contains(&marker)
}

fn trailing_control_lines(markdown: &str) -> Vec<&str> {
    markdown
        .lines()
        .rev()
        .skip_while(|line| line.is_empty())
        .take_while(|line| line.starts_with("<!-- lachesi:") && line.ends_with(" -->"))
        .collect()
}

fn is_control_line(line: &str) -> bool {
    line == RESOLVED_MARKER
        || (line.starts_with("<!-- lachesi:finding:") && line.ends_with(" -->"))
        || (line.starts_with("<!-- lachesi:finding-lineage:") && line.ends_with(" -->"))
}

fn active_markdown(markdown: &str) -> String {
    let resolved = comment_is_resolved(markdown);
    let mut lines = markdown.lines().collect::<Vec<_>>();
    let control_count = trailing_control_lines(markdown).len();
    let mut controls_remaining = control_count;
    while lines.last().is_some_and(|line| {
        if line.is_empty() {
            true
        } else if controls_remaining > 0 && is_control_line(line) {
            controls_remaining -= 1;
            true
        } else {
            false
        }
    }) {
        lines.pop();
    }
    if resolved && lines.last() == Some(&RESOLVED_NOTICE) {
        lines.pop();
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n").trim().to_string()
}

fn resolved_markdown(markdown: &str, lineage_marker: &str, exact_marker: &str) -> String {
    format!(
        "{}\n\n{RESOLVED_NOTICE}\n\n{RESOLVED_MARKER}\n{lineage_marker}\n{exact_marker}",
        active_markdown(markdown)
    )
}

fn validate_identifier(field: &str, value: &str) -> Result<(), FindingPublicationError> {
    if value.trim().is_empty() || value != value.trim() || value.len() > 512 {
        return Err(invalid_request(format!("`{field}` is invalid")));
    }
    Ok(())
}

fn is_exact_finding_marker(value: &str) -> bool {
    value
        .strip_prefix("<!-- lachesi:finding:")
        .and_then(|value| value.strip_suffix(" -->"))
        .is_some_and(|digest| {
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

fn validate_sha(field: &str, value: &str) -> Result<(), FindingPublicationError> {
    if matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(());
    }
    Err(invalid_request(format!(
        "`{field}` must be a full hexadecimal commit id"
    )))
}

fn invalid_request(message: impl Into<String>) -> FindingPublicationError {
    FindingPublicationError {
        code: FindingPublicationErrorCode::InvalidRequest,
        retryable: false,
        message: message.into(),
    }
}

fn publication_api_error(error: ProviderPublicationApiError) -> FindingPublicationError {
    match error.kind {
        ProviderPublicationApiErrorKind::InvalidAnchor => FindingPublicationError {
            code: FindingPublicationErrorCode::AnchorRejected,
            retryable: false,
            message: "The provider rejected the tracked finding comment.".to_string(),
        },
        ProviderPublicationApiErrorKind::PermissionDenied => FindingPublicationError {
            code: FindingPublicationErrorCode::PermissionDenied,
            retryable: false,
            message: "The provider denied finding reconciliation.".to_string(),
        },
        ProviderPublicationApiErrorKind::Unavailable => FindingPublicationError {
            code: FindingPublicationErrorCode::ProviderUnavailable,
            retryable: true,
            message: "The provider is temporarily unavailable for finding reconciliation."
                .to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;
    use crate::finding_publication::{
        finding_lineage_marker, FindingAnchorSide, FindingPublicationLease,
        FindingPublicationReservation, FindingPublicationSchemaVersion, FindingSeverity,
        ProviderInlineCommentPayload,
    };

    const BASE_SHA: &str = "1111111111111111111111111111111111111111";
    const OLD_HEAD_SHA: &str = "2222222222222222222222222222222222222222";
    const HEAD_SHA: &str = "3333333333333333333333333333333333333333";

    #[derive(Debug, Clone)]
    struct MockComment {
        comment: ProviderFindingComment,
        replies: Vec<String>,
    }

    #[derive(Debug)]
    struct MockApiState {
        revision: ProviderPullRequestRevision,
        comments: HashMap<String, MockComment>,
        next_comment_id: u32,
        create_count: u32,
        update_count: u32,
        fail_next_update: bool,
        drift_after_next_update: bool,
        fail_rollback_after_drift: bool,
    }

    #[derive(Debug)]
    struct MockApi(Mutex<MockApiState>);

    impl Default for MockApi {
        fn default() -> Self {
            Self(Mutex::new(MockApiState {
                revision: ProviderPullRequestRevision {
                    base_sha: BASE_SHA.to_string(),
                    head_sha: HEAD_SHA.to_string(),
                },
                comments: HashMap::new(),
                next_comment_id: 100,
                create_count: 0,
                update_count: 0,
                fail_next_update: false,
                drift_after_next_update: false,
                fail_rollback_after_drift: false,
            }))
        }
    }

    impl MockApi {
        fn insert(
            &self,
            request: &FindingPublicationRequest,
            comment_id: &str,
            resolved: bool,
            replies: &[&str],
        ) -> TrackedFindingComment {
            let exact_marker = finding_marker(request);
            let active = render_finding_markdown(request, &exact_marker);
            let markdown = if resolved {
                resolved_markdown(&active, &finding_lineage_marker(request), &exact_marker)
            } else {
                active
            };
            self.0.lock().unwrap().comments.insert(
                comment_id.to_string(),
                MockComment {
                    comment: ProviderFindingComment {
                        identity: ProviderCommentIdentity {
                            comment_id: comment_id.to_string(),
                        },
                        markdown,
                        anchor: Some(request.anchor.clone()),
                    },
                    replies: replies.iter().map(|reply| (*reply).to_string()).collect(),
                },
            );
            TrackedFindingComment {
                finding_fingerprint: request.finding_fingerprint.clone(),
                comment_id: comment_id.to_string(),
            }
        }

        fn set_fail_next_update(&self) {
            self.0.lock().unwrap().fail_next_update = true;
        }

        fn clear_anchor(&self, comment_id: &str) {
            self.0
                .lock()
                .unwrap()
                .comments
                .get_mut(comment_id)
                .expect("mock comment")
                .comment
                .anchor = None;
        }

        fn set_drift_after_next_update(&self) {
            self.0.lock().unwrap().drift_after_next_update = true;
        }

        fn set_drift_and_fail_rollback(&self) {
            let mut state = self.0.lock().unwrap();
            state.drift_after_next_update = true;
            state.fail_rollback_after_drift = true;
        }
    }

    impl ProviderInlineCommentApi for MockApi {
        fn current_revision(
            &self,
            _target: &ProviderPublicationTarget,
        ) -> Result<ProviderPullRequestRevision, ProviderPublicationApiError> {
            Ok(self.0.lock().unwrap().revision.clone())
        }

        fn find_inline_comment(
            &self,
            _target: &ProviderPublicationTarget,
            marker: &str,
            expected: &ProviderInlineCommentPayload,
        ) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .comments
                .values()
                .find(|comment| {
                    comment.comment.anchor.as_ref().is_some_and(|anchor| {
                        anchor.path == expected.path
                            && anchor.start_line == expected.start_line
                            && anchor.end_line == expected.end_line
                            && anchor.side == expected.side
                    }) && comment_has_marker(&comment.comment.markdown, marker)
                })
                .map(|comment| comment.comment.identity.clone()))
        }

        fn find_comment_by_marker(
            &self,
            _target: &ProviderPublicationTarget,
            marker: &str,
        ) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .comments
                .values()
                .find(|comment| comment_has_marker(&comment.comment.markdown, marker))
                .map(|comment| comment.comment.identity.clone()))
        }

        fn create_inline_comment(
            &self,
            _target: &ProviderPublicationTarget,
            payload: &ProviderInlineCommentPayload,
        ) -> Result<ProviderCommentIdentity, ProviderPublicationApiError> {
            let mut state = self.0.lock().unwrap();
            state.next_comment_id += 1;
            state.create_count += 1;
            let comment_id = format!("comment-{}", state.next_comment_id);
            let identity = ProviderCommentIdentity {
                comment_id: comment_id.clone(),
            };
            state.comments.insert(
                comment_id,
                MockComment {
                    comment: ProviderFindingComment {
                        identity: identity.clone(),
                        markdown: payload.markdown.clone(),
                        anchor: Some(FindingLineRange {
                            path: payload.path.clone(),
                            start_line: payload.start_line,
                            end_line: payload.end_line,
                            side: payload.side,
                        }),
                    },
                    replies: Vec::new(),
                },
            );
            Ok(identity)
        }

        fn delete_comment(
            &self,
            _target: &ProviderPublicationTarget,
            identity: &ProviderCommentIdentity,
        ) -> Result<(), ProviderPublicationApiError> {
            self.0.lock().unwrap().comments.remove(&identity.comment_id);
            Ok(())
        }
    }

    impl ProviderFindingReconciliationApi for MockApi {
        fn get_finding_comment(
            &self,
            _target: &ProviderPublicationTarget,
            identity: &ProviderCommentIdentity,
        ) -> Result<Option<ProviderFindingComment>, ProviderPublicationApiError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .comments
                .get(&identity.comment_id)
                .map(|comment| comment.comment.clone()))
        }

        fn update_finding_comment(
            &self,
            _target: &ProviderPublicationTarget,
            identity: &ProviderCommentIdentity,
            markdown: &str,
        ) -> Result<(), ProviderPublicationApiError> {
            let mut state = self.0.lock().unwrap();
            if state.fail_next_update {
                state.fail_next_update = false;
                return Err(ProviderPublicationApiError::unavailable(
                    "temporary update failure",
                ));
            }
            let Some(comment) = state.comments.get_mut(&identity.comment_id) else {
                return Err(ProviderPublicationApiError::invalid_anchor(
                    "missing comment",
                ));
            };
            comment.comment.markdown = markdown.to_string();
            state.update_count += 1;
            if state.drift_after_next_update {
                state.drift_after_next_update = false;
                state.revision.head_sha = "4444444444444444444444444444444444444444".to_string();
                if state.fail_rollback_after_drift {
                    state.fail_rollback_after_drift = false;
                    state.fail_next_update = true;
                }
            }
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct MockStore(Mutex<HashMap<String, ProviderCommentIdentity>>);

    impl FindingPublicationStore for MockStore {
        fn reserve(
            &self,
            _request: &FindingPublicationRequest,
            marker: &str,
        ) -> Result<FindingPublicationReservation, String> {
            if let Some(identity) = self.0.lock().unwrap().get(marker).cloned() {
                return Ok(FindingPublicationReservation::Published(identity));
            }
            Ok(FindingPublicationReservation::Acquired(
                FindingPublicationLease {
                    marker: marker.to_string(),
                    token: "lease".to_string(),
                },
            ))
        }

        fn complete(
            &self,
            lease: &FindingPublicationLease,
            identity: &ProviderCommentIdentity,
        ) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .insert(lease.marker.clone(), identity.clone());
            Ok(())
        }

        fn release(&self, _lease: &FindingPublicationLease) -> Result<(), String> {
            Ok(())
        }
    }

    fn finding(fingerprint: &str, head_sha: &str) -> FindingPublicationRequest {
        FindingPublicationRequest {
            schema_version: FindingPublicationSchemaVersion::V1,
            tenant_id: "tenant-acme".to_string(),
            provider: PullRequestReviewEventProvider::Github,
            workspace: "acme".to_string(),
            repository: "payments".to_string(),
            pull_request_id: 42,
            base_sha: BASE_SHA.to_string(),
            head_sha: head_sha.to_string(),
            finding_fingerprint: fingerprint.to_string(),
            anchor: FindingLineRange {
                path: "src/lib.rs".to_string(),
                start_line: 12,
                end_line: 12,
                side: FindingAnchorSide::New,
            },
            title: format!("Finding {fingerprint}"),
            body: "The finding is actionable.".to_string(),
            severity: FindingSeverity::High,
            suggested_fix: None,
        }
    }

    fn request(
        tracked_comments: Vec<TrackedFindingComment>,
        current_findings: Vec<FindingPublicationRequest>,
    ) -> FindingReconciliationRequest {
        FindingReconciliationRequest {
            schema_version: FindingReconciliationSchemaVersion::V1,
            tenant_id: "tenant-acme".to_string(),
            provider: PullRequestReviewEventProvider::Github,
            workspace: "acme".to_string(),
            repository: "payments".to_string(),
            pull_request_id: 42,
            base_sha: BASE_SHA.to_string(),
            head_sha: HEAD_SHA.to_string(),
            tracked_comments,
            current_findings,
        }
    }

    #[test]
    fn unchanged_finding_keeps_one_comment_and_retry_is_a_noop() {
        let api = MockApi::default();
        let store = MockStore::default();
        let previous = finding("unchanged", OLD_HEAD_SHA);
        let tracked = api.insert(&previous, "comment-1", false, &["human reply"]);
        let current = finding("unchanged", HEAD_SHA);
        let request = request(vec![tracked], vec![current.clone()]);
        let reconciler = FindingReconciler::new(&api, &store);

        let first = reconciler
            .reconcile(&request)
            .expect("first reconciliation");
        assert_eq!(first.status, FindingReconciliationStatus::Succeeded);
        assert_eq!(first.counts.unchanged, 1);
        assert_eq!(first.actions[0].comment_id.as_deref(), Some("comment-1"));
        assert!(first.actions[0].provider_mutated);

        let second = reconciler.reconcile(&request).expect("idempotent retry");
        assert_eq!(second.counts.unchanged, 1);
        assert!(!second.actions[0].provider_mutated);
        let state = api.0.lock().unwrap();
        assert_eq!(state.comments.len(), 1);
        assert_eq!(state.create_count, 0);
        assert_eq!(state.update_count, 1);
        assert_eq!(state.comments["comment-1"].replies, vec!["human reply"]);
        assert_eq!(
            state.comments["comment-1"].comment.markdown,
            render_finding_markdown(&current, &finding_marker(&current))
        );
    }

    #[test]
    fn reconciles_new_changed_moved_fixed_and_reopened_findings() {
        let api = MockApi::default();
        let store = MockStore::default();

        let changed_previous = finding("changed", HEAD_SHA);
        let changed = api.insert(
            &changed_previous,
            "comment-changed",
            false,
            &["keep changed reply"],
        );
        let mut changed_current = finding("changed", HEAD_SHA);
        changed_current.body = "The message changed.".to_string();

        let moved_previous = finding("moved", HEAD_SHA);
        let moved = api.insert(
            &moved_previous,
            "comment-moved",
            false,
            &["keep moved reply"],
        );
        let mut moved_current = finding("moved", HEAD_SHA);
        moved_current.anchor.start_line = 30;
        moved_current.anchor.end_line = 30;

        let fixed_previous = finding("fixed", OLD_HEAD_SHA);
        let fixed = api.insert(
            &fixed_previous,
            "comment-fixed",
            false,
            &["keep fixed reply"],
        );

        let reopened_previous = finding("reopened", OLD_HEAD_SHA);
        let reopened = api.insert(
            &reopened_previous,
            "comment-reopened",
            true,
            &["keep reopened reply"],
        );
        let reopened_current = finding("reopened", HEAD_SHA);
        let new_current = finding("new", HEAD_SHA);

        let summary = FindingReconciler::new(&api, &store)
            .reconcile(&request(
                vec![changed, moved, fixed, reopened],
                vec![
                    changed_current,
                    moved_current,
                    reopened_current,
                    new_current,
                ],
            ))
            .expect("reconcile all cases");

        assert_eq!(summary.status, FindingReconciliationStatus::Succeeded);
        assert_eq!(
            summary.counts,
            FindingReconciliationCounts {
                created: 1,
                updated: 2,
                resolved: 1,
                reopened: 1,
                ..FindingReconciliationCounts::default()
            }
        );
        let state = api.0.lock().unwrap();
        assert_eq!(state.create_count, 2);
        assert!(comment_is_resolved(
            &state.comments["comment-moved"].comment.markdown
        ));
        assert!(comment_is_resolved(
            &state.comments["comment-fixed"].comment.markdown
        ));
        assert!(!comment_is_resolved(
            &state.comments["comment-reopened"].comment.markdown
        ));
        for (id, reply) in [
            ("comment-changed", "keep changed reply"),
            ("comment-moved", "keep moved reply"),
            ("comment-fixed", "keep fixed reply"),
            ("comment-reopened", "keep reopened reply"),
        ] {
            assert_eq!(state.comments[id].replies, vec![reply]);
        }
    }

    #[test]
    fn resolves_an_absent_finding_when_provider_anchor_metadata_is_gone() {
        let api = MockApi::default();
        let store = MockStore::default();
        let previous = finding("fixed", OLD_HEAD_SHA);
        let tracked = api.insert(&previous, "comment-fixed", false, &["keep reply"]);
        api.clear_anchor("comment-fixed");

        let summary = FindingReconciler::new(&api, &store)
            .reconcile(&request(vec![tracked], Vec::new()))
            .expect("resolve anchorless finding");

        assert_eq!(summary.counts.resolved, 1);
        let state = api.0.lock().unwrap();
        assert!(comment_is_resolved(
            &state.comments["comment-fixed"].comment.markdown
        ));
        assert_eq!(state.comments["comment-fixed"].replies, vec!["keep reply"]);
    }

    #[test]
    fn partial_failure_retries_without_duplicate_new_comments() {
        let api = MockApi::default();
        let store = MockStore::default();
        let previous = finding("changed", OLD_HEAD_SHA);
        let tracked = api.insert(&previous, "comment-changed", false, &[]);
        let mut changed = finding("changed", HEAD_SHA);
        changed.body = "Updated content.".to_string();
        let new = finding("new", HEAD_SHA);
        let request = request(vec![tracked], vec![changed, new]);
        api.set_fail_next_update();
        let reconciler = FindingReconciler::new(&api, &store);

        let first = reconciler.reconcile(&request).expect("partial summary");
        assert_eq!(first.status, FindingReconciliationStatus::Partial);
        assert_eq!(first.counts.failed, 1);
        assert_eq!(first.counts.created, 1);

        let second = reconciler.reconcile(&request).expect("retry succeeds");
        assert_eq!(second.status, FindingReconciliationStatus::Succeeded);
        assert_eq!(second.counts.updated, 1);
        assert_eq!(second.counts.created, 1);
        let state = api.0.lock().unwrap();
        assert_eq!(state.create_count, 1);
        assert_eq!(state.comments.len(), 2);
    }

    #[test]
    fn missing_absent_tracked_comment_is_reported_as_stale_state() {
        let tracked = TrackedFindingComment {
            finding_fingerprint: "missing".to_string(),
            comment_id: "comment-missing".to_string(),
        };

        let summary = FindingReconciler::new(&MockApi::default(), &MockStore::default())
            .reconcile(&request(vec![tracked], Vec::new()))
            .expect("auditable stale state");

        assert_eq!(summary.status, FindingReconciliationStatus::Partial);
        assert_eq!(summary.counts.failed, 1);
        assert_eq!(
            summary.actions[0].error.as_ref().map(|error| error.code),
            Some(FindingPublicationErrorCode::AnchorRejected)
        );
        assert!(!summary.actions[0].provider_mutated);
    }

    #[test]
    fn revision_drift_rolls_back_an_in_place_update() {
        let api = MockApi::default();
        let store = MockStore::default();
        let previous = finding("changed", OLD_HEAD_SHA);
        let tracked = api.insert(&previous, "comment-changed", false, &[]);
        let original = api.0.lock().unwrap().comments["comment-changed"]
            .comment
            .markdown
            .clone();
        let mut current = finding("changed", HEAD_SHA);
        current.body = "Updated content.".to_string();
        api.set_drift_after_next_update();

        let summary = FindingReconciler::new(&api, &store)
            .reconcile(&request(vec![tracked], vec![current]))
            .expect("partial summary after drift");

        assert_eq!(summary.status, FindingReconciliationStatus::Partial);
        assert_eq!(summary.counts.failed, 1);
        assert_eq!(
            api.0.lock().unwrap().comments["comment-changed"]
                .comment
                .markdown,
            original
        );
    }

    #[test]
    fn failed_revision_rollback_reports_possible_provider_mutation() {
        let api = MockApi::default();
        let store = MockStore::default();
        let previous = finding("changed", OLD_HEAD_SHA);
        let tracked = api.insert(&previous, "comment-changed", false, &[]);
        let mut current = finding("changed", HEAD_SHA);
        current.body = "Updated content.".to_string();
        api.set_drift_and_fail_rollback();

        let summary = FindingReconciler::new(&api, &store)
            .reconcile(&request(vec![tracked], vec![current]))
            .expect("partial summary after failed rollback");

        assert_eq!(summary.status, FindingReconciliationStatus::Partial);
        assert_eq!(summary.counts.failed, 1);
        assert!(summary.actions[0].provider_mutated);
    }

    #[test]
    fn rejects_invalid_tracked_comment_ids() {
        let tracked = TrackedFindingComment {
            finding_fingerprint: "tracked".to_string(),
            comment_id: " invalid ".to_string(),
        };

        let error = FindingReconciler::new(&MockApi::default(), &MockStore::default())
            .reconcile(&request(vec![tracked], Vec::new()))
            .expect_err("invalid tracked comment");

        assert_eq!(error.code, FindingPublicationErrorCode::InvalidRequest);
    }

    #[test]
    fn rejects_reconciliation_batches_over_the_provider_call_budget() {
        let current_findings = (0..=MAX_RECONCILIATION_FINDINGS)
            .map(|index| finding(&format!("finding-{index}"), HEAD_SHA))
            .collect();

        let error = request(Vec::new(), current_findings)
            .validate()
            .expect_err("oversized reconciliation");

        assert_eq!(error.code, FindingPublicationErrorCode::InvalidRequest);
        assert_eq!(error.message, "reconciliation contains too many findings");
    }

    #[test]
    fn refuses_to_edit_a_comment_with_a_mismatched_lineage_marker() {
        let api = MockApi::default();
        let store = MockStore::default();
        let previous = finding("tracked", OLD_HEAD_SHA);
        let tracked = api.insert(&previous, "comment-1", false, &[]);
        {
            let mut state = api.0.lock().unwrap();
            let mismatched_lineage = state.comments["comment-1"]
                .comment
                .markdown
                .lines()
                .map(|line| {
                    if line.starts_with("<!-- lachesi:finding-lineage:") {
                        "<!-- lachesi:finding-lineage:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa -->"
                    } else {
                        line
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            state
                .comments
                .get_mut("comment-1")
                .unwrap()
                .comment
                .markdown = mismatched_lineage;
        }

        let summary = FindingReconciler::new(&api, &store)
            .reconcile(&request(vec![tracked], vec![finding("tracked", HEAD_SHA)]))
            .expect("auditable failure");

        assert_eq!(summary.status, FindingReconciliationStatus::Partial);
        assert_eq!(summary.counts.failed, 1);
        assert_eq!(
            summary.actions[0].error.as_ref().map(|error| error.code),
            Some(FindingPublicationErrorCode::PermissionDenied)
        );
        assert_eq!(api.0.lock().unwrap().update_count, 0);
    }

    #[test]
    fn marker_like_finding_text_outside_the_trailing_control_block_is_content() {
        let embedded = "<!-- lachesi:finding:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa -->";
        let markdown = format!("{embedded}\nFinding body");

        assert_eq!(active_markdown(&markdown), markdown);
        assert!(!comment_has_marker(&markdown, embedded));

        let lineage = "<!-- lachesi:finding-lineage:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb -->";
        let controlled = format!("{markdown}\n\n{lineage}\n{embedded}");
        assert_eq!(active_markdown(&controlled), markdown);
        assert!(comment_has_marker(&controlled, embedded));
    }

    #[test]
    fn tracked_legacy_exact_marker_remains_reconcilable() {
        let api = MockApi::default();
        let store = MockStore::default();
        let previous = finding("legacy", OLD_HEAD_SHA);
        let previous_marker = finding_marker(&previous);
        let tracked = api.insert(&previous, "comment-legacy", false, &[]);
        {
            let mut state = api.0.lock().unwrap();
            let body = active_markdown(&state.comments["comment-legacy"].comment.markdown);
            state
                .comments
                .get_mut("comment-legacy")
                .unwrap()
                .comment
                .markdown = format!("{body}\n\n{previous_marker}");
        }

        let summary = FindingReconciler::new(&api, &store)
            .reconcile(&request(vec![tracked], vec![finding("legacy", HEAD_SHA)]))
            .expect("legacy comment reconciliation");

        assert_eq!(summary.status, FindingReconciliationStatus::Succeeded);
        assert_eq!(summary.counts.unchanged, 1);
        assert!(summary.actions[0].provider_mutated);
    }

    #[test]
    fn summary_json_is_versioned_and_actionable() {
        let summary = request(Vec::new(), Vec::new()).summary(vec![successful_action(
            "finding-1".to_string(),
            FindingReconciliationActionKind::Resolved,
            Some("comment-1".to_string()),
            Some("comment-1".to_string()),
            true,
        )]);

        assert_eq!(
            serde_json::to_value(summary).expect("summary JSON"),
            serde_json::json!({
                "schemaVersion": "v1",
                "status": "succeeded",
                "tenantId": "tenant-acme",
                "provider": "github",
                "workspace": "acme",
                "repository": "payments",
                "pullRequestId": 42,
                "baseSha": BASE_SHA,
                "headSha": HEAD_SHA,
                "counts": {
                    "unchanged": 0,
                    "created": 0,
                    "updated": 0,
                    "resolved": 1,
                    "reopened": 0,
                    "failed": 0
                },
                "actions": [{
                    "findingFingerprint": "finding-1",
                    "kind": "resolved",
                    "previousCommentId": "comment-1",
                    "commentId": "comment-1",
                    "providerMutated": true,
                    "error": null
                }]
            })
        );
    }

    #[test]
    fn dry_run_summary_is_deterministic_without_provider_mutations() {
        let summary = dry_run_reconciliation_summary(&request(
            vec![TrackedFindingComment {
                finding_fingerprint: "fixed".to_string(),
                comment_id: "comment-fixed".to_string(),
            }],
            vec![finding("new", HEAD_SHA)],
        ))
        .expect("dry-run summary");

        assert_eq!(summary.status, FindingReconciliationStatus::Succeeded);
        assert_eq!(summary.counts.resolved, 1);
        assert_eq!(summary.counts.created, 1);
        assert!(summary
            .actions
            .iter()
            .all(|action| !action.provider_mutated));
        assert!(summary.actions[1]
            .comment_id
            .as_deref()
            .is_some_and(|comment_id| comment_id.starts_with("dry-run-")));
    }
}
