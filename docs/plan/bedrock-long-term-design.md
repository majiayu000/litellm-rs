# Bedrock Long-Term Design Plan

## 0. Metadata

- Task: `bedrock-native-routing-and-current-model-catalog`
- Repository: `litellm-rs`
- Compatibility strategy: required
- Commit strategy: milestone
- Date: 2026-05-20
- GitHub issue: #553

## 1. Problem Statement

Issue #6 started as a `bedrock/us.anthropic.claude-3-5-sonnet-20241022-v2:0`
routing failure, but the durable problem is broader:

- Bedrock has native AWS protocol requirements: SigV4, region-aware endpoints,
  model access permissions, and non-OpenAI request shapes.
- Bedrock now has multiple invocation surfaces:
  - `bedrock-runtime` with `InvokeModel`, `Converse`, and streaming variants.
  - `bedrock-mantle` for Anthropic Messages API access on supported models.
  - Optional OpenAI-compatible proxy deployments such as
    `aws-samples/bedrock-access-gateway`.
- Bedrock model identifiers now encode routing intent:
  - Direct model ID: `anthropic.claude-opus-4-7`
  - Geo inference profile ID: `us.anthropic.claude-opus-4-7`
  - Global inference profile ID: `global.anthropic.claude-opus-4-7`
  - Application inference profile ARN:
    `arn:aws:bedrock:...:application-inference-profile/...`
- Newer models such as Claude Opus 4.7 have model-specific behavior changes,
  including `adaptive` thinking and unsupported sampling parameters.

The long-term fix is not to add one example or a hardcoded alias. The fix is to
make Bedrock model identity, metadata lookup, invocation routing, and proxy
compatibility explicit.

## 2. Design Principles

- Provider routing prefixes are not provider model IDs.
- AWS inference profile IDs are real model IDs and must be preserved on the
  request sent to AWS.
- Model metadata lookup may canonicalize IDs, but request execution must not.
- Native Bedrock and OpenAI-compatible Bedrock proxies are separate provider
  modes.
- New model support should be mostly data/catalog work, not router logic.
- Model-specific parameter restrictions must fail clearly instead of being
  silently dropped.
- Default CI must not require AWS credentials or network access.

## 3. Target Architecture

### 3.1 Provider Modes

| Mode | Selector | Runtime | Auth | Use case |
| --- | --- | --- | --- | --- |
| Native Bedrock Runtime | `bedrock/...` | `BedrockProvider` | AWS SigV4 | Direct AWS Bedrock calls |
| Bedrock Mantle | `bedrock-mantle/...` or provider config | `BedrockMantleProvider` or Bedrock endpoint mode | AWS SigV4 | Anthropic Messages API on `bedrock-mantle` |
| Bedrock OpenAI Proxy | `openai-compatible` with `api_base` | `OpenAILikeProvider` | Proxy API key | User-deployed OpenAI-compatible proxy |

`bedrock-access-gateway` belongs only to the proxy mode. It must not replace the
native Bedrock provider, because native mode is the only path that works without
deploying an extra proxy and the only path that can expose full AWS semantics.

### 3.2 Bedrock Model Identity

Introduce a small typed parser for Bedrock model identifiers:

```rust
pub enum BedrockModelSelector {
    Direct {
        model_id: String,
    },
    GeoInferenceProfile {
        geography: BedrockGeography,
        model_id: String,
        profile_id: String,
    },
    GlobalInferenceProfile {
        model_id: String,
        profile_id: String,
    },
    ApplicationInferenceProfileArn {
        arn: String,
    },
}
```

Parsing examples:

| Input | Route prefix stripped | Selector | Execution ID |
| --- | --- | --- | --- |
| `bedrock/anthropic.claude-opus-4-7` | `anthropic.claude-opus-4-7` | Direct | `anthropic.claude-opus-4-7` |
| `bedrock/us.anthropic.claude-opus-4-7` | `us.anthropic.claude-opus-4-7` | Geo | `us.anthropic.claude-opus-4-7` |
| `bedrock/global.anthropic.claude-opus-4-7` | `global.anthropic.claude-opus-4-7` | Global | `global.anthropic.claude-opus-4-7` |
| `bedrock/arn:aws:bedrock:...` | `arn:aws:bedrock:...` | Application profile ARN | unchanged ARN |

Only the `bedrock/` provider prefix is stripped for routing. The AWS model ID
or inference profile ID is preserved for execution.

### 3.3 Metadata Lookup

Use two IDs:

- `execution_model_id`: exact ID sent to AWS.
- `metadata_model_id`: canonical base model used for local capability and
  pricing lookup.

Lookup order:

1. Exact lookup with `execution_model_id`.
2. If the ID is a geo/global inference profile, lookup by base model ID.
3. If the ID is an application inference profile ARN, use explicit config
   metadata if provided; otherwise return an actionable unsupported-metadata
   error for capability-dependent operations.

This lets `global.anthropic.claude-opus-4-7` preserve global routing while still
using `anthropic.claude-opus-4-7` capability metadata.

### 3.4 Model Catalog Shape

Replace the Bedrock-specific flat `ModelConfig` with a catalog entry that can
represent new endpoint and parameter behavior:

```rust
pub struct BedrockModelCatalogEntry {
    pub model_id: &'static str,
    pub family: BedrockModelFamily,
    pub provider: BedrockFoundationProvider,
    pub endpoint_configs: &'static [BedrockEndpointConfig],
    pub preferred_endpoint: BedrockEndpointKind,
    pub context_window: u32,
    pub max_output_tokens: Option<u32>,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_multimodal: bool,
    pub thinking: BedrockThinkingSupport,
    pub parameter_policy: BedrockParameterPolicy,
    pub pricing_key: Option<&'static str>,
    pub lifecycle: ModelLifecycle,
}
```

Endpoint and API enums:

```rust
pub struct BedrockEndpointConfig {
    pub kind: BedrockEndpointKind,
    pub request_api: BedrockRuntimeApi,
    pub streaming_api: Option<BedrockRuntimeApi>,
}

pub enum BedrockEndpointKind {
    Runtime,
    Mantle,
}

pub enum BedrockRuntimeApi {
    Converse,
    ConverseStream,
    InvokeModel,
    InvokeModelWithResponseStream,
    MantleMessages,
}

pub enum BedrockThinkingSupport {
    None,
    EnabledWithBudget,
    AdaptiveOnly,
}

pub struct BedrockParameterPolicy {
    pub allowed: &'static [&'static str],
    pub rejected: &'static [&'static str],
    pub reject_message: &'static str,
}
```

Claude Opus 4.7 should be represented as:

- `model_id`: `anthropic.claude-opus-4-7`
- `endpoint_configs`: `Runtime` with `Converse` / `ConverseStream`, and
  `Mantle` with `MantleMessages`
- `preferred_endpoint`: `Runtime` for existing `completion()` compatibility
- `context_window`: `1_000_000`
- `max_output_tokens`: `128_000`
- `thinking`: `AdaptiveOnly`
- `parameter_policy.rejected`: `temperature`, `top_p`, `top_k`

### 3.5 Request Execution

Native Bedrock execution becomes:

1. Strip the provider prefix only.
2. Parse `BedrockModelSelector`.
3. Resolve catalog metadata using exact-then-canonical lookup.
4. Validate parameters against the catalog entry.
5. Select endpoint and API:
   - `Converse` / `ConverseStream` for supported chat models.
   - `InvokeModel` / `InvokeModelWithResponseStream` for legacy models.
   - `MantleMessages` only when configured or selected.
6. Send `execution_model_id` unchanged to AWS.
7. Parse response by endpoint/API/family, not by string heuristics alone.

### 3.6 Configuration

Native Bedrock provider config:

```yaml
providers:
  - name: "bedrock-native"
    provider_type: "bedrock"
    aws_region: "${AWS_REGION}"
    aws_access_key_id: "${AWS_ACCESS_KEY_ID}"
    aws_secret_access_key: "${AWS_SECRET_ACCESS_KEY}"
    aws_session_token: "${AWS_SESSION_TOKEN}"
    enabled: true
```

Proxy config:

```yaml
providers:
  - name: "bedrock-proxy"
    provider_type: "openai-compatible"
    api_key: "${BEDROCK_PROXY_API_KEY}"
    base_url: "${BEDROCK_PROXY_BASE_URL}"
    enabled: true
```

Do not use `provider_type: "bedrock"` for an OpenAI-compatible proxy.

## 4. Implementation Plan

### Step A1 - Provider Wiring

- Status: `pending`
- Goal:
  - Make native Bedrock reachable through the same provider registry path as
    OpenAI, Anthropic, Mistral, and Cloudflare.
- Expected files:
  - `src/core/providers/mod.rs`
  - `src/core/providers/factory/registry.rs`
  - `src/core/providers/factory/builder.rs`
  - `src/core/completion/default_router/mod.rs`
- Changes:
  - Add a `Provider::Bedrock` variant behind `providers-extra`.
  - Add Bedrock to all provider dispatch macro arms.
  - Change `ProviderType::Bedrock` factory branch to construct
    `BedrockProvider`, not `OpenAILikeProvider`.
  - Register Bedrock from AWS env vars in `DefaultRouter::new()` when
    `providers-extra` is enabled.
  - Return a clear configuration error when `bedrock/...` is requested but
    credentials are missing.
- Step test commands:
  - `cargo check --no-default-features --features "lite,providers-extra"`
  - `cargo test --lib --no-default-features --features "lite,providers-extra" provider`
- Completion criteria:
  - `bedrock/...` can route to native `BedrockProvider` without proxy semantics.

### Step A2 - Model ID Parser and Metadata Fallback

- Status: `pending`
- Goal:
  - Preserve AWS inference profile IDs at execution time while allowing
    canonical metadata lookup.
- Expected files:
  - `src/core/providers/bedrock/model_id.rs`
  - `src/core/providers/bedrock/utils/mod.rs`
  - `src/core/providers/bedrock/model_config.rs`
  - `src/core/providers/bedrock/provider.rs`
  - `src/core/providers/bedrock/chat/mod.rs`
- Changes:
  - Add `BedrockModelSelector` parser.
  - Replace `normalize_bedrock_model_id` usage in execution paths with
    `strip_bedrock_route_prefix`.
  - Add `metadata_model_id()` fallback for geo/global inference profile IDs.
  - Keep exact execution ID in `BedrockClient::build_url()`.
- Step test commands:
  - `cargo test --lib --no-default-features --features "lite,providers-extra" bedrock_model_id`
  - `cargo test --lib --no-default-features --features "lite,providers-extra" bedrock`
- Completion criteria:
  - `us.` / `eu.` / `jp.` / `au.` / `global.` prefixes are preserved for AWS.
  - Catalog lookup still succeeds for profile IDs when base metadata exists.

### Step A3 - Current Bedrock Catalog Upgrade

- Status: `pending`
- Goal:
  - Move Bedrock model support from one-off IDs to a current, capability-rich
    catalog.
- Expected files:
  - `src/core/providers/bedrock/model_config.rs`
  - `src/core/providers/bedrock/utils/cost.rs`
  - `src/core/providers/bedrock/provider_tests.rs`
  - `docs/providers/README.md`
- Changes:
  - Add/refresh current Claude Opus 4.7, Sonnet 4.6, Haiku 4.5, Nova,
    DeepSeek, MiniMax, Mistral, Moonshot, Qwen, OpenAI OSS, Writer, and Z.AI
    entries according to endpoint availability.
  - Preserve the issue #6 Sonnet 3.5 ID as a legacy regression entry so the
    original failing route keeps resolving.
  - Represent each `bedrock-runtime` and `bedrock-mantle` endpoint with its
    required API protocol explicitly.
  - Mark legacy/EOL models instead of deleting them immediately.
  - Add catalog tests for latest model IDs and older issue #6 model ID.
- Step test commands:
  - `cargo test --lib --no-default-features --features "lite,providers-extra" bedrock_catalog`
  - `cargo test --lib --no-default-features --features "lite,providers-extra" bedrock`
- Completion criteria:
  - Claude Opus 4.7, current Sonnet 4.6, and the issue #6 Sonnet 3.5 model all
    resolve correctly.
  - Endpoint support and parameter policy are visible in one catalog entry.

### Step A4 - Parameter Policy Validation

- Status: `pending`
- Goal:
  - Prevent silent invalid requests for models with changed behavior.
- Expected files:
  - `src/core/providers/bedrock/provider.rs`
  - `src/core/providers/bedrock/chat/converse.rs`
  - `src/core/providers/bedrock/chat/transformations/anthropic.rs`
  - `src/core/providers/bedrock/error.rs`
- Changes:
  - Validate request parameters before sending to AWS.
  - For Claude Opus 4.7, reject `temperature`, `top_p`, and `top_k` with a
    direct error message.
  - For thinking requests, map only valid `adaptive` thinking shape.
  - Keep unsupported parameter behavior per model entry instead of global ifs.
- Step test commands:
  - `cargo test --lib --no-default-features --features "lite,providers-extra" opus_4_7`
  - `cargo test --lib --no-default-features --features "lite,providers-extra" bedrock_parameter_policy`
- Completion criteria:
  - Unsupported model parameters produce deterministic local errors.

### Step A5 - Proxy Documentation and Examples

- Status: `pending`
- Goal:
  - Make native and proxy Bedrock usage obvious without mixing their semantics.
- Expected files:
  - `README.md`
  - `examples/bedrock_completion.rs`
  - `examples/bedrock_proxy_openai_compatible.rs`
  - `examples/README.md`
  - `config/gateway.yaml.example`
  - `docs/providers/README.md`
- Changes:
  - Add a native Bedrock example using a current model such as
    `bedrock/global.anthropic.claude-opus-4-7`, with a cheaper fallback noted
    for smoke testing.
  - Add a proxy example using `api_base` and `openai-compatible`.
  - Document that `bedrock-access-gateway` is optional and external.
  - Document credentials, model access, quota, and AWS Marketplace subscription
    prerequisites.
- Step test commands:
  - `cargo check --examples --no-default-features --features "lite,providers-extra"`
  - `cargo test --doc --no-default-features --features "lite,providers-extra"`
- Completion criteria:
  - Users can choose the native path or proxy path without guessing.

### Step A6 - Optional Live Smoke Tests

- Status: `pending`
- Goal:
  - Validate real AWS behavior without making CI depend on AWS.
- Expected files:
  - `tests/e2e/bedrock_live.rs`
  - `examples/bedrock_completion.rs`
  - `.github/workflows/*` only if secrets are explicitly available
- Changes:
  - Add ignored live tests gated by `LITELLM_RS_LIVE_BEDROCK=1`.
  - Test direct, geo, and global profile IDs when env vars are present.
  - Skip with a clear message when model access, quota, or credentials are
    missing.
- Step test commands:
  - `cargo test --test bedrock_live --no-default-features --features "lite,providers-extra" -- --ignored`
- Completion criteria:
  - Default CI stays offline; maintainers can opt into real Bedrock smoke tests.

## 5. Test Matrix

### Offline Unit Tests

| Area | Test input | Expected result |
| --- | --- | --- |
| Route prefix | `bedrock/anthropic.claude-opus-4-7` | route prefix stripped only |
| Geo profile | `bedrock/us.anthropic.claude-opus-4-7` | execution ID preserves `us.` |
| Global profile | `bedrock/global.anthropic.claude-opus-4-7` | execution ID preserves `global.` |
| Application ARN | `bedrock/arn:aws:bedrock:...` | execution ID preserves ARN |
| Metadata fallback | `global.anthropic.claude-opus-4-7` | catalog fallback uses base ID |
| URL construction | `us.anthropic...` | URL path contains profile ID |
| Parameter policy | Opus 4.7 + `temperature` | local validation error |
| Thinking policy | Opus 4.7 + `enabled` budget | local validation error |
| Thinking policy | Opus 4.7 + `adaptive` | request allowed |

### Build and Test Commands

Default offline gates:

```bash
cargo check --no-default-features --features "lite,providers-extra"
cargo test --lib --no-default-features --features "lite,providers-extra" bedrock
cargo test --lib --no-default-features --features "lite,providers-extra" provider
cargo check --examples --no-default-features --features "lite,providers-extra"
```

Full pre-submission gate when the change touches shared provider dispatch:

```bash
cargo test --lib --tests --no-default-features --features "lite,providers-extra"
cargo clippy --all-targets --all-features -- -D warnings
```

Optional live gate:

```bash
LITELLM_RS_LIVE_BEDROCK=1 \
AWS_ACCESS_KEY_ID=... \
AWS_SECRET_ACCESS_KEY=... \
AWS_REGION=us-east-1 \
cargo test --test bedrock_live --no-default-features --features "lite,providers-extra" -- --ignored
```

## 6. PR Split

| PR | Scope | Risk | Must pass |
| --- | --- | --- | --- |
| PR 1 | Provider wiring and native factory path | Medium | cargo check + provider tests |
| PR 2 | Model ID parser and metadata fallback | Medium | bedrock model ID tests |
| PR 3 | Catalog upgrade with Opus 4.7 and endpoint metadata | Low/Medium | catalog tests |
| PR 4 | Parameter policy validation | Medium | policy tests |
| PR 5 | Docs, examples, optional live smoke | Low | examples check + doc tests |

Do not combine all PRs unless review bandwidth is high. Provider wiring and
model ID semantics are the riskiest parts and should be reviewed separately.

## 7. Non-Goals

- Do not vendor or depend on `bedrock-access-gateway`.
- Do not require users to deploy a proxy for native Bedrock.
- Do not make live AWS calls in default tests.
- Do not silently map unsupported parameters.
- Do not remove older Bedrock model IDs in the same PR as catalog refresh.

## 8. Source Facts Used

- AWS documents Claude Opus 4.7 as active, with 1M context, 128K max output,
  `adaptive` thinking only, and unsupported `temperature`, `top_p`, and `top_k`.
- AWS documents direct, geo, and global model ID formats for Bedrock model
  invocation.
- AWS endpoint availability lists both `bedrock-runtime` and `bedrock-mantle`
  support by model.
- `aws-samples/bedrock-access-gateway` is an OpenAI-compatible proxy for
  Bedrock, with support for chat completions, streaming, tool calls, cross-region
  inference, application inference profiles, reasoning, and prompt caching.

## 9. Execution Log

- Step A1: `pending`
- Step A2: `pending`
- Step A3: `pending`
- Step A4: `pending`
- Step A5: `pending`
- Step A6: `pending`
