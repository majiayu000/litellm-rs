## Contents

- Rate Limiting

## Rate Limiting

### Lock-Free Rate Limiter

```rust
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct RateLimiter {
    // Key: user_id or api_key, Value: (request_count, window_start)
    counters: DashMap<String, RateState>,
    config: RateLimitConfig,
}

struct RateState {
    request_count: AtomicU64,
    token_count: AtomicU64,
    window_start: AtomicU64,
}

#[derive(Clone)]
pub struct RateLimitConfig {
    pub requests_per_minute: u64,
    pub tokens_per_minute: u64,
    pub window_size: Duration,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            counters: DashMap::new(),
            config,
        }
    }

    pub fn check_rate_limit(&self, key: &str) -> Result<(), RateLimitError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let state = self.counters.entry(key.to_string()).or_insert_with(|| {
            RateState {
                request_count: AtomicU64::new(0),
                token_count: AtomicU64::new(0),
                window_start: AtomicU64::new(now),
            }
        });

        let window_start = state.window_start.load(Ordering::SeqCst);
        let window_size_secs = self.config.window_size.as_secs();

        // Reset window if expired
        if now - window_start >= window_size_secs {
            state.request_count.store(0, Ordering::SeqCst);
            state.token_count.store(0, Ordering::SeqCst);
            state.window_start.store(now, Ordering::SeqCst);
        }

        // Check request count
        let current_requests = state.request_count.fetch_add(1, Ordering::SeqCst);
        if current_requests >= self.config.requests_per_minute {
            state.request_count.fetch_sub(1, Ordering::SeqCst);
            let retry_after = window_size_secs - (now - window_start);
            return Err(RateLimitError::RequestsExceeded {
                limit: self.config.requests_per_minute,
                retry_after,
            });
        }

        Ok(())
    }

    pub fn record_tokens(&self, key: &str, tokens: u64) -> Result<(), RateLimitError> {
        if let Some(state) = self.counters.get(key) {
            let current_tokens = state.token_count.fetch_add(tokens, Ordering::SeqCst);
            if current_tokens + tokens > self.config.tokens_per_minute {
                state.token_count.fetch_sub(tokens, Ordering::SeqCst);
                return Err(RateLimitError::TokensExceeded {
                    limit: self.config.tokens_per_minute,
                });
            }
        }
        Ok(())
    }

    pub fn get_remaining(&self, key: &str) -> RateLimitInfo {
        self.counters
            .get(key)
            .map(|state| {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let window_start = state.window_start.load(Ordering::SeqCst);
                let reset_at = window_start + self.config.window_size.as_secs();

                RateLimitInfo {
                    remaining_requests: self.config.requests_per_minute
                        .saturating_sub(state.request_count.load(Ordering::SeqCst)),
                    remaining_tokens: self.config.tokens_per_minute
                        .saturating_sub(state.token_count.load(Ordering::SeqCst)),
                    reset_at,
                }
            })
            .unwrap_or(RateLimitInfo {
                remaining_requests: self.config.requests_per_minute,
                remaining_tokens: self.config.tokens_per_minute,
                reset_at: 0,
            })
    }
}
```

### Rate Limit Middleware

```rust
pub async fn rate_limit_middleware(
    req: ServiceRequest,
    rate_limiter: Arc<RateLimiter>,
) -> Result<ServiceRequest, actix_web::Error> {
    let auth_context = req
        .extensions()
        .get::<AuthContext>()
        .ok_or(AuthError::MissingContext)?;

    rate_limiter
        .check_rate_limit(&auth_context.user_id)
        .map_err(|e| {
            let response = actix_web::HttpResponse::TooManyRequests()
                .insert_header(("Retry-After", e.retry_after().to_string()))
                .insert_header(("X-RateLimit-Limit", rate_limiter.config.requests_per_minute.to_string()))
                .insert_header(("X-RateLimit-Remaining", "0"))
                .json(json!({
                    "error": {
                        "message": "Rate limit exceeded",
                        "type": "rate_limit_error",
                        "code": "rate_limit_exceeded"
                    }
                }));
            actix_web::error::InternalError::from_response(e, response).into()
        })?;

    Ok(req)
}
```
