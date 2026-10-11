//! Crash-safe explicit publication. An uncertain write is reconciled, never retried.
use crate::{config::ReviewProvider, review_storage};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_BODY: usize = 32 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicationScope {
    pub provider: ReviewProvider,
    pub workspace: String,
    pub repo: String,
    pub pr_id: u32,
}
impl PublicationScope {
    pub fn identity_key(&self, local_id: &str) -> String {
        digest(
            &serde_json::to_vec(&(
                self.provider,
                self.workspace.to_ascii_lowercase(),
                self.repo.to_ascii_lowercase(),
                self.pr_id,
                local_id,
            ))
            .expect("serializable identity"),
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommentMode {
    Inline,
    File,
    General,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingAssociation {
    pub review_run_id: String,
    pub finding_id: String,
    pub finding_fingerprint: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovedComment {
    pub scope: PublicationScope,
    pub local_id: String,
    pub head_sha: String,
    pub base_sha: String,
    pub mode: CommentMode,
    pub path: String,
    pub to: Option<u32>,
    pub from: Option<u32>,
    pub parent_id: Option<String>,
    /// Exact user-visible body. The deterministic hidden marker is appended once.
    pub raw: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding_ref: Option<FindingAssociation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_draft_hash: Option<String>,
}
fn identifier(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn sha(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl ApprovedComment {
    pub fn validate(&self) -> Result<(), String> {
        if !identifier(&self.scope.workspace)
            || !identifier(&self.scope.repo)
            || [self.scope.workspace.as_str(), self.scope.repo.as_str()]
                .iter()
                .any(|value| matches!(*value, "." | "..") || value.contains(['/', '\\', ':']))
            || self.scope.pr_id == 0
            || crate::local_review::is_local_review_id(u64::from(self.scope.pr_id))
            || !identifier(&self.local_id)
            || !sha(&self.head_sha)
            || !sha(&self.base_sha)
            || self
                .parent_id
                .as_deref()
                .is_some_and(|value| !identifier(value))
            || self.raw.trim().is_empty()
            || self
                .raw
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\r' | '\n' | '\t'))
            || self.raw.contains("<!-- norn-draft:")
            || self.markdown().len() > MAX_BODY
        {
            return Err("Invalid approved comment identity, revision or body.".into());
        }
        if self.finding_ref.as_ref().is_some_and(|finding| {
            [
                &finding.review_run_id,
                &finding.finding_id,
                &finding.finding_fingerprint,
            ]
            .iter()
            .any(|value| !identifier(value))
        }) || self.original_draft_hash.as_deref().is_some_and(|value| {
            value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err("Invalid original draft or finding association.".into());
        }
        let drive = self
            .path
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && self.path.as_bytes().get(1) == Some(&b':');
        let path_valid = !(self.path.is_empty()
            || self.path.len() > 4096
            || self.path.starts_with('/')
            || self.path.contains('\\')
            || drive
            || self.path.chars().any(char::is_control)
            || self
                .path
                .split('/')
                .any(|part| part.is_empty() || matches!(part, "." | "..")));
        let anchor_valid = match self.mode {
            CommentMode::General => {
                self.path.is_empty() && self.to.is_none() && self.from.is_none()
            }
            CommentMode::File => path_valid && self.to.is_none() && self.from.is_none(),
            CommentMode::Inline => {
                path_valid && matches!((self.to, self.from), (Some(1..), None) | (None, Some(1..)))
            }
        };
        if !anchor_valid {
            return Err(
                "Invalid approved comment anchor; choose a supported mode explicitly.".into(),
            );
        }
        Ok(())
    }
    pub fn identity_key(&self) -> String {
        // Scope aliases cannot bypass the durable duplicate fence.
        self.scope.identity_key(&self.local_id)
    }

    pub fn marker(&self) -> String {
        format!("<!-- norn-draft:{} -->", self.identity_key())
    }
    pub fn markdown(&self) -> String {
        format!("{}\n\n{}", self.raw, self.marker())
    }
    pub(crate) fn payload_hash(&self) -> Result<String, String> {
        serde_json::to_vec(self)
            .map(|bytes| digest(&bytes))
            .map_err(|_| "Cannot encode approved comment.".into())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PublicationState {
    Writing,
    Uncertain,
    Rejected,
    Published,
    PublishedSnapshotChanged,
    PublishedUnverified,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JournalEntry {
    pub comment: ApprovedComment,
    pub payload_hash: String,
    pub attempt: String,
    pub state: PublicationState,
    pub remote_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
}
#[derive(Debug, Clone)]
pub enum BeginPublication {
    Acquired(JournalEntry),
    Existing(JournalEntry),
}
pub trait PublicationJournal {
    fn lookup(&self, identity_key: &str) -> Result<Option<JournalEntry>, String>;
    fn begin(&self, comment: &ApprovedComment) -> Result<BeginPublication, String>;
    fn transition(
        &self,
        entry: &JournalEntry,
        state: PublicationState,
        remote_id: Option<&str>,
    ) -> Result<JournalEntry, String>;
}
#[derive(Default)]
pub struct SqlitePublicationJournal;
fn lookup_at(connection: &Connection, key: &str) -> Result<Option<JournalEntry>, String> {
    let raw: Option<String> = connection
        .query_row(
            "SELECT entry_json FROM local_draft_publications WHERE identity_key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| "Cannot read the publication journal.".to_string())?;
    raw.map(|raw| decode_entry(key, &raw)).transpose()
}
fn decode_entry(key: &str, raw: &str) -> Result<JournalEntry, String> {
    if raw.len() > 96 * 1024 {
        return Err("Publication journal entry exceeds its limit.".into());
    }
    let entry: JournalEntry = serde_json::from_str(raw)
        .map_err(|_| "Publication journal entry is invalid.".to_string())?;
    entry.comment.validate()?;
    if entry.comment.identity_key() != key
        || entry.comment.payload_hash()? != entry.payload_hash
        || !identifier(&entry.attempt)
        || entry.remote_id.as_deref().is_some_and(|id| !identifier(id))
        || matches!(
            entry.state,
            PublicationState::Published
                | PublicationState::PublishedSnapshotChanged
                | PublicationState::PublishedUnverified
        ) != entry.remote_id.is_some()
    {
        return Err("Publication journal entry does not match its identity.".into());
    }
    Ok(entry)
}

pub fn journal_scope_page(
    scope: &PublicationScope,
    after: Option<&str>,
    limit: usize,
) -> Result<Vec<JournalEntry>, String> {
    if limit == 0
        || limit > 50
        || after.is_some_and(|cursor| {
            cursor.len() != 64 || !cursor.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return Err("Invalid publication journal page.".into());
    }
    let connection = review_storage::publication_database()?;
    let provider = match scope.provider {
        ReviewProvider::Github => "github",
        ReviewProvider::Bitbucket => "bitbucket",
    };
    let mut statement = connection.prepare("SELECT identity_key, entry_json FROM local_draft_publications WHERE json_extract(entry_json, '$.comment.scope.provider') = ?1 AND lower(json_extract(entry_json, '$.comment.scope.workspace')) = ?2 AND lower(json_extract(entry_json, '$.comment.scope.repo')) = ?3 AND json_extract(entry_json, '$.comment.scope.prId') = ?4 AND identity_key > ?5 ORDER BY identity_key LIMIT ?6").map_err(|_| "Cannot read scoped publication history.".to_string())?;
    let rows = statement
        .query_map(
            params![
                provider,
                scope.workspace.to_ascii_lowercase(),
                scope.repo.to_ascii_lowercase(),
                scope.pr_id,
                after.unwrap_or(""),
                limit as i64
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|_| "Cannot read scoped publication history.".to_string())?;
    let mut entries = Vec::new();
    let mut bytes = 0;
    for row in rows {
        let (key, raw) = row.map_err(|_| "Cannot read scoped publication history.".to_string())?;
        if bytes + raw.len() > 512 * 1024 {
            break;
        }
        bytes += raw.len();
        entries.push(decode_entry(&key, &raw)?);
    }
    Ok(entries)
}

fn attempt_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    digest(
        format!(
            "{}:{}:{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        )
        .as_bytes(),
    )
}
fn begin_at(
    connection: &mut Connection,
    comment: &ApprovedComment,
) -> Result<BeginPublication, String> {
    comment.validate()?;
    let key = comment.identity_key();
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| "Cannot reserve publication.".to_string())?;
    if let Some(existing) = lookup_at(&tx, &key)? {
        if existing.state != PublicationState::Rejected {
            if existing.payload_hash != comment.payload_hash()? {
                return Err("This draft already has a pending or published payload. Reconcile its original publication before editing or retrying.".into());
            }
            return Ok(BeginPublication::Existing(existing));
        }
    }
    let entry = JournalEntry {
        comment: comment.clone(),
        payload_hash: comment.payload_hash()?,
        attempt: attempt_id(),
        state: PublicationState::Writing,
        remote_id: None,
        confirmed_at: None,
        started_at: Some(
            u64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| "Cannot timestamp publication reservation.")?
                    .as_millis(),
            )
            .map_err(|_| "Cannot timestamp publication reservation.")?,
        ),
    };
    let json = serde_json::to_string(&entry)
        .map_err(|_| "Cannot encode publication reservation.".to_string())?;
    tx.execute("INSERT INTO local_draft_publications(identity_key, entry_json) VALUES (?1, ?2) ON CONFLICT(identity_key) DO UPDATE SET entry_json = excluded.entry_json", params![key, json]).map_err(|_| "Cannot persist publication reservation.".to_string())?;
    tx.commit()
        .map_err(|_| "Cannot commit publication reservation.".to_string())?;
    Ok(BeginPublication::Acquired(entry))
}
fn transition_at(
    connection: &mut Connection,
    entry: &JournalEntry,
    state: PublicationState,
    remote_id: Option<&str>,
) -> Result<JournalEntry, String> {
    if matches!(state, PublicationState::Writing)
        || matches!(
            state,
            PublicationState::Published
                | PublicationState::PublishedSnapshotChanged
                | PublicationState::PublishedUnverified
        ) != remote_id.is_some()
        || remote_id.is_some_and(|id| !identifier(id))
    {
        return Err("Invalid publication transition.".into());
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| "Cannot update publication.".to_string())?;
    let current = lookup_at(&tx, &entry.comment.identity_key())?
        .ok_or("Publication reservation is missing.")?;
    if current.attempt != entry.attempt || current.payload_hash != entry.payload_hash {
        return Err("Publication reservation changed; reload its status.".into());
    }
    if current.state == PublicationState::PublishedUnverified
        && (remote_id != current.remote_id.as_deref()
            || !matches!(
                state,
                PublicationState::Published
                    | PublicationState::PublishedSnapshotChanged
                    | PublicationState::PublishedUnverified
            ))
    {
        return Err(
            "Acknowledged publication cannot be retried or lose its remote identity.".into(),
        );
    }
    if !matches!(
        current.state,
        PublicationState::Writing
            | PublicationState::Uncertain
            | PublicationState::PublishedUnverified
    ) {
        if current.state == state && current.remote_id.as_deref() == remote_id {
            return Ok(current);
        }
        return Err("Publication is already finalized; reload its status.".into());
    }
    // Once a request may have reached the provider, absence cannot prove rejection.
    if current.state == PublicationState::Uncertain && state == PublicationState::Rejected {
        return Err("Uncertain publication requires reconciliation, not a retry.".into());
    }
    let confirmed_at = if remote_id.is_some() {
        Some(match current.confirmed_at {
            Some(time) => time,
            None => u64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| "Cannot timestamp publication acknowledgment.")?
                    .as_millis(),
            )
            .map_err(|_| "Cannot timestamp publication acknowledgment.")?,
        })
    } else {
        None
    };
    let updated = JournalEntry {
        state,
        remote_id: remote_id.map(str::to_owned),
        confirmed_at,
        ..current
    };
    let json = serde_json::to_string(&updated)
        .map_err(|_| "Cannot encode publication status.".to_string())?;
    tx.execute(
        "UPDATE local_draft_publications SET entry_json = ?2 WHERE identity_key = ?1",
        params![updated.comment.identity_key(), json],
    )
    .map_err(|_| "Cannot persist publication status.".to_string())?;
    tx.commit()
        .map_err(|_| "Cannot commit publication status.".to_string())?;
    Ok(updated)
}
impl PublicationJournal for SqlitePublicationJournal {
    fn lookup(&self, key: &str) -> Result<Option<JournalEntry>, String> {
        lookup_at(&review_storage::publication_database()?, key)
    }
    fn begin(&self, comment: &ApprovedComment) -> Result<BeginPublication, String> {
        begin_at(&mut review_storage::publication_database()?, comment)
    }
    fn transition(
        &self,
        entry: &JournalEntry,
        state: PublicationState,
        remote_id: Option<&str>,
    ) -> Result<JournalEntry, String> {
        transition_at(
            &mut review_storage::publication_database()?,
            entry,
            state,
            remote_id,
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRevision {
    pub head_sha: String,
    pub base_sha: String,
}
impl RemoteRevision {
    fn matches(&self, comment: &ApprovedComment) -> bool {
        self.head_sha.eq_ignore_ascii_case(&comment.head_sha)
            && self.base_sha.eq_ignore_ascii_case(&comment.base_sha)
    }
}
pub enum CommentWrite {
    Created(String),
    /// Adapter guarantees that this request did not create a remote comment.
    Rejected,
    /// Timeout, disconnect, malformed acknowledgment, or any ambiguous outcome.
    Unknown,
}
pub trait ApprovedCommentApi {
    fn current_revision(&self, scope: &PublicationScope) -> Result<RemoteRevision, String>;
    fn create_once(&self, comment: &ApprovedComment) -> CommentWrite;
    /// Exact authenticated-author, body+marker, PR, parent and anchor match. An
    /// ambiguous/partial scan must return an error, never a success or absence.
    fn reconcile(&self, comment: &ApprovedComment) -> Result<Option<String>, String>;
}
pub struct ApprovedPublisher<J, A> {
    pub journal: J,
    pub api: A,
}
impl<J: PublicationJournal, A: ApprovedCommentApi> ApprovedPublisher<J, A> {
    /// Call only after explicit approval of this exact payload/snapshot. This
    /// method performs at most one POST for a newly reserved identity.
    pub fn publish_approved(&self, comment: &ApprovedComment) -> Result<JournalEntry, String> {
        comment.validate()?;
        let entry = match self.journal.begin(comment)? {
            BeginPublication::Existing(entry) => return Ok(entry),
            BeginPublication::Acquired(entry) => entry,
        };
        let revision = self.api.current_revision(&comment.scope);
        if !revision
            .as_ref()
            .is_ok_and(|revision| revision.matches(comment))
        {
            self.journal
                .transition(&entry, PublicationState::Rejected, None)?;
            return Err(
                "The current pull request revision could not be verified; no comment was sent."
                    .into(),
            );
        }
        match self.api.create_once(comment) {
            CommentWrite::Rejected => {
                self.journal
                    .transition(&entry, PublicationState::Rejected, None)
            }
            CommentWrite::Unknown => {
                self.journal
                    .transition(&entry, PublicationState::Uncertain, None)
            }
            CommentWrite::Created(id) => {
                if !identifier(&id) {
                    return self
                        .journal
                        .transition(&entry, PublicationState::Uncertain, None);
                }
                let state = self.confirmed_state(comment);
                self.journal.transition(&entry, state, Some(&id))
            }
        }
    }
    fn confirmed_state(&self, comment: &ApprovedComment) -> PublicationState {
        match self.api.current_revision(&comment.scope) {
            Ok(revision) if revision.matches(comment) => PublicationState::Published,
            Ok(_) => PublicationState::PublishedSnapshotChanged,
            Err(_) => PublicationState::PublishedUnverified,
        }
    }
    /// Read-only provider scan; never calls create_once or deletes a comment.
    pub fn reconcile(&self, key: &str) -> Result<JournalEntry, String> {
        let entry = self
            .journal
            .lookup(key)?
            .ok_or("Publication is not recorded.")?;
        if entry.state == PublicationState::PublishedUnverified {
            return self.journal.transition(
                &entry,
                self.confirmed_state(&entry.comment),
                entry.remote_id.as_deref(),
            );
        }
        if !matches!(
            entry.state,
            PublicationState::Writing | PublicationState::Uncertain
        ) {
            return Ok(entry);
        }
        match self.api.reconcile(&entry.comment) {
            Ok(Some(id)) if identifier(&id) => {
                let state = self.confirmed_state(&entry.comment);
                self.journal.transition(&entry, state, Some(&id))
            }
            _ => self
                .journal
                .transition(&entry, PublicationState::Uncertain, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        path::PathBuf,
    };

    #[derive(Clone)]
    struct FileJournal {
        path: PathBuf,
    }
    impl FileJournal {
        fn new() -> (tempfile::TempDir, Self) {
            let directory = tempfile::tempdir().unwrap();
            let journal = Self {
                path: directory.path().join("journal.sqlite3"),
            };
            let mut connection = journal.connection();
            review_storage::initialize_publication_test_database(&mut connection).unwrap();
            (directory, journal)
        }
        fn connection(&self) -> Connection {
            let connection = Connection::open(&self.path).unwrap();
            connection
                .busy_timeout(std::time::Duration::from_secs(2))
                .unwrap();
            connection
                .pragma_update(None, "synchronous", "FULL")
                .unwrap();
            connection
        }
    }
    impl PublicationJournal for FileJournal {
        fn lookup(&self, key: &str) -> Result<Option<JournalEntry>, String> {
            lookup_at(&self.connection(), key)
        }
        fn begin(&self, comment: &ApprovedComment) -> Result<BeginPublication, String> {
            begin_at(&mut self.connection(), comment)
        }
        fn transition(
            &self,
            entry: &JournalEntry,
            state: PublicationState,
            remote_id: Option<&str>,
        ) -> Result<JournalEntry, String> {
            transition_at(&mut self.connection(), entry, state, remote_id)
        }
    }
    fn comment(id: &str) -> ApprovedComment {
        ApprovedComment {
            scope: PublicationScope {
                provider: ReviewProvider::Github,
                workspace: "fixture".into(),
                repo: "demo".into(),
                pr_id: 9,
            },
            local_id: id.into(),
            head_sha: "a".repeat(40),
            base_sha: "b".repeat(40),
            mode: CommentMode::Inline,
            path: "src/time:stamp.rs".into(),
            to: None,
            from: Some(7),
            parent_id: None,
            raw: "Unicode è 界\r\noriginal body".into(),
            finding_ref: None,
            original_draft_hash: None,
        }
    }
    #[derive(Clone, Copy)]
    enum Behavior {
        Success,
        TimeoutAfterSuccess,
        Rejected,
        Unknown,
        ChangeHead,
        LoseRevision,
    }
    struct FakeApi {
        behavior: Cell<Behavior>,
        writes: Cell<usize>,
        reads: Cell<usize>,
        revision: RefCell<RemoteRevision>,
        unavailable: Cell<bool>,
        scan_error: Cell<bool>,
        remote: RefCell<HashMap<String, (ApprovedComment, String)>>,
        journal: RefCell<Option<FileJournal>>,
    }
    impl FakeApi {
        fn new(behavior: Behavior) -> Self {
            Self {
                behavior: Cell::new(behavior),
                writes: Cell::new(0),
                reads: Cell::new(0),
                revision: RefCell::new(RemoteRevision {
                    head_sha: "a".repeat(40),
                    base_sha: "b".repeat(40),
                }),
                unavailable: Cell::new(false),
                scan_error: Cell::new(false),
                remote: RefCell::new(HashMap::new()),
                journal: RefCell::new(None),
            }
        }
    }
    impl ApprovedCommentApi for FakeApi {
        fn current_revision(&self, _: &PublicationScope) -> Result<RemoteRevision, String> {
            self.reads.set(self.reads.get() + 1);
            if self.unavailable.get() {
                Err("synthetic offline".into())
            } else {
                Ok(self.revision.borrow().clone())
            }
        }
        fn create_once(&self, comment: &ApprovedComment) -> CommentWrite {
            if let Some(journal) = self.journal.borrow().as_ref() {
                let entry = journal.lookup(&comment.identity_key()).unwrap().unwrap();
                assert_eq!(entry.state, PublicationState::Writing);
                assert_eq!(&entry.comment, comment);
            }
            self.writes.set(self.writes.get() + 1);
            if matches!(self.behavior.get(), Behavior::Rejected) {
                return CommentWrite::Rejected;
            }
            if matches!(self.behavior.get(), Behavior::Unknown) {
                return CommentWrite::Unknown;
            }
            let id = format!("remote-{}", self.writes.get());
            self.remote
                .borrow_mut()
                .insert(comment.identity_key(), (comment.clone(), id.clone()));
            match self.behavior.get() {
                Behavior::TimeoutAfterSuccess => CommentWrite::Unknown,
                Behavior::ChangeHead => {
                    self.revision.borrow_mut().head_sha = "c".repeat(40);
                    CommentWrite::Created(id)
                }
                Behavior::LoseRevision => {
                    self.unavailable.set(true);
                    CommentWrite::Created(id)
                }
                _ => CommentWrite::Created(id),
            }
        }
        fn reconcile(&self, comment: &ApprovedComment) -> Result<Option<String>, String> {
            if self.scan_error.get() {
                return Err("synthetic incomplete scan".into());
            }
            Ok(self
                .remote
                .borrow()
                .get(&comment.identity_key())
                .filter(|(actual, _)| actual == comment)
                .map(|(_, id)| id.clone()))
        }
    }
    #[test]
    fn success_replay_and_scoped_partial_batch_do_not_duplicate() {
        let (_directory, journal) = FileJournal::new();
        let publisher = ApprovedPublisher {
            journal,
            api: FakeApi::new(Behavior::Success),
        };
        publisher
            .api
            .journal
            .replace(Some(publisher.journal.clone()));
        let draft = comment("one");
        let published = publisher.publish_approved(&draft).unwrap();
        assert_eq!(published.state, PublicationState::Published);
        assert_eq!(published.remote_id.as_deref(), Some("remote-1"));
        assert_eq!(publisher.publish_approved(&draft).unwrap(), published);
        assert_eq!(publisher.api.writes.get(), 1);
        publisher.api.behavior.set(Behavior::Rejected);
        assert_eq!(
            publisher.publish_approved(&comment("two")).unwrap().state,
            PublicationState::Rejected
        );
        assert_eq!(publisher.publish_approved(&draft).unwrap(), published);
        let mut different_provider = draft.clone();
        different_provider.scope.provider = ReviewProvider::Bitbucket;
        publisher.api.behavior.set(Behavior::Success);
        assert_eq!(
            publisher
                .publish_approved(&different_provider)
                .unwrap()
                .state,
            PublicationState::Published
        );
        assert_ne!(different_provider.identity_key(), draft.identity_key());
        let mut edited = draft.clone();
        edited.raw.push_str(" edited");
        assert!(publisher.publish_approved(&edited).is_err());
        assert_eq!(publisher.api.writes.get(), 3);
    }
    #[test]
    fn timeout_after_success_survives_reopen_and_requires_exact_read_only_reconciliation() {
        let (_directory, journal) = FileJournal::new();
        let publisher = ApprovedPublisher {
            journal: journal.clone(),
            api: FakeApi::new(Behavior::TimeoutAfterSuccess),
        };
        let draft = comment("timeout");
        assert_eq!(
            publisher.publish_approved(&draft).unwrap().state,
            PublicationState::Uncertain
        );
        assert_eq!(
            publisher.publish_approved(&draft).unwrap().state,
            PublicationState::Uncertain
        );
        let mut edited = draft.clone();
        edited.raw = "replacement".into();
        assert!(publisher.publish_approved(&edited).is_err());
        let mut alias = draft.clone();
        alias.scope.workspace = "FIXTURE".into();
        assert_eq!(alias.identity_key(), draft.identity_key());
        assert!(publisher.publish_approved(&alias).is_err());
        let reopened = ApprovedPublisher {
            journal,
            api: publisher.api,
        };
        assert_eq!(
            reopened
                .journal
                .lookup(&draft.identity_key())
                .unwrap()
                .unwrap()
                .comment
                .raw,
            draft.raw
        );
        reopened.api.scan_error.set(true);
        assert_eq!(
            reopened.reconcile(&draft.identity_key()).unwrap().state,
            PublicationState::Uncertain
        );
        reopened.api.scan_error.set(false);
        reopened
            .api
            .remote
            .borrow_mut()
            .get_mut(&draft.identity_key())
            .unwrap()
            .0
            .raw
            .push_str(" altered remotely");
        assert_eq!(
            reopened.reconcile(&draft.identity_key()).unwrap().state,
            PublicationState::Uncertain
        );
        reopened
            .api
            .remote
            .borrow_mut()
            .get_mut(&draft.identity_key())
            .unwrap()
            .0 = draft.clone();
        let published = reopened.reconcile(&draft.identity_key()).unwrap();
        assert_eq!(published.state, PublicationState::Published);
        assert_eq!(published.remote_id.as_deref(), Some("remote-1"));
        assert_eq!(reopened.api.writes.get(), 1);
        assert_eq!(reopened.publish_approved(&draft).unwrap(), published);
    }
    #[test]
    fn missing_remote_and_crash_reservations_never_expire_into_post_retries() {
        let (_directory, journal) = FileJournal::new();
        let draft = comment("crash-before-or-after-post");
        assert!(matches!(
            journal.begin(&draft).unwrap(),
            BeginPublication::Acquired(_)
        ));
        let publisher = ApprovedPublisher {
            journal,
            api: FakeApi::new(Behavior::Success),
        };
        assert_eq!(
            publisher.publish_approved(&draft).unwrap().state,
            PublicationState::Writing
        );
        assert_eq!(
            publisher.reconcile(&draft.identity_key()).unwrap().state,
            PublicationState::Uncertain
        );
        assert_eq!(
            publisher.reconcile(&draft.identity_key()).unwrap().state,
            PublicationState::Uncertain
        );
        assert_eq!(
            publisher.publish_approved(&draft).unwrap().state,
            PublicationState::Uncertain
        );
        assert_eq!(publisher.api.writes.get(), 0);
        let entry = publisher
            .journal
            .lookup(&draft.identity_key())
            .unwrap()
            .unwrap();
        assert!(publisher
            .journal
            .transition(&entry, PublicationState::Rejected, None)
            .is_err());
    }
    #[test]
    fn stale_preflight_never_sends_and_postflight_preserves_remote_identity() {
        let (_directory, journal) = FileJournal::new();
        let publisher = ApprovedPublisher {
            journal,
            api: FakeApi::new(Behavior::ChangeHead),
        };
        publisher.api.revision.borrow_mut().head_sha = "c".repeat(40);
        assert!(publisher.publish_approved(&comment("stale")).is_err());
        assert_eq!(publisher.api.writes.get(), 0);
        publisher.api.revision.borrow_mut().head_sha = "a".repeat(40);
        let changed = publisher
            .publish_approved(&comment("changed-after-post"))
            .unwrap();
        assert_eq!(changed.state, PublicationState::PublishedSnapshotChanged);
        assert_eq!(changed.remote_id.as_deref(), Some("remote-1"));
        publisher.api.revision.borrow_mut().head_sha = "a".repeat(40);
        publisher.api.behavior.set(Behavior::LoseRevision);
        let draft = comment("offline-after-ack");
        let unverified = publisher.publish_approved(&draft).unwrap();
        assert_eq!(unverified.state, PublicationState::PublishedUnverified);
        assert_eq!(unverified.remote_id.as_deref(), Some("remote-2"));
        assert_eq!(
            publisher.reconcile(&draft.identity_key()).unwrap(),
            unverified
        );
        publisher.api.unavailable.set(false);
        assert_eq!(
            publisher.reconcile(&draft.identity_key()).unwrap().state,
            PublicationState::Published
        );
        assert_eq!(publisher.api.writes.get(), 2);
    }
    #[test]
    fn rejected_attempts_require_new_approval_and_old_acknowledgments_are_fenced() {
        let (_directory, journal) = FileJournal::new();
        let publisher = ApprovedPublisher {
            journal,
            api: FakeApi::new(Behavior::Rejected),
        };
        let mut draft = comment("retry");
        let rejected = publisher.publish_approved(&draft).unwrap();
        draft.raw.push_str(" corrected");
        publisher.api.behavior.set(Behavior::Success);
        let published = publisher.publish_approved(&draft).unwrap();
        assert_eq!(published.state, PublicationState::Published);
        assert_ne!(published.attempt, rejected.attempt);
        assert!(publisher
            .journal
            .transition(&rejected, PublicationState::Published, Some("late-old-ack"))
            .is_err());
        assert_eq!(
            publisher
                .journal
                .lookup(&draft.identity_key())
                .unwrap()
                .unwrap(),
            published
        );
    }
    #[test]
    fn simultaneous_reservations_have_one_winner_and_corruption_fails_closed() {
        let (_directory, journal) = FileJournal::new();
        let draft = comment("concurrent");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads = (0..2)
            .map(|_| {
                let journal = journal.clone();
                let draft = draft.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    journal.begin(&draft).unwrap()
                })
            })
            .collect::<Vec<_>>();
        let outcomes = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, BeginPublication::Acquired(_)))
                .count(),
            1
        );
        journal
            .connection()
            .execute(
                "UPDATE local_draft_publications SET entry_json = 'corrupt'",
                [],
            )
            .unwrap();
        let publisher = ApprovedPublisher {
            journal,
            api: FakeApi::new(Behavior::Success),
        };
        assert!(publisher.publish_approved(&draft).is_err());
        assert_eq!(publisher.api.writes.get(), 0);
    }
    struct FailTransition {
        journal: FileJournal,
    }
    impl PublicationJournal for FailTransition {
        fn lookup(&self, key: &str) -> Result<Option<JournalEntry>, String> {
            self.journal.lookup(key)
        }
        fn begin(&self, draft: &ApprovedComment) -> Result<BeginPublication, String> {
            self.journal.begin(draft)
        }
        fn transition(
            &self,
            _: &JournalEntry,
            _: PublicationState,
            _: Option<&str>,
        ) -> Result<JournalEntry, String> {
            Err("synthetic disk full".into())
        }
    }
    #[test]
    fn failed_acknowledgment_storage_leaves_a_durable_reconcilable_reservation() {
        let (_directory, journal) = FileJournal::new();
        let publisher = ApprovedPublisher {
            journal: FailTransition {
                journal: journal.clone(),
            },
            api: FakeApi::new(Behavior::Success),
        };
        let draft = comment("disk-full-after-success");
        assert!(publisher.publish_approved(&draft).is_err());
        assert_eq!(
            journal
                .lookup(&draft.identity_key())
                .unwrap()
                .unwrap()
                .state,
            PublicationState::Writing
        );
        let recovered = ApprovedPublisher {
            journal,
            api: publisher.api,
        };
        assert_eq!(
            recovered.publish_approved(&draft).unwrap().state,
            PublicationState::Writing
        );
        assert_eq!(
            recovered.reconcile(&draft.identity_key()).unwrap().state,
            PublicationState::Published
        );
        assert_eq!(recovered.api.writes.get(), 1);
    }
    #[test]
    fn invalid_payloads_never_touch_provider_and_unknown_writes_remain_uncertain() {
        let (_directory, journal) = FileJournal::new();
        let publisher = ApprovedPublisher {
            journal,
            api: FakeApi::new(Behavior::Unknown),
        };
        let original = comment("validate");
        for invalid in [
            "../escape",
            "/absolute",
            "C:/drive",
            "src\\escape",
            "src//empty",
        ] {
            let mut draft = original.clone();
            draft.path = invalid.into();
            assert!(publisher.publish_approved(&draft).is_err());
        }
        let mut draft = original.clone();
        draft.to = Some(7);
        assert!(publisher.publish_approved(&draft).is_err());
        draft = original.clone();
        draft.raw = "<!-- norn-draft:forged -->".into();
        assert!(publisher.publish_approved(&draft).is_err());
        draft.raw = "x".repeat(MAX_BODY);
        assert!(publisher.publish_approved(&draft).is_err());
        assert_eq!(publisher.api.reads.get(), 0);
        assert_eq!(publisher.api.writes.get(), 0);
        assert_eq!(
            publisher.publish_approved(&original).unwrap().state,
            PublicationState::Uncertain
        );
        assert_eq!(
            publisher.publish_approved(&original).unwrap().state,
            PublicationState::Uncertain
        );
        assert_eq!(publisher.api.writes.get(), 1);
        let mut general = comment("general");
        general.mode = CommentMode::General;
        general.path.clear();
        general.from = None;
        assert!(general.validate().is_ok());
        general.mode = CommentMode::File;
        assert!(general.validate().is_err());
        general.path = "src/time:stamp.rs".into();
        assert!(general.validate().is_ok());
    }
    #[test]
    fn failed_reservation_storage_prevents_provider_access() {
        let (_directory, journal) = FileJournal::new();
        journal
            .connection()
            .execute("DROP TABLE local_draft_publications", [])
            .unwrap();
        let publisher = ApprovedPublisher {
            journal,
            api: FakeApi::new(Behavior::Success),
        };
        assert!(publisher
            .publish_approved(&comment("missing-journal"))
            .is_err());
        assert_eq!(publisher.api.writes.get(), 0);
        assert_eq!(publisher.api.reads.get(), 0);
    }
    #[test]
    fn legacy_journal_hashes_survive_widened_storage_and_quoted_markdown() {
        let (_directory, journal) = FileJournal::new();
        let original = comment("legacy");
        let entry = JournalEntry {
            payload_hash: original.payload_hash().unwrap(),
            comment: original,
            attempt: "old-attempt".into(),
            state: PublicationState::Uncertain,
            remote_id: None,
            started_at: None,
            confirmed_at: None,
        };
        let raw = serde_json::to_string(&entry).unwrap();
        assert!(!raw.contains("findingRef"));
        assert!(!raw.contains("originalDraftHash"));
        assert!(!raw.contains("startedAt"));
        let mut connection = journal.connection();
        connection.execute_batch("DROP TABLE local_draft_publications; CREATE TABLE local_draft_publications(identity_key TEXT PRIMARY KEY CHECK(length(identity_key)=64), entry_json TEXT NOT NULL CHECK(length(entry_json)<=49152)); DELETE FROM schema_migrations WHERE version=17;").unwrap();
        connection
            .execute(
                "INSERT INTO local_draft_publications(identity_key,entry_json) VALUES (?1,?2)",
                params![entry.comment.identity_key(), raw],
            )
            .unwrap();
        review_storage::initialize_publication_test_database(&mut connection).unwrap();
        assert_eq!(
            journal
                .lookup(&entry.comment.identity_key())
                .unwrap()
                .unwrap(),
            entry
        );
        let mut quoted = comment("quoted");
        quoted.raw = "\"".repeat(30_000);
        assert!(matches!(
            journal.begin(&quoted).unwrap(),
            BeginPublication::Acquired(_)
        ));
        assert_eq!(
            journal
                .lookup(&quoted.identity_key())
                .unwrap()
                .unwrap()
                .comment
                .raw,
            quoted.raw
        );
    }
}
