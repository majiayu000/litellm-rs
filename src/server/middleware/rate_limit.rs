//! Rate limiting middleware

use super::helpers::middleware_gateway_error_response;
use super::rate_limit_key_policy::effective_requests_per_minute;
use crate::core::rate_limiter::{RateLimitReservation, get_global_rate_limiter};
use crate::core::types::context::{RequestContext, SharedRequestContext};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::{HttpMessage, HttpResponse, ResponseError};
use dashmap::DashMap;
use futures::future::{Ready, ready};
use std::fmt;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Maximum number of distinct client trackers retained by the fallback store.
///
/// The fallback path runs only when the global rate limiter is not initialized.
/// Without a cap, every distinct client IP creates a new entry that lives for
/// the entire process lifetime, which is a memory-exhaustion vector when an
/// attacker rotates source addresses. The value matches `AuthRateLimiter`'s
/// `DEFAULT_MAX_ENTRIES`.
const MAX_FALLBACK_ENTRIES: usize = 10_000;

static GATEWAY_FALLBACK_STORE: OnceLock<Arc<DashMap<String, KeyTracker>>> = OnceLock::new();

const GLOBAL_LIMITER_SOURCE: &str = "global";
const FALLBACK_LIMITER_SOURCE: &str = "fallback";

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

    fn release(&mut self, recorded_at: Instant) -> bool {
        let Some(position) = self.timestamps.iter().position(|&ts| ts == recorded_at) else {
            return false;
        };

        self.timestamps.remove(position);
        true
    }

    /// Check-and-record atomically: returns (allowed, retry_after_secs, recorded_at)
    fn check_and_record(&mut self, limit: u32, window: Duration) -> (bool, u64, Option<Instant>) {
        let now = Instant::now();
        self.timestamps
            .retain(|&ts| now.duration_since(ts) < window);

        let count = self.timestamps.len() as u32;
        if count >= limit {
            let retry_after = self
                .timestamps
                .first()
                .map(|&ts| {
                    let age = now.duration_since(ts);
                    window.saturating_sub(age).as_secs().max(1)
                })
                .unwrap_or(window.as_secs());
            return (false, retry_after, None);
        }

        self.timestamps.push(now);
        (true, 0, Some(now))
    }
}

struct RateLimitPass {
    source: &'static str,
    limit: u32,
    remaining: u32,
    reservation: RecordedRateLimitReservation,
}

struct RateLimitRejection {
    source: &'static str,
    retry_after: u64,
    limit: u32,
}

enum RateLimitReservationSource {
    Global(RateLimitReservation),
    Fallback {
        store: Arc<DashMap<String, KeyTracker>>,
        recorded_at: Instant,
    },
    Noop,
}

enum RecordedRateLimitReservation {
    Global(RateLimitReservation),
    Fallback(Instant),
}

pub(super) struct AuthRateLimitReservation {
    key: String,
    source: RateLimitReservationSource,
}

impl AuthRateLimitReservation {
    pub(super) fn noop() -> Self {
        Self {
            key: String::new(),
            source: RateLimitReservationSource::Noop,
        }
    }

    pub(super) async fn release(self) {
        match self.source {
            RateLimitReservationSource::Global(record_source) => {
                if let Some(global_limiter) = get_global_rate_limiter() {
                    global_limiter
                        .release_recorded(&self.key, record_source)
                        .await;
                }
            }
            RateLimitReservationSource::Fallback { store, recorded_at } => {
                let _removed_entry = store.remove_if_mut(&self.key, |_, tracker| {
                    tracker.release(recorded_at);
                    tracker.timestamps.is_empty()
                });
            }
            RateLimitReservationSource::Noop => {}
        }
    }
}

/// Evict trackers when the fallback store exceeds the cap.
///
/// Two-pass strategy: first drop trackers whose latest timestamp is already
/// outside the rate-limit window (they would re-allow on the next request
/// anyway). If that does not free enough room, drop the trackers whose most
/// recent activity is oldest until the map is back under cap.
fn enforce_fallback_capacity(store: &DashMap<String, KeyTracker>, window: Duration) {
    let now = Instant::now();
    store.retain(|_, tracker| {
        tracker
            .timestamps
            .last()
            .is_some_and(|ts| now.duration_since(*ts) < window)
    });

    let overflow = store.len().saturating_sub(MAX_FALLBACK_ENTRIES);
    if overflow == 0 {
        return;
    }

    let mut candidates: Vec<(Option<Instant>, String)> = store
        .iter()
        .map(|e| (e.value().timestamps.last().copied(), e.key().clone()))
        .collect();
    // Smallest (oldest) first; None first.
    candidates.sort_by_key(|(ts, _)| *ts);
    for (_, key) in candidates.into_iter().take(overflow) {
        store.remove(&key);
    }
}

async fn check_rate_limit_key(
    key: &str,
    requests_per_minute: u32,
    fallback_store: &DashMap<String, KeyTracker>,
) -> Result<RateLimitPass, RateLimitRejection> {
    if let Some(global_limiter) = get_global_rate_limiter() {
        let (result, reservation) = global_limiter
            .check_and_record_with_source_and_limit(key, requests_per_minute)
            .await;

        if !result.allowed {
            return Err(RateLimitRejection {
                source: GLOBAL_LIMITER_SOURCE,
                retry_after: result.retry_after_secs.unwrap_or(60),
                limit: requests_per_minute,
            });
        }

        return Ok(RateLimitPass {
            source: GLOBAL_LIMITER_SOURCE,
            limit: requests_per_minute,
            remaining: result.remaining,
            reservation: RecordedRateLimitReservation::Global(reservation),
        });
    }

    let window = Duration::from_secs(60);
    let (allowed, retry_after, recorded_at) = {
        let mut tracker = fallback_store
            .entry(key.to_string())
            .or_insert_with(KeyTracker::new);
        tracker.check_and_record(requests_per_minute, window)
    };
    if fallback_store.len() > MAX_FALLBACK_ENTRIES {
        enforce_fallback_capacity(fallback_store, window);
    }

    if !allowed {
        return Err(RateLimitRejection {
            source: FALLBACK_LIMITER_SOURCE,
            retry_after,
            limit: requests_per_minute,
        });
    }

    let Some(recorded_at) = recorded_at else {
        return Err(RateLimitRejection {
            source: FALLBACK_LIMITER_SOURCE,
            retry_after: window.as_secs().max(1),
            limit: requests_per_minute,
        });
    };

    let remaining = fallback_store
        .get(key)
        .map(|tracker| requests_per_minute.saturating_sub(tracker.timestamps.len() as u32))
        .unwrap_or(requests_per_minute);

    Ok(RateLimitPass {
        source: FALLBACK_LIMITER_SOURCE,
        limit: requests_per_minute,
        remaining,
        reservation: RecordedRateLimitReservation::Fallback(recorded_at),
    })
}

fn gateway_fallback_store() -> Arc<DashMap<String, KeyTracker>> {
    GATEWAY_FALLBACK_STORE
        .get_or_init(|| Arc::new(DashMap::new()))
        .clone()
}

pub(super) async fn reserve_rate_limit_for_auth_attempt(
    req: &ServiceRequest,
    requests_per_minute: u32,
    trusted_proxies: &[String],
) -> Result<AuthRateLimitReservation, RateLimitError> {
    let key = extract_client_key(req, trusted_proxies);
    let fallback_store = gateway_fallback_store();

    match check_rate_limit_key(&key, requests_per_minute, &fallback_store).await {
        Ok(pass) => {
            debug!(
                client = %key,
                limit = pass.limit,
                remaining = pass.remaining,
                "Rate limit reservation passed for auth attempt ({} limiter)",
                pass.source
            );
            let source = match pass.reservation {
                RecordedRateLimitReservation::Global(reservation) => {
                    RateLimitReservationSource::Global(reservation)
                }
                RecordedRateLimitReservation::Fallback(recorded_at) => {
                    RateLimitReservationSource::Fallback {
                        store: fallback_store,
                        recorded_at,
                    }
                }
            };
            Ok(AuthRateLimitReservation { key, source })
        }
        Err(rejection) => {
            warn!(
                client = %key,
                "Rate limit exceeded before auth verification ({} limiter): retry after {}s",
                rejection.source,
                rejection.retry_after
            );
            Err(RateLimitError::new(rejection.retry_after, rejection.limit))
        }
    }
}

pub(super) async fn enforce_rate_limit_for_rejected_auth(
    req: &ServiceRequest,
    requests_per_minute: u32,
    trusted_proxies: &[String],
) -> Result<(), RateLimitError> {
    let key = extract_client_key(req, trusted_proxies);
    let fallback_store = gateway_fallback_store();

    match check_rate_limit_key(&key, requests_per_minute, &fallback_store).await {
        Ok(pass) => {
            debug!(
                client = %key,
                limit = pass.limit,
                remaining = pass.remaining,
                "Rate limit check passed for rejected auth path ({} limiter)",
                pass.source
            );
            Ok(())
        }
        Err(rejection) => {
            warn!(
                client = %key,
                "Rate limit exceeded for rejected auth path ({} limiter): retry after {}s",
                rejection.source,
                rejection.retry_after
            );
            Err(RateLimitError::new(rejection.retry_after, rejection.limit))
        }
    }
}

/// Lightweight in-process rate limit error for 429 responses
#[derive(Debug, Clone, Copy)]
pub(super) struct RateLimitError {
    retry_after: u64,
    limit: u32,
}

impl RateLimitError {
    fn new(retry_after: u64, limit: u32) -> Self {
        Self { retry_after, limit }
    }

    pub(super) fn gateway_error(&self) -> GatewayError {
        GatewayError::RateLimit {
            message: "Rate limit exceeded. Please retry after the indicated seconds.".to_string(),
            retry_after: Some(self.retry_after),
            rpm_limit: Some(self.limit),
            tpm_limit: None,
        }
    }
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
    requests_per_minute: Option<u32>,
}

impl RateLimitMiddleware {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute: Some(requests_per_minute),
        }
    }

    pub fn optional(requests_per_minute: Option<u32>) -> Self {
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
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = RateLimitMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddlewareService {
            service: Rc::new(service),
            requests_per_minute: self.requests_per_minute,
            fallback_store: gateway_fallback_store(),
        }))
    }
}

/// Service implementation for rate limit middleware
pub struct RateLimitMiddlewareService<S> {
    service: Rc<S>,
    requests_per_minute: Option<u32>,
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
/// 1. Authenticated API key ID / user ID from `RequestContext`, when present
/// 2. `X-Forwarded-For` rightmost non-trusted address — only when peer IP is
///    in `trusted_proxies` (leftmost entries are attacker-controlled)
/// 3. Direct peer address from connection info
fn extract_client_key(req: &ServiceRequest, trusted_proxies: &[String]) -> String {
    if let Some(identity) = authenticated_client_key(req) {
        return identity;
    }

    network_client_key(req, trusted_proxies)
}

fn authenticated_client_key(req: &ServiceRequest) -> Option<String> {
    let extensions = req.extensions();
    if let Some(context) = extensions.get::<SharedRequestContext>() {
        return client_key_from_context(context.as_ref());
    }

    let context = extensions.get::<RequestContext>()?;
    client_key_from_context(context)
}

fn client_key_from_context(context: &RequestContext) -> Option<String> {
    if let Some(api_key_id) = context.api_key_id() {
        return Some(format!("api_key:{}", api_key_id));
    }

    context
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|user_id| !user_id.is_empty())
        .map(|user_id| format!("user:{}", user_id))
}

fn network_client_key(req: &ServiceRequest, trusted_proxies: &[String]) -> String {
    let conn = req.connection_info();
    let peer = conn.peer_addr().unwrap_or("unknown");
    let peer_ip = parse_peer_ip(peer);

    // Only honor X-Forwarded-For when the direct peer is a trusted proxy.
    if trusted_proxies.iter().any(|p| p == &peer_ip)
        && let Some(client_ip) = req
            .headers()
            .get("X-Forwarded-For")
            .and_then(|v| v.to_str().ok())
            .and_then(|val| last_untrusted_xff_ip(val, trusted_proxies))
    {
        return format!("ip:{}", client_ip);
    }

    format!("ip:{}", peer_ip)
}

/// Pick the client IP from an `X-Forwarded-For` header by walking from the
/// right and skipping entries that match `trusted_proxies`.
///
/// Everything left of the rightmost non-trusted entry is attacker-controlled:
/// a client can seed `X-Forwarded-For` with arbitrary addresses before the
/// request reaches the trusted proxy. Using the leftmost entry therefore lets
/// clients rotate identities to dodge per-IP limits.
fn last_untrusted_xff_ip(val: &str, trusted_proxies: &[String]) -> Option<String> {
    let mut fallback = None;
    for entry in val.rsplit(',').map(str::trim).filter(|e| !e.is_empty()) {
        fallback.get_or_insert(entry);
        if !trusted_proxies.iter().any(|p| p == entry) {
            return Some(entry.to_string());
        }
    }
    fallback.map(str::to_string)
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let app_state = req.app_data::<web::Data<AppState>>().cloned();
        let trusted_proxies: Vec<String> = match app_state.as_ref() {
            Some(state) => {
                let cfg = state.config.load();
                cfg.server().trusted_proxies.clone()
            }
            None => Vec::new(),
        };
        let start_time = Instant::now();
        let path = req.path().to_string();
        let method = req.method().to_string();

        let client_key = extract_client_key(&req, &trusted_proxies);
        let requests_per_minute = effective_requests_per_minute(&req, self.requests_per_minute);

        let fallback_store = self.fallback_store.clone();
        let key = client_key.clone();

        Box::pin(async move {
            if let Some(requests_per_minute) = requests_per_minute {
                let pass =
                    match check_rate_limit_key(&key, requests_per_minute, &fallback_store).await {
                        Ok(pass) => pass,
                        Err(rejection) => {
                            warn!(
                                client = %key,
                                path = %path,
                                "Rate limit exceeded ({} limiter): retry after {}s",
                                rejection.source,
                                rejection.retry_after
                            );
                            let err = RateLimitError::new(rejection.retry_after, rejection.limit);
                            let gateway_error = err.gateway_error();
                            return Ok(middleware_gateway_error_response(
                                req,
                                actix_web::Error::from(err),
                                gateway_error,
                            ));
                        }
                    };

                debug!(
                    client = %key,
                    limit = pass.limit,
                    remaining = pass.remaining,
                    "Rate limit check passed ({} limiter)",
                    pass.source
                );
            }

            let res = service.call(req).await?.map_into_left_body();
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
#[path = "rate_limit_tests.rs"]
mod tests;
