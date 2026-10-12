//! Retained human PR approval: native identity, explicit consent, uncertain-write recovery.
use crate::services::bitbucket::workspace_prs::{DetailParams, RepositoryIdentity};
use crate::services::review_scope::Scope;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) fn invalid() -> String {
    "The provider approval could not be verified. Refresh its preview or inspect provider status."
        .into()
}
fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Actor {
    pub account_id: String,
    pub display_name: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Proof {
    pub request: DetailParams,
    pub repository_id: String,
    pub actor: Actor,
    pub title: String,
    pub head_sha: String,
    pub destination_sha: String,
    pub source_branch: String,
    pub destination_branch: String,
    pub source_repository: Option<RepositoryIdentity>,
    pub state: String,
    pub draft: bool,
    pub already_approved: bool,
}
impl Proof {
    pub fn validate(&self) -> Result<(), String> {
        self.request.validate()?;
        if !identifier(&self.repository_id)
            || !identifier(&self.actor.account_id)
            || self.actor.display_name.is_empty()
            || self.actor.display_name.len() > 256
            || self.actor.display_name.chars().any(char::is_control)
            || self.title.len() > 4096
            || self.title.chars().any(char::is_control)
            || !identifier(&self.source_branch)
            || !identifier(&self.destination_branch)
            || !matches!(
                self.state.as_str(),
                "OPEN" | "MERGED" | "DECLINED" | "SUPERSEDED"
            )
            || self.head_sha.len() != self.destination_sha.len()
        {
            return Err(invalid());
        }
        for sha in [&self.head_sha, &self.destination_sha] {
            if crate::services::provider_snapshot::full_sha(sha)? != *sha {
                return Err(invalid());
            }
        }
        if let Some(source) = &self.source_repository {
            source.validate()?;
            if source.provider != self.request.repository.provider {
                return Err(invalid());
            }
        }
        Ok(())
    }
    pub fn scope(&self) -> Scope {
        Scope {
            provider: match self.request.repository.provider {
                crate::config::ReviewProvider::Github => {
                    crate::services::review::ReviewProvider::Github
                }
                crate::config::ReviewProvider::Bitbucket => {
                    crate::services::review::ReviewProvider::Bitbucket
                }
            },
            workspace: self.request.repository.workspace.clone(),
            repo: self.request.repository.repo.clone(),
            pr_id: self.request.pr_id,
        }
    }
    pub fn identity(&self) -> String {
        // Native immutable IDs prevent repository spelling/title/cache-handle
        // changes from bypassing an unresolved POST for the same actual actor.
        let bytes = serde_json::to_vec(&(
            "norn-pr-approval-v1",
            self.request.repository.provider,
            &self.repository_id,
            self.request.pr_id,
            &self.actor.account_id,
            &self.head_sha,
            &self.destination_sha,
        ))
        .expect("serializable approval identity");
        format!("{:x}", Sha256::digest(bytes))
    }
    pub fn same_preview(&self, other: &Self) -> bool {
        let mut current = other.clone();
        current.already_approved = self.already_approved;
        *self == current
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Writing,
    Approved,
    Rejected,
    Uncertain,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub format_version: u32,
    pub proof: Proof,
    pub state: State,
    pub remote_review_id: Option<String>,
    pub updated_at: u64,
    pub attempt: u32,
}
impl Entry {
    pub fn validate(&self) -> Result<(), String> {
        self.proof.validate()?;
        if self.format_version != 1
            || self.attempt == 0
            || self
                .remote_review_id
                .as_deref()
                .is_some_and(|id| !identifier(id))
        {
            return Err(invalid());
        }
        Ok(())
    }
}

use super::bitbucket::workspace_prs::{Api, WriteOutcome};
use crate::review_storage::provider_approval as journal;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const TTL: Duration = Duration::from_secs(300);
const MAX_TOKENS: usize = 32;
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmParams {
    pub request: DetailParams,
    pub approval_id: String,
}
impl ConfirmParams {
    pub fn validate(&self) -> Result<(), String> {
        self.request.validate()?;
        if self.approval_id.len() != 64
            || !self
                .approval_id
                .bytes()
                .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub request: DetailParams,
    pub proof: Proof,
    pub approval_id: Option<String>,
    pub expires_at: Option<u64>,
    pub entry: Option<Entry>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultData {
    pub request: DetailParams,
    pub proof: Proof,
    pub entry: Option<Entry>,
    pub observed_approved: bool,
    pub outdated: bool,
    pub local_saved: bool,
}
pub(crate) trait ApprovalApi: Send + Sync {
    fn inspect(&self, request: &DetailParams, started: Instant) -> Result<Proof, String>;
    fn post(&self, proof: &Proof, started: Instant) -> WriteOutcome;
    fn approved(&self, proof: &Proof, current: &Proof, started: Instant) -> Result<bool, String>;
}
impl ApprovalApi for Api {
    fn inspect(&self, request: &DetailParams, started: Instant) -> Result<Proof, String> {
        self.approval_proof(request, started)
    }
    fn post(&self, proof: &Proof, started: Instant) -> WriteOutcome {
        self.post_approval(proof, started)
    }
    fn approved(&self, proof: &Proof, current: &Proof, started: Instant) -> Result<bool, String> {
        self.approval_observed(proof, current, started)
    }
}
type Factory = dyn Fn(RepositoryIdentity) -> Result<Arc<dyn ApprovalApi>, String> + Send + Sync;
struct Token {
    proof: Proof,
    issued: Instant,
}
pub(crate) struct Manager {
    tokens: Mutex<HashMap<String, Token>>,
    factory: Arc<Factory>,
    shutdown: Arc<AtomicBool>,
}
impl Manager {
    pub fn new(shutdown: Arc<AtomicBool>) -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            factory: Arc::new(|repo| Ok(Arc::new(Api::from_stored(repo)?))),
            shutdown,
        }
    }
    fn check_running(&self) -> Result<(), String> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(
                "Approval stopped during shutdown. Check provider status before sending again."
                    .into(),
            );
        }
        Ok(())
    }
    fn api(&self, request: &DetailParams) -> Result<Arc<dyn ApprovalApi>, String> {
        request.validate()?;
        self.check_running()?;
        (self.factory)(request.repository.clone())
    }
    fn inspect(
        &self,
        api: &dyn ApprovalApi,
        request: &DetailParams,
        started: Instant,
    ) -> Result<Proof, String> {
        let proof = api.inspect(request, started)?;
        proof.validate()?;
        if proof.request != *request {
            return Err(invalid());
        }
        self.check_running()?;
        Ok(proof)
    }
    pub fn preview(&self, request: DetailParams) -> Result<Preview, String> {
        let started = Instant::now();
        let api = self.api(&request)?;
        let proof = self.inspect(api.as_ref(), &request, started)?;
        let entry = journal::pending(&proof)?.or(journal::load(&proof)?);
        let mut approval_id = None;
        let mut expires_at = None;
        if proof.state == "OPEN"
            && !proof.draft
            && !proof.already_approved
            && entry.as_ref().is_none_or(|v| v.state == State::Rejected)
        {
            let mut tokens = self.tokens.lock().map_err(|_| invalid())?;
            tokens.retain(|_, t| t.issued.elapsed() < TTL);
            if tokens.len() >= MAX_TOKENS {
                return Err("Too many approval previews; wait for their expiry.".into());
            }
            let mut nonce = [0u8; 32];
            getrandom::getrandom(&mut nonce).map_err(|_| invalid())?;
            let id = nonce.iter().map(|v| format!("{v:02x}")).collect::<String>();
            tokens.insert(
                id.clone(),
                Token {
                    proof: proof.clone(),
                    issued: Instant::now(),
                },
            );
            approval_id = Some(id);
            expires_at = Some(now() + TTL.as_secs());
        }
        Ok(Preview {
            request,
            proof,
            approval_id,
            expires_at,
            entry,
        })
    }
    pub fn confirm(&self, params: ConfirmParams) -> Result<ResultData, String> {
        params.validate()?;
        // Consume before factory/credentials/network; even a mismatched scope burns the token.
        let token = self
            .tokens
            .lock()
            .map_err(|_| invalid())?
            .remove(&params.approval_id)
            .ok_or_else(|| {
                "Approval preview expired or was consumed. Refresh before confirming.".to_string()
            })?;
        if token.issued.elapsed() >= TTL || token.proof.request != params.request {
            return Err(invalid());
        }
        let started = Instant::now();
        let api = self.api(&params.request)?;
        let proof = self.inspect(api.as_ref(), &params.request, started)?;
        if !token.proof.same_preview(&proof) || proof.state != "OPEN" || proof.draft {
            return Err("PR or authenticated reviewer changed. Refresh the approval preview; no approval was sent.".into());
        }
        if proof.already_approved {
            return Ok(ResultData {
                request: params.request,
                proof,
                entry: None,
                observed_approved: true,
                outdated: false,
                local_saved: true,
            });
        }
        self.check_running()?;
        let claimed = journal::claim(&proof, now())?;
        let outcome = if self.shutdown.load(Ordering::Acquire) {
            WriteOutcome::Rejected
        } else {
            api.post(&proof, started)
        };
        let (state, remote) = match outcome {
            WriteOutcome::Approved(id) => (State::Approved, id),
            WriteOutcome::Rejected => (State::Rejected, None),
            WriteOutcome::Uncertain => (State::Uncertain, None),
        };
        let finished = journal::finish(&claimed, state, remote, now());
        let (entry, local_saved) = match finished {
            Ok(entry) => (entry, true),
            Err(_) => (claimed, false),
        };
        Ok(ResultData {
            request: params.request,
            proof,
            entry: Some(entry),
            observed_approved: state == State::Approved,
            outdated: false,
            local_saved,
        })
    }
    pub fn status(&self, request: DetailParams, reconcile: bool) -> Result<ResultData, String> {
        let started = Instant::now();
        let api = self.api(&request)?;
        let proof = self.inspect(api.as_ref(), &request, started)?;
        let mut entry = journal::pending(&proof)?.or(journal::load(&proof)?);
        let outdated = entry.as_ref().is_some_and(|e| {
            e.proof.head_sha != proof.head_sha || e.proof.destination_sha != proof.destination_sha
        });
        let mut observed = proof.already_approved;
        let mut local_saved = true;
        if let Some(saved) = &entry {
            if matches!(saved.state, State::Writing | State::Uncertain) && reconcile {
                observed = api.approved(&saved.proof, &proof, started)?;
                if observed {
                    match journal::finish(saved, State::Approved, None, now()) {
                        Ok(v) => entry = Some(v),
                        Err(_) => local_saved = false,
                    }
                }
            }
        }
        Ok(ResultData {
            request,
            proof,
            entry,
            observed_approved: observed,
            outdated,
            local_saved,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct OwnedStorage {
        _directory: tempfile::TempDir,
        previous: Option<std::ffi::OsString>,
    }
    impl OwnedStorage {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let previous = std::env::var_os("NORN_REVIEW_DATA_DIR");
            std::env::set_var("NORN_REVIEW_DATA_DIR", directory.path());
            Self {
                _directory: directory,
                previous,
            }
        }
    }
    impl Drop for OwnedStorage {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var("NORN_REVIEW_DATA_DIR", v),
                None => std::env::remove_var("NORN_REVIEW_DATA_DIR"),
            }
        }
    }
    pub(super) fn proof(provider: crate::config::ReviewProvider) -> Proof {
        Proof {
            request: DetailParams {
                repository: RepositoryIdentity {
                    provider,
                    workspace: "owned".into(),
                    repo: "review".into(),
                },
                pr_id: 7,
            },
            repository_id: "immutable-repo-1".into(),
            actor: Actor {
                account_id: "stable-actor-1".into(),
                display_name: "Owned reviewer".into(),
            },
            title: "Owned change".into(),
            head_sha: "a".repeat(40),
            destination_sha: "b".repeat(40),
            source_branch: "feature".into(),
            destination_branch: "main".into(),
            source_repository: None,
            state: "OPEN".into(),
            draft: false,
            already_approved: false,
        }
    }
    struct FakeApi {
        proof: Mutex<Proof>,
        outcome: WriteOutcome,
        posts: AtomicUsize,
    }
    impl ApprovalApi for FakeApi {
        fn inspect(&self, request: &DetailParams, _: Instant) -> Result<Proof, String> {
            let proof = self.proof.lock().unwrap().clone();
            if proof.request != *request {
                return Err(invalid());
            }
            Ok(proof)
        }
        fn post(&self, proof: &Proof, _: Instant) -> WriteOutcome {
            assert_eq!(journal::load(proof).unwrap().unwrap().state, State::Writing);
            self.posts.fetch_add(1, Ordering::SeqCst);
            self.outcome.clone()
        }
        fn approved(&self, proof: &Proof, current: &Proof, _: Instant) -> Result<bool, String> {
            Ok(proof.same_preview(current) && current.already_approved)
        }
    }
    fn manager(api: Arc<FakeApi>, factories: Arc<AtomicUsize>) -> Manager {
        Manager {
            tokens: Mutex::new(HashMap::new()),
            shutdown: Arc::new(AtomicBool::new(false)),
            factory: Arc::new(move |_| {
                factories.fetch_add(1, Ordering::SeqCst);
                Ok(api.clone())
            }),
        }
    }
    fn fake(proof: Proof, outcome: WriteOutcome) -> Arc<FakeApi> {
        Arc::new(FakeApi {
            proof: Mutex::new(proof),
            outcome,
            posts: AtomicUsize::new(0),
        })
    }
    fn confirmed(preview: &Preview) -> ConfirmParams {
        ConfirmParams {
            request: preview.request.clone(),
            approval_id: preview.approval_id.clone().unwrap(),
        }
    }
    #[test]
    fn token_consumption_scope_replay_and_expiry_precede_credentials() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let _owned = OwnedStorage::new();
        let p = proof(crate::config::ReviewProvider::Github);
        let api = fake(p.clone(), WriteOutcome::Approved(Some("55".into())));
        let factories = Arc::new(AtomicUsize::new(0));
        let manager = manager(api.clone(), factories.clone());
        let preview = manager.preview(p.request.clone()).unwrap();
        assert!(crate::services::review_scope::load_owned(&p.scope())
            .unwrap()
            .is_none());
        let mut params = confirmed(&preview);
        params.request.repository.provider = crate::config::ReviewProvider::Bitbucket;
        assert!(manager.confirm(params).is_err());
        assert_eq!(factories.load(Ordering::SeqCst), 1);
        assert!(manager.confirm(confirmed(&preview)).is_err());
        let expired = manager.preview(p.request.clone()).unwrap();
        manager
            .tokens
            .lock()
            .unwrap()
            .get_mut(expired.approval_id.as_ref().unwrap())
            .unwrap()
            .issued = Instant::now() - Duration::from_secs(301);
        assert!(manager.confirm(confirmed(&expired)).is_err());
        assert_eq!(factories.load(Ordering::SeqCst), 2);
        assert_eq!(api.posts.load(Ordering::SeqCst), 0);
        let valid = manager.preview(p.request.clone()).unwrap();
        let result = manager.confirm(confirmed(&valid)).unwrap();
        assert_eq!(result.entry.unwrap().state, State::Approved);
        assert!(result.observed_approved);
        assert!(manager.confirm(confirmed(&valid)).is_err());
        assert_eq!(api.posts.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn metadata_actor_fork_state_and_pin_drift_never_post() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let _owned = OwnedStorage::new();
        for field in 0..8 {
            let p = proof(crate::config::ReviewProvider::Github);
            let api = fake(p.clone(), WriteOutcome::Approved(None));
            let manager = manager(api.clone(), Arc::new(AtomicUsize::new(0)));
            let preview = manager.preview(p.request.clone()).unwrap();
            let mut drift = p.clone();
            match field {
                0 => drift.head_sha = "c".repeat(40),
                1 => drift.destination_sha = "c".repeat(40),
                2 => drift.actor.account_id = "different-actor".into(),
                3 => drift.repository_id = "different-repo".into(),
                4 => drift.title = "Changed title".into(),
                5 => drift.draft = true,
                6 => drift.state = "MERGED".into(),
                _ => {
                    drift.source_repository = Some(RepositoryIdentity {
                        provider: crate::config::ReviewProvider::Github,
                        workspace: "fork".into(),
                        repo: "review".into(),
                    })
                }
            };
            *api.proof.lock().unwrap() = drift;
            assert!(manager.confirm(confirmed(&preview)).is_err());
            assert_eq!(api.posts.load(Ordering::SeqCst), 0);
            assert!(journal::load(&p).unwrap().is_none());
        }
    }
    #[test]
    fn uncertain_send_survives_restart_and_new_heads_block_post_until_get_reconciliation() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let _owned = OwnedStorage::new();
        for provider in [
            crate::config::ReviewProvider::Github,
            crate::config::ReviewProvider::Bitbucket,
        ] {
            let p = proof(provider);
            let api = fake(p.clone(), WriteOutcome::Uncertain);
            let manager1 = manager(api.clone(), Arc::new(AtomicUsize::new(0)));
            let preview = manager1.preview(p.request.clone()).unwrap();
            let result = manager1.confirm(confirmed(&preview)).unwrap();
            assert_eq!(result.entry.unwrap().state, State::Uncertain);
            drop(manager1);
            let restarted = manager(api.clone(), Arc::new(AtomicUsize::new(0)));
            let again = restarted.preview(p.request.clone()).unwrap();
            assert!(again.approval_id.is_none());
            let mut changed = p.clone();
            changed.head_sha = "c".repeat(40);
            *api.proof.lock().unwrap() = changed;
            assert!(restarted
                .preview(p.request.clone())
                .unwrap()
                .approval_id
                .is_none());
            let status = restarted.status(p.request.clone(), true).unwrap();
            assert!(status.outdated);
            assert!(!status.observed_approved);
            assert_eq!(status.entry.unwrap().state, State::Uncertain);
            let mut approved = p.clone();
            approved.already_approved = true;
            *api.proof.lock().unwrap() = approved;
            let status = restarted.status(p.request.clone(), true).unwrap();
            assert_eq!(status.entry.unwrap().state, State::Approved);
            assert!(status.observed_approved);
            assert_eq!(api.posts.load(Ordering::SeqCst), 1);
        }
    }
    #[test]
    fn writing_claim_blocks_alias_replay_rejection_allows_new_explicit_attempt_and_delete_cascades()
    {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let _owned = OwnedStorage::new();
        let p = proof(crate::config::ReviewProvider::Github);
        let claimed = journal::claim(&p, 1).unwrap();
        assert!(journal::claim(&p, 2).is_err());
        let mut alias = p.clone();
        alias.request.repository.workspace = "Owned".into();
        assert_eq!(p.identity(), alias.identity());
        assert!(journal::claim(&alias, 2).is_err());
        assert!(journal::pending(&alias).is_err());
        let rejected = journal::finish(&claimed, State::Rejected, None, 2).unwrap();
        assert_eq!(rejected.attempt, 1);
        let next = journal::claim(&p, 3).unwrap();
        assert_eq!(next.attempt, 2);
        assert!(journal::finish(&claimed, State::Approved, None, 4).is_err());
        let connection = crate::review_storage::publication_database().unwrap();
        connection
            .execute(
                "DELETE FROM ai_review_stores WHERE tenant_id='local' AND review_key=?1",
                [p.scope().key()],
            )
            .unwrap();
        assert!(journal::load(&p).unwrap().is_none());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM provider_pr_approvals", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn journal_quota_corruption_and_failed_receipt_never_erase_unresolved_attempt() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let _owned = OwnedStorage::new();
        let p = proof(crate::config::ReviewProvider::Github);
        let claimed = journal::claim(&p, 1).unwrap();
        let connection = crate::review_storage::publication_database().unwrap();
        connection.execute_batch("CREATE TRIGGER owned_receipt_failure BEFORE UPDATE ON provider_pr_approvals BEGIN SELECT RAISE(ABORT,'owned fixture'); END;").unwrap();
        assert!(journal::finish(&claimed, State::Approved, Some("55".into()), 2).is_err());
        assert_eq!(journal::load(&p).unwrap().unwrap().state, State::Writing);
        assert!(journal::claim(&p, 3).is_err());
        let backup = _owned
            ._directory
            .path()
            .join("owned-approval-backup.sqlite3");
        crate::review_storage::create_consistent_database_backup(&backup).unwrap();
        crate::review_storage::verify_database_backup(&backup).unwrap();
        let recovered = rusqlite::Connection::open_with_flags(
            &backup,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let receipt: String = recovered
            .query_row(
                "SELECT entry_json FROM provider_pr_approvals WHERE identity_key=?1",
                [p.identity()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(serde_json::from_str::<Entry>(&receipt).unwrap(), claimed);
        connection
            .execute_batch("DROP TRIGGER owned_receipt_failure;")
            .unwrap();
        journal::finish(&claimed, State::Rejected, None, 2).unwrap();
        let mut corrupt = claimed.clone();
        corrupt.proof.request.repository.provider = crate::config::ReviewProvider::Bitbucket;
        connection
            .execute(
                "UPDATE provider_pr_approvals SET entry_json=?1 WHERE identity_key=?2",
                rusqlite::params![serde_json::to_string(&corrupt).unwrap(), p.identity()],
            )
            .unwrap();
        assert!(journal::load(&p).is_err());
        assert!(journal::claim(&p, 3).is_err());
        connection
            .execute("DELETE FROM provider_pr_approvals", [])
            .unwrap();
        let tx = connection.unchecked_transaction().unwrap();
        for n in 0..1000 {
            let mut entry = claimed.clone();
            entry.proof.head_sha = format!("{n:040x}");
            entry.state = State::Approved;
            tx.execute("INSERT INTO provider_pr_approvals(identity_key,scope_key,provider,repository_id,actor_id,pr_id,state,entry_json) VALUES (?1,?2,'github',?3,?4,7,'approved',?5)",rusqlite::params![entry.proof.identity(),p.scope().key(),p.repository_id,p.actor.account_id,serde_json::to_string(&entry).unwrap()]).unwrap();
        }
        tx.commit().unwrap();
        assert!(journal::claim(&p, 4).unwrap_err().contains("full"));
    }
    #[test]
    fn already_approved_draft_shutdown_and_bounded_previews_do_not_post() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let _owned = OwnedStorage::new();
        let mut p = proof(crate::config::ReviewProvider::Github);
        p.already_approved = true;
        let api = fake(p.clone(), WriteOutcome::Approved(None));
        let manager = manager(api.clone(), Arc::new(AtomicUsize::new(0)));
        assert!(manager
            .preview(p.request.clone())
            .unwrap()
            .approval_id
            .is_none());
        p.already_approved = false;
        p.draft = true;
        *api.proof.lock().unwrap() = p.clone();
        assert!(manager
            .preview(p.request.clone())
            .unwrap()
            .approval_id
            .is_none());
        p.draft = false;
        *api.proof.lock().unwrap() = p.clone();
        for _ in 0..MAX_TOKENS {
            manager.preview(p.request.clone()).unwrap();
        }
        assert!(manager.preview(p.request.clone()).is_err());
        manager.shutdown.store(true, Ordering::Release);
        assert!(manager.status(p.request, false).is_err());
        assert_eq!(api.posts.load(Ordering::SeqCst), 0);
    }
}
