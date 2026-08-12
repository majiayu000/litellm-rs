# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Wired the `ollama` selector to its native chat, streaming, embeddings, tools, and health runtime behind `providers-extended`, with policy-bound public/private endpoint handling for #837.
- Wired `guardrails` into canonical chat request/response execution with default-on prompt-injection protection and an explicit `guardrails.enabled: false` opt-out.
- Added gateway `ip_access` configuration and registered its middleware ahead of authentication, handlers, and provider execution; default empty rules remain allow-all.
- Wired `enterprise.audit_logging` into request audit middleware with structured JSON stderr/file output, and aligned the observability facade with configured Langfuse/OpenTelemetry/Datadog lifecycle callbacks.
- Added real default-off Cargo gates for the module-only `a2a`, `mcp`, and `webhooks` experimental libraries.
- Added the `pricing.unpriced_model_policy` and `pricing.unpriced_fallback_cost_per_1k_tokens` configuration surface for #831 fail-closed unpriced-model enforcement; `pricing.allow_degraded` remains startup-only.
- Added `gateway_unpriced_events_total{provider,model_bucket,policy,outcome}` and `gateway_unpriced_spend_total{provider,model_bucket,policy,outcome}` Prometheus metrics for unpriced-model rejects, router candidate exclusions, and fallback settlements.

### Changed
- Breaking behavior: runtime requests for unpriced models now fail closed by default even when `pricing.allow_degraded=true`; deployments that intentionally allow unpriced traffic must set `pricing.unpriced_model_policy=allow_unpriced` and configure a finite `pricing.unpriced_fallback_cost_per_1k_tokens`.

### Deprecated
- Deprecated the unreachable `core::batch::BatchProcessor`, duplicate `core::virtual_keys::VirtualKeyManager`, `core::semantic_cache`, `core::analytics`, default-off `core::a2a`/`core::mcp`/`core::realtime`/`core::webhooks`, and optional `core::user_management::UserManager` public surfaces for the 0.6 line. They remain available behind their documented compatibility/features until the approved 0.7 removal; see `docs/architecture/GH838-subsystem-migration-0.6-to-0.7.md`.
- Deprecated the `providers-extended` public `amazon_nova` native module for the 0.6.0 line while preserving its symbols, constructors, and runtime behavior. The catalog policy now records the same Amazon Nova endpoint/auth contract, five canonical models, token pricing, multimodal/tool metadata, and provider capabilities as the native implementation. Direct Rust imports should migrate to the `amazon_nova` catalog selector before the planned 0.7.0 native demotion; see `docs/providers/GH837-migration-0.6-to-0.7.md`.
- Deprecated the `providers-extended` public `github` native module for the 0.6.0 line while preserving its symbols, constructors, and runtime behavior. The catalog policy now records the same `GITHUB_MODELS_API_BASE` (`https://models.inference.ai.azure.com`) endpoint/Bearer `GITHUB_TOKEN` auth contract, all 16 GitHub Models, token pricing, multimodal/tool metadata, and provider capabilities as the native implementation, with health served by the OpenAI-compatible catalog route. Direct Rust imports should migrate to the `github` catalog selector before the planned 0.7.0 native demotion; see `docs/providers/GH837-migration-0.6-to-0.7.md`.
- Deprecated the `providers-extended` public `custom_api` module and its `CustomHttpxConfig`, `CustomApiErrorMapper`, and `CustomHttpxProvider` exports for the 0.6.0 line. Existing symbols, signatures, and runtime behavior remain available in 0.6.x, but arbitrary URL/method/template/parser support is no longer a product goal and the surface is scheduled for removal in 0.7.0. See `docs/providers/GH837-migration-0.6-to-0.7.md` for alternatives.

### Removed
- Removed the module-only `topaz` provider implementation from the `providers-extended` surface for #837. It had no gateway factory or dispatch path, so runtime provider selection is unchanged; downstream crates directly importing `litellm_rs::core::providers::topaz` must remove that import or restore the old implementation from git history.
- Removed the module-only `sap_ai` provider implementation from the `providers-extended` surface for #837. It had no gateway factory or dispatch path, so runtime provider selection is unchanged; downstream crates directly importing `litellm_rs::core::providers::sap_ai` must remove that import or restore the old implementation from git history.
- Removed the module-only `vercel_ai` provider implementation from the `providers-extended` surface for #837. It had no gateway factory or dispatch path, so runtime provider selection is unchanged; downstream crates directly importing `litellm_rs::core::providers::vercel_ai` must remove that import or restore the old implementation from git history.
- Removed the module-only `empower` provider implementation from the `providers-extended` surface for #837. It had no gateway factory or dispatch path, so runtime provider selection is unchanged; downstream crates directly importing `litellm_rs::core::providers::empower` must remove that import or restore the old implementation from git history.

### Fixed
- Fixed IP access denial so blocked requests no longer execute the downstream service before returning `403 Forbidden`.
- Redis-backed distributed rate limiting now fails closed by default when Redis commands fail, emits `rate_limiter_degraded_total{operation,mode}`, and keeps the old local fallback only behind `rate_limit.redis_failure_mode: fail_open_local`.

## [0.5.0] - 2026-04-30

### Added
- Merge pull request #412 from majiayu000/feat/provider-model-refresh-2026-04-21
- feat(models): update model catalogs for OpenAI, Anthropic, and Zhipu AI (#388)
- feat(router): add zai prefix alias to zhipu routing
- feat(router): add moonshot/minimax/zhipu dynamic and prefix routing
- feat(router): add atomic routing metrics counters (#376)
- feat(anthropic): add beta headers, structured outputs, and built-in tool types (#324)
- feat(openai): add store/metadata/service_tier params, update image models, mark deprecated (#322)
- feat: replace wildcard re-exports with explicit pub use in lib.rs (#315)
- feat(providers): reject unknown provider type strings with clear error at parse time (#311)
- feat(mistral): add missing params - frequency_penalty, presence_penalty, n, parallel_tool_calls, guardrails (#302)
- feat(core): enable user_management module with stub DB implementations (#296)
- feat: add CI job to compile-check disabled modules (#295)
- feat: enable virtual_keys module with stub database implementations (#292)
- feat(openai): add GPT-5.4 family and fix GPT-4.1 context window (#287)
- feat(gemini): add Gemini 3.1 models, fix systemInstruction and tool call handling (#291)
- feat(mistral): overhaul model catalog with 36+ current models (#290)
- feat: add reasoning_effort parameter and Developer message role for o-series models (#289)
- feat(anthropic): add claude-sonnet-4-6, claude-haiku-4-5; fix opus-4-6 limits and thinking serialization (#288)
- feat(openai-like): forward extra_params to upstream provider (#286)
- feat(config): implement YAML env var substitution in Config::from_file (#285)
- feat(mcp): add lightweight JSON Schema validation for tool arguments (#212) (#232)
- feat(a2a): add periodic health checks and exclude Unknown agents from routing (#213) (#227)
- feat(storage): add cache-aside pattern for API key verification (#207) (#228)
- feat(router): add structured tracing for routing decisions (#229)
- feat(config): add environment variable support for cache/rate-limit/enterprise (#66)
- feat(config): add schema_version field to GatewayConfig (#68)
- feat(examples): add hello example and fix broken bin references (#57)

### Fixed
- fix(cli): add gateway release entrypoint
- fix(router): execute with capability-aware deployments
- fix(auth): normalize brute-force lockout keys
- Merge pull request #455 from majiayu000/fix/issue-408-rate-limit-stable-client-key
- fix(rate-limit): ignore untrusted auth headers
- Merge pull request #454 from majiayu000/fix/issue-407-cors-validation-gate
- fix(server): fail fast on invalid cors config
- Merge pull request #453 from majiayu000/fix/issue-409-embedding-array-validation
- fix(embeddings): reject non-string array input
- Merge pull request #452 from majiayu000/fix/issue-413-sdk-chat-model
- fix(sdk): preserve explicit chat model
- Merge pull request #451 from majiayu000/fix/issue-414-vertex-gemini3-models
- fix(vertex-ai): route Gemini 3 models
- Merge pull request #450 from majiayu000/fix/issue-424-gemini-thinking-pricing
- fix(pricing): align Gemini thinking cost
- Merge pull request #449 from majiayu000/fix/issue-436-storage-file-config
- fix(storage): honor configured file storage
- Merge pull request #448 from majiayu000/fix/issue-438-streaming-deployment-lifecycle
- fix(ai): hold deployment leases for streams
- Merge pull request #447 from majiayu000/fix/issue-439-openai-error-envelope
- fix(ai): return OpenAI error envelopes
- Merge pull request #446 from majiayu000/fix/issue-433-filtered-key-pagination
- fix(keys): paginate filtered key listings
- Merge pull request #445 from majiayu000/fix/issue-432-key-admin-promotion
- fix(keys): block non-admin management permission grants
- fix(models): align GPT-5.4 Pro token limits
- Merge pull request #418 from majiayu000/fix/gstack-health-2026-04-24
- fix(deps): address TLS migration review
- Merge pull request #441 from majiayu000/fix/issue-434-key-manager
- Merge pull request #444 from majiayu000/fix/issue-440-virtual-key-persistence
- fix(storage): keep virtual key last-used monotonic
- fix: align Homebrew release automation (#429)
- fix(storage): require explicit sqlite fallback (#442)
- fix: add utility pricing for Gemini flash variants (#425)
- fix(storage): preserve virtual key spend during usage updates
- fix(storage): reject placeholder vector backends (#443)
- fix(storage): avoid virtual key usage races
- fix(keys): bound last used cache
- fix(storage): persist virtual keys
- fix(keys): share key manager across requests
- fix(release): publish gateway binary only (#431)
- fix(deps): eliminate vulnerable TLS and YAML chains
- fix(sdk): wire execute_stream_request to provider dispatch (#396) (#402)
- fix(sdk): parse data URI to extract correct media_type for Anthropic multimodal (#401)
- fix(sdk): implement atomic round-robin rotation in LoadBalancer (#397)
- fix(auth): guard is_admin_route() against prefix confusion (SEC-04) (#393)
- fix(auth): replace prefix match with exact equality in is_public_route (#390)
- fix(errors): replace Box<dyn Error> with typed errors at trait boundaries (#384)
- fix(a2a): auto-trigger agent health checks before routing (#381)
- fix(mcp): add optional JSON Schema validation for MCP tool parameters (#380)
- fix(storage): wrap multi-step DB operations in SeaORM transactions (#377)
- fix(storage): add cache-aside invalidation on API key usage write (#378)
- fix(streaming): add CancellationToken to cancel provider streams on client disconnect (#379)
- fix(providers): wire 6 unreachable provider types into factory (#374)
- fix(security): redact sensitive fields in Debug impls for config structs (#369)
- fix(auth): tighten password reset rate limit to 5 requests per 15 minutes (#371)
- fix(rust): add rust-toolchain.toml pinning stable channel (#368)
- fix(router): wire min_requests and success_threshold into circuit breaker (#367)
- fix(providers): implement 5 missing from_config_async branches (#365)
- fix(a2a): replace hardcoded request ID=1 with unique IDs (#361)
- fix(streaming): add VecDeque buffer size limit to prevent OOM (#362)
- fix(responses): address 6 correctness issues from code review (#329)
- fix(responses): resolve CI failures in Responses API implementation (#328)
- fix(macros): remove dead helper functions from provider_config! macro (#321)
- fix(dead_code): resolve 55 of 56 dead_code suppressions (#278) (#320)
- fix(lib): restore FunctionCall and ToolCall to public re-exports (#319)
- fix(errors): replace .unwrap() in production hot paths (#261) (#312)
- fix(openai): update capability lists for GPT-5.4, o3, o4-mini (#274) (#306)
- fix(config): replace hardcoded default string comparison in StorageConfig merge logic (#310)
- fix(config): remove dead hot_reload entries from ConfigPresets (#308)
- fix(core): gate user_management behind storage feature flag (#300)
- fix: deep-merge reasoning object and make effort/max_tokens mutually exclusive (#301)
- fix(openrouter): add HTTP-Referer/X-Title headers and wire reasoning param (#299)
- fix: implement user_management DB ops and wire TeamManager to persistent storage (#298)
- fix: gate virtual_keys module behind gateway feature flag (#294)
- fix: resolve critical TODOs in teams, redis pubsub, and monitoring (#293)
- fix(security): migrate API key hashing to HMAC-SHA256 with server secret (#254)
- fix(auth): implement basic RBAC with admin/user roles in check_permission (#242) (#251)
- fix(security): enforce minimum 32-byte JWT secret length (#240) (#250)
- fix(security): reject empty OAuth allowed_origins instead of permitting all (#241) (#247)
- fix(router): add circular alias and fallback cycle detection (#214) (#234)
- fix(streaming): add idle timeout to SSE streams to prevent zombie connections (#205)
- fix(auth): reject empty JWT secret on startup instead of warn (#204)
- fix(auth): separate access and refresh token verification (#203)
- fix(router): use min_requests and success_threshold in circuit breaker (#200)
- fix(streaming): cancel upstream provider stream on client disconnect (#198)
- fix(provider): add missing from_config_async branches for catalog-covered provider types (#197)
- fix(config): change Redis default to enabled=false (#196)
- fix(streaming): handle SSE errors with proper error events instead of HTTP 200 (#185)
- fix(storage): replace relative ./data path with absolute path in local file storage (#184)
- fix(config): fix boolean merge one-way override in CacheConfig (#183)
- fix(a2a): replace hardcoded request ID=1 with atomic counter (#182)
- fix(config): validate port range to reject values >65535 (#181)
- fix(auth): add input validation for API key creation (#180)
- fix(router): rename CostBased strategy to PriorityBased (#178)
- fix(storage): implement 4 unimplemented S3 methods (#177)
- fix(streaming): add VecDeque buffer capacity limit to prevent OOM (#176)
- fix(security): redact secrets in Debug impl for AuthConfig and ProviderConfig (#175)
- fix(auth): add rate limiting to password reset endpoint (#174)
- fix(storage): replace hardcoded relative SQLite path with platform-aware default_sqlite_path() (#156)
- fix(perf): throttle api_key last_used DB writes to every 5 minutes (#153)
- fix(storage): apply max_connections config to Redis connection pool (#148)
- fix(storage): remove dead BatchOperations referencing nonexistent Database enum (#147)
- fix(security): mask usernames in login log messages to prevent PII leak (#146)
- fix(provider): replace from_f64().unwrap() with safe error handling across providers (#130)
- fix(auth): use transactional reset_password_with_token to eliminate TOCTOU race (#129)
- fix(perf): replace blocking parking_lot::Mutex with tokio::sync::Mutex in memory cache (#133)
- fix(api): forward stream_options field in chat completion requests (#131)
- fix: remove unwrap() panic in Mistral transform_request (closes #77) (#127)
- fix: remove unwrap() panics in vertex_ai provider (closes #78) (#128)
- fix: remove unwrap() panic in S3 cache storage_class parse (closes #76) (#126)
- fix: remove unwrap() panics in openai provider (closes #79) (#125)
- fix(server): mount missing auth/keys/teams/budget/health routes in create_app (#112)
- fix(provider): OpenAILikeProvider::name() returns actual provider name (#117)
- fix(middleware): X-Request-ID generated twice and not returned in responses (#111)
- fix(api): unify pricing routes from /api/v1/ to /v1/ prefix (#123)
- fix(cache): log Redis write failure in dual-cache set_with_size (#121)
- fix(middleware): remove no-op CorsMiddleware implementation (#120)
- fix(budget): eliminate TOCTOU race in create_budget() via Entry API (#116)
- fix(error): replace wildcard with explicit match arms for 11 GatewayError variants (#115)
- fix(sync): eliminate read-modify-write race in AtomicValue::update() (#114)
- fix(security): add ownership verification to API key CRUD endpoints (IDOR) (#110)
- fix(security): SSRF protection for custom API endpoint_url (#109)
- fix(auth): add IP-based rate limiting to /auth/login endpoint (#108)
- fix(api): GET /auth/me incorrectly registered as POST method (#113)
- fix(security): CORS empty origins list no longer defaults to wildcard '*' (#107)
- fix(auth): wrap password reset token ops in database transaction (#73)
- fix(server): add X-Forwarded-For trusted proxy validation (#72)
- fix(auth): replace unwrap_or_else with proper error handling in auth middleware (#67)
- fix(config): correct boolean merge logic in config system (#65)
- fix(deps): consolidate reqwest to single version 0.12.x (#48)
- fix(deps): upgrade quinn-proto to fix CVE-2026-0037 (#50)
- fix(deps): upgrade rand from 0.8 to 0.9 (#47)
- fix(lint): resolve 314 collapsible_if warnings for clippy 1.94.0 (#49)
- fix(security): add rate limiting and unify error messages for registration (#42)
- fix(security): reject session auth until proper session store is implemented (#41)
- fix(security): reject refresh tokens in authenticate_jwt (#39)
- fix(security): use SHA-256 for rate limit key hashing (#40)
- fix(ci): pin rust toolchain and add PR guardrails (#34)
- fix(security): consolidated security hardening — audit fixes, auth hash, env validation, route bypass, OAuth, concurrency (#33)
- fix(error): preserve provider identity in map_http_status_to_error (FUT-59) (#22)
- fix: harden boundary guard and stabilize router/error mapping integration (#14)
- fix(sse): map reasoning_content to thinking delta (#11)

### Changed
- style(config): format serde_norway migration cleanup
- refactor(deps): use explicit maintained crate names
- refactor(router): remove redundant dead-code zai/ prefix check (#405)
- refactor(providers): split LLMProvider into focused sub-traits (#383)
- fix(a2a): auto-trigger agent health checks before routing (#381)
- refactor: split factory.rs into registry, resolver, builder, coordinator modules (#317)
- refactor(config): split gateway.rs tests and fix pricing source path (#318)
- refactor(errors): split utils.rs (1435 lines) into focused sub-modules (#316)
- refactor(deps): replace async-trait with native AFIT in core traits (#246) (#252)
- refactor(provider): eliminate unwrap() in provider request/response paths (#245) (#248)
- refactor(provider): remove associated types from LLMProvider trait (#238)
- refactor: extract test modules from oversized gateway_error files (#221) (#237)
- refactor(config): consolidate default values into single source of truth (#235)
- refactor(error): simplify From<ProviderError> to use GatewayError::Provider directly (#233)
- refactor(storage): add transaction wrapping and optimistic locking for DB operations (#206) (#230)
- refactor(config): replace Arc<Config> with AtomicValue for atomic hot reload (#209) (#226)
- refactor(provider): consolidate 5 dispatch macros into single parametric macro (#224)
- refactor(quality): eliminate unwrap() calls in auth and security paths (#215) (#231)
- refactor(storage): remove dead legacy migration files (#222)
- refactor(error): consolidate GatewayError from 29 to 15 variants (#160)
- refactor(provider): remove orphan LLMProvider implementations (#159)
- refactor: split openai/transformer.rs into focused sub-modules (#154)
- refactor(provider): remove standalone impls for catalog-covered providers (#151)
- refactor(provider): remove dead Provider enum variants without factory paths (#150)
- refactor(provider): consolidate OpenAI dual LLMProvider implementations (#149)
- refactor: remove deprecated legacy config types (#132)
- refactor: remove duplicate LiteLLMError and OpenAIError type definitions (#124)
- refactor: 3-phase architectural refactoring (God Module, Type, Error) (#58)
- perf(observability): shorten record_request write lock hold time (#19)
- perf(recovery): remove blocking mutexes in circuit breaker async path (#15)
- perf(health): replace std rwlock with async monitor locks (#20)
- perf(cache): remove deep clone in hit path via Arc payload (#16)
- refactor(streaming): dedupe done marker handling for pilot providers (#24)




### Removed
- Removed the legacy `google-gateway` binary from Cargo, release archives, CI artifacts, and Docker images. The published gateway distribution now focuses on the main `gateway` executable.

## [0.4.2] - 2026-02-28

### Fixed
- fix(ci): fallback to grep when ripgrep is unavailable




## [0.4.1] - 2026-02-28

### Fixed
- fix(clippy): satisfy strict lints in audio service and router tests




## [0.4.0] - 2026-02-28




### Changed
- **Provider Infra**: `BaseConfig::for_provider()` now delegates environment loading with the original provider input while keeping normalized default resolution in one place, removing duplicated normalization flow.
- **Provider Infra**: `BaseConfig::provider_env_key()` env-key normalization now explicitly covers trimmed/case-variant provider input via regression test.
- **Provider Infra**: `BaseConfig::provider_env_key()` now normalizes provider names internally, and `from_env()` reuses normalized env helpers directly to remove duplicated normalization flow.
- **Provider Infra**: Centralized provider environment variable key/value resolution in `BaseConfig` helpers (`provider_env_key`, `env_value`) to remove repeated env lookup formatting.
- **Provider Infra**: Centralized endpoint URL construction in `BaseConfig::build_endpoint()` and reused it for chat/embeddings endpoints to remove duplicated formatting logic.
- **Provider Infra**: Centralized default API version assignment in `BaseConfig::default_api_version()` to remove repeated provider-specific conditionals.
- **Provider Infra**: `BaseConfig::for_provider` now normalizes provider names (trim + lowercase) before catalog/fallback resolution to prevent casing/spacing drift.
- **Provider Infra**: Removed legacy alias fallback in `BaseConfig` and kept canonical provider-name defaults only to avoid alias drift.
- **Provider Infra**: Extracted `legacy_default_base_url()` helper in `BaseConfig` to isolate non-catalog fallback mapping and simplify maintenance while preserving behavior.
- **Provider Infra**: `BaseConfig::for_provider` now consults Tier-1 provider catalog defaults first, reducing duplicated base URL definitions while preserving existing fallback behavior.
- **Provider Infra**: Removed the unused `CommonProviderConfig` duplicate from `core::providers::shared`, keeping provider base config responsibilities centralized in `core::providers::base` and reducing schema duplication.

### Added
- **Provider Tests**: Added B1 batch coverage to validate `aiml_api`, `anyscale`, `bytez`, and `comet_api` selectors and creation paths resolve through Tier-1 catalog to `OpenAILike` providers.
- **Provider Tests**: Added B2 batch coverage to validate `compactifai`, `aleph_alpha`, `yi`, and `lambda_ai` selector and creation paths resolve through Tier-1 catalog to `OpenAILike` providers.
- **Provider Tests**: Added B3 batch coverage to validate `ovhcloud`, `maritalk`, `siliconflow`, and `lemonade` selector and creation paths resolve through Tier-1 catalog to `OpenAILike` providers.

## [0.3.0] - 2026-02-05

### Added
- **Agent Coordinator**: New `core::agent` module for managing concurrent agent lifecycles with cancellation, timeouts, and stats.
- **Utilities**: Added `utils::event` publish/subscribe broker and `utils::sync` concurrent containers.

### Changed
- **Providers**: Migrated `ai21`, `amazon_nova`, `datarobot`, and `deepseek` to pooled HTTP provider hooks.
- **HTTP Client**: Standardized pooled client usage and shared client caching across core/providers.
- **Routing**: Refined provider routing and OpenAI-compatible request/response handling.

### Fixed
- **Auth Context**: Corrected user/api-key context propagation in auth routes and middleware.
- **SSRF Validation**: DNS resolution failures no longer hard-fail SSRF checks while preserving IP safety.
- **Observability**: Prometheus label handling now safely maps provider identifiers.
- **Concurrency**: Event broker handles zero capacity; VersionedMap retry now guarantees progress under contention.
- **Packaging**: Track core cache sources and add root README for crates.io.

## [0.1.3] - 2025-09-18

### Fixed
- **docs.rs Build**: Fixed documentation build failure on docs.rs by excluding `vector-db` feature
  - Added `all-features = false` to `package.metadata.docs.rs` configuration
  - Explicitly listed features that work with docs.rs read-only filesystem
- **Internationalization**: Translated all Chinese comments and documentation to English
  - Cleaned 40+ files with hundreds of Chinese comments
  - Improved accessibility for international developers
  - Maintained technical accuracy in all translations

### Changed
- **Configuration**: Updated `Cargo.toml` metadata for better docs.rs compatibility
- **Documentation**: All code comments are now in English

## [0.1.1] - 2025-7-28

### Fixed
- **Security**: Excluded sensitive configuration file `config/gateway.yaml` from published package
- **Package**: Only include example configuration files (`.example`, `.template`) in published crate
- **Privacy**: Prevent accidental exposure of API keys and secrets in published package

## [0.1.0] - 2025-07-28

### Added
- Initial release of Rust LiteLLM Gateway
- High-performance AI Gateway with OpenAI-compatible APIs
- Intelligent routing and load balancing capabilities
- Support for multiple AI providers (OpenAI, Anthropic, Google, etc.)
- Enterprise features including authentication and monitoring
- Actix-web based web server with async/await support
- PostgreSQL and Redis integration for data persistence and caching
- Comprehensive configuration management via YAML
- Rate limiting and request throttling
- WebSocket support for real-time communication
- Prometheus metrics integration
- OpenTelemetry tracing support
- Vector database integration (Qdrant)
- S3-compatible object storage support
- JWT-based authentication system
- Docker and Kubernetes deployment configurations
- Comprehensive API documentation
- Integration tests and examples

### Features
- **Core Gateway**: OpenAI-compatible API endpoints
- **Multi-Provider Support**: Seamless integration with various AI providers
- **Load Balancing**: Intelligent request distribution
- **Caching**: Redis-based response caching
- **Monitoring**: Prometheus metrics and OpenTelemetry tracing
- **Authentication**: JWT-based security
- **Rate Limiting**: Configurable request throttling
- **WebSocket**: Real-time streaming support
- **Storage**: PostgreSQL for persistence, S3 for object storage
- **Vector DB**: Qdrant integration for embeddings
- **Deployment**: Docker, Kubernetes, and systemd configurations

[Unreleased]: https://github.com/majiayu000/litellm-rs/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/majiayu000/litellm-rs/compare/v0.1.3...v0.3.0
[0.1.3]: https://github.com/majiayu000/litellm-rs/compare/v0.1.1...v0.1.3
[0.1.1]: https://github.com/majiayu000/litellm-rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/majiayu000/litellm-rs/releases/tag/v0.1.0
