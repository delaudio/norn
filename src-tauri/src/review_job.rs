//! Durable coordination contract for provider-triggered headless review jobs.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::operational_telemetry::OperationalTelemetry;
use crate::review_event::{
    PullRequestReviewEvent, PullRequestReviewEventKind, PullRequestReviewEventProvider,
    PullRequestRevision,
};
use crate::review_storage;

const DEFAULT_LEASE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewJobSchemaVersion {
    #[serde(rename = "v1")]
    V1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ReviewJobScope {
    FullBranch {
        base_sha: String,
        head_sha: String,
    },
    Incremental {
        previous_head_sha: String,
        current_head_sha: String,
    },
}

impl ReviewJobScope {
    pub fn previous_head_sha(&self) -> Option<&str> {
        match self {
            Self::FullBranch { .. } => None,
            Self::Incremental {
                previous_head_sha, ..
            } => Some(previous_head_sha),
        }
    }

    pub fn current_head_sha(&self) -> &str {
        match self {
            Self::FullBranch { head_sha, .. } => head_sha,
            Self::Incremental {
                current_head_sha, ..
            } => current_head_sha,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    DeadLetter,
    Cancelled,
}

impl ReviewJobStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::DeadLetter => "dead_letter",
            Self::Cancelled => "cancelled",
        }
    }

    pub(crate) fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "dead_letter" => Ok(Self::DeadLetter),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown shared review job status: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewJobRequest {
    pub schema_version: ReviewJobSchemaVersion,
    pub id: String,
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub delivery_id: String,
    pub workspace: String,
    pub repository: String,
    pub pull_request_id: u64,
    pub trigger: PullRequestReviewEventKind,
    pub base: PullRequestRevision,
    pub head: PullRequestRevision,
    pub scope: ReviewJobScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewJobRecord {
    pub request: ReviewJobRequest,
    pub status: ReviewJobStatus,
    pub attempt_count: u32,
    pub lease_expires_at: Option<String>,
    pub run_id: Option<String>,
    pub error_code: Option<String>,
    pub next_attempt_at: Option<String>,
    pub terminal_reason: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewJobIgnoredReason {
    Draft,
    Closed,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewJobEnqueueOutcome {
    Queued(Box<ReviewJobRecord>),
    DuplicateDelivery(Box<ReviewJobRecord>),
    DuplicateHead(Option<Box<ReviewJobRecord>>),
    Ignored {
        reason: ReviewJobIgnoredReason,
        cancelled_queued_jobs: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewConcurrencyLimits {
    pub per_repository: usize,
    pub per_pull_request: usize,
}

impl Default for ReviewConcurrencyLimits {
    fn default() -> Self {
        Self {
            per_repository: 2,
            per_pull_request: 1,
        }
    }
}

impl ReviewConcurrencyLimits {
    pub fn validate(self) -> Result<Self, String> {
        if self.per_repository == 0 {
            return Err("`perRepository` concurrency must be positive".to_string());
        }
        if self.per_pull_request == 0 {
            return Err("`perPullRequest` concurrency must be positive".to_string());
        }
        if self.per_pull_request > self.per_repository {
            return Err("`perPullRequest` concurrency must not exceed `perRepository`".to_string());
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewJobExecution {
    Completed {
        run_id: String,
    },
    Failed {
        run_id: Option<String>,
        error_code: String,
    },
    Cancelled {
        run_id: Option<String>,
        reason_code: Option<String>,
    },
}

pub trait ReviewJobStore {
    fn enqueue(&self, event: &PullRequestReviewEvent) -> Result<ReviewJobEnqueueOutcome, String>;

    fn suppress(
        &self,
        event: &PullRequestReviewEvent,
        reason: ReviewJobIgnoredReason,
    ) -> Result<usize, String>;

    fn claim_next(
        &self,
        limits: ReviewConcurrencyLimits,
    ) -> Result<Option<ReviewJobRecord>, String>;

    fn finish(
        &self,
        job_id: &str,
        expected_attempt_count: u32,
        execution: &ReviewJobExecution,
    ) -> Result<ReviewJobRecord, String>;

    fn renew_lease(
        &self,
        job_id: &str,
        expected_attempt_count: u32,
    ) -> Result<ReviewJobRecord, String>;

    /// The caller must supply an operator identity obtained from its authorization boundary.
    fn retry_dead_letter(
        &self,
        job_id: &str,
        authorized_operator_id: &str,
    ) -> Result<ReviewJobRecord, String>;

    fn get(&self, job_id: &str) -> Result<Option<ReviewJobRecord>, String>;
}

pub trait ReviewJobExecutor {
    fn execute(&self, request: &ReviewJobRequest) -> ReviewJobExecution;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SqliteReviewJobStore;

impl ReviewJobStore for SqliteReviewJobStore {
    fn enqueue(&self, event: &PullRequestReviewEvent) -> Result<ReviewJobEnqueueOutcome, String> {
        review_storage::enqueue_shared_review_job(event)
    }

    fn suppress(
        &self,
        event: &PullRequestReviewEvent,
        reason: ReviewJobIgnoredReason,
    ) -> Result<usize, String> {
        review_storage::suppress_shared_review_jobs(event, reason)
    }

    fn claim_next(
        &self,
        limits: ReviewConcurrencyLimits,
    ) -> Result<Option<ReviewJobRecord>, String> {
        review_storage::claim_next_shared_review_job(limits)
    }

    fn finish(
        &self,
        job_id: &str,
        expected_attempt_count: u32,
        execution: &ReviewJobExecution,
    ) -> Result<ReviewJobRecord, String> {
        review_storage::finish_shared_review_job(job_id, expected_attempt_count, execution)
    }

    fn renew_lease(
        &self,
        job_id: &str,
        expected_attempt_count: u32,
    ) -> Result<ReviewJobRecord, String> {
        review_storage::renew_shared_review_job_lease(job_id, expected_attempt_count)
    }

    fn retry_dead_letter(
        &self,
        job_id: &str,
        authorized_operator_id: &str,
    ) -> Result<ReviewJobRecord, String> {
        review_storage::retry_dead_letter_shared_review_job(job_id, authorized_operator_id)
    }

    fn get(&self, job_id: &str) -> Result<Option<ReviewJobRecord>, String> {
        review_storage::get_shared_review_job(job_id)
    }
}

#[derive(Debug)]
pub struct ReviewJobCoordinator<S, E> {
    store: S,
    executor: E,
    limits: ReviewConcurrencyLimits,
    lease_heartbeat_interval: Duration,
    telemetry: OperationalTelemetry,
}

impl<S, E> ReviewJobCoordinator<S, E>
where
    S: ReviewJobStore + Sync,
    E: ReviewJobExecutor,
{
    pub fn new(store: S, executor: E, limits: ReviewConcurrencyLimits) -> Result<Self, String> {
        Ok(Self {
            store,
            executor,
            limits: limits.validate()?,
            lease_heartbeat_interval: DEFAULT_LEASE_HEARTBEAT_INTERVAL,
            telemetry: OperationalTelemetry::default(),
        })
    }

    pub fn with_telemetry(mut self, telemetry: OperationalTelemetry) -> Self {
        self.telemetry = telemetry;
        self
    }

    pub fn telemetry(&self) -> OperationalTelemetry {
        self.telemetry.clone()
    }

    #[cfg(test)]
    fn with_lease_heartbeat_interval(mut self, interval: Duration) -> Self {
        self.lease_heartbeat_interval = interval;
        self
    }

    pub fn accept_event(
        &self,
        event: &PullRequestReviewEvent,
    ) -> Result<ReviewJobEnqueueOutcome, String> {
        event.validate().map_err(|error| error.to_string())?;
        self.telemetry.record_received(event.provider);
        if event.kind == PullRequestReviewEventKind::Closed {
            return Ok(ReviewJobEnqueueOutcome::Ignored {
                reason: ReviewJobIgnoredReason::Closed,
                cancelled_queued_jobs: self
                    .store
                    .suppress(event, ReviewJobIgnoredReason::Closed)?,
            });
        }
        if event.draft {
            return Ok(ReviewJobEnqueueOutcome::Ignored {
                reason: ReviewJobIgnoredReason::Draft,
                cancelled_queued_jobs: self.store.suppress(event, ReviewJobIgnoredReason::Draft)?,
            });
        }
        let outcome = self.store.enqueue(event)?;
        if let ReviewJobEnqueueOutcome::Queued(job) = &outcome {
            self.telemetry
                .record_queued(event.provider, &event.delivery_id, &job.request.id);
        }
        Ok(outcome)
    }

    pub fn run_next(&self) -> Result<Option<ReviewJobRecord>, String> {
        let Some(job) = self.store.claim_next(self.limits)? else {
            return Ok(None);
        };
        let (stop_tx, stop_rx) = mpsc::channel();
        let (renewal_error_tx, renewal_error_rx) = mpsc::channel();
        let heartbeat_store = &self.store;
        let heartbeat_interval = self.lease_heartbeat_interval;
        let heartbeat_job_id = job.request.id.clone();
        let heartbeat_attempt_count = job.attempt_count;
        let execution_started = Instant::now();
        let execution = std::thread::scope(|scope| {
            scope.spawn(move || loop {
                match stop_rx.recv_timeout(heartbeat_interval) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Err(error) =
                            heartbeat_store.renew_lease(&heartbeat_job_id, heartbeat_attempt_count)
                        {
                            let _ = renewal_error_tx.send(error);
                            break;
                        }
                    }
                }
            });
            let execution = self.executor.execute(&job.request);
            let _ = stop_tx.send(());
            execution
        });
        let renewal_error = renewal_error_rx.try_recv().ok();
        match self
            .store
            .finish(&job.request.id, job.attempt_count, &execution)
        {
            Ok(finished) => {
                let queue_wait = queue_wait_duration(&job);
                let review_duration = execution_started.elapsed();
                match finished.status {
                    ReviewJobStatus::Completed => self.telemetry.record_completed(
                        job.request.provider,
                        queue_wait,
                        review_duration,
                    ),
                    ReviewJobStatus::Queued
                    | ReviewJobStatus::DeadLetter
                    | ReviewJobStatus::Failed => {
                        self.telemetry.record_failure(
                            job.request.provider,
                            queue_wait,
                            review_duration,
                            finished.status == ReviewJobStatus::Queued,
                            finished.status == ReviewJobStatus::DeadLetter,
                        );
                    }
                    ReviewJobStatus::Running | ReviewJobStatus::Cancelled => {}
                }
                Ok(Some(finished))
            }
            Err(finish_error) => match renewal_error {
                Some(renewal_error) => Err(format!(
                    "Failed to finish shared review job after lease renewal error: \
                     {finish_error}; renewal: {renewal_error}"
                )),
                None => Err(finish_error),
            },
        }
    }

    pub fn retry_dead_letter(
        &self,
        job_id: &str,
        authorized_operator_id: &str,
    ) -> Result<ReviewJobRecord, String> {
        self.store.retry_dead_letter(job_id, authorized_operator_id)
    }
}

fn queue_wait_duration(job: &ReviewJobRecord) -> Duration {
    let created = job.created_at.parse::<u64>().ok();
    let started = job
        .started_at
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok());
    match (created, started) {
        (Some(created), Some(started)) => Duration::from_millis(started.saturating_sub(created)),
        _ => Duration::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use super::*;
    use crate::review_event::{PullRequestEventActor, PullRequestReviewEventSchemaVersion};

    const BASE_SHA: &str = "1111111111111111111111111111111111111111";
    const HEAD_SHA: &str = "2222222222222222222222222222222222222222";

    #[derive(Debug, Default)]
    struct RecordingStore {
        calls: Mutex<Vec<&'static str>>,
        claimed: Mutex<Option<ReviewJobRecord>>,
        fail_renewal: bool,
        finish_dead_letter: bool,
    }

    impl ReviewJobStore for RecordingStore {
        fn enqueue(&self, _: &PullRequestReviewEvent) -> Result<ReviewJobEnqueueOutcome, String> {
            self.calls.lock().expect("calls").push("enqueue");
            Ok(ReviewJobEnqueueOutcome::Queued(Box::new(job())))
        }

        fn suppress(
            &self,
            _: &PullRequestReviewEvent,
            _: ReviewJobIgnoredReason,
        ) -> Result<usize, String> {
            self.calls.lock().expect("calls").push("suppress");
            Ok(1)
        }

        fn claim_next(
            &self,
            _: ReviewConcurrencyLimits,
        ) -> Result<Option<ReviewJobRecord>, String> {
            self.calls.lock().expect("calls").push("claim");
            Ok(self.claimed.lock().expect("claimed").take())
        }

        fn finish(
            &self,
            _: &str,
            _: u32,
            execution: &ReviewJobExecution,
        ) -> Result<ReviewJobRecord, String> {
            self.calls.lock().expect("calls").push("finish");
            let mut record = job();
            if self.finish_dead_letter {
                assert!(matches!(execution, ReviewJobExecution::Failed { .. }));
                record.status = ReviewJobStatus::DeadLetter;
            } else {
                assert!(matches!(execution, ReviewJobExecution::Completed { .. }));
                record.status = ReviewJobStatus::Completed;
            }
            Ok(record)
        }

        fn renew_lease(&self, _: &str, _: u32) -> Result<ReviewJobRecord, String> {
            self.calls.lock().expect("calls").push("renew");
            if self.fail_renewal {
                Err("temporary_storage_error".to_string())
            } else {
                Ok(job())
            }
        }

        fn retry_dead_letter(&self, _: &str, _: &str) -> Result<ReviewJobRecord, String> {
            Ok(job())
        }

        fn get(&self, _: &str) -> Result<Option<ReviewJobRecord>, String> {
            Ok(None)
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct SuccessfulExecutor;

    impl ReviewJobExecutor for SuccessfulExecutor {
        fn execute(&self, _: &ReviewJobRequest) -> ReviewJobExecution {
            ReviewJobExecution::Completed {
                run_id: "run-1".to_string(),
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct SlowSuccessfulExecutor;

    impl ReviewJobExecutor for SlowSuccessfulExecutor {
        fn execute(&self, _: &ReviewJobRequest) -> ReviewJobExecution {
            std::thread::sleep(Duration::from_millis(20));
            ReviewJobExecution::Completed {
                run_id: "run-slow".to_string(),
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct FailedExecutor;

    impl ReviewJobExecutor for FailedExecutor {
        fn execute(&self, _: &ReviewJobRequest) -> ReviewJobExecution {
            ReviewJobExecution::Failed {
                run_id: Some("run-failed".to_string()),
                error_code: "invalid_policy".to_string(),
            }
        }
    }

    fn event(kind: PullRequestReviewEventKind, draft: bool) -> PullRequestReviewEvent {
        PullRequestReviewEvent {
            schema_version: PullRequestReviewEventSchemaVersion::V1,
            kind,
            provider: PullRequestReviewEventProvider::Github,
            tenant_id: "tenant-acme".to_string(),
            workspace: "acme".to_string(),
            repository: "payments".to_string(),
            pull_request_id: 42,
            base: PullRequestRevision {
                ref_name: "main".to_string(),
                sha: BASE_SHA.to_string(),
            },
            head: PullRequestRevision {
                ref_name: "feature/retry".to_string(),
                sha: HEAD_SHA.to_string(),
            },
            provider_updated_at_ms: Some(1_785_168_000_000),
            draft,
            closed_outcome: (kind == PullRequestReviewEventKind::Closed)
                .then_some(crate::review_event::PullRequestClosedOutcome::ClosedWithoutMerge),
            actor: PullRequestEventActor {
                id: "user-7".to_string(),
                login: "reviewer".to_string(),
                display_name: None,
            },
            delivery_id: "delivery-1".to_string(),
        }
    }

    fn job() -> ReviewJobRecord {
        ReviewJobRecord {
            request: ReviewJobRequest {
                schema_version: ReviewJobSchemaVersion::V1,
                id: "job-1".to_string(),
                tenant_id: "tenant-acme".to_string(),
                provider: PullRequestReviewEventProvider::Github,
                delivery_id: "delivery-1".to_string(),
                workspace: "acme".to_string(),
                repository: "payments".to_string(),
                pull_request_id: 42,
                trigger: PullRequestReviewEventKind::Opened,
                base: PullRequestRevision {
                    ref_name: "main".to_string(),
                    sha: BASE_SHA.to_string(),
                },
                head: PullRequestRevision {
                    ref_name: "feature/retry".to_string(),
                    sha: HEAD_SHA.to_string(),
                },
                scope: ReviewJobScope::FullBranch {
                    base_sha: BASE_SHA.to_string(),
                    head_sha: HEAD_SHA.to_string(),
                },
            },
            status: ReviewJobStatus::Running,
            attempt_count: 1,
            lease_expires_at: Some("2000".to_string()),
            run_id: None,
            error_code: None,
            next_attempt_at: None,
            terminal_reason: None,
            created_at: "1000".to_string(),
            started_at: Some("1001".to_string()),
            finished_at: None,
        }
    }

    #[test]
    fn draft_and_closed_events_suppress_queued_work_without_enqueueing() {
        for event in [
            event(PullRequestReviewEventKind::Opened, true),
            event(PullRequestReviewEventKind::Closed, false),
        ] {
            let coordinator = ReviewJobCoordinator::new(
                RecordingStore::default(),
                SuccessfulExecutor,
                ReviewConcurrencyLimits::default(),
            )
            .expect("coordinator");
            let outcome = coordinator.accept_event(&event).expect("accept event");

            assert!(matches!(
                outcome,
                ReviewJobEnqueueOutcome::Ignored {
                    cancelled_queued_jobs: 1,
                    ..
                }
            ));
            assert_eq!(
                coordinator.store.calls.lock().expect("calls").as_slice(),
                ["suppress"]
            );
        }
    }

    #[test]
    fn every_eligible_event_kind_enqueues_review_work() {
        for kind in [
            PullRequestReviewEventKind::Opened,
            PullRequestReviewEventKind::Reopened,
            PullRequestReviewEventKind::Synchronized,
            PullRequestReviewEventKind::ReadyForReview,
        ] {
            let coordinator = ReviewJobCoordinator::new(
                RecordingStore::default(),
                SuccessfulExecutor,
                ReviewConcurrencyLimits::default(),
            )
            .expect("coordinator");

            assert!(matches!(
                coordinator
                    .accept_event(&event(kind, false))
                    .expect("accept eligible event"),
                ReviewJobEnqueueOutcome::Queued(_)
            ));
            assert_eq!(
                coordinator.store.calls.lock().expect("calls").as_slice(),
                ["enqueue"]
            );
        }
    }

    #[test]
    fn run_next_claims_executes_and_finishes_one_job() {
        let store = RecordingStore::default();
        *store.claimed.lock().expect("claimed") = Some(job());
        let coordinator = ReviewJobCoordinator::new(
            store,
            SuccessfulExecutor,
            ReviewConcurrencyLimits::default(),
        )
        .expect("coordinator");

        let completed = coordinator
            .run_next()
            .expect("run next")
            .expect("completed job");

        assert_eq!(completed.status, ReviewJobStatus::Completed);
        assert_eq!(
            coordinator.store.calls.lock().expect("calls").as_slice(),
            ["claim", "finish"]
        );
    }

    #[test]
    fn synthetic_failed_job_records_failure_timing_and_dead_letter_metrics() {
        let telemetry = OperationalTelemetry::default();
        let store = RecordingStore {
            claimed: Mutex::new(Some(job())),
            finish_dead_letter: true,
            ..RecordingStore::default()
        };
        let coordinator =
            ReviewJobCoordinator::new(store, FailedExecutor, ReviewConcurrencyLimits::default())
                .expect("coordinator")
                .with_telemetry(telemetry.clone());

        let finished = coordinator
            .run_next()
            .expect("run synthetic failed job")
            .expect("finished job");
        assert_eq!(finished.status, ReviewJobStatus::DeadLetter);
        let metrics = telemetry.prometheus();
        assert!(metrics.contains("norn_review_jobs_total{provider=\"github\",outcome=\"failed\""));
        assert!(metrics.contains("norn_review_dead_letter_jobs_total"));
        assert!(metrics.contains("norn_review_queue_wait_ms_count"));
        assert!(metrics.contains("norn_review_duration_ms_count"));
    }

    #[test]
    fn run_next_renews_the_lease_while_execution_is_active() {
        let store = RecordingStore {
            claimed: Mutex::new(Some(job())),
            ..RecordingStore::default()
        };
        let coordinator = ReviewJobCoordinator::new(
            store,
            SlowSuccessfulExecutor,
            ReviewConcurrencyLimits::default(),
        )
        .expect("coordinator")
        .with_lease_heartbeat_interval(Duration::from_millis(2));

        coordinator.run_next().expect("run slow review");

        let calls = coordinator.store.calls.lock().expect("calls");
        assert_eq!(calls.first(), Some(&"claim"));
        assert!(calls.contains(&"renew"));
        assert_eq!(calls.last(), Some(&"finish"));
    }

    #[test]
    fn run_next_attempts_fenced_finish_after_a_renewal_error() {
        let store = RecordingStore {
            claimed: Mutex::new(Some(job())),
            fail_renewal: true,
            ..RecordingStore::default()
        };
        let coordinator = ReviewJobCoordinator::new(
            store,
            SlowSuccessfulExecutor,
            ReviewConcurrencyLimits::default(),
        )
        .expect("coordinator")
        .with_lease_heartbeat_interval(Duration::from_millis(2));

        let completed = coordinator
            .run_next()
            .expect("fenced finish succeeds")
            .expect("completed job");

        assert_eq!(completed.status, ReviewJobStatus::Completed);
        let calls = coordinator.store.calls.lock().expect("calls");
        assert!(calls.contains(&"renew"));
        assert_eq!(calls.last(), Some(&"finish"));
    }

    #[test]
    fn concurrency_limits_reject_zero_and_inverted_values() {
        assert!(ReviewConcurrencyLimits {
            per_repository: 0,
            per_pull_request: 0,
        }
        .validate()
        .is_err());
        assert!(ReviewConcurrencyLimits {
            per_repository: 1,
            per_pull_request: 2,
        }
        .validate()
        .is_err());
    }

    #[test]
    fn v1_job_request_json_is_stable_and_contains_no_payload_or_secret_fields() {
        let value = serde_json::to_value(job().request).expect("serialize job request");

        assert_eq!(
            value,
            serde_json::json!({
                "schemaVersion": "v1",
                "id": "job-1",
                "tenantId": "tenant-acme",
                "provider": "github",
                "deliveryId": "delivery-1",
                "workspace": "acme",
                "repository": "payments",
                "pullRequestId": 42,
                "trigger": "opened",
                "base": {
                    "refName": "main",
                    "sha": BASE_SHA
                },
                "head": {
                    "refName": "feature/retry",
                    "sha": HEAD_SHA
                },
                "scope": {
                    "kind": "fullBranch",
                    "baseSha": BASE_SHA,
                    "headSha": HEAD_SHA
                }
            })
        );
        let rendered = value.to_string().to_ascii_lowercase();
        for forbidden in [
            "actor",
            "payload",
            "credential",
            "secret",
            "token",
            "webhook",
        ] {
            assert!(!rendered.contains(forbidden), "{forbidden} leaked into job");
        }
    }
}
