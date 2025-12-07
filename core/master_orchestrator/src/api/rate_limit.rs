use std::sync::Arc;
use std::time::Duration;
use governor::clock::Clock;
use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorTooManyRequests,
    Error, HttpMessage,
};
use dashmap::DashMap;
use futures_util::future::{ready, LocalBoxFuture, Ready};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    RateLimiter,
    Quota,
};
use std::num::NonZeroU32;

/// Rate limiting configuration
#[derive(Clone)]
pub struct RateLimitConfig {
    /// Number of requests allowed per time window
    pub requests: NonZeroU32,
    /// Time window in seconds
    pub window_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            // Default: 100 requests per minute
            requests: NonZeroU32::new(100).unwrap(),
            window_secs: 60,
        }
    }
}

/// Global rate limiter state shared across requests
pub struct RateLimiterState {
    // Map of user IDs to their rate limiters
    limiters: DashMap<String, Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
    config: RateLimitConfig,
}

impl RateLimiterState {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            limiters: DashMap::new(),
            config,
        }
    }

    fn get_limiter(&self, user_id: &str) -> Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>> {
        if let Some(limiter) = self.limiters.get(user_id) {
            limiter.clone()
        } else {
            let quota = Quota::with_period(Duration::from_secs(self.config.window_secs))
                .unwrap()
                .allow_burst(self.config.requests);
            let limiter = Arc::new(RateLimiter::direct(quota));
            self.limiters.insert(user_id.to_string(), limiter.clone());
            limiter
        }
    }
}

#[derive(Clone)]
pub struct RateLimitMiddleware {
    state: Arc<RateLimiterState>,
}

impl RateLimitMiddleware {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            state: Arc::new(RateLimiterState::new(config)),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + Clone + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RateLimitMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddlewareService {
            service,
            state: self.state.clone(),
        }))
    }
}

#[derive(Clone)]
pub struct RateLimitMiddlewareService<S> {
    service: S,
    state: Arc<RateLimiterState>,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + Clone + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Extract user ID from JWT claims if available
        let user_id = req
            .extensions()
            .get::<super::auth::Claims>()
            .map(|claims| claims.sub.clone())
            .unwrap_or_else(|| "anonymous".to_string());

        let limiter = self.state.get_limiter(&user_id);

        // Check rate limit
        match limiter.check() {
            Ok(_) => {
                let fut = self.service.call(req);
                Box::pin(async move { fut.await })
            }
            Err(negative) => {
                let clock = governor::clock::DefaultClock::default();
                let wait_time = negative.wait_time_from(clock.now());
                Box::pin(async move {
                    Err(ErrorTooManyRequests(format!(
                        "Rate limit exceeded. Please try again in {} seconds",
                        wait_time.as_secs()
                    )))
                })
            }
        }
    }
}