use std::num::NonZeroU32;

use governor::{Quota, RateLimiter};

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
    pub fn new(requests_per_second: u32) -> Self {
        let quota = Quota::per_second(
            NonZeroU32::new(requests_per_second)
                .unwrap_or_else(|| panic!("rate limit must be > 0, got {requests_per_second}")),
        );
        Self {
            inner: std::sync::Arc::new(RateLimiter::direct(quota)),
        }
    }

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
        let limiter = RateLimit::new(10);
        let start = Instant::now();
        for _ in 0..11 {
            limiter.wait().await;
        }
        let elapsed = start.elapsed();
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
        assert!(
            elapsed < Duration::from_millis(200),
            "expected near-instant, got {elapsed:?}"
        );
    }

    #[test]
    #[should_panic(expected = "rate limit must be > 0")]
    fn rate_limit_zero_panics() {
        let _ = RateLimit::new(0);
    }

    #[tokio::test]
    async fn rate_limit_one_per_second_delays() {
        let limiter = RateLimit::new(1);
        let start = Instant::now();
        limiter.wait().await;
        limiter.wait().await;
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(500),
            "expected ~1s delay for 1 req/s, got {elapsed:?}"
        );
    }
}
