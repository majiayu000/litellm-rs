# Bedrock Native Routing and Model Catalog Plan

## 0. Metadata

- Task: GitHub issue #553, "Design: long-term Bedrock native routing and current model catalog"
- Repository: litellm-rs
- Compatibility strategy: required
- Submit strategy: final_only
- Scope: design only; implementation PRs should be split after this plan lands

## 1. Problem Statement

Issue #6 exposed a concrete `bedrock/us.anthropic.claude-3-5-sonnet-20241022-v2:0`
routing gap, but the durable problem is broader than one alias. Bedrock has
multiple invocation surfaces and model ID semantics that must be represented
explicitly instead of being normalized into a single OpenAI-compatible shape.

The long-term design must support:

- Native `bedrock-runtime` calls with AWS SigV4 and unmodified AWS model IDs.
- `bedrock-mantle` Anthropic Messages calls for supported Claude models.
- OpenAI-compatible proxy deployments, such as `aws-samples/bedrock-access-gateway`,
  as `openai_compatible` providers rather than native Bedrock providers.
- Geo and global inference profile IDs, such as `us.anthropic...` and
  `global.anthropic...`, without stripping the execution model ID.
- Model-specific parameter policies, including Claude Opus 4.7's adaptive
  thinking behavior and sampling parameter restrictions as documented for the
  Anthropic Messages API.
- Offline default CI, with live AWS smoke tests gated by explicit opt-in env.

## 2. Current State

The repository already contains a native Bedrock implementation surface:

- `src/core/providers/bedrock/config.rs` defines AWS credential and region config.
- `src/core/providers/bedrock/client.rs` signs requests with SigV4 and builds
  `bedrock-runtime` URLs.
- `src/core/providers/bedrock/chat/converse.rs` and
  `src/core/providers/bedrock/chat/invoke.rs` route native Converse and Invoke calls.
- `src/core/providers/bedrock/model_config.rs` and
  `src/core/providers/bedrock/utils/cost.rs` maintain static model metadata and pricing.

However, the runtime factory currently treats `ProviderType::Bedrock` as an
explicit OpenAI-like branch:

- `src/core/providers/registry/types.rs` marks Bedrock as `ExplicitOpenAiLike`.
- `src/core/providers/factory/registry.rs` maps `ProviderType::Bedrock` to
  `OpenAILikeProvider`.
- `src/core/providers/factory/builder.rs` requires an `api_key` and builds a
  default `https://bedrock-runtime.us-east-1.amazonaws.com` OpenAI-like config.

This means the native Bedrock code exists, but the normal provider factory path
does not instantiate `BedrockProvider`.

Current model ID handling is also too lossy for inference profiles. The helper
`normalize_bedrock_model_id` strips `bedrock/`, geo prefixes such as `us.`, and
region-like prefixes such as `us-east-1.`. That is useful for metadata fallback,
but it is not safe as the execution ID for AWS calls because AWS inference
profile IDs are real model identifiers.

## 3. Source Contracts

Implementation PRs should validate behavior against these contracts:

- GitHub issue #553: design requirements and done-when criteria.
- GitHub issue #6: original `bedrock/us.anthropic...` failure and follow-up
  comments distinguishing native Bedrock from OpenAI-compatible gateways.
- AWS Bedrock Anthropic Messages API:
  `https://docs.aws.amazon.com/bedrock/latest/userguide/inference-messages-api.html`
- AWS Bedrock inference profile API:
  `https://docs.aws.amazon.com/bedrock/latest/APIReference/API_GetInferenceProfile.html`
- AWS Bedrock inference profile support:
  `https://docs.aws.amazon.com/bedrock/latest/userguide/inference-profiles-support.html`
- Anthropic Claude model overview, including Claude API and AWS Bedrock IDs:
  `https://platform.claude.com/docs/en/about-claude/models/overview`
- Anthropic Claude Opus 4.7 migration notes:
  `https://platform.claude.com/docs/en/about-claude/models/whats-new-claude-4-7`

## 4. Design Decisions

### 4.1 Provider Surfaces

Bedrock must have three explicit surfaces:

| Surface | Auth | Transport | Provider selector |
| --- | --- | --- | --- |
| Native Bedrock runtime | AWS SigV4 | `bedrock-runtime.{region}.amazonaws.com` | `bedrock` / `aws-bedrock` |
| Bedrock Mantle Anthropic Messages | AWS SigV4 in current `BedrockConfig`; Bedrock API key only after config support exists | `bedrock-mantle.{region}.api.aws/anthropic/v1/messages` | Native Bedrock endpoint mode |
| OpenAI-compatible Bedrock proxy | Proxy API key | Proxy `/v1` chat/completions contract | `openai_compatible` |

The native Bedrock provider should own AWS model IDs, SigV4, Converse, Invoke,
and Mantle-specific Anthropic Messages transport. OpenAI-compatible proxy usage
must stay outside `ProviderType::Bedrock` so proxy behavior does not silently
change native AWS semantics.

### 4.2 Model ID Parser

Introduce a typed parser instead of returning a single normalized string:

```rust
pub struct ParsedBedrockModelId {
    pub user_selector: String,
    pub execution_model_id: String,
    pub metadata_lookup_ids: Vec<String>,
    pub profile_kind: Option<BedrockInferenceProfileKind>,
    pub family_hint: Option<BedrockModelFamily>,
}

pub enum BedrockInferenceProfileKind {
    Geo { prefix: String },
    Global,
    Regional { region: String },
    ApplicationArn,
}
```

Parsing rules:

- Strip only the user-facing `bedrock/` selector from `execution_model_id`.
- Preserve `us.`, `eu.`, `apac.`, `global.`, ARNs, and other AWS execution IDs.
- Build `metadata_lookup_ids` in exact-then-canonical order.
- Canonical fallback may strip geo/global/profile prefixes for metadata only.
- No execution path may use a canonicalized fallback ID unless the caller
  explicitly configured that canonical ID.

Examples:

| User model | Execution ID | Metadata lookup order |
| --- | --- | --- |
| `bedrock/us.anthropic.claude-3-5-sonnet-20241022-v2:0` | `us.anthropic.claude-3-5-sonnet-20241022-v2:0` | exact, `anthropic.claude-3-5-sonnet-20241022-v2:0` |
| `bedrock/global.anthropic.claude-sonnet-4-v1:0` | `global.anthropic.claude-sonnet-4-v1:0` | exact, `anthropic.claude-sonnet-4-v1:0` |
| `anthropic.claude-opus-4-7` | `anthropic.claude-opus-4-7` | exact |
| Bedrock inference profile ARN | ARN unchanged | exact, model ID from configured profile metadata if known |

### 4.3 Catalog Shape

Replace the current hardcoded `ModelConfig` plus separate pricing table with a
single source of truth that can project into both shapes while keeping existing
public APIs compatible.

Required catalog fields:

```rust
pub struct BedrockModelCatalogEntry {
    pub model_id: &'static str,
    pub canonical_model_id: Option<&'static str>,
    pub display_name: &'static str,
    pub family: BedrockModelFamily,
    pub endpoint_support: BedrockEndpointSupport,
    pub profile_support: BedrockProfileSupport,
    pub lifecycle: BedrockModelLifecycle,
    pub limits: BedrockModelLimits,
    pub capabilities: BedrockModelCapabilities,
    pub parameter_policy: BedrockParameterPolicy,
    pub pricing: Option<ModelPricing>,
    pub source: BedrockCatalogSource,
}
```

Endpoint support should answer which transports are valid:

```rust
pub struct BedrockEndpointSupport {
    pub runtime_invoke: bool,
    pub runtime_invoke_stream: bool,
    pub runtime_converse: bool,
    pub runtime_converse_stream: bool,
    pub mantle_messages: bool,
}
```

Parameter policy should be enforced before serialization. Unsupported or
forbidden parameters should raise a typed invalid-request error rather than
being dropped or sent to AWS to fail later.

Claude Opus 4.7 Messages-API Bedrock seed entry:

- Bedrock ID: `anthropic.claude-opus-4-7`
- Source: Anthropic model overview lists this as the AWS Bedrock ID for Claude
  Opus 4.7 through the Messages-API Bedrock endpoint; implementation must still
  verify account and region availability before enabling live runtime support.
- Context: 1M tokens
- Max synchronous output: 128K tokens
- Thinking: adaptive thinking only when explicitly enabled
- Sampling: reject non-default `temperature`, `top_p`, and `top_k`
- Endpoint: Bedrock Messages API via `bedrock-mantle`; runtime support should
  be enabled only if verified for the AWS account/region path used by tests.

### 4.4 Exact-Then-Canonical Metadata Lookup

Metadata lookup must prefer exact IDs. This allows profile-specific overrides:

1. Look up `execution_model_id` exactly.
2. If no exact match and the parser identifies a profile prefix, look up the
   canonical base model ID.
3. If no match exists, return a clear unsupported-model error with both the
   execution ID and fallback IDs.

This supports inference profiles without requiring duplicate full catalog rows
for every geo/global prefix.

### 4.5 Request Transformation

Provider request transformation must use the catalog policy:

- Select operation from endpoint support and request streaming flag.
- Do not model streaming as separate model API types only; a Converse model can
  support both `converse` and `converse-stream`.
- Use `anthropic_version: bedrock-2023-05-31` for runtime Anthropic Messages
  payloads unless AWS documents a newer required value.
- Use `anthropic-version: 2023-06-01` header for Mantle Anthropic Messages.
- Serialize model-specific thinking controls through a typed policy rather than
  generic `extra_params` string copying.
- Refuse forbidden parameters locally for models such as Claude Opus 4.7.

### 4.6 Configuration

Native runtime config should accept AWS-native auth names:

```yaml
providers:
  - name: bedrock-native
    provider_type: bedrock
    aws_region: us-east-1
    aws_access_key_id: ${AWS_ACCESS_KEY_ID}
    aws_secret_access_key: ${AWS_SECRET_ACCESS_KEY}
    aws_session_token: ${AWS_SESSION_TOKEN}
```

Mantle mode should be explicit:

```yaml
providers:
  - name: bedrock-mantle
    provider_type: bedrock
    endpoint_mode: mantle_messages
    aws_region: us-east-1
    aws_access_key_id: ${AWS_ACCESS_KEY_ID}
    aws_secret_access_key: ${AWS_SECRET_ACCESS_KEY}
    aws_session_token: ${AWS_SESSION_TOKEN}
```

OpenAI-compatible proxy mode should not use `provider_type: bedrock`:

```yaml
providers:
  - name: bedrock-access-gateway
    provider_type: openai_compatible
    base_url: https://your-bedrock-access-gateway.example.com/v1
    api_key: ${BEDROCK_ACCESS_GATEWAY_API_KEY}
```

## 5. Follow-Up Implementation PRs

### Step B1 - Wire Native Bedrock Factory

- Status: pending
- Goal: make `ProviderType::Bedrock` instantiate native `BedrockProvider`.
- Expected files:
  - `src/core/providers/factory/builder.rs`
  - `src/core/providers/factory/registry.rs`
  - `src/core/providers/registry/types.rs`
  - `src/core/providers/registry/lifecycle.rs`
  - `tests/integration/provider_factory_tests.rs`
- Concrete changes:
  - Build `BedrockConfig` from AWS-specific config fields.
  - Keep proxy deployments on `ProviderType::OpenAICompatible`.
  - Update lifecycle/dispatch metadata from `ExplicitOpenAiLike` to native.
- Test commands:
  - `cargo test provider_factory bedrock --lib`
  - `cargo test --test integration provider_factory`
- Done when:
  - Native Bedrock provider creation no longer requires a generic `api_key`.
  - OpenAI-compatible proxy examples still route through OpenAI-like provider.

### Step B2 - Add Typed Bedrock Model ID Parser

- Status: pending
- Goal: preserve execution IDs while supporting metadata fallback.
- Expected files:
  - `src/core/providers/bedrock/utils/mod.rs`
  - `src/core/providers/bedrock/model_id.rs`
  - `src/core/providers/bedrock/mod.rs`
- Concrete changes:
  - Add `ParsedBedrockModelId`.
  - Replace lossy normalization at execution boundaries.
  - Keep `normalize_bedrock_model_id` only as a compatibility metadata helper,
    or deprecate it after call sites are migrated.
- Test commands:
  - `cargo test bedrock_model_id --lib`
  - `cargo test normalize_bedrock_model_id --lib`
- Done when:
  - `bedrock/us.anthropic...` executes with `us.anthropic...`.
  - Exact and canonical lookup IDs are tested separately.

### Step B3 - Converge Bedrock Catalog and Pricing

- Status: pending
- Goal: replace split model config/pricing tables with one catalog source.
- Expected files:
  - `src/core/providers/bedrock/model_config.rs`
  - `src/core/providers/bedrock/utils/cost.rs`
  - `src/core/cost/calculator/*`
- Concrete changes:
  - Add catalog entry type and projections for `ModelConfig` and `ModelPricing`.
  - Add endpoint support, lifecycle, profile support, limits, capabilities, and
    source metadata.
  - Add seed entries for current Claude, Nova, Titan, Llama, Mistral, Cohere,
    AI21, embedding, rerank, image, and video IDs already in the repository.
- Test commands:
  - `cargo test bedrock::model_config --lib`
  - `cargo test bedrock::utils::cost --lib`
- Done when:
  - No Bedrock model ID exists in pricing without matching capability metadata.
  - No Bedrock model ID exists in metadata without expected pricing state or an
    explicit `pricing: None` reason.

### Step B4 - Enforce Parameter Policy

- Status: pending
- Goal: validate model-specific request parameters before transport.
- Expected files:
  - `src/core/providers/bedrock/chat/converse.rs`
  - `src/core/providers/bedrock/chat/transformations/anthropic.rs`
  - `src/core/providers/bedrock/model_config.rs`
  - `src/core/types/thinking.rs`
- Concrete changes:
  - Add policy for allowed, forbidden, renamed, and additional-model fields.
  - Reject Claude Opus 4.7 non-default `temperature`, `top_p`, and `top_k`.
  - Add adaptive-thinking serialization and visible errors for unsupported
    extended thinking budgets.
- Test commands:
  - `cargo test bedrock_parameter_policy --lib`
  - `cargo test bedrock anthropic --lib`
- Done when:
  - Unsupported sampling and thinking settings fail before HTTP calls.
  - Allowed model settings serialize into the documented Bedrock shape.

### Step B5 - Split Native Docs from Proxy Docs

- Status: pending
- Goal: make user setup unambiguous.
- Expected files:
  - `docs/providers/bedrock.md`
  - `docs/providers/openai-compatible-bedrock-proxy.md`
  - `docs/providers/README.md`
  - `README.md`
  - `examples/providers/README.md`
- Concrete changes:
  - Add native Bedrock runtime examples with AWS env names.
  - Add Mantle Anthropic Messages examples.
  - Add Bedrock Access Gateway examples under OpenAI-compatible docs only.
  - Add warning that geo/global profile prefixes are preserved for execution.
- Test commands:
  - `cargo test doc_examples --lib`
  - `cargo check`
- Done when:
  - A user can choose native runtime, Mantle, or proxy without reading code.

### Step B6 - Add Optional Live AWS Smoke Tests

- Status: pending
- Goal: keep default CI offline while enabling real Bedrock verification.
- Expected files:
  - `tests/live/bedrock_runtime.rs`
  - `tests/live/bedrock_mantle.rs`
  - `.github/workflows/*` only if maintainers opt in later
- Concrete changes:
  - Gate tests behind `LITELLM_RS_LIVE_BEDROCK=1`.
  - Require `AWS_REGION`, AWS credentials, and explicit model env vars.
  - Add runtime Converse, streaming, inference profile, and Mantle smoke paths.
- Test commands:
  - `cargo test bedrock --lib`
  - `LITELLM_RS_LIVE_BEDROCK=1 cargo test --test live_bedrock -- --ignored`
- Done when:
  - Default CI does not require AWS credentials.
  - Maintainers can run one documented live command to test real AWS behavior.

## 6. Offline Regression Matrix

| Area | Required regression |
| --- | --- |
| Provider wiring | `ProviderType::Bedrock` constructs native provider; proxy examples construct `OpenAILikeProvider` |
| Model parser | `bedrock/`, geo, global, region-like, and ARN IDs preserve execution ID |
| Metadata lookup | exact match wins; canonical fallback only supplies metadata |
| Runtime routing | non-streaming and streaming select correct operation from endpoint support |
| Mantle routing | Anthropic Messages request uses Mantle URL and header version |
| Parameter policy | forbidden sampling fails locally; adaptive thinking serializes only when supported |
| Catalog integrity | pricing and model metadata stay in sync |
| Docs | native and proxy examples parse as config fixtures |

## 7. Non-Goals

- Do not auto-detect or scrape AWS account model access during default CI.
- Do not make OpenAI-compatible proxy behavior masquerade as native Bedrock.
- Do not silently downgrade native Bedrock failures to proxy calls.
- Do not strip geo/global inference profile IDs on the execution path.
- Do not require live AWS credentials for normal unit, integration, or PR checks.

## 8. Completion Criteria for the Issue

This design issue is complete when this document is merged under `docs/plan/`.
Follow-up implementation issues or PRs can then be opened from Steps B1-B6.
