//! Deterministic retry policy for durable shared review jobs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewJobFailureClass {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    RetryAt(u64),
    DeadLetter,
}

pub const MAX_ATTEMPTS: u32 = 3;
const BASE_DELAY_MS: u64 = 1_000;
const MAX_DELAY_MS: u64 = 60_000;

pub fn classify_error(code: &str) -> ReviewJobFailureClass {
    match code {
        "provider_rate_limited"
        | "provider_unavailable"
        | "network_timeout"
        | "worker_lease_expired" => ReviewJobFailureClass::Retryable,
        _ => ReviewJobFailureClass::Permanent,
    }
}

/// Computes a bounded exponential delay. `jitter_seed` is persisted job identity
/// material, making the outcome reproducible under a fake clock and stable on
/// process restart without synchronizing every retry.
pub fn retry_decision(
    now_ms: u64,
    attempt_count: u32,
    failure: ReviewJobFailureClass,
    jitter_seed: u64,
) -> RetryDecision {
    if failure == ReviewJobFailureClass::Permanent || attempt_count >= MAX_ATTEMPTS {
        return RetryDecision::DeadLetter;
    }
    let exponent = attempt_count.saturating_sub(1).min(16);
    let delay = BASE_DELAY_MS
        .saturating_mul(1_u64 << exponent)
        .min(MAX_DELAY_MS);
    let jitter = jitter_seed.wrapping_mul(1_103_515_245).wrapping_add(12_345) % (delay / 4 + 1);
    RetryDecision::RetryAt(now_ms.saturating_add(delay.saturating_add(jitter)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_is_deterministic_with_a_fake_clock() {
        assert_eq!(
            retry_decision(10_000, 1, ReviewJobFailureClass::Retryable, 42),
            retry_decision(10_000, 1, ReviewJobFailureClass::Retryable, 42)
        );
        assert!(matches!(
            retry_decision(10_000, 3, ReviewJobFailureClass::Retryable, 42),
            RetryDecision::DeadLetter
        ));
    }
    #[test]
    fn permanent_errors_are_dead_lettered_immediately() {
        assert_eq!(
            classify_error("invalid_policy"),
            ReviewJobFailureClass::Permanent
        );
        assert_eq!(
            retry_decision(0, 1, ReviewJobFailureClass::Permanent, 1),
            RetryDecision::DeadLetter
        );
    }
}
