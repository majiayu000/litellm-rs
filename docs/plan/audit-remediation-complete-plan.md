# Audit remediation complete execution plan

- Planned version: v1
- Date: 2026-05-01
- Applicable warehouse: `/Users/apple/Desktop/code/AI/gateway/litellm-rs`
- Target baseline: `main` at `de594c81`
- Execution mode: change one step or one tightly coupled bundle -> test now -> update this plan -> continue
- Raw audit scope: 77 findings from four agents
- Deduplicated execution scope: 72 remediation items: 20 Critical, 22 High, 30 Medium

## 0. Execution constraints

- Objective: remove the confirmed audit issues while keeping the gateway usable after every merged change.
- Source of truth: this file defines sequencing; `PLAN_AUDIT_REMEDIATION.md` remains the detailed per-finding spec.
- Compatibility: preserve public OpenAI-compatible request/response behavior unless the change fixes a documented compatibility bug.
- Submission strategy: one Critical/High ID per branch/PR unless the IDs are inseparable by code path; Medium items may be grouped by sweep if file ownership is disjoint.
- Branch strategy: branch from latest `main`; avoid stacking feature branches for independent fixes.
- Worktree strategy: use separate worktrees for parallel work; two workers must not edit the same file.
- PR size rule: prefer <= 10 files and <= 500 changed lines excluding docs, migrations, generated snapshots, and `Cargo.lock`.
- Commit hygiene: DCO sign-off for this campaign; no `Co-Authored-By` or AI markers unless the owner overrides.
- Compatibility risk rule: provider wire-format fixes must add regression fixtures before or with the implementation.
- Data risk rule: destructive migrations require a copy/verify step before drop/delete steps.
- Stop condition: stop and revise this plan if an item has no reproducible file-level evidence, a baseline command fails for unrelated reasons that block verification, or a proposed migration could lose data without an owner decision.

## 1. Baseline and current evidence

- Current branch: `main`
- Current local plan files are untracked:
  - `PLAN_AUDIT_REMEDIATION.md`
  - `PLAN_AUDIT_EXECUTION.md`
- Tracker count in `PLAN_AUDIT_REMEDIATION.md`: C=20, H=22, M=30, total=72.
- `reqwest::Client::new()` baseline outside obvious tests is currently about 48 hits; raw full-tree count is about 53 hits. H19 must be planned as a multi-batch migration, not a four-batch `~17` cleanup.
- Raw agent reports are not yet persisted in the repository. They should be saved under `docs/audit-2026-05-01/` before substantial remediation begins.

Required kickoff commands:

```bash
git status --short --branch
git fetch origin
git pull --ff-only origin main
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo check --no-default-features --features lite
rg -n "reqwest::Client::new\(\)" src/ -g '!**/*test*' -g '!**/tests.rs' > /tmp/litellm-rs-reqwest-baseline.txt
wc -l /tmp/litellm-rs-reqwest-baseline.txt
wc -l src/core/cost/calculator.rs src/core/providers/anthropic/{models,client}.rs src/core/providers/base/sse.rs src/core/providers/openai_like/provider.rs src/server/routes/ai/chat.rs src/core/providers/factory/builder.rs
```

If `cargo check --no-default-features --features lite` fails at kickoff, record it as the M20 baseline failure; do not block P0 security/correctness work on that existing failure.

## 2. Architecture inventory

| Area | Canonical files | Current issue pattern |
|------|-----------------|-----------------------|
| HTTP boundary | `src/server/routes/ai/chat.rs`, `src/core/models/openai/requests.rs`, `src/core/streaming/types.rs` | Gateway conversion drops provider fields; streaming structs emit nulls and lack reasoning/tool fields. |
| Provider transforms | `src/core/providers/anthropic/client.rs`, `src/core/providers/bedrock/chat/converse.rs`, `src/core/providers/gemini/client.rs`, `src/core/providers/base/sse.rs` | Provider-specific tool/thinking/cache semantics are flattened or silently dropped. |
| Provider registry | `src/core/providers/mod.rs`, `src/core/providers/provider_type.rs`, `src/core/providers/factory/registry.rs`, `src/core/providers/registry/catalog.rs` | Supported-provider truth is split across enum, parsers, factory, and catalog. |
| Pricing | `src/core/cost/`, `src/core/providers/base/pricing.rs`, `src/services/pricing/`, `src/core/providers/anthropic/mod.rs`, `config/pricing.yaml` | Multiple pricing systems coexist and persistence is partially unwired. |
| Persistence | `src/storage/database/migration/`, `src/storage/database/entities/`, `src/storage/database/seaorm_db/` | Pricing migration is not registered; team/user data has parallel table tracks. |
| Budget/rate limit | `src/server/state.rs`, `src/core/budget/provider_limits.rs`, `src/core/rate_limiter/limiter.rs` | Quota state is per-process and restart-volatile. |
| Security/config | `src/core/mcp/config.rs`, `src/core/a2a/config.rs`, `src/config/mod.rs`, `src/config/models/auth.rs`, `src/server/middleware/auth_rate_limiter.rs` | SSRF guard not shared, missing env vars are tolerated, auth defaults are misleading. |
| Outbound HTTP | provider files, `src/services/pricing/service.rs`, `src/monitoring/alerts/channels.rs`, `src/storage/vector/qdrant.rs` | Ad hoc clients lack shared timeouts, user-agent, proxy behavior, and pooling. |
| SDK/router | `src/sdk/client/routing.rs`, `src/core/router/` | SDK has parallel routing logic instead of delegating to the core router. |

## 3. Redundancy and convergence findings

| id | category | files and symbols | evidence | impact | risk | convergence direction |
|----|----------|-------------------|----------|--------|------|-----------------------|
| R1 | Gateway/provider field loss | `ChatCompletionDelta`, `convert_stream_chunk`, provider SSE transformers | C1, C3, C4, C19, H1 | high | medium | Expand canonical streaming/usage structs first, then provider transforms. |
| R2 | Provider tool contracts drift | Anthropic, Bedrock, Gemini transforms | C2, C5, C6, H4, H5 | high | medium | Add fixtures for each provider and map tool calls/results explicitly. |
| R3 | Security guard duplication | A2A SSRF helper vs MCP scheme-only validation | C7, M7 | high | medium | Promote a shared URL validation module and reuse it from A2A/MCP. |
| R4 | SQL construction policy split | pgvector SQL builder and hand-rolled escaping | C8 | high | medium | Centralize identifier validation/quoting and parameterize values. |
| R5 | Quota state split from storage | budget manager, rate limiter, Redis config | C13, C14, H9, M15 | high | high | Persist budgets and add Redis-backed distributed limiters behind config. |
| R6 | Pricing systems duplicate | core cost, provider base pricing, services pricing, Anthropic pricing | C11, C12, H18, H22, M16, M17, M26 | high | high | Choose one pricing service and migrate callers in stages. |
| R7 | Provider source-of-truth drift | `ProviderType`, factory, catalog, enum dispatch | C17, C18, H13, H17, M23 | high | high | Build one registry table and generate/derive parsing, display, catalog classification, and factory coverage tests from it. |
| R8 | Parallel team/user persistence | `um_*` tables vs `users/teams` tables | C15, M27 | high | high | Pick canonical tables, copy data, verify parity, then drop legacy. |
| R9 | Config permissiveness | env substitution, missing `deny_unknown_fields`, default auth | C16, H20, H21, H11 | medium | medium | Make invalid config fail early with explicit diagnostics. |
| R10 | Outbound HTTP client sprawl | 48-53 current `reqwest::Client::new()` hits | H19, M30 | medium | medium | Introduce a single outbound client factory and migrate in batches. |

## 4. Priority scoring

| id | finding | impact | effort | risk | confidence | score | phase |
|----|---------|--------|--------|------|------------|-------|-------|
| C7 | MCP SSRF guard | 5 | 2 | 3 | 5 | 20 | P0 |
| C20 | Stable complete cache key | 5 | 2 | 3 | 5 | 20 | P0 |
| C9 | Debug-log PII | 5 | 2 | 2 | 5 | 21 | P0 |
| H11 | JWT secret hard rules | 4 | 1 | 2 | 5 | 17 | P0 |
| C13 | Persist budgets | 5 | 3 | 4 | 5 | 18 | P0 |
| C14 | Distributed rate limit | 5 | 3 | 4 | 5 | 18 | P0 |
| C8 | pgvector SQL hardening | 4 | 3 | 3 | 4 | 10 | P0 |
| C4/C19 | Streaming public type compatibility | 5 | 2 | 3 | 5 | 20 | P1 |
| C1/C3/H1/H4/H5 | Anthropic thinking/tool/cache correctness | 5 | 4 | 4 | 5 | 17 | P1 |
| C2 | Bedrock tool/result correctness | 5 | 3 | 3 | 5 | 19 | P1 |
| C5 | Gemini tool finish reason | 4 | 1 | 2 | 5 | 17 | P1 |
| C6 | LiteLLM helper forwards tool/schema params | 4 | 2 | 2 | 5 | 16 | P1 |
| H19 | Shared outbound client migration | 3 | 4 | 3 | 5 | 8 | P1 |
| C11/C12/H18/H22 | Pricing convergence | 5 | 5 | 5 | 4 | 10 | P2 |
| C15 | Team/user convergence | 5 | 5 | 5 | 4 | 10 | P2 |
| C17/C18/H13/H17 | Provider registry convergence | 5 | 5 | 5 | 4 | 10 | P2 |
| H20/H21 | Config strictness | 4 | 3 | 3 | 5 | 14 | P2 |
| M-series | Hygiene sweeps | 2-3 | 1-3 | 1-3 | 3-5 | varies | P3 |

## 5. Execution phases

## Phase A - Evidence, baselines, and guardrails

Goal: make the remediation campaign reproducible before touching runtime behavior.

### Step A1 Persist raw audit evidence

- status: `completed`
- Target: raw agent reports are stored in the repo and counts are traceable.
- Expected changes:
  - `docs/audit-2026-05-01/agent-1-api-data-integrity.md`
  - `docs/audit-2026-05-01/agent-2-error-security.md`
  - `docs/audit-2026-05-01/agent-3-architecture.md`
  - `docs/audit-2026-05-01/agent-4-config-persistence.md`
  - `docs/audit-2026-05-01/README.md`
- Detailed changes:
  - Copy the raw findings into durable markdown files.
  - Add a README explaining 77 raw findings versus 72 deduplicated remediation items.
  - Link `PLAN_AUDIT_REMEDIATION.md`, `PLAN_AUDIT_EXECUTION.md`, and this plan.
- step-level test command:
  - `rg -n "Total|Critical|High|Medium|C1|H1|M1" docs/audit-2026-05-01`
  - `awk '/^\\| C[0-9]+ /{c++} /^\\| H[0-9]+ /{h++} /^\\| M[0-9]+ /{m++} END{print c,h,m,c+h+m}' PLAN_AUDIT_REMEDIATION.md`
- Completion judgment:
  - Raw evidence is no longer chat-only.
  - Deduped count is visible and matches 20/22/30.

### Step A2 Capture baseline health and known failures

- status: `completed`
- Target: every future PR can compare against a known baseline.
- Expected changes:
  - `docs/audit-2026-05-01/baseline-2026-05-01.md`
- Detailed changes:
  - Record exact output summaries for fmt, clippy, tests, lite check, reqwest count, god-file LOC.
  - If a command fails, classify whether it is existing failure or remediation-induced.
- step-level test command:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all-features`
  - `cargo check --no-default-features --features lite`
  - `rg -n "reqwest::Client::new\\(\\)" src/ -g '!**/*test*' -g '!**/tests.rs' | wc -l`
- Completion judgment:
  - Baseline document has command, result, and short interpretation for each check.

### Step A3 Add low-noise guard scripts

- status: `completed`
- Target: prevent reintroducing known classes of bugs while fixes land.
- Expected changes:
  - `scripts/guards/check_outbound_http_clients.sh`
  - `scripts/guards/check_log_pii.sh`
  - optional CI workflow references after local scripts prove stable
- Detailed changes:
  - First version may be advisory; fail only on new obvious violations.
  - Exclude tests and comments where appropriate.
- step-level test command:
  - `bash scripts/guards/check_outbound_http_clients.sh || true`
  - `bash scripts/guards/check_log_pii.sh || true`
  - `shellcheck scripts/guards/check_outbound_http_clients.sh scripts/guards/check_log_pii.sh`
- Completion judgment:
  - Scripts produce actionable output and do not block current baseline unexpectedly.

## Phase B - Foundations used by multiple fixes

Goal: build shared primitives once so P0/P1 fixes do not duplicate logic.

### Step B1 Shared outbound HTTP client factory

- status: `completed`
- Covers: H19, M30, partial C7/M7
- Expected changes:
  - `src/core/http/mod.rs`
  - `src/core/http/outbound.rs`
  - `src/core/mod.rs` if needed for module export
- Detailed changes:
  - Add `OutboundProfile`, `build_outbound_client`, and `default_outbound_client`.
  - Configure connect timeout, request timeout, pool idle timeout, pool per host, and user-agent.
  - Do not migrate callers in this step.
- step-level test command:
  - `cargo test core::http`
  - `cargo check --all-features`
- Completion judgment:
  - Helper builds and can be imported by downstream fixes.

### Step B2 Shared SSRF guard module

- status: `completed`
- Covers: C7, M7
- Expected changes:
  - `src/core/net/mod.rs`
  - `src/core/net/ssrf_guard.rs`
  - `src/core/a2a/config.rs`
  - `src/core/mcp/config.rs` later in C7
- Detailed changes:
  - Promote A2A host/IP guard into shared code.
  - Use `url::Url` parsing instead of ad hoc host extraction.
  - Include test cases for localhost, private IPv4, link-local, IPv6 loopback, ULA, and public hosts.
  - Keep DNS-rebinding resolver hardening as M7 follow-up.
- step-level test command:
  - `cargo test ssrf_guard`
  - `cargo test core::a2a::config`
- Completion judgment:
  - A2A still validates as before and shared module has direct unit coverage.

### Step B3 Cache key policy helper

- status: `completed`
- Covers: C20, M29
- Expected changes:
  - `src/core/cache/key_policy.rs`
  - `src/core/cache/key_generator.rs`
  - `Cargo.toml` if `blake3` dependency is not already available
- Detailed changes:
  - Add `CACHE_KEY_SCHEMA_VERSION`.
  - Canonicalize request-relevant JSON using stable sorted keys.
  - Hash model, messages, tools, tool_choice, response_format schema, thinking/reasoning params, and relevant extra params.
  - Keep user-specific scoping explicit.
- step-level test command:
  - `cargo test key_policy`
  - `cargo test key_generator`
  - `cargo check --all-features`
- Completion judgment:
  - Tests prove different tool/schema/version inputs produce different keys and unchanged inputs are stable.

### Step B4 Provider registry skeleton

- status: `completed`
- Covers: C17, C18, H7, H13, H17, M23
- Expected changes:
  - `src/core/providers/registry/types.rs`
  - `src/core/providers/registry/mod.rs`
  - tests under provider registry modules
- Detailed changes:
  - Add table type with canonical name, aliases, kind, dispatchability, and catalog-only classification.
  - Do not replace the existing factory in this step.
  - Add a coverage test that compares `ProviderType` parse/display/factory lists to the table.
- step-level test command:
  - `cargo test provider_registry`
  - `cargo test provider_type`
- Completion judgment:
  - Registry table exists and drift is observable before refactor.

## Phase C - P0 stop-the-bleeding fixes

Goal: close active security, quota, privacy, and cache correctness holes first.

### Step C1 C7 MCP SSRF guard

- status: `completed`
- Expected changes:
  - `src/core/mcp/config.rs`
  - `src/core/net/ssrf_guard.rs`
  - `src/core/mcp/tests` or module tests
- Detailed changes:
  - Parse HTTP/SSE/WebSocket URLs with `Url`.
  - Apply shared outbound URL guard after scheme validation.
  - Add `LITELLM_MCP_ALLOW_PRIVATE_TARGETS=1` dev bypass only if owner accepts local-MCP compatibility risk.
- step-level test command:
  - `cargo test mcp::config`
  - `cargo test ssrf_guard`
- Completion judgment:
  - MCP rejects localhost, private, and metadata IPs by default.

### Step C2 C20 cache key rewrite

- status: `completed`
- Expected changes:
  - `src/core/cache/key_generator.rs`
  - `src/core/cache/types.rs`
  - `src/core/cache/key_policy.rs`
- Detailed changes:
  - Replace `DefaultHasher` with deterministic versioned `blake3`.
  - Add schema version prefix.
  - Include omitted request dimensions.
  - Document Redis cold-cache effect.
- step-level test command:
  - `cargo test cache::key`
  - `cargo test core::cache`
- Completion judgment:
  - Determinism and dimension tests pass.

### Step C3 C9 debug-log PII

- status: `completed`
- Expected changes:
  - `src/core/providers/bedrock/client.rs`
  - `src/core/providers/milvus/provider.rs`
  - `src/core/audit/config.rs`
  - `scripts/guards/check_log_pii.sh`
- Detailed changes:
  - Stop logging full request bodies at debug.
  - Replace with request id, provider, model, safe size, and redacted hints.
  - Extend redactor for `Bearer`, JWT-like strings, `AKIA`, `gw-`, `sk-ant-`, and Authorization headers.
- step-level test command:
  - `cargo test audit`
  - `bash scripts/guards/check_log_pii.sh`
- Completion judgment:
  - No production debug log writes full request body by default.

### Step C4 H11 JWT hard-rules

- status: `completed`
- Expected changes:
  - `src/config/models/auth.rs`
  - `config/gateway.yaml.example`
  - config validation tests
- Detailed changes:
  - Deny lowercase-only secrets as hard error if not already hard.
  - Deny placeholder literals containing `Replace`, `change-me`, `your-secret-key`.
  - Replace example value with `${LITELLM_JWT_SECRET}` and document env requirement.
- step-level test command:
  - `cargo test config::models::auth`
  - `cargo test config::validation`
- Completion judgment:
  - Example placeholder cannot pass validation as a real secret.

### Step C5 C8 pgvector SQL hardening

- status: `completed`
- Expected changes:
  - `src/core/providers/pg_vector/config.rs`
  - `src/core/providers/pg_vector/provider.rs`
- Detailed changes:
  - Tighten schema/table validation to `^[A-Za-z_][A-Za-z0-9_]{0,62}$`.
  - Centralize identifier quoting after validation.
  - Parameterize threshold and limit where the executor supports binding.
  - Delete or isolate `to_sql_string`; no hand-rolled escaping for values.
- step-level test command:
  - `cargo test pg_vector`
  - optional with Postgres: `cargo test pgvector -- --ignored`
- Completion judgment:
  - Malicious identifiers fail validation and query values are not string-concatenated.

### Step C6 H9 auth-rate-limiter cap and cleanup

- status: `completed`
- Expected changes:
  - `src/server/middleware/auth_rate_limiter.rs`
  - `src/server/http.rs` or server startup module
- Detailed changes:
  - Add max entries and deterministic eviction/cleanup.
  - Run cleanup from background task instead of relying only on auth-hit paths.
  - Add tests for cap enforcement and cleanup.
- step-level test command:
  - `cargo test auth_rate_limiter`
  - `cargo test server::middleware`
- Completion judgment:
  - Unbounded memory growth path is closed.

### Step C7 C13 budget persistence

- status: `completed`
- Expected changes:
  - new migration for budget spend snapshots
  - `src/core/budget/provider_limits.rs`
  - `src/server/state.rs`
  - storage repository modules
- Detailed changes:
  - Persist provider/model spend counters and restore on startup.
  - Decide async flush cadence and acceptable lag.
  - Record schema-mismatch behavior as fail-safe and observable.
- step-level test command:
  - `cargo test budget`
  - integration/manual: spend, restart, assert spend restored
- Completion judgment:
  - Restart no longer resets spend accounting.
- Owner decision:
  - Confirm whether <=30s eventual consistency is acceptable or synchronous write is required.

### Step C8 C14 Redis-backed distributed rate limiter

- status: `completed`
- Expected changes:
  - `src/core/rate_limiter/`
  - `src/storage/redis/`
  - rate-limit config models
- Detailed changes:
  - Add Redis strategy with atomic Lua or equivalent server-side operation.
  - Keep in-process limiter as fallback/configurable mode.
  - Add multi-replica simulation test.
- step-level test command:
  - `cargo test rate_limiter`
  - Redis integration if available: `cargo test redis_rate_limit -- --ignored`
- Completion judgment:
  - Two gateway instances share the same rate limit state under Redis mode.

## Phase D - P1 provider contract correctness

Goal: make OpenAI-compatible clients receive the same semantic information providers returned.

### Step D1 C4 + C19 streaming public type expansion

- status: `completed`
- Expected changes:
  - `src/core/streaming/types.rs`
  - `src/server/routes/ai/chat.rs`
  - `src/core/providers/base/sse.rs`
  - tests for serialized chunks
- Detailed changes:
  - Add `thinking`, `tool_call_id`, `refusal`, and `function_call` where compatible.
  - Add `#[serde(skip_serializing_if = "Option::is_none")]` to optional streaming fields.
  - Update conversion code so new fields are not dropped.
- step-level test command:
  - `cargo test streaming::types`
  - `cargo test routes::ai::chat`
- Completion judgment:
  - Serialized chunks omit nulls and preserve reasoning/tool metadata.

### Step D2 H1 usage thinking/cache details

- status: `completed`
- Expected changes:
  - `src/server/routes/ai/chat.rs`
  - usage response structs if needed
- Detailed changes:
  - Forward `thinking_usage`.
  - Add cache creation/read token detail fields after C3 provider work defines canonical mapping.
- step-level test command:
  - `cargo test convert_usage`
  - `cargo test usage`
- Completion judgment:
  - Usage round-trip retains thinking/cache details.

### Step D3 C1 Anthropic streaming deltas

- status: `completed`
- Expected changes:
  - `src/core/providers/base/sse.rs`
  - Anthropic streaming fixtures/tests
- Detailed changes:
  - Parse `content_block_start`, `input_json_delta`, `thinking_delta`, `signature_delta`.
  - Accumulate tool arguments correctly.
  - Emit unknown event warnings instead of silent defaulting.
- step-level test command:
  - `cargo test anthropic_stream`
  - fixture test: `anthropic_stream_tool_use`
- Completion judgment:
  - Claude streaming tool arguments and thinking content reach public chunks.

### Step D4 C3 Anthropic non-stream thinking and cache usage

- status: `completed`
- Expected changes:
  - `src/core/providers/anthropic/client.rs`
  - `src/core/types/thinking.rs` if needed
  - usage tests
- Detailed changes:
  - Preserve `thinking` blocks.
  - Map `cache_creation_input_tokens` and `cache_read_input_tokens`.
  - Add stop-reason mapping for `stop_sequence`; handle refusal/pause as explicit observable states.
- step-level test command:
  - `cargo test anthropic_client`
  - `cargo test anthropic_usage`
- Completion judgment:
  - Non-stream Claude thinking and cache usage are not dropped.

### Step D5 H4 + H5 Anthropic message transforms

- status: `completed`
- Expected changes:
  - `src/core/providers/anthropic/client.rs`
- Detailed changes:
  - Preserve assistant text content when adding `tool_use` blocks.
  - Emit tool-role messages as `tool_result` blocks with `tool_use_id`.
  - Add tests for assistant content plus tool call and tool-result turn.
- step-level test command:
  - `cargo test anthropic_transform_messages`
- Completion judgment:
  - Anthropic multi-turn tool conversations keep every turn.

### Step D6 C2 Bedrock Converse tool/result content

- status: `completed`
- Expected changes:
  - `src/core/providers/bedrock/chat/converse.rs`
  - Bedrock tests/fixtures
- Detailed changes:
  - Map Tool/Function-role messages to `ToolResult`.
  - Map ToolResult content parts.
  - For unsupported Image/Audio/Document interim behavior, return explicit not-implemented rather than drop.
- step-level test command:
  - `cargo test bedrock_converse`
  - fixture test: `bedrock_tool_result_round_trip`
- Completion judgment:
  - Bedrock tool conversations do not lose turns silently.

### Step D7 C5 Gemini finish reason

- status: `completed`
- Expected changes:
  - `src/core/providers/gemini/client.rs`
- Detailed changes:
  - If Gemini response contains function calls, emit `FinishReason::ToolCalls` even when provider finish reason says STOP.
  - Add regression test.
- step-level test command:
  - `cargo test gemini_tool`
  - `cargo test gemini_client`
- Completion judgment:
  - OpenAI-compatible tool loops can continue on Gemini.

### Step D8 C6 LiteLLM helper params

- status: `completed`
- Expected changes:
  - `src/core/completion/types.rs`
  - `src/core/completion/conversion.rs`
  - helper tests
- Detailed changes:
  - Forward `tools`, `tool_choice`, `response_format`, thinking/reasoning, and metadata.
  - Keep Python LiteLLM compatibility surface documented.
- step-level test command:
  - `cargo test completion::conversion`
- Completion judgment:
  - Helper API no longer hardcodes tool/schema params to `None`.

### Step D9 H2/H3/H6 request boundary fields

- status: `completed`
- Expected changes:
  - `src/core/models/openai/requests.rs`
  - `src/server/routes/ai/chat.rs`
- Detailed changes:
  - Add `parallel_tool_calls`, `extra_body`/flatten map, prediction, safety settings, cache control.
  - Widen seed path to avoid `u32` to `i32` wrap.
  - Decide whether `response_type` is wired or removed.
- step-level test command:
  - `cargo test openai_requests`
  - `cargo test routes::ai::chat`
- Completion judgment:
  - HTTP boundary preserves provider-specific knobs without unsafe casts.

### Step D10 H7/H8 provider capability routes

- status: `completed`
- Expected changes:
  - `src/core/providers/factory/registry.rs`
  - `src/core/providers/mod.rs`
  - capability tests
- Detailed changes:
  - Fix catalog guard ordering or remove duplicate explicit branches.
  - Route embeddings/images through all provider variants, returning explicit not-implemented per provider where unsupported.
- step-level test command:
  - `cargo test provider_factory`
  - `cargo test provider_capabilities`
- Completion judgment:
  - Tier-2 provider branches are not shadowed and capability failures are explicit.

### Step D11 H10/H12 error-handling polish

- status: `completed`
- Expected changes:
  - `src/server/http.rs`
  - `src/core/audit/logger.rs`
- Detailed changes:
  - Replace app-factory CORS `expect` with graceful validation path.
  - Log audit flush/drop failures and expose metric/counter if available.
- step-level test command:
  - `cargo test server::http`
  - `cargo test audit::logger`
- Completion judgment:
  - Hot reload cannot crash worker on invalid CORS config; audit failures are observable.

### Step D12 H19 outbound HTTP migration batches

- status: `completed`
- Expected changes:
  - first batch: services, monitoring, observability
  - provider batches: codestral, databricks, baseten, deepgram, github_copilot, oci, azure_ai, gradient_ai, vertex_ai, exa_ai, firecrawl, v0, azure, qdrant, a2a, rerank, OpenAI API methods, pg_vector, watsonx, GitHub, Amazon Nova, Ollama, Replicate, Clarifai, Stability, ElevenLabs, Snowflake, AI21, Empower, Datarobot
- Detailed changes:
  - Replace ad hoc clients with `default_outbound_client().clone()` or provider-specific `OnceLock<Client>` built with shared profile.
  - Keep batches small and rerun count after each.
- step-level test command:
  - `cargo check --all-features`
  - `rg -n "reqwest::Client::new\\(\\)" src/ -g '!**/*test*' -g '!**/tests.rs' | wc -l`
- Completion judgment:
  - Only intentional exceptions remain and guard script enforces the policy.

## Phase E - P2 architecture and persistence convergence

Goal: remove parallel systems after correctness/security work is stable.

### Step E1 C12 pricing migration registration

- status: `completed`
- Expected changes:
  - `src/storage/database/migration/mod.rs`
  - `src/storage/database/entities/mod.rs`
  - `src/storage/database/entities/pricing.rs`
  - optional missing `pricing_history` module fix or split
- Detailed changes:
  - Register pricing migration.
  - Resolve phantom `pricing_history` references.
  - Add migration smoke test.
- step-level test command:
  - `cargo test migration`
  - `cargo check --all-features`
- Completion judgment:
  - Fresh DB creates pricing tables and code compiles with pricing entities active.

### Step E2 H18/H22 pricing source resolver

- status: `completed`
- Expected changes:
  - `config/pricing.yaml`
  - `src/config/models/gateway.rs`
  - `config/gateway.yaml.example`
- Detailed changes:
  - Choose one pricing-source path resolver.
  - Delete unused `config/pricing.yaml` or wire it through the chosen resolver.
  - Document precedence: env var, data dir, config fallback.
- step-level test command:
  - `cargo test gateway_config`
  - `rg -n "pricing.yaml|model_prices_extended" src config`
- Completion judgment:
  - Example and runtime resolve the same path.

### Step E3 C11 pricing system unification

- status: `in_progress`
- Expected changes:
  - `src/core/cost/`
  - `src/core/providers/base/pricing.rs`
  - `src/services/pricing/`
  - provider pricing callers
- Detailed changes:
  - Pick `services::pricing` or a new canonical pricing service as the single runtime source.
  - Migrate callers away from hardcoded tables in waves.
  - Keep compatibility adapters until all callers are moved.
  - Delete duplicated pricing structs/functions only after coverage proves parity.
- step-level test command:
  - `cargo test pricing`
  - `cargo test cost`
  - `rg "ModelPricing\\b" src/`
- Completion judgment:
  - One pricing model/service remains and all cost entry points agree.

### Step E4 C15 team/user convergence

- status: `pending`
- Expected changes:
  - `src/storage/database/migration/m20240301_000001_create_user_management_tables.rs`
  - `src/storage/database/migration/m20240301_000002_create_teams_table.rs`
  - `src/storage/database/seaorm_db/user_management_ops.rs`
  - `src/storage/database/seaorm_db/team_repository.rs`
  - routes using either system
- Detailed changes:
  - Inventory endpoints and repositories using `um_*` vs canonical `users/teams`.
  - Add copy migration from legacy to canonical.
  - Add parity tests.
  - Drop legacy tables only in a later PR after owner confirmation.
- step-level test command:
  - `cargo test team_repository`
  - `cargo test user_management`
  - DB migration integration if available
- Completion judgment:
  - A team/user created by any API is visible through the canonical repository.
- Owner decision:
  - Confirm no deployment depends on raw SQL against `um_*` names before drop migration.

### Step E5 C16 AuthConfig default consistency

- status: `completed`
- Expected changes:
  - `src/config/models/auth.rs`
  - config builder/tests
- Detailed changes:
  - Decide between `enable_jwt: false` by default or `jwt_secret: Option<String>`.
  - Ensure default config either validates or clearly represents disabled JWT.
- step-level test command:
  - `cargo test auth_config_default`
  - `cargo test config::models::auth`
- Completion judgment:
  - `AuthConfig::default()` is not invalid by construction.

### Step E6 C17/C18 provider source-of-truth convergence

- status: `pending`
- Expected changes:
  - `src/core/providers/provider_type.rs`
  - `src/core/providers/factory/registry.rs`
  - `src/core/providers/mod.rs`
  - `src/core/providers/registry/`
- Detailed changes:
  - Replace scattered parse/display/support lists with registry-driven functions.
  - Generate or derive factory-supported set from registry metadata.
  - Keep enum dispatch if performance/simplicity decision favors it; otherwise plan a dyn-safe trait refactor separately.
- step-level test command:
  - `cargo test provider_type`
  - `cargo test provider_factory`
- Completion judgment:
  - Parseable provider types cannot be silently unreachable.
- Owner decision:
  - Confirm enum dispatch versus trait-object direction.

### Step E7 H13/H17 orphan provider decisions

- status: `pending`
- Expected changes:
  - provider modules under `src/core/providers/`
  - provider registry/factory
- Detailed changes:
  - For each orphan provider dir, decide `wire`, `catalog-only`, `stub`, or `delete`.
  - Keep likely Tier-2 providers such as Gemini/Cohere/Ollama/HuggingFace if owner wants them.
  - Delete only in small reversible batches.
- step-level test command:
  - `cargo test provider_factory`
  - `cargo check --all-features`
  - `find src/core/providers -maxdepth 2 -name mod.rs`
- Completion judgment:
  - Every provider directory has a declared lifecycle state.

### Step E8 H14 SDK delegates to UnifiedRouter

- status: `pending`
- Expected changes:
  - `src/sdk/client/routing.rs`
  - SDK client tests
  - `src/core/router/` if public API needs a small adapter
- Detailed changes:
  - Replace SDK-specific load balancing with a wrapper around the core router.
  - Add fixed-seed provider selection parity test.
- step-level test command:
  - `cargo test sdk`
  - `cargo test router`
- Completion judgment:
  - SDK and gateway choose providers consistently.

### Step E9 H15 god-file splits

- status: `pending`
- Expected changes:
  - `src/core/cost/calculator.rs`
  - `src/core/providers/anthropic/models.rs`
  - `src/core/providers/anthropic/client.rs`
  - `src/core/providers/base/sse.rs`
- Detailed changes:
  - Split only after behavior fixes and pricing convergence reduce churn.
  - Move tests with the modules.
  - Preserve public exports during split.
- step-level test command:
  - `cargo test anthropic`
  - `cargo test sse`
  - `cargo test cost`
  - `wc -l` for affected files
- Completion judgment:
  - Large files are below the agreed ceiling or have owner-approved exceptions.

### Step E10 H20/H21 config strictness

- status: `pending`
- Expected changes:
  - `src/config/mod.rs`
  - `src/config/models/*.rs`
  - config tests
- Detailed changes:
  - Missing `${ENV_VAR}` values fail with explicit missing list.
  - Add `#[serde(deny_unknown_fields)]` to top-level and major config structs.
  - Add config-lint test for `config/gateway.yaml.example`.
- step-level test command:
  - `cargo test config`
  - `cargo test gateway_yaml_example`
- Completion judgment:
  - Config typos and unresolved env vars fail before runtime.

## Phase F - P3 Medium sweeps

Goal: close remaining medium issues after the main architecture is stable.

### Step F1 Stream/provider serialization sweep

- status: `pending`
- Covers: M1, M3, M4, M5, M6, M28
- Expected changes:
  - `src/server/routes/ai/chat.rs`
  - `src/core/completion/conversion.rs`
  - `src/core/providers/bedrock/chat/converse.rs`
  - `src/core/providers/anthropic/client.rs`
  - `src/core/providers/gemini/client.rs`
  - `src/core/streaming/providers.rs`
- step-level test command:
  - `cargo test routes::ai::chat`
  - `cargo test bedrock anthropic gemini`
- Completion judgment:
  - Medium provider drops are either fixed or explicitly not-implemented.

### Step F2 Security/privacy hygiene sweep

- status: `pending`
- Covers: M8, M9, M10, M11, M12
- Expected changes:
  - `src/core/mcp/server.rs`
  - `src/core/audit/config.rs`
  - `src/server/routes/auth/password.rs` or `src/auth/password.rs`
  - `src/core/secret_managers/file.rs`
  - `src/core/providers/openai_like/provider.rs`
- step-level test command:
  - `cargo test mcp audit auth secret_managers openai_like`
- Completion judgment:
  - Known medium privacy/timing/info-leak paths are closed.

### Step F3 Runtime/config hygiene sweep

- status: `pending`
- Covers: M13, M14, M15, M20, M21, M22
- Expected changes:
  - `src/storage/database/mod.rs`
  - `src/storage/files/mod.rs`
  - `src/server/state.rs`
  - `Cargo.toml`
  - CI workflows
  - `config/gateway.yaml.example`
- step-level test command:
  - `cargo check --no-default-features --features lite`
  - `cargo test config storage`
- Completion judgment:
  - Feature flags and runtime paths are coherent and covered by CI.

### Step F4 Provider/cost API cleanup sweep

- status: `pending`
- Covers: M16, M17, M18, M19, M23, M24, M25, M29
- Expected changes:
  - `src/core/providers/mod.rs`
  - `src/core/providers/openai_like/provider.rs`
  - `src/core/mcp/mod.rs`
  - `src/core/providers/macros/`
  - `Cargo.toml`
  - `src/lib.rs`
  - `src/core/cache/key_generator.rs`
- step-level test command:
  - `cargo test providers mcp cache`
  - `cargo check --all-features`
- Completion judgment:
  - Dead structs/macros/aliases are either deleted or documented as intentional public API.

## 6. Regression matrix

Run at the end of every Critical or High PR:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
bash scripts/guards/check_pr_scope.sh
bash scripts/guards/check_pr_overlap.sh
```

Run at the end of P0:

```bash
cargo test ssrf_guard
cargo test cache::key
cargo test auth_rate_limiter
cargo test budget
cargo test rate_limiter
cargo test pg_vector
rg -n "debug!.*body|info!.*body" src/
```

Run at the end of P1:

```bash
cargo test streaming::types
cargo test anthropic
cargo test bedrock
cargo test gemini
cargo test completion::conversion
rg -n "reqwest::Client::new\(\)" src/ -g '!**/*test*' -g '!**/tests.rs'
```

Run at the end of P2:

```bash
cargo test migration
cargo test pricing
cargo test team_repository
cargo test provider_factory
cargo test config
rg "ModelPricing\b" src/
rg "um_users|um_teams" src/
```

Final closure:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo check --no-default-features --features lite
bash scripts/guards/check_pr_scope.sh
bash scripts/guards/check_pr_overlap.sh
```

## 7. Parallel lane map

No parallel agents are launched by this plan. If the owner chooses to parallelize later, use this lane map:

| lane | owner | allowed files | blocked by | notes |
|------|-------|---------------|------------|-------|
| Lane security | local/worker | `src/core/net`, `src/core/mcp`, `src/core/a2a`, auth limiter, audit redaction | B2 | Owns C7, C9, H9, H11, F2. |
| Lane provider | local/worker | Anthropic, Bedrock, Gemini, streaming types, completion helper | D1 first | Owns C1-C6, C19, H1-H8. |
| Lane persistence | local/worker | migrations, entities, budget, rate limiter, team/user repos | A2 | Owns C12-C15, C13/C14. |
| Lane pricing | local/worker | pricing service/cost/provider pricing/config pricing files | E1/E2 | Owns C11, H18, H22, M16/M17/M26. |
| Lane registry | local/worker | provider registry/type/factory/provider module lifecycle | B4 | Owns C17/C18, H13/H17/M23. |
| Lane hygiene | local/worker | medium sweep files only | related phase complete | Owns M-series. |

## 8. Owner decisions required

| decision | needed before | default recommendation |
|----------|---------------|------------------------|
| Allow local/private MCP URLs through env bypass? | C7 | Yes, but disabled by default and loudly warned. |
| Budget persistence write model: async flush or sync write? | C13 | Async flush <=30s lag unless strict billing guarantees are required. |
| Redis rate limit fallback behavior when Redis is down | C14 | Fail closed for paid/quota APIs, configurable fail open for local dev. |
| Pricing canonical system | C11 | Use `services::pricing` as runtime service; convert `core::cost` to adapter then delete duplicates. |
| Drop or preserve `config/pricing.yaml` | H18/H22 | Delete unless owner wants YAML pricing config as first-class input. |
| Canonical team/user tables | C15 | Keep `users/teams`; migrate `um_*` data in two steps. |
| Provider dispatch direction | C17/C18 | Keep enum dispatch for now; registry table removes drift without dyn-safety rewrite. |
| Orphan provider lifecycle | H13 | Wire likely Tier-2 providers, delete obvious stubs in small batches. |

## 9. Execution Log

- 2026-05-02
  - Step E5 AuthConfig default consistency: `completed`
    - Modified files:
      - `src/config/models/auth.rs`
      - `src/config/models/gateway_tests.rs`
      - `src/config/validation/tests.rs`
      - `tests/integration/config_validation_tests.rs`
    - Main changes:
      - Changed the model-level and serde default for `enable_jwt` to `false`, while preserving API key authentication as enabled by default.
      - Kept explicit JWT configurations fail-closed: empty, short, placeholder, or weak secrets still fail whenever JWT is enabled.
      - Added a regression test proving configs that omit `enable_jwt` and `jwt_secret` deserialize with JWT disabled and validate without a generated secret.
    - Execute tests:
      - `cargo fmt --all -- --check` -> pass
      - `cargo test auth_config_default` -> pass (`1` matching test)
      - `cargo test config::models::auth` -> pass (`24` tests)
      - `cargo test config::validation` -> pass (`158` tests)
      - `cargo test config_validation_tests` -> pass (`31` integration tests)
      - `cargo test test_gateway_config_validate_empty_jwt_secret` -> pass (`1` matching test)
      - `cargo test test_gateway_config_empty_jwt_secret` -> pass (`1` integration matching test)
      - `git diff --check` -> pass
      - `cargo check --all-features` -> pass
      - `cargo clippy --lib --tests --bins --all-features -- -D warnings --force-warn clippy::collapsible-if` -> pass (`collapsible_if` remains warning by command design)
  - Step E3 Bedrock pricing convergence: `in_progress`
    - Modified files:
      - `src/core/providers/bedrock/utils/cost.rs`
      - `src/core/providers/bedrock/provider.rs`
    - Main changes:
      - Added a compatibility adapter from Bedrock's AWS-model static pricing entries into the shared `core::cost::types::ModelPricing` shape.
      - Routed Bedrock provider model metadata through the shared cost model shape while preserving the existing Bedrock pricing table as the data source for AWS model IDs.
      - Added a regression test proving the shared-shape adapter preserves model ID, input/output rates, and currency.
    - Execute tests:
      - `cargo test --features providers-extra core::providers::bedrock::utils::cost::tests::test_core_model_pricing_lookup -- --exact` -> pass (`1` test)
      - `cargo test --features providers-extra core::providers::bedrock::utils::cost::tests::` -> pass (`50` tests)
      - `cargo test --features providers-extra core::providers::bedrock::provider_tests::` -> pass (`48` tests)
      - `cargo fmt --all -- --check` -> pass
      - `git diff --check` -> pass
      - `cargo test pricing` -> pass (`175` lib filtered tests, `1` integration filtered test)
      - `cargo test cost` -> pass (`326` lib filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo clippy --lib --tests --bins --all-features -- -D warnings --force-warn clippy::collapsible-if` -> pass (`collapsible_if` remains warning by command design)
  - Step E3 Anthropic/Gemini registry convergence: `in_progress`
    - Modified files:
      - `src/core/providers/anthropic/models.rs`
      - `src/core/providers/gemini/models.rs`
    - Main changes:
      - Added adapters from Anthropic and Gemini per-million-token registry pricing into the shared `core::cost::types::ModelPricing` shape.
      - Routed registry fallback cost calculations through the shared cost model shape while preserving provider-local metadata needed for Anthropic batch discounts and Gemini image/video/audio costs.
      - Added regression tests for unit conversion, cache pricing conversion, image pricing conversion, and unchanged cost behavior.
    - Execute tests:
      - `cargo test core::providers::anthropic::models::tests::` -> pass (`5` tests)
      - `cargo test --features providers-extended core::providers::gemini::models::tests::` -> pass (`25` tests)
      - `cargo fmt --all -- --check` -> pass
      - `git diff --check` -> pass
      - `cargo test pricing` -> pass (`176` lib filtered tests, `1` integration filtered test)
      - `cargo test cost` -> pass (`326` lib filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo clippy --lib --tests --bins --all-features -- -D warnings --force-warn clippy::collapsible-if` -> pass (`collapsible_if` remains warning by command design)
  - Step E3 Spark registry convergence: `in_progress`
    - Modified files:
      - `src/core/providers/spark/model_info.rs`
    - Main changes:
      - Added an adapter from Spark per-million-token registry pricing into the shared `core::cost::types::ModelPricing` shape.
      - Routed Spark fallback cost calculation through the shared cost model shape.
      - Added a regression test for Spark unit conversion and currency preservation.
    - Execute tests:
      - `cargo test --features providers-extended core::providers::spark::model_info::tests::` -> pass (`6` tests)
      - `cargo fmt --all -- --check` -> pass
      - `git diff --check` -> pass
      - `cargo test pricing` -> pass (`176` lib filtered tests, `1` integration filtered test)
      - `cargo test cost` -> pass (`326` lib filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo clippy --lib --tests --bins --all-features -- -D warnings --force-warn clippy::collapsible-if` -> pass (`collapsible_if` remains warning by command design)
- 2026-05-01
  - Step E3 OpenAI cost convergence: `in_progress`
    - Modified files:
      - `src/core/providers/openai/client.rs`
      - `src/core/providers/openai/client_tests.rs`
    - Main changes:
      - Routed `OpenAIProvider::calculate_cost` through shared `core::cost::calculator::generic_cost_per_token`.
      - Kept the previous default-model-info fallback so unknown/custom OpenAI-compatible model names still return the compatibility cost instead of hard failing.
      - Strengthened the provider cost test to prove `gpt-4o-mini` no longer prices as zero.
    - Execute tests:
      - `cargo test core::providers::openai::client_tests::test_calculate_cost` -> pass (`2` matching tests)
      - `cargo fmt --all -- --check` -> pass
      - `cargo test pricing` -> pass (`175` lib filtered tests, `1` integration filtered test)
      - `cargo clippy --lib --tests --bins --features "postgres sqlite redis s3 metrics tracing websockets analytics" -- -D warnings --force-warn clippy::collapsible-if` -> pass
  - Step E3 Gemini pricing convergence: `in_progress`
    - Modified files:
      - `src/core/providers/gemini/models.rs`
    - Main changes:
      - Routed Gemini's basic `CostCalculator::calculate_cost` through shared `core::cost::calculator::generic_cost_per_token` using the `vertex_ai` pricing source.
      - Kept the Gemini registry fallback so provider-only model aliases such as `gemini-1.0-pro` remain priced even when the shared catalog does not have an exact entry.
    - Execute tests:
      - `cargo test --features providers-extended core::providers::gemini::models::tests::test_cost_calculation` -> pass (`3` matching tests)
      - `cargo test --features providers-extended core::providers::gemini::models::tests::test_cost_calculation_keeps_registry_fallback` -> pass (`covered by the same filtered run`)
      - `cargo fmt --all -- --check` -> pass
      - `cargo test pricing` -> pass (`175` lib filtered tests, `1` integration filtered test)
      - `cargo check --all-features` -> pass
      - `cargo clippy --lib --tests --bins --features "postgres sqlite redis s3 metrics tracing websockets analytics providers-extended" -- -D warnings --force-warn clippy::collapsible-if` -> pass
  - Step E3 provider pricing convergence: `in_progress`
    - Modified files:
      - `src/core/providers/openai/client.rs`
      - `src/core/providers/openai/client_tests.rs`
      - `src/core/providers/anthropic/models.rs`
      - `src/core/providers/anthropic/mod.rs`
    - Main changes:
      - Routed `OpenAIProvider::get_model_pricing` through the shared `core::cost::calculator::get_model_pricing` source before using OpenAI's static registry as a compatibility fallback.
      - Fixed the provider-level pricing helper that previously returned `None` because `get_model_info` intentionally emits default model metadata with no pricing fields.
      - Added a regression test proving `gpt-4o-mini` provider pricing now uses the shared cost source.
      - Routed Anthropic's basic `CostCalculator::calculate_cost` through shared `core::cost::calculator::generic_cost_per_token`, keeping the provider registry fallback for compatibility.
    - Execute tests:
      - `cargo fmt --all -- --check` -> pass
      - `cargo test core::providers::openai::client_tests::test_model_pricing` -> pass (`2` matching tests)
      - `cargo test core::providers::openai::client_tests::test_model_pricing_prefers_shared_cost_source` -> pass (`1` test)
      - `cargo test core::providers::anthropic::tests::test_cost_estimation` -> pass (`1` test)
      - `cargo test pricing` -> pass (`175` lib filtered tests, `1` integration filtered test)
  - Step E3 utility convergence: `in_progress`
    - Modified files:
      - `src/utils/ai/models/pricing.rs`
      - `src/utils/ai/models/tests.rs`
      - `src/core/cost/calculator.rs`
    - Main changes:
      - Routed `ModelUtils::get_model_pricing` through `core::cost::calculator::get_model_pricing` before using its legacy fallback table.
      - Added provider inference for OpenAI, Anthropic, Gemini/Vertex AI, DeepSeek, Moonshot, MiniMax, and Zhipu/GLM model IDs.
      - Added a regression test proving `gpt-4o-mini` now uses the canonical cost source instead of falling through to the broad `gpt-4` legacy branch.
      - Updated the higher-level model utility pricing test to use approximate float comparison for shared-source converted rates.
      - Applied `cargo fmt` to fix the PR lint failure from the previous pushed commit.
    - Execute tests:
      - `cargo fmt --all -- --check` -> pass
      - `cargo test utils::ai::models::tests::test_model_pricing` -> pass (`1` test)
      - `cargo test utils::ai::models::pricing` -> pass (`41` tests)
      - `cargo test utils::ai::models` -> pass (`112` tests)
      - `cargo clippy --lib --tests --bins --features "postgres sqlite redis s3 metrics tracing websockets analytics" -- -D warnings --force-warn clippy::collapsible-if` -> pass
  - Step E3 continuation: `in_progress`
    - Modified files:
      - `src/core/cost/calculator.rs`
      - `src/core/providers/mod.rs`
      - `src/config/models/pricing.rs`
      - `src/config/mod.rs`
      - `src/core/a2a/registry.rs`
    - Main changes:
      - Removed the orphan `src/config/models/pricing.rs` model set, which was not registered in `config::models`.
      - Removed the unused `core::providers::ModelPricing` facade struct and self-only tests.
      - Changed `core::cost::get_model_pricing` to prefer the shared LiteLLM pricing catalog before falling back to legacy hardcoded provider tables.
      - Updated cost tests for shared-source pricing and tolerant float comparisons after per-token to per-1K conversion.
      - Fixed PR CI test fixtures: config test JWT secret now satisfies strengthened validation, and A2A registry tests use a validation-safe `.invalid` host instead of an SSRF-blocked reserved IP.
    - Execute tests:
      - `cargo test core::providers::tests` -> pass (`3` tests)
      - `cargo test core::cost::calculator` -> pass (`61` tests)
      - `cargo test config::tests::test_config_from_file` -> pass (`1` test)
      - `cargo test core::a2a::registry` -> pass (`17` tests)
      - `cargo test pricing` -> pass (`173` lib filtered tests, `1` integration filtered test)
      - `cargo clippy --lib --tests --bins --features "postgres sqlite redis s3 metrics tracing websockets analytics" -- -D warnings --force-warn clippy::collapsible-if` -> pass
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
      - `git diff --check` -> pass
  - Step E3 partial: `in_progress`
    - Modified files:
      - `src/core/pricing.rs`
      - `src/core/mod.rs`
      - `src/services/pricing/types.rs`
      - `src/core/providers/base/pricing.rs`
    - Main changes:
      - Introduced `core::pricing::LiteLLMModelInfo` as the shared LiteLLM pricing data model.
      - Re-exported the shared model from `services::pricing` so existing service callers keep the same public import.
      - Converted `core::providers::base::pricing::ModelPricing` into a compatibility alias over the shared model, removing one parallel pricing struct.
      - Aligned provider-base pricing loading with `config/model_prices_extended.json` and added a regression test for that shared file.
    - Execute tests:
      - `cargo test pricing` -> pass (`176` lib filtered tests, `1` integration filtered test)
      - `cargo clippy --lib --tests --bins --features "postgres sqlite redis s3 metrics tracing websockets analytics" -- -D warnings --force-warn clippy::collapsible-if` -> pass
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step E2: `completed`
    - Modified files:
      - `src/config/models/gateway.rs`
      - `src/config/models/gateway_tests.rs`
      - `tests/integration/config_validation_tests.rs`
      - `config/pricing.yaml`
    - Main changes:
      - Aligned the runtime default pricing source with `config/gateway.yaml.example`: `config/model_prices_extended.json`.
      - Removed the unused `config/pricing.yaml` file, which no runtime code loaded.
      - Added a regression test proving the default pricing source remains relative and matches the example.
      - Updated the valid integration config fixture to satisfy the stronger JWT secret validation introduced earlier.
    - Execute tests:
      - `cargo test gateway_config` -> pass (`59` lib filtered tests, `18` integration filtered tests)
      - `rg -n "pricing.yaml|model_prices_extended" src config` -> pass (`2` expected `model_prices_extended` hits, `0` `pricing.yaml` hits)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step E1: `completed`
    - Modified files:
      - `src/storage/database/migration/mod.rs`
      - `src/storage/database/entities/mod.rs`
      - `src/storage/database/entities/pricing.rs`
      - `src/storage/database/entities/pricing_history.rs`
      - `tests/integration/database_tests.rs`
    - Main changes:
      - Registered the existing pricing tables migration in timestamp order.
      - Split the phantom `pricing_history` entity into a real SeaORM entity module and exported both pricing entities.
      - Fixed the previously hidden entity compile issues for f64 equality and SeaORM string length annotations.
      - Added a fresh SQLite migration smoke test that verifies `model_pricing` and `pricing_history` are created.
    - Execute tests:
      - `cargo test migration` -> pass (`1` main filtered test, `2` integration filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D12: `completed`
    - Modified files:
      - `scripts/guards/check_outbound_http_clients.sh`
      - `src/services/pricing/service.rs`
      - `src/monitoring/alerts/channels.rs`
      - `src/core/observability/{metrics,logging}.rs`
      - `src/storage/vector/qdrant.rs`
      - provider, A2A, rerank, webhook, budget alert, and HTTP utility files containing the previous ad hoc client constructors.
    - Main changes:
      - Replaced all `reqwest::Client::new()` call sites under `src/` with the shared outbound client helper.
      - Preserved specialized streaming/custom client builders while routing their fallbacks to shared or builder-based construction.
      - Tightened the outbound-client guard default from the old campaign baseline to zero, so future ad hoc client additions fail by default.
    - Execute tests:
      - `bash scripts/guards/check_outbound_http_clients.sh` -> pass (`0` hits, max `0`)
      - `rg -n "reqwest::Client::new\\(\\)|Client::new\\(\\)" src/ -g '!**/*test*' -g '!**/tests.rs'` -> pass (`0` hits)
      - `shellcheck scripts/guards/check_outbound_http_clients.sh` -> pass
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D11: `completed`
    - Modified files:
      - `src/server/http.rs`
      - `src/core/audit/logger.rs`
    - Main changes:
      - Replaced the app-factory CORS `expect` with a restrictive fallback that logs invalid hot-reload configuration instead of panicking the worker.
      - Made audit shutdown flush/close failures observable through error logs.
      - Made `log_sync` report failed background sends instead of silently dropping channel errors.
    - Execute tests:
      - `cargo test server::http` -> pass (`3` filtered tests)
      - `cargo test audit::logger` -> pass (`9` filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D10: `completed`
    - Modified files:
      - `src/core/providers/factory/registry.rs`
      - `src/core/providers/mod.rs`
    - Main changes:
      - Moved the catalog-backed provider factory fallback after all explicit provider branches so catalog metadata cannot shadow provider-specific builders.
      - Routed embeddings and image generation through the shared `Provider` dispatch path instead of OpenAI-only match arms.
      - Added capability regression tests proving unsupported Anthropic embeddings/images now return provider-specific errors rather than the previous generic `unknown` provider surface.
    - Execute tests:
      - `cargo test provider_factory` -> pass (`14` integration filtered tests; lib filter matched `0`)
      - `cargo test provider_capabilities` -> pass (`7` lib filtered tests, `1` integration filtered test)
      - `cargo test from_config_async` -> pass (`4` filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D9: `completed`
    - Modified files:
      - `src/core/models/openai/requests.rs`
      - `src/server/routes/ai/chat.rs`
      - `src/core/cache/key_generator.rs`
      - `src/core/semantic_cache/validation.rs`
      - `src/core/semantic_cache/tests.rs`
    - Main changes:
      - Added HTTP request fields for `parallel_tool_calls`, `prediction`, `safety_settings`, `cache_control`, and flattened unknown `extra_body`.
      - Preserved `response_format.response_type` instead of forcing it to `None`.
      - Changed OpenAI-compatible `seed` boundary to `i64` and validated it before converting to the internal `i32` field.
      - Forwarded provider-specific boundary knobs into `CoreChatRequest.extra_params`.
    - Execute tests:
      - `cargo test openai_requests` -> pass (`1` filtered test)
      - `cargo test routes::ai::chat` -> pass (`12` filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D8: `completed`
    - Modified files:
      - `src/core/completion/types.rs`
      - `src/core/completion/conversion.rs`
    - Main changes:
      - Added LiteLLM-compatible helper options for `response_format`, `thinking`, `reasoning_effort`, and `metadata`.
      - Forwarded `tools`, `tool_choice`, `response_format`, `thinking`, `reasoning_effort`, and `metadata` into `ChatRequest`.
      - Added conversion regression coverage so helper options no longer become hardcoded `None`.
    - Execute tests:
      - `cargo test completion::conversion` -> pass (`27` filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D7: `completed`
    - Modified files:
      - `src/core/providers/gemini/client.rs`
    - Main changes:
      - Gemini response transforms now emit `FinishReason::ToolCalls` whenever a candidate contains `functionCall` parts.
      - Added a regression test covering Gemini returning `finishReason: STOP` with a function call.
    - Execute tests:
      - `cargo test gemini_finish_reason` -> pass under default features with `0` filtered tests because Gemini provider tests are all-features gated
      - `cargo test --all-features gemini_finish_reason` -> pass (`1` filtered test)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D6: `completed`
    - Modified files:
      - `src/core/providers/bedrock/chat/converse.rs`
    - Main changes:
      - Converted Tool/Function-role messages into Bedrock Converse `toolResult` blocks instead of dropping them.
      - Converted assistant `tool_calls` into Converse `toolUse` blocks while preserving existing text content.
      - Mapped `ContentPart::ToolUse` and `ContentPart::ToolResult` in message parts.
      - Returned explicit `NotImplemented` errors for image/audio/document parts that would previously be dropped silently.
    - Execute tests:
      - `cargo test bedrock_converse` -> pass under default features with `0` filtered tests because Bedrock provider tests are all-features gated
      - `cargo test --all-features bedrock_converse` -> pass (`2` filtered tests)
      - `cargo test --all-features bedrock_tool_result_round_trip` -> pass (`1` filtered test)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D5: `completed`
    - Modified files:
      - `src/core/providers/anthropic/client.rs`
    - Main changes:
      - Preserved assistant text content when adding Anthropic `tool_use` blocks.
      - Converted tool/function-role messages into Anthropic `tool_result` blocks with `tool_use_id`.
      - Added regression tests for assistant text plus tool call and tool-result turns.
    - Execute tests:
      - `cargo test anthropic_transform_messages` -> pass (`2` filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D4: `completed`
    - Modified files:
      - `src/core/providers/anthropic/client.rs`
      - `src/core/providers/base/sse.rs`
      - `src/core/types/responses/logprobs.rs`
      - `src/core/types/responses/usage.rs`
      - `src/core/models/openai/responses.rs`
      - `src/server/routes/ai/chat.rs`
      - `src/core/providers/openai/transformer/response.rs`
      - `src/core/cost/types.rs`
    - Main changes:
      - Preserved Anthropic non-stream `thinking` content blocks and signatures on assistant messages.
      - Mapped Anthropic `cache_creation_input_tokens` and `cache_read_input_tokens` into canonical prompt token details.
      - Kept `cached_tokens` populated from cache-read tokens for OpenAI-compatible clients.
      - Added explicit `FinishReason` variants and HTTP conversion strings for `stop_sequence`, `refusal`, and `pause_turn`.
      - Reused the same Anthropic stop-reason mapping in SSE and non-stream transforms.
    - Execute tests:
      - `cargo test anthropic_client` -> pass (`1` filtered test)
      - `cargo test anthropic_usage` -> pass (`4` filtered tests)
      - `cargo test finish_reason` -> pass (`31` filtered tests)
      - `cargo test prompt_tokens_details` -> pass (`3` filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D3: `completed`
    - Modified files:
      - `src/core/providers/base/sse.rs`
      - `src/core/types/thinking.rs`
      - `src/core/types/responses/delta.rs`
      - `src/core/providers/ollama/streaming.rs`
    - Main changes:
      - Parsed Anthropic `content_block_start` tool-use events into OpenAI-compatible tool-call deltas.
      - Parsed Anthropic `input_json_delta.partial_json` chunks into streaming tool-call argument deltas.
      - Parsed Anthropic `thinking_delta` and `signature_delta` events into streaming thinking deltas.
      - Added a `signature` field to the canonical `ThinkingDelta` type.
      - Added warnings for unknown Anthropic SSE event and content-block delta types.
    - Execute tests:
      - `cargo test anthropic_stream` -> pass (`2` filtered tests)
      - `cargo test anthropic_stream_tool_use` -> pass (`1` filtered test)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D2: `completed`
    - Modified files:
      - `src/core/models/openai/responses.rs`
      - `src/server/routes/ai/chat.rs`
      - `src/core/streaming/handler.rs`
      - `src/core/streaming/types.rs`
      - `src/core/cache/llm_cache.rs`
      - `src/core/cache/mod.rs`
      - `src/core/semantic_cache/types.rs`
    - Main changes:
      - Exposed `thinking_usage` on the public OpenAI-compatible `Usage` response type.
      - Preserved `thinking_usage` in the gateway response conversion path.
      - Mirrored thinking token counts into `completion_tokens_details.reasoning_tokens` when provider details omit it.
      - Added serialization coverage that omits absent optional usage details and retains present thinking usage.
      - Deferred cache creation/read token public fields to Step D4, where Anthropic cache usage is mapped canonically.
    - Execute tests:
      - `cargo test convert_usage` -> pass (`7` filtered tests)
      - `cargo test usage` -> pass (`158` lib filtered tests, `7` integration filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step D1: `completed`
    - Modified files:
      - `src/core/streaming/types.rs`
      - `src/core/streaming/handler.rs`
      - `src/core/completion/stream.rs`
      - `src/server/routes/ai/chat.rs`
    - Main changes:
      - Added public streaming delta fields for thinking, `reasoning_content`, `tool_call_id`, `refusal`, and legacy `function_call`.
      - Added `skip_serializing_if = "Option::is_none"` to optional streaming chunk, choice, delta, tool-call, and function-call fields so SSE JSON no longer emits nulls for absent values.
      - Updated gateway chunk conversion to preserve core thinking deltas and legacy function-call deltas at the HTTP streaming boundary.
      - Updated existing stream constructors/tests to use the expanded delta shape.
    - Execute tests:
      - `cargo test streaming::types` -> pass (`39` filtered tests)
      - `cargo test routes::ai::chat` -> pass (`8` filtered tests)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step C8: `completed`
    - Modified files:
      - `src/core/rate_limiter/limiter.rs`
      - `src/core/rate_limiter/mod.rs`
      - `src/server/http.rs`
      - `src/storage/redis/mod.rs`
      - `src/storage/redis/rate_limit.rs`
    - Main changes:
      - Added Redis-backed fixed-window rate-limit operations using atomic Lua scripts.
      - Added optional Redis backend support to the global `RateLimiter`, with in-process fallback when Redis is disabled, unavailable, or errors at runtime.
      - Wired HTTP server initialization to attach the shared Redis pool to the global limiter when `redis.enabled` is usable.
      - Preserved lite builds by gating Redis-backed limiter fields and imports to gateway builds.
    - Execute tests:
      - `cargo test rate_limiter` -> pass (`70` filtered tests)
      - `cargo test redis::rate_limit` -> pass (`3` filtered tests, live Redis shared-state check passed on this machine)
      - `cargo check --all-features` -> pass
      - `cargo check --no-default-features --features lite` -> pass
  - Step C7: `completed`
    - Modified files:
      - `src/core/budget/mod.rs`
      - `src/core/budget/provider_limits.rs`
      - `src/server/http.rs`
      - `src/server/state.rs`
      - `src/storage/database/entities/budget_limit_snapshot.rs`
      - `src/storage/database/entities/mod.rs`
      - `src/storage/database/migration/m20240501_000001_create_budget_limit_snapshots.rs`
      - `src/storage/database/migration/mod.rs`
      - `src/storage/database/seaorm_db/budget_limit_ops.rs`
      - `src/storage/database/seaorm_db/mod.rs`
    - Main changes:
      - Added a `budget_limit_snapshots` migration/entity for provider and model budget state.
      - Added SeaORM load/upsert/delete operations plus a background persistence worker for budget mutations.
      - Added provider/model budget snapshots, persistence events, restore helpers, and persistence-aware `UnifiedBudgetLimits` construction.
      - Loaded persisted budget snapshots during HTTP server initialization and attached the persistence worker when the database table is available.
    - Execute tests:
      - `cargo test budget_limit_ops` -> pass (`1` filtered test)
      - `cargo test provider_limits` -> pass (`21` filtered tests)
      - `cargo test budget` -> pass (`186` lib filtered tests, `1` integration filtered test)
      - `cargo check --all-features` -> pass
  - Step C6: `completed`
    - Modified files:
      - `src/server/middleware/auth_rate_limiter.rs`
      - `src/server/middleware/mod.rs`
      - `src/server/http.rs`
    - Main changes:
      - Added a default `10_000` tracker cap plus `with_max_entries(...)` for focused tests and future config wiring.
      - Changed `check_allowed` so never-seen clients do not create `DashMap` entries.
      - Added deterministic cleanup and capacity enforcement on failure recording, preferring active lockouts when evicting.
      - Started a once-per-process background cleanup task from HTTP server initialization.
    - Execute tests:
      - `cargo test auth_rate_limiter` -> pass (`29` filtered tests)
      - `cargo test server::middleware` -> pass (`60` filtered tests)
      - `cargo check --all-features` -> pass
  - Step C5: `completed`
    - Modified files:
      - `src/core/providers/pg_vector/config.rs`
      - `src/core/providers/pg_vector/provider.rs`
    - Main changes:
      - Centralized PostgreSQL identifier quoting and tightened schema/table validation to ASCII letter/underscore start, ASCII alphanumeric/underscore body, and 63-byte maximum.
      - Parameterized search threshold, metadata filter values, and limit in prepared search statements.
      - Removed the public `StatementParam::to_sql_string()` helper to avoid hand-rolled value escaping being reused.
    - Execute tests:
      - `cargo test pg_vector` -> pass with `0` tests under default features; rerun with all features for real coverage
      - `cargo test --all-features pg_vector` -> pass (`41` tests)
      - `cargo check --all-features` -> pass
  - Step C4: `completed`
    - Modified files:
      - `src/config/models/auth.rs`
      - `src/config/validation/tests.rs`
      - `config/gateway.yaml.example`
    - Main changes:
      - Rejected JWT placeholder/default secrets case-insensitively, including `ReplaceWith...`, `change-me`, `your-secret-key`, and unresolved placeholder-like values.
      - Hardened weak-secret validation so lowercase/digit secrets without uppercase letters are rejected.
      - Replaced the example literal JWT secret with `${LITELLM_JWT_SECRET}` and documented the env requirement.
    - Execute tests:
      - `cargo test config::models::auth` -> pass (`23` tests)
      - `cargo test config::validation::ssrf::tests::test_real_world_api_endpoints -- --nocapture` -> pass after one transient DNS-related failure in the wider run
      - `cargo test config::validation` -> pass (`158` tests)
      - `cargo check --all-features` -> pass
  - Step C3: `completed`
    - Modified files:
      - `src/core/providers/bedrock/client.rs`
      - `src/core/providers/milvus/provider.rs`
      - `src/core/audit/config.rs`
      - `src/core/audit/logger.rs`
      - `scripts/guards/check_log_pii.sh`
    - Main changes:
      - Removed raw Bedrock request body and error body logging; logs now keep status and body byte counts only.
      - Removed raw Milvus request body logging; logs now keep URL and body byte count only.
      - Expanded audit redaction defaults for Bearer tokens, JWT-like strings, AWS `AKIA` keys, Anthropic `sk-ant-` keys, gateway `gw-` keys, and Authorization header values.
      - Tightened the log PII guard baseline from `19` to `14` suspicious body-log hits and avoided flagging safe `body_bytes` fields.
    - Execute tests:
      - `cargo test audit` -> pass (`54` tests)
      - `bash scripts/guards/check_log_pii.sh` -> pass (`14` hits, baseline max `14`)
      - `cargo check --all-features` -> pass
  - Step C2: `completed`
    - Modified files:
      - `src/core/cache/key_policy.rs`
      - `src/core/cache/types.rs`
    - Main changes:
      - Documented the intentional Redis/cache cold-start effect of `CACHE_KEY_SCHEMA_VERSION`.
      - Replaced `CacheKey`'s internal precomputed `DefaultHasher` hash with a deterministic SHA-256-derived `u64`.
      - Completed the cache-key rewrite started in B3, which already moved wire keys to stable versioned SHA-256 digests and added request-dimension coverage tests.
    - Execute tests:
      - `cargo test core::cache` -> pass (`134` tests)
      - `cargo check --all-features` -> pass
  - Step C1: `completed`
    - Modified files:
      - `src/core/mcp/config.rs`
    - Main changes:
      - Applied the shared outbound SSRF guard to MCP HTTP, SSE, and WebSocket transports.
      - Preserved stdio command-path behavior.
      - Added tests rejecting localhost, loopback, RFC1918, link-local metadata, and IPv6 loopback MCP targets by default.
    - Execute tests:
      - `cargo test core::mcp::config` -> pass (`19` tests)
      - `cargo test ssrf_guard` -> pass (`8` tests)
      - `cargo check --all-features` -> pass
  - Step B4: `completed`
    - Modified files:
      - `src/core/providers/registry/mod.rs`
      - `src/core/providers/registry/types.rs`
    - Main changes:
      - Added an observational provider registry matrix with canonical name, aliases, dispatch kind, dispatchability, and catalog-backed classification.
      - Added drift tests tying the matrix to `ProviderType`, `Provider::factory_supported_provider_types()`, and the Tier-1 catalog.
      - Did not replace existing provider factory routing in this step.
    - Execute tests:
      - `cargo test provider_registry` -> pass (`30` filtered tests)
      - `cargo test provider_type` -> pass (`60` lib filtered tests, `1` integration filtered test)
      - `cargo check --all-features` -> pass
  - Step B3: `completed`
    - Modified files:
      - `src/core/cache/mod.rs`
      - `src/core/cache/key_policy.rs`
      - `src/core/cache/key_generator.rs`
    - Main changes:
      - Added cache key schema versioning with stable canonical JSON and SHA-256 digests.
      - Replaced `DefaultHasher` in cache key generation with versioned stable digests.
      - Included full chat request identity for tools, tool choice, response format schema, reasoning effort, logprobs, and other response-affecting fields.
      - Preserved explicit user scoping and made embedding input order part of cache identity.
    - Execute tests:
      - `cargo test key_policy` -> pass (`5` filtered tests)
      - `cargo test key_generator` -> pass (`22` tests)
      - `cargo check --all-features` -> pass
  - Step B2: `completed`
    - Modified files:
      - `src/core/mod.rs`
      - `src/core/net/mod.rs`
      - `src/core/net/ssrf_guard.rs`
      - `src/core/a2a/config.rs`
    - Main changes:
      - Promoted A2A private/reserved host and IP checks into `core::net::ssrf_guard`.
      - Switched shared URL handling to `url::Url` while preserving the existing A2A validation surface.
      - Added direct SSRF guard tests for private IPv4, IPv6 loopback/ULA/link-local, metadata hosts, unsupported schemes, and public hosts.
    - Execute tests:
      - `cargo test ssrf_guard` -> pass (`8` tests)
      - `cargo test core::a2a::config` -> pass (`28` tests)
      - `cargo check --all-features` -> pass
  - Step B1: `completed`
    - Modified files:
      - `src/core/mod.rs`
      - `src/core/http/mod.rs`
      - `src/core/http/outbound.rs`
    - Main changes:
      - Added `OutboundProfile`, `build_outbound_client`, and `default_outbound_client`.
      - Exposed the new helper through `core::http` without migrating existing callers yet.
    - Execute tests:
      - `cargo test core::http` -> pass (`3` tests)
      - `cargo check --all-features` -> pass
  - Step A3: `completed`
    - Modified files:
      - `scripts/guards/check_outbound_http_clients.sh`
      - `scripts/guards/check_log_pii.sh`
    - Main changes:
      - Added baseline-aware guard scripts for H19 outbound HTTP client sprawl and C9/M12 suspicious body logging.
      - Guards currently block regressions above baseline while allowing existing audit debt to be reduced in later steps.
    - Execute tests:
      - `bash scripts/guards/check_outbound_http_clients.sh` -> pass (`50` hits, baseline max `50`)
      - `bash scripts/guards/check_log_pii.sh` -> pass (`19` hits, baseline max `19`)
      - `shellcheck scripts/guards/check_outbound_http_clients.sh scripts/guards/check_log_pii.sh` -> pass
  - Step A2: `completed`
    - Modified files:
      - `docs/audit-2026-05-01/baseline-2026-05-01.md`
    - Main changes:
      - Recorded branch state, toolchain versions, fmt/clippy/test/lite baseline, outbound HTTP client count, and large-file LOC baseline.
      - Captured existing file-system-loop warning from the repository self-symlink as baseline noise.
    - Execute tests:
      - `git fetch origin && git status --short --branch` -> pass (`main...origin/main`, docs-only untracked changes)
      - `cargo fmt --all -- --check` -> pass
      - `cargo clippy --all-targets --all-features -- -D warnings` -> pass
      - `cargo test --all-features` -> pass (`10361` lib tests, `4` main tests, `144` integration tests, `3` connection-pool tests, `96` doctests)
      - `cargo check --no-default-features --features lite` -> pass
      - `rg -n "reqwest::Client::new\\(\\)" src/ -g '!**/*test*' -g '!**/tests.rs' | wc -l` -> pass (`50`)
      - `wc -l src/core/cost/calculator.rs src/core/providers/anthropic/{models,client}.rs src/core/providers/base/sse.rs src/core/providers/openai_like/provider.rs src/server/routes/ai/chat.rs src/core/providers/factory/builder.rs` -> pass
  - Step A1: `completed`
    - Modified files:
      - `docs/audit-2026-05-01/README.md`
      - `docs/audit-2026-05-01/raw-consolidated-findings.md`
      - `docs/audit-2026-05-01/agent-1-api-data-integrity.md`
      - `docs/audit-2026-05-01/agent-2-error-security.md`
      - `docs/audit-2026-05-01/agent-3-architecture.md`
      - `docs/audit-2026-05-01/agent-4-config-persistence.md`
    - Main changes:
      - Persisted the consolidated raw audit findings and clarified 77 raw findings versus 72 deduplicated remediation items.
      - Added per-agent provenance stubs because the original full per-agent transcripts were not present in this checkout.
    - Execute tests:
      - `rg -n "Total|Critical|High|Medium|C1|H1|M1" docs/audit-2026-05-01` -> pass
      - `awk '/^\\| C[0-9]+ /{c++} /^\\| H[0-9]+ /{h++} /^\\| M[0-9]+ /{m++} END{print c,h,m,c+h+m}' PLAN_AUDIT_REMEDIATION.md` -> pass (`20 22 30 72`)
  - Step planning: `completed`
    - Created this complete plan.
    - Current first executable step is A1.
    - No source code changes made by this plan.

## 10. Handoff

- mode: `plan_first`
- artifacts:
  - `/Users/apple/Desktop/code/AI/gateway/litellm-rs/docs/plan/audit-remediation-complete-plan.md`
  - `/Users/apple/Desktop/code/AI/gateway/litellm-rs/PLAN_AUDIT_REMEDIATION.md`
  - `/Users/apple/Desktop/code/AI/gateway/litellm-rs/PLAN_AUDIT_EXECUTION.md`
- verification_owner: local executor for each step; PR CI must repeat the relevant matrix.
- stop_conditions:
  - no file-level evidence for a planned change
  - baseline health fails in a way that invalidates verification
  - destructive migration lacks owner confirmation
  - a provider wire-format change lacks a regression fixture
- lane_map: see section 7.
