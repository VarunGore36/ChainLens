use std::time::Duration;

use rand::Rng;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl RetryPolicy {
    pub fn rpc_default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(8),
        }
    }

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
        let a = policy.delay_for_attempt(1);
        let b = policy.delay_for_attempt(1);
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
