use std::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    thread,
};

use crate::{
    config::{RepoRef, ReviewProvider},
    local_repo,
    local_review::{
        local_review_snapshot_for_configured_path, LocalReviewCancellation, LocalReviewSnapshot,
    },
    services::{
        bitbucket::{
            get_pr_diff_native, get_pull_request_native, list_comments_native,
            list_pull_requests_native, ListPrOptions, PrComment, PullRequestDetail,
            PullRequestSummary,
        },
        review::{
            get_ai_review_run_state_native, get_local_ai_review_run_state_native,
            load_ai_review_store_native, load_local_ai_review_store_native, AiReviewRunState,
            AiReviewRunStatus, AiReviewRunStore,
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LoadState {
    Idle,
    Loading,
    Ready,
    Failed(String),
}

impl LoadState {
    pub(super) fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    pub(super) fn error(&self) -> Option<&str> {
        match self {
            Self::Failed(error) => Some(error),
            _ => None,
        }
    }
}

pub(super) enum LoadEvent {
    CurrentRepo {
        request_id: u64,
        result: Result<RepoRef, String>,
    },
    PullRequests {
        request_id: u64,
        result: Result<Vec<PullRequestSummary>, String>,
    },
    LocalSnapshot {
        request_id: u64,
        result: Result<LocalReviewSnapshot, String>,
    },
    LocalRepoEligibility {
        request_id: u64,
        repo_generation: u64,
        eligible_repositories: Vec<RepoEligibilityIdentity>,
    },
    Detail {
        request_id: u64,
        result: Result<PullRequestDetail, String>,
    },
    Comments {
        request_id: u64,
        result: Result<Vec<PrComment>, String>,
    },
    Diff {
        request_id: u64,
        result: Result<String, String>,
    },
    AiReview {
        request_id: u64,
        pr_id: u32,
        state: Option<AiReviewRunState>,
        output: Result<Option<String>, String>,
    },
    ReviewMarkers {
        request_id: u64,
        marker_generation: u64,
        reviewed: Vec<u32>,
        running: Vec<u32>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RepoEligibilityIdentity {
    provider: ReviewProvider,
    workspace: String,
    repo: String,
    local_path: Option<String>,
}

impl RepoEligibilityIdentity {
    pub(super) fn from_repo(repo: &RepoRef) -> Self {
        Self {
            provider: repo.provider,
            workspace: repo.workspace.clone(),
            repo: repo.repo.clone(),
            local_path: repo.local_path.clone(),
        }
    }

    pub(super) fn matches(&self, repo: &RepoRef) -> bool {
        self.provider == repo.provider
            && self.workspace == repo.workspace
            && self.repo == repo.repo
            && self.local_path == repo.local_path
    }
}

pub(super) struct Loader {
    sender: Sender<LoadEvent>,
    receiver: Receiver<LoadEvent>,
    local_snapshot_cancellation: Mutex<Option<LocalReviewCancellation>>,
}

impl Loader {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver,
            local_snapshot_cancellation: Mutex::new(None),
        }
    }

    pub(super) fn try_recv(&self) -> Option<LoadEvent> {
        self.receiver.try_recv().ok()
    }

    pub(super) fn resolve_current_repo(&self, request_id: u64) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            let _ = sender.send(LoadEvent::CurrentRepo {
                request_id,
                result: local_repo::resolve_current_repo(),
            });
        });
    }

    pub(super) fn pull_requests(
        &self,
        request_id: u64,
        provider: ReviewProvider,
        workspace: String,
        repo: String,
        state: String,
    ) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            let opts = ListPrOptions {
                state: Some(state),
                page: Some(1),
                pagelen: Some(50),
                query: None,
                updated_after: None,
            };
            let result =
                list_pull_requests_native(Some(provider), workspace.as_str(), repo.as_str(), &opts)
                    .map(|page| page.values);
            let _ = sender.send(LoadEvent::PullRequests { request_id, result });
        });
    }

    pub(super) fn local_snapshot(
        &self,
        request_id: u64,
        provider: ReviewProvider,
        workspace: String,
        repo: String,
        local_path: String,
    ) {
        self.cancel_local_snapshot();
        let cancellation = LocalReviewCancellation::new();
        if let Ok(mut active) = self.local_snapshot_cancellation.lock() {
            *active = Some(cancellation.clone());
        }
        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = local_review_snapshot_for_configured_path(
                provider,
                workspace.as_str(),
                repo.as_str(),
                std::path::Path::new(local_path.as_str()),
                cancellation,
            );
            let _ = sender.send(LoadEvent::LocalSnapshot { request_id, result });
        });
    }

    pub(super) fn local_repo_eligibility(
        &self,
        request_id: u64,
        repo_generation: u64,
        repos: Vec<RepoRef>,
    ) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            let eligible_repositories = repos
                .iter()
                .filter(|repo| local_repo::has_usable_configured_path(repo))
                .map(RepoEligibilityIdentity::from_repo)
                .collect();
            let _ = sender.send(LoadEvent::LocalRepoEligibility {
                request_id,
                repo_generation,
                eligible_repositories,
            });
        });
    }

    pub(super) fn cancel_local_snapshot(&self) {
        if let Ok(mut active) = self.local_snapshot_cancellation.lock() {
            if let Some(cancellation) = active.take() {
                cancellation.cancel();
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the loader boundary carries identifiers for two coordinated asynchronous requests"
    )]
    pub(super) fn pull_request_resources(
        &self,
        request_id: u64,
        ai_request_id: u64,
        provider: ReviewProvider,
        workspace: String,
        repo: String,
        pr_id: u32,
        ai_review_store: AiReviewRunStore,
    ) {
        let detail_sender = self.sender.clone();
        let detail_workspace = workspace.clone();
        let detail_repo = repo.clone();
        thread::spawn(move || {
            let result = get_pull_request_native(
                Some(provider),
                detail_workspace.as_str(),
                detail_repo.as_str(),
                pr_id,
            );
            let _ = detail_sender.send(LoadEvent::Detail { request_id, result });
        });

        let comments_sender = self.sender.clone();
        let comments_workspace = workspace.clone();
        let comments_repo = repo.clone();
        thread::spawn(move || {
            let result = list_comments_native(
                Some(provider),
                comments_workspace.as_str(),
                comments_repo.as_str(),
                pr_id,
            );
            let _ = comments_sender.send(LoadEvent::Comments { request_id, result });
        });

        let diff_sender = self.sender.clone();
        let diff_workspace = workspace.clone();
        let diff_repo = repo.clone();
        thread::spawn(move || {
            let result = get_pr_diff_native(
                Some(provider),
                diff_workspace.as_str(),
                diff_repo.as_str(),
                pr_id,
            );
            let _ = diff_sender.send(LoadEvent::Diff { request_id, result });
        });

        self.ai_review(ai_request_id, workspace, repo, pr_id, ai_review_store);
    }

    pub(super) fn ai_review(
        &self,
        request_id: u64,
        workspace: String,
        repo: String,
        pr_id: u32,
        store: AiReviewRunStore,
    ) {
        self.ai_review_matching(request_id, workspace, repo, pr_id, store, None);
    }

    pub(super) fn ai_review_for_snapshot(
        &self,
        request_id: u64,
        workspace: String,
        repo: String,
        pr_id: u32,
        store: AiReviewRunStore,
        snapshot_sha256: String,
    ) {
        self.ai_review_matching(
            request_id,
            workspace,
            repo,
            pr_id,
            store,
            Some(snapshot_sha256),
        );
    }

    fn ai_review_matching(
        &self,
        request_id: u64,
        workspace: String,
        repo: String,
        pr_id: u32,
        store: AiReviewRunStore,
        expected_head_sha: Option<String>,
    ) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            let state = if let Some(snapshot_sha256) = expected_head_sha.as_deref() {
                get_local_ai_review_run_state_native(&store, &workspace, &repo, snapshot_sha256)
                    .unwrap_or(None)
            } else {
                get_ai_review_run_state_native(&store, &workspace, &repo, pr_id)
            }
            .filter(|state| {
                reviewed_head_matches(
                    state.reviewed_head_sha.as_deref(),
                    expected_head_sha.as_deref(),
                )
            });
            let loaded_store = if let Some(snapshot_sha256) = expected_head_sha.as_deref() {
                load_local_ai_review_store_native(&workspace, &repo, snapshot_sha256)
            } else {
                load_ai_review_store_native(&workspace, &repo, pr_id)
            };
            let output = loaded_store.map(|store| {
                store.and_then(|store| {
                    store.review_runs.iter().rev().find_map(|run| {
                        reviewed_head_matches(
                            run.reviewed_head_sha.as_deref(),
                            expected_head_sha.as_deref(),
                        )
                        .then(|| run.summary_markdown.clone())
                        .flatten()
                    })
                })
            });
            let _ = sender.send(LoadEvent::AiReview {
                request_id,
                pr_id,
                state,
                output,
            });
        });
    }

    pub(super) fn review_markers(
        &self,
        request_id: u64,
        workspace: String,
        repo: String,
        pr_ids: Vec<u32>,
        store: AiReviewRunStore,
        marker_generation: u64,
    ) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            let mut reviewed = Vec::new();
            let mut running = Vec::new();
            for pr_id in pr_ids {
                if matches!(
                    get_ai_review_run_state_native(&store, &workspace, &repo, pr_id)
                        .map(|state| state.status),
                    Some(AiReviewRunStatus::Running)
                ) {
                    running.push(pr_id);
                }
                if matches!(
                    load_ai_review_store_native(&workspace, &repo, pr_id),
                    Ok(Some(store)) if !store.review_runs.is_empty()
                ) {
                    reviewed.push(pr_id);
                }
            }
            let _ = sender.send(LoadEvent::ReviewMarkers {
                request_id,
                marker_generation,
                reviewed,
                running,
            });
        });
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.cancel_local_snapshot();
    }
}

fn reviewed_head_matches(actual: Option<&str>, expected: Option<&str>) -> bool {
    expected.is_none_or(|expected| actual == Some(expected))
}

#[cfg(test)]
mod tests {
    use super::reviewed_head_matches;

    #[test]
    fn local_review_state_requires_the_full_snapshot_identity() {
        assert!(reviewed_head_matches(Some("current"), Some("current")));
        assert!(!reviewed_head_matches(Some("previous"), Some("current")));
        assert!(!reviewed_head_matches(None, Some("current")));
        assert!(reviewed_head_matches(Some("provider-head"), None));
    }
}
