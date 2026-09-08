//! Token-bucket rate limiter for outbound RPC requests.
//!
//! Hosted providers meter compute units per second, not raw request count.
//! A single `eth_getBlockByNumber` with full transactions costs far more than
//! `eth_blockNumber`. This limiter caps *request* rate as a baseline; the
//! provider's actual compute-unit budget is the ceiling that Phase 11 measures.
//!
//! # Why governor
//!
//! The `governor` crate implements the GCRA (Generic Cell Rate Algorithm)
//! — a variant of the token-bucket algorithm that is both lock-free and
//! allocation-free on the hot path. It is the standard choice in the Rust
//! ecosystem for rate limiting.

use std::num::NonZeroU32;

use governor::{Quota, RateLimiter};

/// A shared, cloneable rate limiter.
///
/// Each call to [`wait`](Self::wait) blocks until a token is available. The
/// limiter is safe to share across tasks — governor's internal state is
/// `Sync`.
#[derive(Clone, Debug)]
pub struct RateLimit {
    inner: std::sync::Arc<DirectRateLimiter>,
}

type DirectRateLimiter = governor::RateLimiter<
    governor::state::NotKeyed,
    governor::state::InMemoryState,
    governor::clock::DefaultClock,
>;

impl RateLimit {
    /// Create a limiter that allows `requests_per_second` requests per second.
    ///
    /// Panics if `requests_per_second` is 0 — there is no sensible meaning for
    /// a zero-rate limiter. The caller validates the config before reaching
    /// this point.
    pub fn new(requests_per_second: u32) -> Self {
        let quota = Quota::per_second(
            NonZeroU32::new(requests_per_second)
                .unwrap_or_else(|| panic!("rate limit must be > 0, got {requests_per_second}")),
        );
        Self {
            inner: std::sync::Arc::new(RateLimiter::direct(quota)),
        }
    }

    /// Block until a request token is available.
    pub async fn wait(&self) {
        self.inner.until_ready().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn rate_limit_delays_excess_requests() {
        // 10 requests per second — the 11th must wait ~100ms.
        let limiter = RateLimit::new(10);
        let start = Instant::now();
        for _ in 0..11 {
            limiter.wait().await;
        }
        let elapsed = start.elapsed();
        // The 11th request should have waited at least ~50ms (one token
        // interval). With scheduler jitter, 30ms is a safe lower bound.
        assert!(
            elapsed >= Duration::from_millis(30),
            "expected some delay, got {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn requests_within_limit_are_not_delayed() {
        let limiter = RateLimit::new(100);
        let start = Instant::now();
        for _ in 0..10 {
            limiter.wait().await;
        }
        let elapsed = start.elapsed();
        // 10 requests at 100/s should complete in ~90ms + scheduler overhead.
        assert!(
            elapsed < Duration::from_millis(200),
            "expected near-instant, got {elapsed:?}"
        );
    }
}
