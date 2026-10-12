//! Native review ownership. Legacy rows remain separate migration input.
use super::review::{AiReviewStoreData, ReviewProvider, ReviewRun};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub provider: ReviewProvider,
    pub workspace: String,
    pub repo: String,
    pub pr_id: u32,
}

impl Scope {
    pub fn key(&self) -> String {
        let provider = match self.provider {
            ReviewProvider::Github => "github",
            ReviewProvider::Bitbucket => "bitbucket",
        };
        format!(
            "provider:{provider}:{}:{}:{}:{}:{}",
            self.workspace.len(),
            self.workspace,
            self.repo.len(),
            self.repo,
            self.pr_id
        )
    }

    fn owns(&self, run: &ReviewRun) -> bool {
        run.provider == self.provider
            && run.workspace == self.workspace
            && run.repo == self.repo
            && run.pr_id == self.pr_id
    }

    pub fn validate_owned(&self, store: &AiReviewStoreData) -> Result<(), String> {
        if store.review_runs.iter().any(|run| !self.owns(run)) {
            return Err("Saved native review contains a different provider or PR.".into());
        }
        unique_ids(store)?;
        Ok(())
    }

    /// Project only attributable legacy conversations. A shared thread ID with
    /// foreign provenance is ambiguous and cannot supply agent resume context.
    pub fn project_legacy(&self, mut store: AiReviewStoreData) -> AiReviewStoreData {
        let foreign_threads: HashSet<_> = store
            .review_runs
            .iter()
            .filter(|run| !self.owns(run))
            .filter_map(|run| run.thread_id.clone())
            .collect();
        store.review_runs.retain(|run| self.owns(run));
        let proved_threads: HashSet<_> = store
            .review_runs
            .iter()
            .filter_map(|run| run.thread_id.as_deref())
            .filter(|id| !foreign_threads.contains(*id))
            .collect();
        store
            .threads
            .retain(|thread| proved_threads.contains(thread.id.as_str()));
        if store
            .active_thread_id
            .as_ref()
            .is_some_and(|id| !store.threads.iter().any(|thread| thread.id == *id))
        {
            store.active_thread_id = None;
        }
        store
    }

    pub fn merge(
        &self,
        owned: Option<AiReviewStoreData>,
        legacy: Option<AiReviewStoreData>,
    ) -> Result<Option<AiReviewStoreData>, String> {
        if owned.is_none() && legacy.is_none() {
            return Ok(None);
        }
        let mut result = legacy
            .map(|store| self.project_legacy(store))
            .unwrap_or_default();
        unique_ids(&result)?;
        if let Some(owned) = owned {
            self.validate_owned(&owned)?;
            if owned
                .review_runs
                .iter()
                .any(|run| result.review_runs.iter().any(|old| old.id == run.id))
            {
                return Err("Saved review has more than one owning row.".into());
            }
            // A native reply may own the updated conversation while original
            // runs stay in the legacy row. Never duplicate mutable run ownership.
            for thread in owned.threads {
                result.threads.retain(|old| old.id != thread.id);
                result.threads.push(thread);
            }
            result.review_runs.extend(owned.review_runs);
            if owned.active_thread_id.is_some() {
                result.active_thread_id = owned.active_thread_id;
            }
            // Native runs use epoch milliseconds, as does the shared engine.
            // Retain stable order for unrecognized legacy timestamps.
            if result
                .review_runs
                .iter()
                .all(|run| run.created_at.parse::<u64>().is_ok())
            {
                result
                    .review_runs
                    .sort_by_key(|run| run.created_at.parse::<u64>().expect("timestamps checked"));
            }
        }
        Ok(Some(result))
    }
}

fn unique_ids(store: &AiReviewStoreData) -> Result<(), String> {
    let mut runs = HashSet::new();
    let mut threads = HashSet::new();
    if store
        .review_runs
        .iter()
        .any(|run| run.id.is_empty() || !runs.insert(&run.id))
        || store
            .threads
            .iter()
            .any(|thread| thread.id.is_empty() || !threads.insert(&thread.id))
    {
        return Err("Saved review identities are missing or ambiguous.".into());
    }
    if store
        .active_thread_id
        .as_ref()
        .is_some_and(|id| !store.threads.iter().any(|thread| thread.id == *id))
    {
        return Err("Saved review active conversation is unavailable.".into());
    }
    Ok(())
}

pub(crate) fn load_owned(scope: &Scope) -> Result<Option<AiReviewStoreData>, String> {
    let owned = crate::review_storage::load_provider_review_json(scope)?
        .map(|json| {
            serde_json::from_str(&json).map_err(|_| "Saved native review is invalid.".to_string())
        })
        .transpose()?;
    if let Some(store) = &owned {
        scope.validate_owned(store)?;
    }
    Ok(owned)
}

pub fn load(scope: &Scope) -> Result<Option<AiReviewStoreData>, String> {
    let owned = load_owned(scope)?;
    let legacy =
        super::review::load_ai_review_store_native(&scope.workspace, &scope.repo, scope.pr_id)?;
    scope.merge(owned, legacy)
}

pub fn save(scope: &Scope, store: &AiReviewStoreData) -> Result<(), String> {
    scope.validate_owned(store)?;
    let json = serde_json::to_string(store)
        .map_err(|_| "Cannot encode saved native review.".to_string())?;
    crate::review_storage::save_provider_review_json(scope, &json)
}

/// Preserve stale-target diagnostics for explicitly selected foreign IDs,
/// without returning their findings, messages or resume tokens.
pub(crate) fn foreign_run_exists(scope: &Scope, id: &str) -> Result<bool, String> {
    let legacy =
        super::review::load_ai_review_store_native(&scope.workspace, &scope.repo, scope.pr_id)?;
    if legacy.is_some_and(|store| {
        store
            .review_runs
            .iter()
            .any(|run| run.id == id && !scope.owns(run))
    }) {
        return Ok(true);
    }
    let mut other = scope.clone();
    other.provider = match scope.provider {
        ReviewProvider::Github => ReviewProvider::Bitbucket,
        ReviewProvider::Bitbucket => ReviewProvider::Github,
    };
    let Some(json) = crate::review_storage::load_provider_review_json(&other)? else {
        return Ok(false);
    };
    let store: AiReviewStoreData =
        serde_json::from_str(&json).map_err(|_| "Saved native review is invalid.".to_string())?;
    other.validate_owned(&store)?;
    Ok(store.review_runs.iter().any(|run| run.id == id))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn scope(provider: ReviewProvider) -> Scope {
        Scope {
            provider,
            workspace: "shared".into(),
            repo: "project".into(),
            pr_id: 42,
        }
    }
    pub(crate) fn fixture(provider: ReviewProvider, id: &str, thread: &str) -> AiReviewStoreData {
        serde_json::from_value(json!({
            "activeThreadId": thread,
            "threads": [{"id": thread, "title": id, "createdAt": "1000", "updatedAt": "1000", "claudeSessionId": "owned-resume", "messages": []}],
            "reviewRuns": [{"id":id, "schemaVersion":"0.1", "provider":provider, "workspace":"shared", "repo":"project", "prId":42,
                "sourceBranch":"feature", "destinationBranch":"main", "status":"succeeded", "turnKind":"initial", "createdAt":"1000", "finishedAt":"1001", "diffFingerprint":"owned", "threadId":thread, "summaryMarkdown":"Owned review"}]
        })).expect("owned fixture")
    }

    #[test]
    fn native_keys_are_provider_qualified_and_length_framed() {
        let gh = scope(ReviewProvider::Github);
        let bb = scope(ReviewProvider::Bitbucket);
        assert_ne!(gh.key(), bb.key());
        let mut a = gh.clone();
        a.workspace = "a:b/é".into();
        a.repo = "c".into();
        let mut b = gh;
        b.workspace = "a".into();
        b.repo = "b/é:c".into();
        assert_ne!(a.key(), b.key());
        assert!(!a.key().starts_with("local:"));
    }

    #[test]
    fn legacy_projection_keeps_only_proved_unambiguous_conversations() {
        let gh = fixture(ReviewProvider::Github, "github-run", "shared-thread");
        let mut mixed = gh.clone();
        let foreign = fixture(ReviewProvider::Bitbucket, "bitbucket-run", "shared-thread");
        mixed.review_runs.extend(foreign.review_runs);
        let projected = scope(ReviewProvider::Github).project_legacy(mixed);
        assert_eq!(projected.review_runs.len(), 1);
        assert!(projected.threads.is_empty());
        assert!(projected.active_thread_id.is_none());
        let mut unproved = AiReviewStoreData {
            threads: gh.threads,
            ..Default::default()
        };
        unproved.active_thread_id = Some("shared-thread".into());
        assert!(scope(ReviewProvider::Github)
            .project_legacy(unproved)
            .threads
            .is_empty());
    }

    #[test]
    fn native_rows_reject_foreign_or_duplicate_run_and_thread_ownership() {
        let scoped = scope(ReviewProvider::Github);
        assert!(scoped
            .validate_owned(&fixture(ReviewProvider::Bitbucket, "run", "thread"))
            .is_err());
        let mut store = fixture(ReviewProvider::Github, "run", "thread");
        store.review_runs.push(store.review_runs[0].clone());
        assert!(scoped.validate_owned(&store).is_err());
        store.review_runs.pop();
        store.threads.push(store.threads[0].clone());
        assert!(scoped.validate_owned(&store).is_err());
    }

    #[test]
    fn native_reply_can_own_conversation_without_copying_legacy_runs() {
        let scoped = scope(ReviewProvider::Github);
        let legacy = fixture(ReviewProvider::Github, "old-run", "thread");
        let mut owned = fixture(ReviewProvider::Github, "new-run", "thread");
        owned.threads[0].title = "Updated conversation".into();
        owned.review_runs[0].created_at = "2000".into();
        let merged = scoped
            .merge(Some(owned), Some(legacy.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(merged.review_runs.len(), 2);
        assert_eq!(merged.threads.len(), 1);
        assert_eq!(merged.threads[0].title, "Updated conversation");
        assert!(scoped.merge(Some(legacy.clone()), Some(legacy)).is_err());
    }
}
