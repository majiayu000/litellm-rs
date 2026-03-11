//! Rate limiting middleware

use crate::core::rate_limiter::get_global_rate_limiter;
use crate::server::state::AppState;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::{HttpResponse, ResponseError};
use dashmap::DashMap;
use futures::future::{Ready, ready};
use sha2::{Digest, Sha256};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Fallback per-key tracker for sliding window when global rate limiter is unavailable
struct KeyTracker {
    timestamps: Vec<Instant>,
}

impl KeyTracker {
    fn new() -> Self {
        Self {
            timestamps: Vec::new(),
        }
    }

    /// Check-and-record atomically: returns (allowed, retry_after_secs)
    fn check_and_record(&mut self, limit: u32, window: Duration) -> (bool, u64) {
        let now = Instant::now();
        // Evict timestamps outside the window
        self.timestamps
            .retain(|&ts| now.duration_since(ts) < window);

        let count = self.timestamps.len() as u32;
        if count >= limit {
            // Estimate when the oldest entry expires
            let retry_after = self
                .timestamps
                .first()
                .map(|&ts| {
                    let age = now.duration_since(ts);
                    window.saturating_sub(age).as_secs().max(1)
                })
                .unwrap_or(window.as_secs());
            (false, retry_after)
        } else {
            self.timestamps.push(now);
            (true, 0)
        }
    }
}

/// Lightweight in-process rate limit error for 429 responses
#[derive(Debug)]
struct RateLimitError {
    retry_after: u64,
    limit: u32,
}

impl fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Too Many Requests")
    }
}

impl ResponseError for RateLimitError {
    fn status_code(&self) -> StatusCode {
        StatusCode::TOO_MANY_REQUESTS
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", self.retry_after.to_string()))
            .insert_header(("X-RateLimit-Limit", self.limit.to_string()))
            .json(serde_json::json!({
                "error": {
                    "message": "Rate limit exceeded. Please retry after the indicated seconds.",
                    "type": "rate_limit_error",
                    "code": 429
                }
            }))
    }
}

/// Rate limit middleware for Actix-web
pub struct RateLimitMiddleware {
    requests_per_minute: u32,
}

impl RateLimitMiddleware {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute,
        }
    }
}

impl Default for RateLimitMiddleware {
    fn default() -> Self {
        Self::new(60)
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = RateLimitMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddlewareService {
            service,
            requests_per_minute: self.requests_per_minute,
            fallback_store: Arc::new(DashMap::new()),
        }))
    }
}

/// Service implementation for rate limit middleware
pub struct RateLimitMiddlewareService<S> {
    service: S,
    requests_per_minute: u32,
    /// Fallback in-process store used when the global rate limiter is not initialized
    fallback_store: Arc<DashMap<String, KeyTracker>>,
}

/// Extract a client identifier from the request.
///
/// Priority:
/// 1. `Authorization` header value (API key / Bearer token) — identifies the authenticated caller
/// 2. `X-Forwarded-For` first address
/// 3. Direct peer IP from connection info
fn extract_client_key(req: &ServiceRequest) -> String {
    if let Some(auth) = req.headers().get("Authorization")
        && let Ok(val) = auth.to_str()
    {
        // Hash the token so raw secrets never reside in memory as map keys
        let hash = Sha256::digest(val.as_bytes());
        return format!("auth:{:x}", hash);
    }

    let conn = req.connection_info();
    if let Some(forwarded) = req.headers().get("X-Forwarded-For")
        && let Ok(val) = forwarded.to_str()
    {
        let first = val.split(',').next().unwrap_or(val).trim();
        if !first.is_empty() {
            return first.to_string();
        }
    }

    conn.peer_addr().unwrap_or("unknown").to_string()
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let _app_state = req.app_data::<web::Data<AppState>>().cloned();
        let requests_per_minute = self.requests_per_minute;
        let start_time = Instant::now();
        let path = req.path().to_string();
        let method = req.method().to_string();

        let client_key = extract_client_key(&req);

        // --- Try global rate limiter first ---
        if let Some(global_limiter) = get_global_rate_limiter() {
            let limit = global_limiter.limit();
            // service.call() returns a lazy future; it only executes on .await.
            // We must call it here because it consumes `req`, but we will NOT
            // await it if the rate check fails — so no downstream work is wasted.
            let fut = self.service.call(req);
            let key = client_key.clone();

            return Box::pin(async move {
                let result = global_limiter.check_and_record(&key).await;

                if !result.allowed {
                    let retry_after = result.retry_after_secs.unwrap_or(60);
                    warn!(
                        client = %key,
                        path = %path,
                        "Rate limit exceeded (global limiter): retry after {}s",
                        retry_after
                    );
                    let err = RateLimitError { retry_after, limit };
                    return Err(actix_web::Error::from(err));
                }

                debug!(
                    client = %key,
                    remaining = result.remaining,
                    "Rate limit check passed (global limiter)"
                );

                let res = fut.await?;
                let duration = start_time.elapsed();
                info!(
                    "{} {} completed in {:?} with status {}",
                    method,
                    path,
                    duration,
                    res.status()
                );
                Ok(res)
            });
        }

        // --- Fallback: in-process sliding window using middleware's requests_per_minute ---
        let fallback_store = self.fallback_store.clone();
        let fut = self.service.call(req);
        let key = client_key.clone();

        Box::pin(async move {
            let window = Duration::from_secs(60);
            let (allowed, retry_after) = {
                let mut tracker = fallback_store
                    .entry(key.clone())
                    .or_insert_with(KeyTracker::new);
                tracker.check_and_record(requests_per_minute, window)
            };

            if !allowed {
                warn!(
                    client = %key,
                    path = %path,
                    "Rate limit exceeded (fallback limiter): retry after {}s",
                    retry_after
                );
                let err = RateLimitError {
                    retry_after,
                    limit: requests_per_minute,
                };
                return Err(actix_web::Error::from(err));
            }

            debug!(
                client = %key,
                limit = requests_per_minute,
                "Rate limit check passed (fallback limiter)"
            );

            let res = fut.await?;
            let duration = start_time.elapsed();
            info!(
                "{} {} completed in {:?} with status {}",
                method,
                path,
                duration,
                res.status()
            );
            Ok(res)
        })
    }
}
