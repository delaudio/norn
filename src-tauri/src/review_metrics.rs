//! Deterministic review-effectiveness aggregation over stored review runs and feedback.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::review_event::PullRequestReviewEventProvider;
use crate::review_feedback::{
    ReviewFindingFeedbackAction, ReviewFindingFeedbackEvent, ReviewFindingFeedbackIdentity,
};

pub const REVIEW_EFFECTIVENESS_SCHEMA_VERSION: &str = "lachesi.review-effectiveness.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewEffectivenessRunStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewMetricSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl ReviewMetricSeverity {
    const ALL: [Self; 5] = [
        Self::Info,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Critical,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|severity| severity.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewMetricCategory {
    Bug,
    Security,
    Performance,
    Architecture,
    Typing,
    Test,
    Maintainability,
    Docs,
    Other,
}

impl ReviewMetricCategory {
    const ALL: [Self; 9] = [
        Self::Bug,
        Self::Security,
        Self::Performance,
        Self::Architecture,
        Self::Typing,
        Self::Test,
        Self::Maintainability,
        Self::Docs,
        Self::Other,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bug => "bug",
            Self::Security => "security",
            Self::Performance => "performance",
            Self::Architecture => "architecture",
            Self::Typing => "typing",
            Self::Test => "test",
            Self::Maintainability => "maintainability",
            Self::Docs => "docs",
            Self::Other => "other",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.as_str() == value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEffectivenessFinding {
    pub fingerprint: String,
    pub severity: ReviewMetricSeverity,
    pub category: ReviewMetricCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEffectivenessRun {
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repo: String,
    pub pr_id: u64,
    pub run_id: String,
    pub status: ReviewEffectivenessRunStatus,
    pub created_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub findings: Vec<ReviewEffectivenessFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewEffectivenessFilter {
    pub tenant_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<PullRequestReviewEventProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_ms: Option<i64>,
}

impl Default for ReviewEffectivenessFilter {
    fn default() -> Self {
        Self {
            tenant_id: "local".to_string(),
            provider: None,
            workspace: None,
            repo: None,
            from_ms: None,
            to_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewMetricCount {
    pub key: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewMetricRate {
    pub numerator: u64,
    pub denominator: u64,
    pub basis_points: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewFeedbackMetrics {
    pub eligible_findings: u64,
    pub findings_with_feedback: u64,
    pub findings_without_feedback: u64,
    pub accepted_findings: u64,
    pub false_positive_findings: u64,
    pub fixed_findings: u64,
    pub dismissed_findings: u64,
    pub reopened_findings: u64,
    pub coverage_rate: ReviewMetricRate,
    pub acceptance_rate: ReviewMetricRate,
    pub false_positive_rate: ReviewMetricRate,
    pub fixed_rate: ReviewMetricRate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewLatencyMetrics {
    pub sample_count: u64,
    pub total_ms: u64,
    pub average_ms: Option<u64>,
    pub median_ms: Option<u64>,
    pub minimum_ms: Option<u64>,
    pub maximum_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewEffectivenessSummary {
    pub review_count: u64,
    pub finding_count: u64,
    pub findings_by_severity: Vec<ReviewMetricCount>,
    pub findings_by_category: Vec<ReviewMetricCount>,
    pub feedback: ReviewFeedbackMetrics,
    pub time_to_first_review: ReviewLatencyMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryReviewEffectiveness {
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repo: String,
    pub summary: ReviewEffectivenessSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewEffectivenessReport {
    pub schema_version: String,
    pub filter: ReviewEffectivenessFilter,
    pub summary: ReviewEffectivenessSummary,
    pub repositories: Vec<RepositoryReviewEffectiveness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewEffectivenessError {
    InvalidFilter(&'static str),
    InvalidRun(String),
    InvalidFeedback(String),
}

impl fmt::Display for ReviewEffectivenessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFilter(message) => formatter.write_str(message),
            Self::InvalidRun(message) => write!(formatter, "Invalid stored review run: {message}"),
            Self::InvalidFeedback(message) => {
                write!(formatter, "Invalid stored finding feedback: {message}")
            }
        }
    }
}

impl std::error::Error for ReviewEffectivenessError {}

type RepositoryKey = (PullRequestReviewEventProvider, String, String);
type PullRequestKey = (PullRequestReviewEventProvider, String, String, u64);
type FindingKey = (
    PullRequestReviewEventProvider,
    String,
    String,
    u64,
    String,
    String,
);

pub fn aggregate_review_effectiveness(
    runs: &[ReviewEffectivenessRun],
    feedback_events: &[ReviewFindingFeedbackEvent],
    filter: ReviewEffectivenessFilter,
) -> Result<ReviewEffectivenessReport, ReviewEffectivenessError> {
    validate_review_effectiveness_filter(&filter)?;
    for run in runs.iter().filter(|run| {
        run.tenant_id == filter.tenant_id && run_matches_repository_filter(run, &filter)
    }) {
        validate_run(run)?;
    }
    for event in feedback_events
        .iter()
        .filter(|event| feedback_matches_filter(event, &filter))
    {
        event
            .validate()
            .map_err(|error| ReviewEffectivenessError::InvalidFeedback(error.to_string()))?;
    }

    let tenant_runs = runs
        .iter()
        .filter(|run| {
            run.tenant_id == filter.tenant_id && run_matches_repository_filter(run, &filter)
        })
        .collect::<Vec<_>>();
    let latest_feedback = latest_feedback_by_finding(feedback_events, &filter)?;
    let summary = aggregate_summary(&tenant_runs, &latest_feedback, &filter, None);

    let mut repository_keys = tenant_runs
        .iter()
        .filter(|run| run.status == ReviewEffectivenessRunStatus::Succeeded)
        .filter(|run| {
            run.finished_at_ms
                .is_some_and(|finished| timestamp_in_filter(finished, &filter))
        })
        .filter(|run| run_matches_repository_filter(run, &filter))
        .map(|run| (run.provider, run.workspace.clone(), run.repo.clone()))
        .collect::<Vec<_>>();
    repository_keys.sort_by(|left, right| {
        left.0
            .as_str()
            .cmp(right.0.as_str())
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    repository_keys.dedup();
    let repositories = repository_keys
        .into_iter()
        .map(|key| RepositoryReviewEffectiveness {
            provider: key.0,
            workspace: key.1.clone(),
            repo: key.2.clone(),
            summary: aggregate_summary(&tenant_runs, &latest_feedback, &filter, Some(&key)),
        })
        .collect();

    Ok(ReviewEffectivenessReport {
        schema_version: REVIEW_EFFECTIVENESS_SCHEMA_VERSION.to_string(),
        filter,
        summary,
        repositories,
    })
}

fn aggregate_summary(
    tenant_runs: &[&ReviewEffectivenessRun],
    latest_feedback: &HashMap<FindingKey, ReviewFindingFeedbackAction>,
    filter: &ReviewEffectivenessFilter,
    repository: Option<&RepositoryKey>,
) -> ReviewEffectivenessSummary {
    let all_successful = tenant_runs
        .iter()
        .copied()
        .filter(|run| run.status == ReviewEffectivenessRunStatus::Succeeded)
        .filter(|run| repository.is_none_or(|key| run_matches_repository(run, key)))
        .filter(|run| run_matches_repository_filter(run, filter))
        .collect::<Vec<_>>();
    let selected_runs = all_successful
        .iter()
        .copied()
        .filter(|run| {
            run.finished_at_ms
                .is_some_and(|finished| timestamp_in_filter(finished, filter))
        })
        .collect::<Vec<_>>();

    let mut severity_counts = zero_counts(
        ReviewMetricSeverity::ALL
            .into_iter()
            .map(ReviewMetricSeverity::as_str),
    );
    let mut category_counts = zero_counts(
        ReviewMetricCategory::ALL
            .into_iter()
            .map(ReviewMetricCategory::as_str),
    );
    let mut feedback = ReviewFeedbackAccumulator::default();
    for run in &selected_runs {
        for finding in &run.findings {
            *severity_counts
                .get_mut(finding.severity.as_str())
                .expect("known severity key") += 1;
            *category_counts
                .get_mut(finding.category.as_str())
                .expect("known category key") += 1;
            feedback.record(latest_feedback.get(&finding_key(run, &finding.fingerprint)));
        }
    }

    ReviewEffectivenessSummary {
        review_count: selected_runs.len() as u64,
        finding_count: feedback.eligible,
        findings_by_severity: counts_to_vec(severity_counts),
        findings_by_category: counts_to_vec(category_counts),
        feedback: feedback.finish(),
        time_to_first_review: first_review_latency(&all_successful, filter),
    }
}

pub(crate) fn validate_review_effectiveness_filter(
    filter: &ReviewEffectivenessFilter,
) -> Result<(), ReviewEffectivenessError> {
    if filter.tenant_id.trim().is_empty() || filter.tenant_id != filter.tenant_id.trim() {
        return Err(ReviewEffectivenessError::InvalidFilter(
            "`tenantId` must be non-empty without surrounding whitespace",
        ));
    }
    if filter.repo.is_some() && filter.workspace.is_none() {
        return Err(ReviewEffectivenessError::InvalidFilter(
            "`repo` requires a `workspace` filter",
        ));
    }
    for (name, value) in [
        ("workspace", filter.workspace.as_deref()),
        ("repo", filter.repo.as_deref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty() || value != value.trim()) {
            return Err(ReviewEffectivenessError::InvalidFilter(match name {
                "workspace" => "`workspace` must be non-empty without surrounding whitespace",
                _ => "`repo` must be non-empty without surrounding whitespace",
            }));
        }
    }
    if filter.from_ms.is_some_and(|value| value < 0) || filter.to_ms.is_some_and(|value| value < 0)
    {
        return Err(ReviewEffectivenessError::InvalidFilter(
            "time filters must be non-negative Unix milliseconds",
        ));
    }
    if matches!((filter.from_ms, filter.to_ms), (Some(from), Some(to)) if from >= to) {
        return Err(ReviewEffectivenessError::InvalidFilter(
            "`fromMs` must be less than `toMs`",
        ));
    }
    Ok(())
}

fn validate_run(run: &ReviewEffectivenessRun) -> Result<(), ReviewEffectivenessError> {
    for (name, value) in [
        ("tenantId", run.tenant_id.as_str()),
        ("workspace", run.workspace.as_str()),
        ("repo", run.repo.as_str()),
        ("runId", run.run_id.as_str()),
    ] {
        if value.trim().is_empty() || value != value.trim() {
            return Err(ReviewEffectivenessError::InvalidRun(format!(
                "`{name}` must be non-empty without surrounding whitespace"
            )));
        }
    }
    if run.pr_id == 0 {
        return Err(ReviewEffectivenessError::InvalidRun(
            "`prId` must be a positive integer".to_string(),
        ));
    }
    if run.created_at_ms < 0
        || run
            .finished_at_ms
            .is_some_and(|finished| finished < run.created_at_ms)
    {
        return Err(ReviewEffectivenessError::InvalidRun(
            "timestamps must be non-negative and finish at or after creation".to_string(),
        ));
    }
    let mut fingerprints = HashSet::new();
    for finding in &run.findings {
        if finding.fingerprint.trim().is_empty()
            || finding.fingerprint != finding.fingerprint.trim()
        {
            return Err(ReviewEffectivenessError::InvalidRun(
                "finding fingerprints must be non-empty without surrounding whitespace".to_string(),
            ));
        }
        if !fingerprints.insert(finding.fingerprint.as_str()) {
            return Err(ReviewEffectivenessError::InvalidRun(format!(
                "finding fingerprint `{}` occurs more than once in run `{}`",
                finding.fingerprint, run.run_id
            )));
        }
    }
    Ok(())
}

fn latest_feedback_by_finding(
    events: &[ReviewFindingFeedbackEvent],
    filter: &ReviewEffectivenessFilter,
) -> Result<HashMap<FindingKey, ReviewFindingFeedbackAction>, ReviewEffectivenessError> {
    let mut latest = HashMap::<FindingKey, (&ReviewFindingFeedbackEvent, i64)>::new();
    for event in events
        .iter()
        .filter(|event| feedback_matches_filter(event, filter))
    {
        let occurred_at = event.occurred_at.parse::<i64>().map_err(|_| {
            ReviewEffectivenessError::InvalidFeedback(
                "`occurredAt` must be Unix milliseconds".to_string(),
            )
        })?;
        if filter.to_ms.is_some_and(|to| occurred_at >= to) {
            continue;
        }
        let key = feedback_key(
            &event.identity,
            &event.review_run_id,
            &event.finding_fingerprint,
        );
        let replace = latest.get(&key).is_none_or(|(current, timestamp)| {
            occurred_at > *timestamp
                || (occurred_at == *timestamp && event.event_id > current.event_id)
        });
        if replace {
            latest.insert(key, (event, occurred_at));
        }
    }
    Ok(latest
        .into_iter()
        .map(|(key, (event, _))| (key, event.action))
        .collect())
}

fn feedback_matches_filter(
    event: &ReviewFindingFeedbackEvent,
    filter: &ReviewEffectivenessFilter,
) -> bool {
    event.identity.tenant_id == filter.tenant_id
        && filter
            .provider
            .is_none_or(|provider| event.identity.provider == provider)
        && filter
            .workspace
            .as_deref()
            .is_none_or(|workspace| event.identity.workspace == workspace)
        && filter
            .repo
            .as_deref()
            .is_none_or(|repo| event.identity.repo == repo)
}

fn run_matches_repository_filter(
    run: &ReviewEffectivenessRun,
    filter: &ReviewEffectivenessFilter,
) -> bool {
    filter
        .provider
        .is_none_or(|provider| run.provider == provider)
        && filter
            .workspace
            .as_deref()
            .is_none_or(|workspace| run.workspace == workspace)
        && filter.repo.as_deref().is_none_or(|repo| run.repo == repo)
}

fn run_matches_repository(run: &ReviewEffectivenessRun, key: &RepositoryKey) -> bool {
    run.provider == key.0 && run.workspace == key.1 && run.repo == key.2
}

fn timestamp_in_filter(timestamp: i64, filter: &ReviewEffectivenessFilter) -> bool {
    filter.from_ms.is_none_or(|from| timestamp >= from)
        && filter.to_ms.is_none_or(|to| timestamp < to)
}

fn finding_key(run: &ReviewEffectivenessRun, fingerprint: &str) -> FindingKey {
    feedback_key(
        &ReviewFindingFeedbackIdentity {
            tenant_id: run.tenant_id.clone(),
            provider: run.provider,
            workspace: run.workspace.clone(),
            repo: run.repo.clone(),
            pr_id: run.pr_id,
        },
        &run.run_id,
        fingerprint,
    )
}

fn feedback_key(
    identity: &ReviewFindingFeedbackIdentity,
    run_id: &str,
    fingerprint: &str,
) -> FindingKey {
    (
        identity.provider,
        identity.workspace.clone(),
        identity.repo.clone(),
        identity.pr_id,
        run_id.to_string(),
        fingerprint.to_string(),
    )
}

fn first_review_latency(
    runs: &[&ReviewEffectivenessRun],
    filter: &ReviewEffectivenessFilter,
) -> ReviewLatencyMetrics {
    let mut first_by_pr = HashMap::<PullRequestKey, &ReviewEffectivenessRun>::new();
    for run in runs {
        let Some(finished) = run.finished_at_ms else {
            continue;
        };
        let key = (
            run.provider,
            run.workspace.clone(),
            run.repo.clone(),
            run.pr_id,
        );
        let replace = first_by_pr.get(&key).is_none_or(|current| {
            let current_finished = current
                .finished_at_ms
                .expect("first-review entries have completion times");
            finished < current_finished
                || (finished == current_finished && run.run_id < current.run_id)
        });
        if replace {
            first_by_pr.insert(key, run);
        }
    }
    let mut latencies = first_by_pr
        .into_values()
        .filter_map(|run| {
            let finished = run.finished_at_ms?;
            timestamp_in_filter(finished, filter)
                .then(|| u64::try_from(finished - run.created_at_ms).ok())
                .flatten()
        })
        .collect::<Vec<_>>();
    latencies.sort_unstable();
    let total_ms = latencies.iter().copied().sum::<u64>();
    let sample_count = latencies.len() as u64;
    let median_ms = if latencies.is_empty() {
        None
    } else if latencies.len() % 2 == 1 {
        Some(latencies[latencies.len() / 2])
    } else {
        let upper = latencies.len() / 2;
        Some(
            ((u128::from(latencies[upper - 1]) + u128::from(latencies[upper])) / 2)
                .try_into()
                .expect("median of two u64 values fits in u64"),
        )
    };
    ReviewLatencyMetrics {
        sample_count,
        total_ms,
        average_ms: (sample_count > 0).then_some(total_ms / sample_count.max(1)),
        median_ms,
        minimum_ms: latencies.iter().copied().min(),
        maximum_ms: latencies.iter().copied().max(),
    }
}

fn zero_counts<'a>(keys: impl Iterator<Item = &'a str>) -> BTreeMap<String, u64> {
    keys.map(|key| (key.to_string(), 0)).collect()
}

fn counts_to_vec(counts: BTreeMap<String, u64>) -> Vec<ReviewMetricCount> {
    counts
        .into_iter()
        .map(|(key, count)| ReviewMetricCount { key, count })
        .collect()
}

fn rate(numerator: u64, denominator: u64) -> ReviewMetricRate {
    ReviewMetricRate {
        numerator,
        denominator,
        basis_points: (denominator > 0).then(|| {
            let scaled = u128::from(numerator) * 10_000 / u128::from(denominator);
            u32::try_from(scaled).unwrap_or(10_000)
        }),
    }
}

#[derive(Default)]
struct ReviewFeedbackAccumulator {
    eligible: u64,
    with_feedback: u64,
    accepted: u64,
    false_positive: u64,
    fixed: u64,
    dismissed: u64,
    reopened: u64,
}

impl ReviewFeedbackAccumulator {
    fn record(&mut self, action: Option<&ReviewFindingFeedbackAction>) {
        self.eligible += 1;
        let Some(action) = action else {
            return;
        };
        self.with_feedback += 1;
        match action {
            ReviewFindingFeedbackAction::Accepted => self.accepted += 1,
            ReviewFindingFeedbackAction::Fixed => {
                self.accepted += 1;
                self.fixed += 1;
            }
            ReviewFindingFeedbackAction::FalsePositive => self.false_positive += 1,
            ReviewFindingFeedbackAction::Dismissed => self.dismissed += 1,
            ReviewFindingFeedbackAction::Reopened => self.reopened += 1,
        }
    }

    fn finish(self) -> ReviewFeedbackMetrics {
        ReviewFeedbackMetrics {
            eligible_findings: self.eligible,
            findings_with_feedback: self.with_feedback,
            findings_without_feedback: self.eligible.saturating_sub(self.with_feedback),
            accepted_findings: self.accepted,
            false_positive_findings: self.false_positive,
            fixed_findings: self.fixed,
            dismissed_findings: self.dismissed,
            reopened_findings: self.reopened,
            coverage_rate: rate(self.with_feedback, self.eligible),
            acceptance_rate: rate(self.accepted, self.eligible),
            false_positive_rate: rate(self.false_positive, self.eligible),
            fixed_rate: rate(self.fixed, self.eligible),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review_feedback::ReviewFindingFeedbackEvent;

    fn run(
        tenant: &str,
        repo: &str,
        pr_id: u64,
        run_id: &str,
        created_at_ms: i64,
        finished_at_ms: i64,
        findings: Vec<(&str, ReviewMetricSeverity, ReviewMetricCategory)>,
    ) -> ReviewEffectivenessRun {
        ReviewEffectivenessRun {
            tenant_id: tenant.to_string(),
            provider: PullRequestReviewEventProvider::Github,
            workspace: "acme".to_string(),
            repo: repo.to_string(),
            pr_id,
            run_id: run_id.to_string(),
            status: ReviewEffectivenessRunStatus::Succeeded,
            created_at_ms,
            finished_at_ms: Some(finished_at_ms),
            findings: findings
                .into_iter()
                .map(
                    |(fingerprint, severity, category)| ReviewEffectivenessFinding {
                        fingerprint: fingerprint.to_string(),
                        severity,
                        category,
                    },
                )
                .collect(),
        }
    }

    fn feedback(
        tenant: &str,
        repo: &str,
        pr_id: u64,
        run_id: &str,
        fingerprint: &str,
        action: ReviewFindingFeedbackAction,
        occurred_at: i64,
    ) -> ReviewFindingFeedbackEvent {
        ReviewFindingFeedbackEvent {
            event_id: format!("{run_id}-{fingerprint}-{occurred_at}"),
            identity: ReviewFindingFeedbackIdentity {
                tenant_id: tenant.to_string(),
                provider: PullRequestReviewEventProvider::Github,
                workspace: "acme".to_string(),
                repo: repo.to_string(),
                pr_id,
            },
            review_run_id: run_id.to_string(),
            finding_fingerprint: fingerprint.to_string(),
            action,
            occurred_at: occurred_at.to_string(),
            actor_id: "reviewer-1".to_string(),
            reason: None,
        }
    }

    #[test]
    fn zero_data_report_has_stable_empty_denominators() {
        let report = aggregate_review_effectiveness(&[], &[], ReviewEffectivenessFilter::default())
            .expect("empty report");

        assert_eq!(report.schema_version, REVIEW_EFFECTIVENESS_SCHEMA_VERSION);
        assert_eq!(report.summary.review_count, 0);
        assert_eq!(report.summary.finding_count, 0);
        assert_eq!(report.summary.feedback.coverage_rate.denominator, 0);
        assert_eq!(report.summary.feedback.coverage_rate.basis_points, None);
        assert_eq!(report.summary.time_to_first_review.sample_count, 0);
        assert_eq!(report.summary.time_to_first_review.median_ms, None);
        assert!(report.repositories.is_empty());
    }

    #[test]
    fn partial_feedback_keeps_missing_findings_in_every_denominator() {
        let runs = vec![run(
            "tenant-acme",
            "payments",
            42,
            "run-1",
            1_000,
            1_500,
            vec![
                (
                    "accepted",
                    ReviewMetricSeverity::High,
                    ReviewMetricCategory::Bug,
                ),
                (
                    "fixed",
                    ReviewMetricSeverity::Medium,
                    ReviewMetricCategory::Security,
                ),
                (
                    "missing",
                    ReviewMetricSeverity::Low,
                    ReviewMetricCategory::Docs,
                ),
            ],
        )];
        let feedback_events = vec![
            feedback(
                "tenant-acme",
                "payments",
                42,
                "run-1",
                "accepted",
                ReviewFindingFeedbackAction::Accepted,
                1_600,
            ),
            feedback(
                "tenant-acme",
                "payments",
                42,
                "run-1",
                "fixed",
                ReviewFindingFeedbackAction::Fixed,
                1_700,
            ),
        ];
        let report = aggregate_review_effectiveness(
            &runs,
            &feedback_events,
            ReviewEffectivenessFilter {
                tenant_id: "tenant-acme".to_string(),
                ..ReviewEffectivenessFilter::default()
            },
        )
        .expect("partial report");

        assert_eq!(report.summary.finding_count, 3);
        assert_eq!(report.summary.feedback.findings_with_feedback, 2);
        assert_eq!(report.summary.feedback.findings_without_feedback, 1);
        assert_eq!(report.summary.feedback.accepted_findings, 2);
        assert_eq!(report.summary.feedback.fixed_findings, 1);
        assert_eq!(
            report.summary.feedback.acceptance_rate,
            ReviewMetricRate {
                numerator: 2,
                denominator: 3,
                basis_points: Some(6_666),
            }
        );
    }

    #[test]
    fn filters_tenants_repositories_and_completion_ranges_deterministically() {
        let runs = vec![
            run("tenant-acme", "catalog", 1, "run-catalog", 100, 200, vec![]),
            run(
                "tenant-acme",
                "payments",
                2,
                "run-payments-first",
                200,
                400,
                vec![],
            ),
            run(
                "tenant-acme",
                "payments",
                2,
                "run-payments-second",
                500,
                700,
                vec![],
            ),
            run("tenant-other", "payments", 3, "run-other", 100, 300, vec![]),
        ];
        let filter = ReviewEffectivenessFilter {
            tenant_id: "tenant-acme".to_string(),
            workspace: Some("acme".to_string()),
            repo: Some("payments".to_string()),
            from_ms: Some(300),
            to_ms: Some(800),
            ..ReviewEffectivenessFilter::default()
        };
        let report = aggregate_review_effectiveness(&runs, &[], filter).expect("filtered report");

        assert_eq!(report.summary.review_count, 2);
        assert_eq!(report.repositories.len(), 1);
        assert_eq!(report.repositories[0].repo, "payments");
        assert_eq!(report.summary.time_to_first_review.sample_count, 1);
        assert_eq!(report.summary.time_to_first_review.average_ms, Some(200));
        assert_eq!(report.summary.time_to_first_review.median_ms, Some(200));
    }

    #[test]
    fn first_review_latency_reports_the_integer_median() {
        let runs = vec![
            run(
                "tenant-acme",
                "payments",
                1,
                "run-first",
                1_000,
                1_500,
                vec![],
            ),
            run(
                "tenant-acme",
                "payments",
                2,
                "run-second",
                2_000,
                2_700,
                vec![],
            ),
        ];

        let report = aggregate_review_effectiveness(
            &runs,
            &[],
            ReviewEffectivenessFilter {
                tenant_id: "tenant-acme".to_string(),
                ..ReviewEffectivenessFilter::default()
            },
        )
        .expect("latency report");

        assert_eq!(report.summary.time_to_first_review.sample_count, 2);
        assert_eq!(report.summary.time_to_first_review.median_ms, Some(600));
    }

    #[test]
    fn json_schema_has_stable_names_and_rejects_unknown_fields() {
        let report = aggregate_review_effectiveness(&[], &[], ReviewEffectivenessFilter::default())
            .expect("report");
        let value = serde_json::to_value(&report).expect("serialize report");
        assert_eq!(value["schemaVersion"], REVIEW_EFFECTIVENESS_SCHEMA_VERSION);
        assert_eq!(
            value
                .as_object()
                .expect("report object")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["filter", "repositories", "schemaVersion", "summary"]
        );
        assert_eq!(
            value["summary"]
                .as_object()
                .expect("summary object")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec![
                "feedback",
                "findingCount",
                "findingsByCategory",
                "findingsBySeverity",
                "reviewCount",
                "timeToFirstReview",
            ]
        );
        assert!(value["summary"]["feedback"]["coverageRate"]["numerator"].is_u64());
        assert!(value["summary"]["timeToFirstReview"]["sampleCount"].is_u64());
        assert!(value["summary"]["timeToFirstReview"]["medianMs"].is_null());
        assert!(serde_json::from_value::<ReviewEffectivenessReport>(value).is_ok());
    }

    #[test]
    fn malformed_out_of_scope_data_cannot_affect_the_selected_report() {
        let mut other_run = run(
            "tenant-other",
            "payments",
            42,
            "run-other",
            1_000,
            1_500,
            vec![],
        );
        other_run.run_id = " invalid ".to_string();
        let mut other_feedback = feedback(
            "tenant-other",
            "payments",
            42,
            "run-other",
            "finding-other",
            ReviewFindingFeedbackAction::Accepted,
            1_600,
        );
        other_feedback.actor_id.clear();
        let mut other_repo_run = run(
            "tenant-acme",
            "catalog",
            7,
            "run-catalog",
            1_000,
            1_500,
            vec![],
        );
        other_repo_run.run_id = " invalid ".to_string();
        let mut other_repo_feedback = feedback(
            "tenant-acme",
            "catalog",
            7,
            "run-catalog",
            "finding-catalog",
            ReviewFindingFeedbackAction::Accepted,
            1_600,
        );
        other_repo_feedback.actor_id.clear();

        let report = aggregate_review_effectiveness(
            &[other_run, other_repo_run],
            &[other_feedback, other_repo_feedback],
            ReviewEffectivenessFilter {
                tenant_id: "tenant-acme".to_string(),
                workspace: Some("acme".to_string()),
                repo: Some("payments".to_string()),
                ..ReviewEffectivenessFilter::default()
            },
        )
        .expect("other tenant data is outside the report boundary");

        assert_eq!(report.summary.review_count, 0);
        assert!(report.repositories.is_empty());
    }

    #[test]
    fn duplicate_fingerprints_in_one_run_are_rejected() {
        let duplicate = run(
            "tenant-acme",
            "payments",
            42,
            "run-duplicate",
            1_000,
            1_500,
            vec![
                (
                    "finding-duplicate",
                    ReviewMetricSeverity::High,
                    ReviewMetricCategory::Bug,
                ),
                (
                    "finding-duplicate",
                    ReviewMetricSeverity::Low,
                    ReviewMetricCategory::Docs,
                ),
            ],
        );

        let error = aggregate_review_effectiveness(
            &[duplicate],
            &[],
            ReviewEffectivenessFilter {
                tenant_id: "tenant-acme".to_string(),
                ..ReviewEffectivenessFilter::default()
            },
        )
        .expect_err("duplicate fingerprints are ambiguous");

        assert!(error.to_string().contains("occurs more than once"));
    }
}
