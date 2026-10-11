//! Native approval boundary. Preview reads providers; only apply can write once.
use super::{
    bitbucket::approved_comments::{RemoteComment, StoredApprovedCommentApi},
    review, workspace_data as data,
};
use crate::draft_publication::{
    self as journal, ApprovedComment, ApprovedCommentApi, ApprovedPublisher, BeginPublication,
    CommentMode, CommentWrite, FindingAssociation, JournalEntry, PublicationJournal,
    PublicationScope, PublicationState, RemoteRevision, SqlitePublicationJournal,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_ITEMS: usize = 20;
const TTL: Duration = Duration::from_secs(300);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareParams {
    pub scope: PublicationScope,
    pub local_ids: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyParams {
    pub scope: PublicationScope,
    pub approval_id: String,
    pub local_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelParams {
    pub scope: PublicationScope,
    pub approval_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusParams {
    pub scope: PublicationScope,
    pub after: Option<String>,
    pub limit: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReconcileParams {
    pub scope: PublicationScope,
    pub local_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentsParams {
    pub scope: PublicationScope,
    pub offset: usize,
    pub limit: usize,
    pub version: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItem {
    pub comment: ApprovedComment,
    pub markdown: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preparation {
    pub scope: PublicationScope,
    pub approval_id: String,
    pub expires_at: u64,
    pub key: String,
    pub record_version: String,
    pub items: Vec<PreviewItem>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub scope: PublicationScope,
    pub entry: JournalEntry,
    pub local_saved: bool,
    pub draft_removed: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPage {
    pub scope: PublicationScope,
    pub entries: Vec<JournalEntry>,
    pub next_after: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentsResult {
    pub scope: PublicationScope,
    pub comments: Vec<RemoteComment>,
    pub version: String,
    pub offset: usize,
    pub total_items: usize,
    pub next_offset: Option<usize>,
}

pub(crate) trait PublicationApi: ApprovedCommentApi + Send + Sync {
    fn validate_positions(&self, comments: &[ApprovedComment]) -> Result<(), String>;
    fn create_cancellable(
        &self,
        comment: &ApprovedComment,
        stopped: &dyn Fn() -> bool,
    ) -> CommentWrite;
    fn comments(&self, scope: &PublicationScope) -> Result<Vec<RemoteComment>, String>;
}
impl PublicationApi for StoredApprovedCommentApi {
    fn validate_positions(&self, comments: &[ApprovedComment]) -> Result<(), String> {
        self.validate_positions(comments)
    }
    fn create_cancellable(
        &self,
        comment: &ApprovedComment,
        stopped: &dyn Fn() -> bool,
    ) -> CommentWrite {
        self.create_once_cancellable(comment, stopped)
    }
    fn comments(&self, scope: &PublicationScope) -> Result<Vec<RemoteComment>, String> {
        self.list_comments(scope)
    }
}
type ApiFactory =
    dyn Fn(crate::config::ReviewProvider) -> Result<Arc<dyn PublicationApi>, String> + Send + Sync;
struct Approval {
    scope: PublicationScope,
    key: String,
    record_version: String,
    issued: Instant,
    items: HashMap<String, ApprovedComment>,
    signatures: HashMap<String, String>,
    consumed: HashSet<String>,
    completed_findings: HashSet<String>,
    stopped: Arc<AtomicBool>,
    api: Arc<dyn PublicationApi>,
}
pub(crate) struct Manager {
    approvals: Mutex<HashMap<String, Approval>>,
    factory: Arc<ApiFactory>,
    shutdown: Arc<AtomicBool>,
}
impl Manager {
    pub fn new(shutdown: Arc<AtomicBool>) -> Self {
        Self {
            approvals: Mutex::new(HashMap::new()),
            factory: Arc::new(|provider| {
                Ok(Arc::new(StoredApprovedCommentApi::from_stored(provider)?))
            }),
            shutdown,
        }
    }
    fn api(&self, scope: &PublicationScope) -> Result<Arc<dyn PublicationApi>, String> {
        validate_scope(scope)?;
        (self.factory)(scope.provider)
    }
    pub fn prepare(&self, params: PrepareParams) -> Result<Preparation, String> {
        validate_scope(&params.scope)?;
        if params.local_ids.is_empty()
            || params.local_ids.len() > MAX_ITEMS
            || params.local_ids.iter().collect::<HashSet<_>>().len() != params.local_ids.len()
            || params.local_ids.iter().any(|id| !valid_id(id))
        {
            return Err("Select between one and twenty distinct drafts.".into());
        }
        if self.shutdown.load(Ordering::SeqCst) {
            return Err("Publication is shutting down.".into());
        }
        let key = scope_key(&params.scope);
        let (record_version, drafts) = data::draft_snapshot(&key)?;
        let api = self.api(&params.scope)?;
        let revision = api.current_revision(&params.scope)?;
        let mut comments = Vec::new();
        let mut signatures = HashMap::new();
        for local_id in params.local_ids {
            let draft = drafts
                .iter()
                .find(|draft| draft.local_id == local_id)
                .ok_or("The selected draft is unavailable; refresh the list.")?;
            let mode =
                match draft.publication_mode.as_deref() {
                    Some("general") => CommentMode::General,
                    Some("file") => CommentMode::File,
                    Some("inline") => CommentMode::Inline,
                    None if draft.path.is_empty() => CommentMode::General,
                    None if draft.to.is_some() || draft.from.is_some() => CommentMode::Inline,
                    None => CommentMode::File,
                    _ => return Err(
                        "This draft mode cannot be published. Choose an explicit supported mode."
                            .into(),
                    ),
                };
            if draft.pr_id != params.scope.pr_id {
                return Err("Draft belongs to another pull request.".into());
            }
            let comment = ApprovedComment {
                scope: params.scope.clone(),
                local_id: draft.local_id.clone(),
                head_sha: draft
                    .review_head_sha
                    .clone()
                    .unwrap_or_else(|| revision.head_sha.clone()),
                base_sha: draft
                    .review_base_sha
                    .clone()
                    .unwrap_or_else(|| revision.base_sha.clone()),
                mode,
                path: draft.path.clone(),
                to: draft.to,
                from: draft.from,
                parent_id: draft.parent_id.clone(),
                raw: draft.raw.clone(),
                finding_ref: draft.finding_ref.as_ref().map(|r| FindingAssociation {
                    review_run_id: r.review_run_id.clone(),
                    finding_id: r.finding_id.clone(),
                    finding_fingerprint: r.finding_fingerprint.clone(),
                }),
                original_draft_hash: Some(draft_hash(draft)?),
            };
            comment.validate()?;
            if let Some(entry) = SqlitePublicationJournal.lookup(&comment.identity_key())? {
                if entry.state != PublicationState::Rejected {
                    return Err("A selected draft has an existing publication. Inspect or reconcile it before preparing another send.".into());
                }
            }
            signatures.insert(comment.local_id.clone(), source_signature(&comment, false)?);
            comments.push(comment);
        }
        api.validate_positions(&comments)?;
        let previews: Vec<_> = comments
            .iter()
            .map(|comment| PreviewItem {
                markdown: comment.markdown(),
                comment: comment.clone(),
            })
            .collect();
        if serde_json::to_vec(&previews)
            .map_err(|_| "Cannot encode publication preview.")?
            .len()
            > 512 * 1024
        {
            return Err("Selected preview is too large; publish fewer drafts at once.".into());
        }
        data::with_locked_drafts(&key, |version, items| {
            if version != record_version {
                return Err("Drafts changed while preparing; refresh and prepare again.".into());
            }
            for comment in &comments {
                check_draft(comment, items)?;
            }
            Ok(())
        })?;
        let approval_id = token();
        let approval = Approval {
            scope: params.scope.clone(),
            key: key.clone(),
            record_version: record_version.clone(),
            issued: Instant::now(),
            items: comments
                .into_iter()
                .map(|comment| (comment.local_id.clone(), comment))
                .collect(),
            signatures,
            consumed: HashSet::new(),
            completed_findings: HashSet::new(),
            stopped: Arc::new(AtomicBool::new(false)),
            api,
        };
        let mut approvals = self
            .approvals
            .lock()
            .map_err(|_| "Publication approval is unavailable.")?;
        approvals.retain(|_, approval| {
            approval.issued.elapsed() < TTL && !approval.stopped.load(Ordering::SeqCst)
        });
        if approvals.len() >= 8 {
            return Err("Too many publication previews; cancel an older preview first.".into());
        }
        approvals.insert(approval_id.clone(), approval);
        Ok(Preparation {
            scope: params.scope,
            approval_id,
            expires_at: now_ms() + TTL.as_millis() as u64,
            key,
            record_version,
            items: previews,
        })
    }
    pub fn cancel(&self, params: CancelParams) -> Result<bool, String> {
        validate_scope(&params.scope)?;
        let mut approvals = self
            .approvals
            .lock()
            .map_err(|_| "Publication approval is unavailable.")?;
        if let Some(approval) = approvals.get(&params.approval_id) {
            if approval.scope != params.scope {
                return Err("Approval belongs to another pull request.".into());
            }
            approval.stopped.store(true, Ordering::SeqCst);
        }
        Ok(approvals.remove(&params.approval_id).is_some())
    }
    pub fn apply(&self, params: ApplyParams) -> Result<ItemResult, String> {
        validate_scope(&params.scope)?;
        let (comment, key, version, expected_signature, allow_published, api, stopped) = {
            let mut approvals = self
                .approvals
                .lock()
                .map_err(|_| "Publication approval is unavailable.")?;
            let approval = approvals
                .get_mut(&params.approval_id)
                .ok_or("Publication approval is unavailable; prepare again.")?;
            if approval.scope != params.scope
                || approval.issued.elapsed() >= TTL
                || approval.stopped.load(Ordering::SeqCst)
                || self.shutdown.load(Ordering::SeqCst)
            {
                return Err(
                    "Publication approval expired, was canceled, or belongs to another PR.".into(),
                );
            }
            let comment = approval
                .items
                .get(&params.local_id)
                .cloned()
                .ok_or("Draft was not part of this approval.")?;
            if !approval.consumed.insert(params.local_id.clone()) {
                return Err("This approval item has already been consumed. Inspect its publication status instead of retrying.".into());
            }
            let allow = comment
                .finding_ref
                .as_ref()
                .is_some_and(|r| approval.completed_findings.contains(&association_key(r)));
            (
                comment,
                approval.key.clone(),
                approval.record_version.clone(),
                approval.signatures[&params.local_id].clone(),
                allow,
                approval.api.clone(),
                approval.stopped.clone(),
            )
        };
        if source_signature(&comment, allow_published)? != expected_signature {
            return Err("Saved finding changed; prepare a fresh preview.".into());
        }
        let guarded = GuardedJournal {
            key: &key,
            version: &version,
            comment: &comment,
        };
        let api = CancellableApi {
            api: api.as_ref(),
            stopped: stopped.as_ref(),
            before_post: Some((&key, &version, &expected_signature, allow_published)),
            shutdown: self.shutdown.as_ref(),
        };
        let entry = ApprovedPublisher {
            journal: guarded,
            api,
        }
        .publish_approved(&comment)?;
        let result = finish(&key, entry);
        if let Ok(ref result) = result {
            let (version, drafts) = data::draft_snapshot(&key)?;
            let mut approvals = self
                .approvals
                .lock()
                .map_err(|_| "Publication approval is unavailable.")?;
            if let Some(approval) = approvals.get_mut(&params.approval_id) {
                let intact = approval
                    .items
                    .values()
                    .filter(|item| !approval.consumed.contains(&item.local_id))
                    .all(|item| check_draft(item, &drafts).is_ok());
                if intact {
                    approval.record_version = version;
                } else {
                    approval.stopped.store(true, Ordering::SeqCst);
                }
                if result.local_saved {
                    if let Some(r) = &comment.finding_ref {
                        approval.completed_findings.insert(association_key(r));
                    }
                }
            }
        }
        result
    }
    pub fn status(&self, params: StatusParams) -> Result<StatusPage, String> {
        validate_scope(&params.scope)?;
        let entries =
            journal::journal_scope_page(&params.scope, params.after.as_deref(), params.limit)?;
        let next_after = entries.last().map(|entry| entry.comment.identity_key());
        Ok(StatusPage {
            scope: params.scope,
            entries,
            next_after,
        })
    }
    pub fn reconcile(&self, params: ReconcileParams) -> Result<ItemResult, String> {
        validate_scope(&params.scope)?;
        if !valid_id(&params.local_id) {
            return Err("Invalid draft identity.".into());
        }
        let key = params.scope.identity_key(&params.local_id);
        let entry = SqlitePublicationJournal
            .lookup(&key)?
            .ok_or("Publication is not recorded.")?;
        if entry.comment.scope.provider != params.scope.provider
            || !entry
                .comment
                .scope
                .workspace
                .eq_ignore_ascii_case(&params.scope.workspace)
            || !entry
                .comment
                .scope
                .repo
                .eq_ignore_ascii_case(&params.scope.repo)
            || entry.comment.scope.pr_id != params.scope.pr_id
        {
            return Err("Publication belongs to another scope.".into());
        }
        let api = self.api(&entry.comment.scope)?;
        let api = CancellableApi {
            api: api.as_ref(),
            stopped: self.shutdown.as_ref(),
            before_post: None,
            shutdown: self.shutdown.as_ref(),
        };
        let entry = ApprovedPublisher {
            journal: SqlitePublicationJournal,
            api,
        }
        .reconcile(&key)?;
        finish(&scope_key(&entry.comment.scope), entry)
    }
    pub fn comments(&self, params: CommentsParams) -> Result<CommentsResult, String> {
        if params.limit == 0
            || params.limit > 100
            || params.offset > 1000
            || params.version.as_ref().is_some_and(|version| {
                version.len() != 64 || !version.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            return Err("Invalid remote comment page size.".into());
        }
        let all = self.api(&params.scope)?.comments(&params.scope)?;
        let version =
            hash(&serde_json::to_vec(&all).map_err(|_| "Cannot encode remote comments.")?);
        if params
            .version
            .as_ref()
            .is_some_and(|expected| expected != &version)
        {
            return Err("Remote comments changed; refresh before reading the next page.".into());
        }
        if params.offset > all.len() {
            return Err("Invalid remote comment page offset.".into());
        }
        let mut comments = Vec::new();
        let mut bytes = 0;
        for comment in all.iter().skip(params.offset).take(params.limit) {
            let size = serde_json::to_vec(comment)
                .map_err(|_| "Cannot encode remote comment.")?
                .len();
            if bytes + size > 512 * 1024 {
                break;
            }
            bytes += size;
            comments.push(comment.clone());
        }
        let end = params.offset + comments.len();
        Ok(CommentsResult {
            scope: params.scope,
            comments,
            version,
            offset: params.offset,
            total_items: all.len(),
            next_offset: (end < all.len()).then_some(end),
        })
    }
}
fn finish(key: &str, entry: JournalEntry) -> Result<ItemResult, String> {
    let scope = entry.comment.scope.clone();
    if entry.state != PublicationState::Published {
        return Ok(ItemResult {
            scope,
            entry,
            local_saved: false,
            draft_removed: false,
        });
    }
    // ACK stays in SQLite even if finding persistence or canonical cleanup fails.
    let local_saved = review::acknowledge_draft_publication(&entry).is_ok();
    let draft_removed =
        local_saved && data::remove_acknowledged_draft(key, &entry).unwrap_or(false);
    Ok(ItemResult {
        scope,
        entry,
        local_saved,
        draft_removed,
    })
}
struct GuardedJournal<'a> {
    key: &'a str,
    version: &'a str,
    comment: &'a ApprovedComment,
}
impl PublicationJournal for GuardedJournal<'_> {
    fn lookup(&self, key: &str) -> Result<Option<JournalEntry>, String> {
        SqlitePublicationJournal.lookup(key)
    }
    fn begin(&self, comment: &ApprovedComment) -> Result<BeginPublication, String> {
        if comment != self.comment {
            return Err("Publication payload changed.".into());
        }
        data::with_locked_drafts(self.key, |version, drafts| {
            if version != self.version {
                return Err("Draft record changed; prepare a fresh preview.".into());
            }
            check_draft(comment, drafts)?;
            SqlitePublicationJournal.begin(comment)
        })
    }
    fn transition(
        &self,
        entry: &JournalEntry,
        state: PublicationState,
        remote_id: Option<&str>,
    ) -> Result<JournalEntry, String> {
        SqlitePublicationJournal.transition(entry, state, remote_id)
    }
}
struct CancellableApi<'a> {
    api: &'a dyn PublicationApi,
    stopped: &'a AtomicBool,
    shutdown: &'a AtomicBool,
    before_post: Option<(&'a str, &'a str, &'a str, bool)>,
}
impl ApprovedCommentApi for CancellableApi<'_> {
    fn current_revision(&self, scope: &PublicationScope) -> Result<RemoteRevision, String> {
        if self.stopped.load(Ordering::SeqCst) || self.shutdown.load(Ordering::SeqCst) {
            return Err("Publication canceled.".into());
        }
        self.api.current_revision(scope)
    }
    fn create_once(&self, comment: &ApprovedComment) -> CommentWrite {
        self.api.create_cancellable(comment, &|| {
            self.stopped.load(Ordering::SeqCst)
                || self.shutdown.load(Ordering::SeqCst)
                || self
                    .before_post
                    .is_some_and(|(key, expected_version, signature, allow)| {
                        !source_signature(comment, allow).is_ok_and(|actual| actual == signature)
                            || data::with_locked_drafts(key, |version, drafts| {
                                if version != expected_version {
                                    return Err("Approved draft record changed.".into());
                                }
                                check_draft(comment, drafts)
                            })
                            .is_err()
                    })
        })
    }
    fn reconcile(&self, comment: &ApprovedComment) -> Result<Option<String>, String> {
        self.api.reconcile(comment)
    }
}
fn check_draft(comment: &ApprovedComment, drafts: &[data::DraftComment]) -> Result<(), String> {
    let draft = drafts
        .iter()
        .find(|draft| draft.local_id == comment.local_id)
        .ok_or("Approved draft is unavailable.")?;
    if comment.original_draft_hash.as_deref() != Some(&draft_hash(draft)?) {
        return Err("Approved draft changed; prepare again.".into());
    }
    Ok(())
}
fn association_key(reference: &FindingAssociation) -> String {
    format!(
        "{}:{}:{}",
        reference.review_run_id, reference.finding_id, reference.finding_fingerprint
    )
}
fn source_signature(comment: &ApprovedComment, allow_published: bool) -> Result<String, String> {
    let Some(reference) = &comment.finding_ref else {
        return Ok(String::new());
    };
    let store = review::load_ai_review_store_native(
        &comment.scope.workspace,
        &comment.scope.repo,
        comment.scope.pr_id,
    )?
    .ok_or("The saved review is unavailable.")?;
    let runs = store
        .review_runs
        .iter()
        .filter(|run| run.id == reference.review_run_id)
        .collect::<Vec<_>>();
    let [run] = runs.as_slice() else {
        return Err("Saved review identity is missing or ambiguous.".into());
    };
    let provider = match comment.scope.provider {
        crate::config::ReviewProvider::Github => review::ReviewProvider::Github,
        crate::config::ReviewProvider::Bitbucket => review::ReviewProvider::Bitbucket,
    };
    if run.provider != provider
        || run.workspace != comment.scope.workspace
        || run.repo != comment.scope.repo
        || run.pr_id != comment.scope.pr_id
        || run.reviewed_head_sha.as_deref() != Some(&comment.head_sha)
        || run.reviewed_base_sha.as_deref() != Some(&comment.base_sha)
    {
        return Err("Draft metadata does not match its saved review scope and revisions.".into());
    }
    let findings = run
        .findings
        .iter()
        .filter(|finding| {
            finding.id == reference.finding_id
                && finding.fingerprint == reference.finding_fingerprint
        })
        .collect::<Vec<_>>();
    let [finding] = findings.as_slice() else {
        return Err("Saved finding identity is missing or ambiguous.".into());
    };
    if matches!(
        finding.status,
        review::ReviewFindingStatus::Dismissed
            | review::ReviewFindingStatus::Fixed
            | review::ReviewFindingStatus::Stale
    ) || (finding.status == review::ReviewFindingStatus::Published && !allow_published)
    {
        return Err("Finding is no longer available for publication.".into());
    }
    if comment.mode != CommentMode::General && comment.parent_id.is_none() {
        let anchor = finding
            .anchor
            .as_ref()
            .ok_or("Saved finding has no file anchor.")?;
        if anchor.path != comment.path {
            return Err("Draft file differs from its saved finding anchor.".into());
        }
        if comment.mode == CommentMode::Inline {
            let (to, from) = match anchor.side {
                review::ReviewAnchorSide::New => (Some(anchor.start_line), None),
                review::ReviewAnchorSide::Old => (None, Some(anchor.start_line)),
            };
            if anchor.end_line.is_some_and(|end| end != anchor.start_line)
                || comment.to != to
                || comment.from != from
            {
                return Err("Draft line differs from its saved finding anchor.".into());
            }
        }
    }
    let feedback = crate::review_storage::get_finding_feedback_state(
        &crate::review_feedback::ReviewFindingFeedbackTarget {
            identity: crate::review_feedback::ReviewFindingFeedbackIdentity {
                tenant_id: "local".into(),
                provider: match comment.scope.provider {
                    crate::config::ReviewProvider::Github => {
                        crate::review_event::PullRequestReviewEventProvider::Github
                    }
                    crate::config::ReviewProvider::Bitbucket => {
                        crate::review_event::PullRequestReviewEventProvider::Bitbucket
                    }
                },
                workspace: comment.scope.workspace.clone(),
                repo: comment.scope.repo.clone(),
                pr_id: u64::from(comment.scope.pr_id),
            },
            review_run_id: reference.review_run_id.clone(),
            finding_fingerprint: reference.finding_fingerprint.clone(),
        },
    )?;
    if matches!(
        feedback.disposition,
        crate::review_feedback::ReviewFindingDisposition::Dismissed
            | crate::review_feedback::ReviewFindingDisposition::FalsePositive
            | crate::review_feedback::ReviewFindingDisposition::Fixed
    ) {
        return Err("Reviewer feedback no longer permits publishing this finding.".into());
    }
    let mut value = serde_json::to_value(finding).map_err(|_| "Cannot encode saved finding.")?;
    let object = value.as_object_mut().ok_or("Invalid saved finding.")?;
    object.remove("status");
    object.remove("publication");
    Ok(hash(
        &serde_json::to_vec(&(run.id.as_str(), value))
            .map_err(|_| "Cannot encode saved finding.")?,
    ))
}
pub(crate) fn validate_scope(scope: &PublicationScope) -> Result<(), String> {
    if !valid_id(&scope.workspace)
        || !valid_id(&scope.repo)
        || [&scope.workspace, &scope.repo]
            .iter()
            .any(|s| matches!(s.as_str(), "." | "..") || s.contains(['/', '\\', ':']))
        || scope.pr_id == 0
        || crate::local_review::is_local_review_id(u64::from(scope.pr_id))
    {
        return Err("Select a valid provider pull request.".into());
    }
    Ok(())
}
fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
fn scope_key(scope: &PublicationScope) -> String {
    format!(
        "drafts.{}:{}/{}/{}",
        match scope.provider {
            crate::config::ReviewProvider::Github => "github",
            crate::config::ReviewProvider::Bitbucket => "bitbucket",
        },
        scope.workspace,
        scope.repo,
        scope.pr_id
    )
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn draft_hash(draft: &data::DraftComment) -> Result<String, String> {
    Ok(hash(
        &serde_json::to_vec(draft).map_err(|_| "Cannot encode draft.")?,
    ))
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn token() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    hash(
        format!(
            "{}:{}:{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
        .as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, ffi::OsString, sync::atomic::AtomicUsize};
    struct Environment {
        _lock: std::sync::MutexGuard<'static, ()>,
        _directory: tempfile::TempDir,
        values: Vec<(&'static str, Option<OsString>)>,
    }
    impl Environment {
        fn new() -> Self {
            let lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
                .lock()
                .unwrap();
            let directory = tempfile::tempdir().unwrap();
            let values = ["NORN_DATA_DIR", "NORN_REVIEW_DATA_DIR"]
                .into_iter()
                .map(|key| {
                    let old = std::env::var_os(key);
                    std::env::set_var(key, directory.path());
                    (key, old)
                })
                .collect();
            Self {
                _lock: lock,
                _directory: directory,
                values,
            }
        }
    }
    impl Drop for Environment {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
    #[derive(Default)]
    struct FakeApi {
        posts: AtomicUsize,
        results: Mutex<VecDeque<CommentWrite>>,
        created: Mutex<HashMap<String, String>>,
    }
    impl ApprovedCommentApi for FakeApi {
        fn current_revision(&self, _: &PublicationScope) -> Result<RemoteRevision, String> {
            Ok(RemoteRevision {
                head_sha: "b".repeat(40),
                base_sha: "a".repeat(40),
            })
        }
        fn create_once(&self, comment: &ApprovedComment) -> CommentWrite {
            self.create_cancellable(comment, &|| false)
        }
        fn reconcile(&self, comment: &ApprovedComment) -> Result<Option<String>, String> {
            Ok(self.created.lock().unwrap().get(&comment.local_id).cloned())
        }
    }
    impl PublicationApi for FakeApi {
        fn validate_positions(&self, comments: &[ApprovedComment]) -> Result<(), String> {
            for comment in comments {
                comment.validate()?;
                if comment.head_sha != "b".repeat(40) {
                    return Err("stale".into());
                }
            }
            Ok(())
        }
        fn create_cancellable(
            &self,
            comment: &ApprovedComment,
            stopped: &dyn Fn() -> bool,
        ) -> CommentWrite {
            if stopped() {
                return CommentWrite::Rejected;
            }
            let entry = SqlitePublicationJournal
                .lookup(&comment.identity_key())
                .unwrap()
                .unwrap();
            assert_eq!(
                entry.state,
                PublicationState::Writing,
                "durable reservation precedes POST"
            );
            let number = self.posts.fetch_add(1, Ordering::SeqCst) + 1;
            let id = number.to_string();
            self.created
                .lock()
                .unwrap()
                .insert(comment.local_id.clone(), id.clone());
            self.results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(CommentWrite::Created(id))
        }
        fn comments(&self, _: &PublicationScope) -> Result<Vec<RemoteComment>, String> {
            Ok(vec![])
        }
    }
    fn scope() -> PublicationScope {
        PublicationScope {
            provider: crate::config::ReviewProvider::Github,
            workspace: "synthetic".into(),
            repo: "test".into(),
            pr_id: 7,
        }
    }
    fn manager(api: Arc<FakeApi>) -> Manager {
        Manager {
            approvals: Mutex::new(HashMap::new()),
            factory: Arc::new(move |_| Ok(api.clone())),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }
    fn stage(id: &str, raw: &str) {
        let scope = scope();
        let key = scope_key(&scope);
        let (version, _) = data::draft_snapshot(&key).unwrap();
        data::record_mutate(data::RecordMutateParams {
            key,
            expected_version: version,
            mutation: data::RecordMutation::PutDraft {
                draft: Box::new(data::DraftComment {
                    local_id: id.into(),
                    pr_id: 7,
                    raw: raw.into(),
                    path: String::new(),
                    to: None,
                    from: None,
                    parent_id: None,
                    created_at: 1,
                    source: Some("manual".into()),
                    finding_ref: None,
                    publication_mode: Some("general".into()),
                    review_head_sha: None,
                    review_base_sha: None,
                }),
            },
        })
        .unwrap();
    }
    fn prepare(manager: &Manager, ids: &[&str]) -> Preparation {
        manager
            .prepare(PrepareParams {
                scope: scope(),
                local_ids: ids.iter().map(|id| (*id).into()).collect(),
            })
            .unwrap()
    }
    fn apply(manager: &Manager, preparation: &Preparation, id: &str) -> Result<ItemResult, String> {
        manager.apply(ApplyParams {
            scope: scope(),
            approval_id: preparation.approval_id.clone(),
            local_id: id.into(),
        })
    }
    #[test]
    fn explicit_single_use_batch_and_scope_cas_boundaries() {
        let _environment = Environment::new();
        let api = Arc::new(FakeApi::default());
        let manager = manager(api.clone());
        stage("one", "Original CRLF\r\n世界");
        stage("two", "Other body");
        let preview = prepare(&manager, &["one", "two"]);
        assert_eq!(api.posts.load(Ordering::SeqCst), 0);
        assert!(preview.items[0].markdown.contains("\r\n世界"));
        let mut wrong = scope();
        wrong.provider = crate::config::ReviewProvider::Bitbucket;
        assert!(manager
            .apply(ApplyParams {
                scope: wrong,
                approval_id: preview.approval_id.clone(),
                local_id: "one".into()
            })
            .is_err());
        let first = apply(&manager, &preview, "one").unwrap();
        assert!(first.local_saved && first.draft_removed);
        assert!(apply(&manager, &preview, "one").is_err());
        assert!(apply(&manager, &preview, "two").unwrap().draft_removed);
        assert_eq!(api.posts.load(Ordering::SeqCst), 2);
        assert!(data::draft_snapshot(&scope_key(&scope()))
            .unwrap()
            .1
            .is_empty());
        stage("three", "before");
        let preview = prepare(&manager, &["three"]);
        stage("three", "edited after preview");
        assert!(apply(&manager, &preview, "three").is_err());
        assert_eq!(api.posts.load(Ordering::SeqCst), 2);
        let preview = prepare(&manager, &["three"]);
        manager
            .cancel(CancelParams {
                scope: scope(),
                approval_id: preview.approval_id.clone(),
            })
            .unwrap();
        assert!(apply(&manager, &preview, "three").is_err());
        assert_eq!(api.posts.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn uncertain_result_blocks_edits_and_restart_reconciles_without_post() {
        let _environment = Environment::new();
        let api = Arc::new(FakeApi::default());
        api.results.lock().unwrap().push_back(CommentWrite::Unknown);
        let manager = manager(api.clone());
        stage("uncertain", "send once");
        let result = apply(&manager, &prepare(&manager, &["uncertain"]), "uncertain").unwrap();
        assert_eq!(result.entry.state, PublicationState::Uncertain);
        assert!(!result.draft_removed);
        let key = scope_key(&scope());
        let (version, _) = data::draft_snapshot(&key).unwrap();
        assert!(data::record_mutate(data::RecordMutateParams {
            key: key.clone(),
            expected_version: version,
            mutation: data::RecordMutation::RemoveDraft {
                local_id: "uncertain".into()
            }
        })
        .is_err());
        assert!(manager
            .prepare(PrepareParams {
                scope: scope(),
                local_ids: vec!["uncertain".into()]
            })
            .is_err());
        drop(manager);
        let recovered = super::tests::manager(api.clone())
            .reconcile(ReconcileParams {
                scope: scope(),
                local_id: "uncertain".into(),
            })
            .unwrap();
        assert_eq!(recovered.entry.state, PublicationState::Published);
        assert!(recovered.draft_removed);
        assert_eq!(api.posts.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn corrupted_or_expired_approval_and_partial_batch_preserve_other_drafts() {
        let _environment = Environment::new();
        let api = Arc::new(FakeApi::default());
        api.results
            .lock()
            .unwrap()
            .extend([CommentWrite::Created("123".into()), CommentWrite::Unknown]);
        let manager = manager(api.clone());
        stage("one", "first");
        stage("two", "second");
        stage("three", "third");
        let preview = prepare(&manager, &["one", "two", "three"]);
        assert!(apply(&manager, &preview, "one").unwrap().draft_removed);
        assert_eq!(
            apply(&manager, &preview, "two").unwrap().entry.state,
            PublicationState::Uncertain
        );
        manager
            .cancel(CancelParams {
                scope: scope(),
                approval_id: preview.approval_id.clone(),
            })
            .unwrap();
        assert!(apply(&manager, &preview, "three").is_err());
        assert_eq!(
            data::draft_snapshot(&scope_key(&scope()))
                .unwrap()
                .1
                .iter()
                .map(|draft| draft.local_id.as_str())
                .collect::<Vec<_>>(),
            vec!["two", "three"]
        );
        let preview = prepare(&manager, &["three"]);
        manager
            .approvals
            .lock()
            .unwrap()
            .get_mut(&preview.approval_id)
            .unwrap()
            .issued = Instant::now() - TTL;
        assert!(apply(&manager, &preview, "three").is_err());
        assert_eq!(api.posts.load(Ordering::SeqCst), 2);
    }
    fn associated_draft() -> serde_json::Value {
        use serde_json::json;
        let original = json!({"activeThreadId":null,"threads":[],"retained":{"chat":"keep"},"reviewRuns":[{
            "id":"run","schemaVersion":"1","provider":"github","workspace":"synthetic","repo":"test","prId":7,
            "sourceBranch":"feature","destinationBranch":"main","reviewedHeadSha":"b".repeat(40),"reviewedBaseSha":"a".repeat(40),"status":"succeeded","turnKind":"initial","createdAt":"1","finishedAt":"2","diffFingerprint":"diff",
            "findings":[{"id":"finding","fingerprint":"fingerprint","title":"Validate input","severity":"high","confidence":"high","category":"bug","status":"new","summary":"Original evidence", "source":"llm","evidenceIds":[],"publication":null}]}]});
        crate::review_storage::save_review_json("synthetic", "test", 7, &original.to_string())
            .unwrap();
        stage("associated", "Edited finding text");
        let key = scope_key(&scope());
        let (version, drafts) = data::draft_snapshot(&key).unwrap();
        let mut draft = drafts[0].clone();
        draft.finding_ref = Some(data::FindingReference {
            review_run_id: "run".into(),
            finding_id: "finding".into(),
            finding_fingerprint: "fingerprint".into(),
        });
        draft.review_head_sha = Some("b".repeat(40));
        draft.review_base_sha = Some("a".repeat(40));
        data::record_mutate(data::RecordMutateParams {
            key,
            expected_version: version,
            mutation: data::RecordMutation::PutDraft {
                draft: Box::new(draft),
            },
        })
        .unwrap();
        original
    }
    #[test]
    fn finding_acknowledgment_and_feedback_are_atomic_and_repairable() {
        let _environment = Environment::new();
        let api = Arc::new(FakeApi::default());
        let manager = manager(api.clone());
        let original = associated_draft();
        let preview = prepare(&manager, &["associated"]);
        let comment = &preview.items[0].comment;
        use crate::review_feedback::{
            ReviewFindingFeedbackAction, ReviewFindingFeedbackEvent, ReviewFindingFeedbackIdentity,
        };
        let conflict = ReviewFindingFeedbackEvent {
            event_id: format!("publication:{}", comment.identity_key()),
            identity: ReviewFindingFeedbackIdentity {
                tenant_id: "local".into(),
                provider: crate::review_event::PullRequestReviewEventProvider::Github,
                workspace: "synthetic".into(),
                repo: "test".into(),
                pr_id: 7,
            },
            review_run_id: "run".into(),
            finding_fingerprint: "fingerprint".into(),
            action: ReviewFindingFeedbackAction::Accepted,
            occurred_at: "1".into(),
            actor_id: "other".into(),
            reason: None,
        };
        crate::review_storage::record_finding_feedback(&conflict).unwrap();
        let result = apply(&manager, &preview, "associated").unwrap();
        assert_eq!(result.entry.state, PublicationState::Published);
        assert!(!result.local_saved);
        assert!(!result.draft_removed);
        let saved = crate::review_storage::load_review_json("synthetic", "test", 7)
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
            original,
            "feedback conflict rolls back finding mutation"
        );
        assert_eq!(
            data::draft_snapshot(&scope_key(&scope())).unwrap().1.len(),
            1
        );
        assert_eq!(api.posts.load(Ordering::SeqCst), 1);
        assert!(
            !manager
                .reconcile(ReconcileParams {
                    scope: scope(),
                    local_id: "associated".into()
                })
                .unwrap()
                .local_saved
        );
        assert_eq!(api.posts.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn confirmed_finding_publication_records_feedback_and_stale_source_cannot_send() {
        let _environment = Environment::new();
        let api = Arc::new(FakeApi::default());
        let manager = manager(api.clone());
        let mut original = associated_draft();
        let preview = prepare(&manager, &["associated"]);
        original["reviewRuns"][0]["findings"][0]["summary"] = serde_json::json!("Changed evidence");
        crate::review_storage::save_review_json("synthetic", "test", 7, &original.to_string())
            .unwrap();
        assert!(apply(&manager, &preview, "associated").is_err());
        assert_eq!(api.posts.load(Ordering::SeqCst), 0);
        let preview = prepare(&manager, &["associated"]);
        let result = apply(&manager, &preview, "associated").unwrap();
        assert!(result.local_saved && result.draft_removed);
        let saved: serde_json::Value = serde_json::from_str(
            &crate::review_storage::load_review_json("synthetic", "test", 7)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(saved["retained"], original["retained"]);
        assert_eq!(
            saved["reviewRuns"][0]["findings"][0]["summary"],
            "Changed evidence"
        );
        assert_eq!(saved["reviewRuns"][0]["findings"][0]["status"], "published");
        assert!(
            manager
                .reconcile(ReconcileParams {
                    scope: scope(),
                    local_id: "associated".into()
                })
                .unwrap()
                .local_saved
        );
        assert_eq!(api.posts.load(Ordering::SeqCst), 1);
    }
}
