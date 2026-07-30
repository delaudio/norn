use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use crate::{
    config::{RepoRef, ReviewProvider},
    local_repo,
    services::{
        bitbucket::{
            get_pr_diff_native, get_pull_request_native, list_comments_native,
            list_pull_requests_native, ListPrOptions, PrComment, PullRequestDetail,
            PullRequestSummary,
        },
        review::{
            get_ai_review_run_state_native, load_ai_review_store_native, AiReviewRunState,
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
        reviewed: Vec<u32>,
        running: Vec<u32>,
    },
}

pub(super) struct Loader {
    sender: Sender<LoadEvent>,
    receiver: Receiver<LoadEvent>,
}

impl Loader {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self { sender, receiver }
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
        let sender = self.sender.clone();
        thread::spawn(move || {
            let state = get_ai_review_run_state_native(&store, &workspace, &repo, pr_id);
            let output = load_ai_review_store_native(&workspace, &repo, pr_id).map(|store| {
                store.and_then(|store| {
                    store
                        .review_runs
                        .iter()
                        .rev()
                        .find_map(|run| run.summary_markdown.clone())
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
                reviewed,
                running,
            });
        });
    }
}
