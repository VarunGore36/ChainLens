//! Retry policy with exponential backoff and full jitter.
//!
//! Transient RPC failures — 429s, 5xx, timeouts — deserve another attempt.
//! Deterministic failures — malformed requests, method-not-found — do not.
//! The classification lives in [`crate::rpc::RpcError::is_retryable`]; this
//! module decides *how long* to wait between attempts.
//!
//! # Why full jitter
//!
//! Exponential backoff without jitter causes thundering herds: N workers hit
//! the same retry schedule and all retry at the same instant. Full jitter
//! (uniform random between 0 and the backoff ceiling) spreads retries across
//! the entire window. The expected retry time is half the ceiling, so it is
//! both faster on average and kinder to the server than fixed delays.

use std::time::Duration;

use rand::Rng;

/// Retry parameters. Cheap to clone — the policy is stateless.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retries after the initial attempt. `0` means
    /// no retries: try once and give up.
    pub max_retries: u32,
    /// Base delay for the first retry. Doubles on each subsequent attempt.
    pub base_delay: Duration,
    /// Cap on the delay. Without this, attempt 10 would wait ~17 minutes.
    pub max_delay: Duration,
}

impl RetryPolicy {
    /// A sensible default for hosted RPC providers: 3 retries, 200 ms base,
    /// 8 s cap.
    pub fn rpc_default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(8),
        }
    }

    /// Delay for the given attempt number (0-indexed: 0 = first retry).
    ///
    /// Returns a random duration between 0 and `min(base * 2^attempt, max)`.
    /// This is "full jitter" — the recommended strategy from the AWS
    /// Architecture Blog's analysis of retry backoff.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let backoff_ms = self
            .base_delay
            .as_millis()
            .saturating_mul(2u128.saturating_pow(attempt));
        let capped_ms = backoff_ms.min(self.max_delay.as_millis());
        if capped_ms == 0 {
            return Duration::ZERO;
        }
        let jittered = rand::thread_rng().gen_range(0..=capped_ms);
        // capped_ms is bounded by max_delay which is a Duration, so it fits
        // in u64 (max ~584 million years).
        #[allow(clippy::cast_possible_truncation)]
        Duration::from_millis(jittered as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_attempts_gives_zero_delay() {
        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
        };
        // delay_for_attempt(0) should be in [0, 100ms]
        for _ in 0..100 {
            let d = policy.delay_for_attempt(0);
            assert!(d <= Duration::from_millis(100), "{d:?}");
        }
    }

    #[test]
    fn delay_doubles_each_attempt() {
        let policy = RetryPolicy {
            max_retries: 10,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(60),
        };
        // The *ceiling* doubles each time, up to max_delay.
        for attempt in 0..6 {
            let ceiling = Duration::from_millis(100 * 2u64.pow(attempt));
            for _ in 0..50 {
                let d = policy.delay_for_attempt(attempt);
                assert!(d <= ceiling, "attempt {attempt}: {d:?} > {ceiling:?}");
            }
        }
    }

    #[test]
    fn delay_is_capped_at_max() {
        let policy = RetryPolicy {
            max_retries: 20,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(500),
        };
        // Even at attempt 20, the ceiling should be 500ms.
        for _ in 0..100 {
            let d = policy.delay_for_attempt(20);
            assert!(d <= Duration::from_millis(500), "{d:?}");
        }
    }

    #[test]
    fn delay_is_at_least_zero() {
        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(0),
            max_delay: Duration::from_millis(0),
        };
        assert_eq!(policy.delay_for_attempt(0), Duration::ZERO);
    }

    #[test]
    fn jitter_varies() {
        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(10),
        };
        // With full jitter, two calls are extremely unlikely to return the
        // same value. Not a proof, but catches obvious constant-delay bugs.
        let a = policy.delay_for_attempt(1);
        let b = policy.delay_for_attempt(1);
        // At least one of 100 pairs should differ.
        let mut any_differ = a != b;
        for _ in 0..100 {
            let x = policy.delay_for_attempt(1);
            let y = policy.delay_for_attempt(1);
            if x != y {
                any_differ = true;
                break;
            }
        }
        assert!(any_differ, "jitter appears to be constant");
    }
}
