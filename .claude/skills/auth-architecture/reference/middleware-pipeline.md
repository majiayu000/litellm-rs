## Middleware Pipeline

### AuthMiddleware

`AuthMiddleware` / `AuthMiddlewareService` (`src/server/middleware/auth.rs`) is an
actix-web `Transform`/`Service` pair registered in `src/server/http.rs`. It reads
`AppState.config`, then for non-public routes:

1. **Fail-closed gate** — if `enable_jwt` and `enable_api_key` are both false and
   `allow_anonymous` is false, every request gets 401 "Authentication is not
   configured" even if config validation was bypassed.
2. **Brute-force lockout** — `AuthRateLimiter::check_allowed(client_id)`; on
   lockout the response is 429 "Too many failed attempts. Try again in N seconds"
   backed by `GatewayError::RateLimit`.
3. **Method gating** — a credential of a disabled method is rejected with 401.
4. **Authentication** — `app_state.auth.authenticate(auth_method, context)`
   (i.e. `AuthSystem::authenticate`). On failure: 401 with the semantic error
   message (`middleware_gateway_error_response` renders OpenAI-shaped bodies on
   OpenAI-compatible paths); infrastructure errors return a generic 500
   ("Authentication service temporarily unavailable") that never leaks internals
   (`authentication_unavailable_response`).
5. **Authorization** — after success:
   - `api_key_allows_endpoint(api_key, path)` checks the key's runtime
     `allowed_endpoints` patterns (403 on mismatch).
   - `operation_for_path(path)` maps AI paths to operations (`chat`,
     `completions`, `embeddings`, `images`, `audio`, `moderations`, `rerank`,
     `files`, `fine_tuning`, `models`, `responses`, ...); `check_permission`
     enforces the two-role model (see below). 403 on denial.
6. **Context injection** — `User` and `ApiKey` are inserted into request
   extensions; the shared `SharedRequestContext` carries user/api-key/team IDs.

Public routes (`is_public_route`): `/health`, `/auth/login`,
`/auth/login/callback`, `/auth/register`, `/auth/forgot-password`,
`/auth/reset-password`, `/auth/verify-email`, `/admin/dashboard*`, `/docs`,
`/openapi.json`. `/auth/refresh` also skips header extraction.

### Route-level permission check

`check_permission(user, api_key, operation)` in `src/server/routes/ai/context.rs`:

- Unauthenticated callers are always denied.
- API keys carrying `"*"` or `"system.admin"` (directly or via runtime policy)
  are admin keys and pass everything.
- A key explicitly granting the operation (e.g. `api.chat` matches operation
  `chat`; `use:api` grants all non-management operations) passes.
- If the key has any explicit permission list, access is otherwise denied —
  limited keys stay limited even when owned by admins.
- Admin user roles (`SuperAdmin`, `Admin`) pass everything.
- Management operations (`keys.list_all`, `users.manage`, `config.manage`,
  `teams.manage`, `analytics.admin`) require that admin role or admin key;
  all other operations are allowed for any authenticated caller.

### Brute-force protection

`AuthRateLimiter` (`src/server/middleware/auth_rate_limiter.rs`) is a separate
`DashMap<String, AuthAttemptTracker>` keyed by client identifier:
`{ip}:api_key:{sha256(key)}`, `{ip}:jwt:{sha256(token)}`, or `ip:{ip}` for
anonymous/session attempts. Defaults: 5 failures within a 300 s window trigger
lockout starting at 60 s, doubling each repeat lockout (exponential backoff).
Trackers are capped at `DEFAULT_MAX_ENTRIES` (10,000).

### Gateway rate limiting after auth

`RateLimitMiddleware` runs innermost of the custom layers so it can pick a key
from authenticated context: `api_key:{id}` or `user:{id}` first, else a network
key from client IP + trusted proxies. Per-key RPM override comes from
`api_key.rate_limits.rpm`, falling back to the configured default. Rejections
emit 429 with `Retry-After` and `X-RateLimit-Limit` headers. See
[rate-limiting details](rate-limiting.md) for limiter internals.
