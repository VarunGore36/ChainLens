use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ConnectInfo;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

type GovernorRateLimiter = governor::RateLimiter<
    governor::state::NotKeyed,
    governor::state::InMemoryState,
    governor::clock::DefaultClock,
>;

const MIN_RATE: NonZeroU32 = match NonZeroU32::new(1) {
    Some(v) => v,
    None => unreachable!(),
};

#[derive(Clone)]
pub struct ApiRateLimit {
    limiter: Arc<GovernorRateLimiter>,
}

impl fmt::Debug for ApiRateLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiRateLimit").finish()
    }
}

impl ApiRateLimit {
    pub fn new(requests_per_second: u32) -> Self {
        let safe_limit = requests_per_second.max(1);
        let quota = Quota::per_second(NonZeroU32::new(safe_limit).unwrap_or(MIN_RATE));
        Self {
            limiter: Arc::new(RateLimiter::direct(quota)),
        }
    }

    pub async fn middleware(
        ConnectInfo(_addr): ConnectInfo<SocketAddr>,
        axum::extract::State(limit): axum::extract::State<Self>,
        request: axum::extract::Request,
        next: Next,
    ) -> Response {
        if limit.limiter.check().is_err() {
            let mut response = Response::new(axum::body::Body::from("rate limit exceeded"));
            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            return response;
        }

        next.run(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_creation() {
        let limit = ApiRateLimit::new(10);
        assert!(limit.limiter.check().is_ok());
    }

    #[test]
    fn rate_limit_zero_becomes_one() {
        let limit = ApiRateLimit::new(0);
        assert!(limit.limiter.check().is_ok());
    }
}
