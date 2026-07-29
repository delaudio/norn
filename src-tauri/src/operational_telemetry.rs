//! Bounded, provider-neutral operational telemetry for automated reviews.
//!
//! Repository and tenant identifiers, prompts, findings, and provider payloads
//! are intentionally never emitted as metric labels.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::review_event::PullRequestReviewEventProvider;

const TRACE_CAPACITY: usize = 1_024;
const HISTOGRAM_BUCKETS_MS: [u64; 7] = [10, 100, 1_000, 10_000, 60_000, 300_000, 900_000];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationalTrace {
    pub correlation_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub job_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationOutcome {
    Completed,
    Failed,
}

#[derive(Debug, Clone, Default)]
pub struct OperationalTelemetry {
    state: Arc<Mutex<TelemetryState>>,
}

#[derive(Debug, Default)]
struct TelemetryState {
    counters: BTreeMap<MetricKey, u64>,
    histograms: BTreeMap<HistogramKey, Histogram>,
    traces: BTreeMap<String, OperationalTrace>,
    trace_order: VecDeque<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MetricKey {
    name: &'static str,
    provider: &'static str,
    outcome: &'static str,
    repository_scope: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct HistogramKey {
    name: &'static str,
    provider: &'static str,
    repository_scope: &'static str,
}

#[derive(Debug, Clone)]
struct Histogram {
    buckets: [u64; HISTOGRAM_BUCKETS_MS.len()],
    sum_ms: u64,
    count: u64,
}

impl Default for Histogram {
    fn default() -> Self {
        Self {
            buckets: [0; HISTOGRAM_BUCKETS_MS.len()],
            sum_ms: 0,
            count: 0,
        }
    }
}

impl OperationalTelemetry {
    pub fn correlation_for_delivery(&self, delivery_id: &str) -> String {
        correlation_id(delivery_id)
    }

    pub fn record_received(&self, provider: PullRequestReviewEventProvider) {
        self.increment("lachesi_review_events_received_total", provider, "received");
    }

    pub fn record_queued(
        &self,
        provider: PullRequestReviewEventProvider,
        delivery_id: &str,
        job_id: &str,
    ) -> String {
        self.increment("lachesi_review_jobs_total", provider, "queued");
        let correlation_id = self.correlation_for_delivery(delivery_id);
        let trace = OperationalTrace {
            correlation_id: correlation_id.clone(),
            provider,
            job_id: job_id.to_string(),
        };
        if let Ok(mut state) = self.state.lock() {
            if !state.traces.contains_key(job_id) {
                state.trace_order.push_back(job_id.to_string());
            }
            state.traces.insert(job_id.to_string(), trace);
            while state.trace_order.len() > TRACE_CAPACITY {
                if let Some(expired) = state.trace_order.pop_front() {
                    state.traces.remove(&expired);
                }
            }
        }
        correlation_id
    }

    pub fn record_completed(
        &self,
        provider: PullRequestReviewEventProvider,
        queue_wait: Duration,
        review_duration: Duration,
    ) {
        self.increment("lachesi_review_jobs_total", provider, "completed");
        self.observe("lachesi_review_queue_wait_ms", provider, queue_wait);
        self.observe("lachesi_review_duration_ms", provider, review_duration);
    }

    pub fn record_failure(
        &self,
        provider: PullRequestReviewEventProvider,
        queue_wait: Duration,
        review_duration: Duration,
        retrying: bool,
        dead_letter: bool,
    ) {
        self.increment("lachesi_review_jobs_total", provider, "failed");
        if retrying {
            self.increment("lachesi_review_retries_total", provider, "scheduled");
        }
        if dead_letter {
            self.increment(
                "lachesi_review_dead_letter_jobs_total",
                provider,
                "dead_lettered",
            );
        }
        self.observe("lachesi_review_queue_wait_ms", provider, queue_wait);
        self.observe("lachesi_review_duration_ms", provider, review_duration);
    }

    pub fn record_publication(
        &self,
        provider: PullRequestReviewEventProvider,
        outcome: PublicationOutcome,
        duration: Duration,
    ) {
        let outcome = match outcome {
            PublicationOutcome::Completed => "completed",
            PublicationOutcome::Failed => "failed",
        };
        self.increment("lachesi_review_publications_total", provider, outcome);
        self.observe("lachesi_review_publication_duration_ms", provider, duration);
    }

    pub fn trace_for_job(&self, job_id: &str) -> Option<OperationalTrace> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.traces.get(job_id).cloned())
    }

    pub fn prometheus(&self) -> String {
        let Ok(state) = self.state.lock() else {
            return String::new();
        };
        let mut output = String::new();
        for (key, count) in &state.counters {
            output.push_str(&format!(
                "{}{{provider=\"{}\",outcome=\"{}\",repository_scope=\"{}\"}} {}\n",
                key.name, key.provider, key.outcome, key.repository_scope, count
            ));
        }
        for (key, histogram) in &state.histograms {
            for (index, bucket) in HISTOGRAM_BUCKETS_MS.iter().enumerate() {
                output.push_str(&format!(
                    "{}_bucket{{provider=\"{}\",repository_scope=\"{}\",le=\"{}\"}} {}\n",
                    key.name, key.provider, key.repository_scope, bucket, histogram.buckets[index]
                ));
            }
            output.push_str(&format!(
                "{}_bucket{{provider=\"{}\",repository_scope=\"{}\",le=\"+Inf\"}} {}\n",
                key.name, key.provider, key.repository_scope, histogram.count
            ));
            output.push_str(&format!(
                "{}_sum{{provider=\"{}\",repository_scope=\"{}\"}} {}\n",
                key.name, key.provider, key.repository_scope, histogram.sum_ms
            ));
            output.push_str(&format!(
                "{}_count{{provider=\"{}\",repository_scope=\"{}\"}} {}\n",
                key.name, key.provider, key.repository_scope, histogram.count
            ));
        }
        output
    }

    fn increment(
        &self,
        name: &'static str,
        provider: PullRequestReviewEventProvider,
        outcome: &'static str,
    ) {
        if let Ok(mut state) = self.state.lock() {
            *state
                .counters
                .entry(MetricKey {
                    name,
                    provider: provider.as_str(),
                    outcome,
                    repository_scope: "repository",
                })
                .or_default() += 1;
        }
    }

    fn observe(
        &self,
        name: &'static str,
        provider: PullRequestReviewEventProvider,
        duration: Duration,
    ) {
        let milliseconds = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        if let Ok(mut state) = self.state.lock() {
            let histogram = state
                .histograms
                .entry(HistogramKey {
                    name,
                    provider: provider.as_str(),
                    repository_scope: "repository",
                })
                .or_default();
            histogram.count = histogram.count.saturating_add(1);
            histogram.sum_ms = histogram.sum_ms.saturating_add(milliseconds);
            for (index, bucket) in HISTOGRAM_BUCKETS_MS.iter().enumerate() {
                if milliseconds <= *bucket {
                    histogram.buckets[index] = histogram.buckets[index].saturating_add(1);
                }
            }
        }
    }
}

fn correlation_id(delivery_id: &str) -> String {
    let digest = Sha256::digest(delivery_id.as_bytes());
    format!("correlation:{}", hex::encode(&digest[..12]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_job_emits_bounded_redacted_counters_and_timing() {
        let telemetry = OperationalTelemetry::default();
        let correlation = telemetry.record_queued(
            PullRequestReviewEventProvider::Github,
            "delivery-secret-123",
            "job-42",
        );
        telemetry.record_failure(
            PullRequestReviewEventProvider::Github,
            Duration::from_millis(42),
            Duration::from_millis(120),
            true,
            true,
        );
        let metrics = telemetry.prometheus();
        assert!(metrics.contains("lachesi_review_jobs_total{provider=\"github\",outcome=\"queued\",repository_scope=\"repository\"} 1"));
        assert!(metrics.contains("lachesi_review_retries_total"));
        assert!(metrics.contains("lachesi_review_dead_letter_jobs_total"));
        assert!(metrics.contains("lachesi_review_queue_wait_ms_count"));
        assert!(metrics.contains("lachesi_review_duration_ms_count"));
        assert!(!metrics.contains("delivery-secret-123"));
        assert!(!metrics.contains("job-42"));
        assert_eq!(
            telemetry
                .trace_for_job("job-42")
                .expect("trace")
                .correlation_id,
            correlation
        );
    }
}
