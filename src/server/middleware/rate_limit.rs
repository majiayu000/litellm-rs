//! Rate limiting middleware

use crate::core::rate_limiter::get_global_rate_limiter;
use crate::server::state::AppState;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::http::StatusCode;
use actix_web::http::header::HeaderName;
use actix_web::web;
use actix_web::{HttpResponse, ResponseError};
use dashmap::DashMap;
use futures::future::{Ready, ready};
use sha2::{Digest, Sha256};
use std::fmt;
use std::future::Future;
use std::net::SocketAddr;
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

/// Extract the IP address (without port) from a peer address string.
///
/// Handles IPv4 (`1.2.3.4:5678` → `1.2.3.4`) and IPv6 (`[::1]:5678` → `::1`).
fn parse_peer_ip(peer: &str) -> String {
    peer.parse::<SocketAddr>()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| peer.to_string())
}

/// Extract a client identifier from the request.
///
/// Priority:
/// 1. Configured API key header value
/// 2. `Authorization` header value (API key / Bearer token)
/// 3. `X-Forwarded-For` first address — only when peer IP is in `trusted_proxies`
/// 4. Direct peer address from connection info
fn extract_client_key(
    req: &ServiceRequest,
    trusted_proxies: &[String],
    api_key_header: &str,
) -> String {
    let auth_token = header_value(req.headers(), api_key_header)
        .or_else(|| header_value(req.headers(), "Authorization"));

    if let Some(token) = auth_token {
        // Hash the token so raw secrets never reside in memory as map keys
        let hash = Sha256::digest(token.as_bytes());
        return format!("auth:{:x}", hash);
    }

    let conn = req.connection_info();
    let peer = conn.peer_addr().unwrap_or("unknown");
    let peer_ip = parse_peer_ip(peer);

    if trusted_proxies.iter().any(|p| p == &peer_ip)
        && let Some(forwarded) = req.headers().get("X-Forwarded-For")
        && let Ok(val) = forwarded.to_str()
        && let first = val.split(',').next().unwrap_or("").trim()
        && !first.is_empty()
    {
        return first.to_string();
    }

    peer_ip
}

fn header_value(headers: &actix_web::http::header::HeaderMap, header_name: &str) -> Option<String> {
    let header_name = HeaderName::try_from(header_name.trim()).ok()?;
    headers
        .get(&header_name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
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
        let app_state = req.app_data::<web::Data<AppState>>().cloned();
        let (trusted_proxies, api_key_header): (Vec<String>, String) = match app_state.as_ref() {
            Some(state) => {
                let cfg = state.config.load();
                (
                    cfg.server().trusted_proxies.clone(),
                    cfg.auth().api_key_header.clone(),
                )
            }
            None => (Vec::new(), "x-api-key".to_string()),
        };
        let requests_per_minute = self.requests_per_minute;
        let start_time = Instant::now();
        let path = req.path().to_string();
        let method = req.method().to_string();

        let client_key = extract_client_key(&req, &trusted_proxies, &api_key_header);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_peer_ip_ipv4_with_port() {
        assert_eq!(parse_peer_ip("127.0.0.1:1234"), "127.0.0.1");
    }

    #[test]
    fn test_parse_peer_ip_ipv4_no_port() {
        assert_eq!(parse_peer_ip("10.0.0.1"), "10.0.0.1");
    }

    #[test]
    fn test_parse_peer_ip_ipv6_with_port() {
        assert_eq!(parse_peer_ip("[::1]:8080"), "::1");
    }

    #[test]
    fn test_parse_peer_ip_unknown_falls_back() {
        assert_eq!(parse_peer_ip("unknown"), "unknown");
    }

    #[test]
    fn test_trusted_proxy_match() {
        let proxies = ["10.0.0.1".to_string()];
        assert!(proxies.iter().any(|p| p == "10.0.0.1"));
    }

    #[test]
    fn test_trusted_proxy_no_match() {
        let proxies = ["10.0.0.1".to_string()];
        assert!(!proxies.iter().any(|p| p == "10.0.0.2"));
    }

    #[test]
    fn test_trusted_proxy_empty_list() {
        let proxies: Vec<String> = vec![];
        assert!(!proxies.iter().any(|p| p == "127.0.0.1"));
    }
}
