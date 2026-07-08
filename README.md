# litellm-rs

A high-performance Rust library and gateway for calling LLM APIs in an OpenAI-compatible format. Ships with 50+ built-in OpenAI-compatible providers plus first-class adapters for OpenAI, Anthropic, AWS Bedrock, Mistral, and Cloudflare.

[![Crates.io](https://img.shields.io/crates/v/litellm-rs.svg)](https://crates.io/crates/litellm-rs)
[![Documentation](https://docs.rs/litellm-rs/badge.svg)](https://docs.rs/litellm-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- **60+ runtime-wired providers** - OpenAI, Anthropic, AWS Bedrock, Mistral, Cloudflare, plus 50+ OpenAI-compatible providers via the Tier 1 catalog. See [Provider Support](#provider-support) for the full matrix.
- **OpenAI-Compatible API** - Drop-in replacement for OpenAI SDK
- **High Performance** - 10,000+ requests/second, <10ms routing overhead
- **Intelligent Routing** - Load balancing, failover, cost optimization
- **Gateway Controls** - Auth, rate limiting, deterministic caching, metrics, and health endpoints

## Quick Start (5 Minutes, API-Only Recommended)

Most users use this project as a unified API library, not as a gateway server. Start with API-only mode first.

```toml
[dependencies]
litellm-rs = { version = "0.5", default-features = false, features = ["lite"] }
```

For crate users, no `make` is required.

## Usage

### As a Library (API Integration)

```rust
use litellm_rs::{completion, user_message, system_message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let response = completion(
        "gpt-4",
        vec![
            system_message("You are a helpful assistant."),
            user_message("Hello!"),
        ],
        None,
    ).await?;

    println!("{}", response.choices[0].message.content.as_ref().unwrap());
    Ok(())
}
```

### As a Gateway Server

#### Run from source repository

```bash
git clone https://github.com/majiayu000/litellm-rs.git
cd litellm-rs
cp config/gateway.dev.yaml.example config/gateway.yaml
cargo run --bin gateway
```

#### Install binary and run

```bash
cargo install litellm-rs --bin gateway
mkdir -p config
curl -L https://raw.githubusercontent.com/majiayu000/litellm-rs/main/config/gateway.dev.yaml.example -o config/gateway.yaml
gateway
```

Notes:

- `gateway` requires the `storage` feature at build time.
- Default features include `sqlite`, so default `cargo run`/`cargo install` satisfy this requirement.
- The development config starts without provider credentials or auth secrets and uses the local `vllm` catalog provider. Use `config/gateway.yaml.example` for production-style deployments with real provider keys and auth enabled.

#### Router Configuration

The gateway router config maps these fields into the runtime router:

- `router.strategy` selects the deployment routing strategy.
- `router.circuit_breaker.failure_threshold` controls consecutive failures before cooldown.
- `router.circuit_breaker.recovery_timeout` controls cooldown duration in seconds.
- `router.circuit_breaker.min_requests` sets the sample size required before cooldown.
- `router.circuit_breaker.success_threshold` sets the successes required to recover from cooldown.
- `router.load_balancer.health_check_enabled` enables pre-call deployment health checks.

`router.load_balancer.sticky_sessions` and `router.load_balancer.session_timeout` are reserved for future session affinity. Non-default values fail config validation until runtime affinity is implemented.

#### Core Subsystem Runtime Status

Runtime wiring decisions are tracked in [`src/core/subsystem_registry.rs`](./src/core/subsystem_registry.rs), and tests assert that every module exported from `src/core/mod.rs` is either referenced by the gateway runtime or explicitly classified. The current issue-838 subsystem decisions are:

| Subsystem | Decision | Runtime status |
| --- | --- | --- |
| `core/guardrails` | experimental-gate | Module-only. `GuardrailEngine` is not configured or executed on completion requests. |
| `core/ip_access` | experimental-gate | Middleware exists but is not registered in the Actix stack. |
| `core/mcp` | experimental-gate | MCP gateway is not mounted; Responses API only passes MCP tool descriptors through to providers. |
| `core/a2a` | experimental-gate | A2A gateway types compile, but no route or `AppState` entry mounts them. |
| `core/realtime` | experimental-gate | Realtime WebSocket types exist, but no gateway route is mounted. |
| `core/observability` and `core/integrations` | experimental-gate | Basic tracing, metrics middleware, and health endpoints are wired elsewhere; Langfuse/OpenTelemetry managers and exporters are not initialized by the binary. |
| `core/batch` | experimental-gate | `/v1/batches` is wired as a provider proxy; `core::batch::BatchProcessor` is not constructed. |
| `core/webhooks` | experimental-gate | `WebhookManager` is not configured or constructed by the gateway runtime. |
| `core/semantic_cache` | config-rejected | `cache.semantic_cache=true` fails validation until runtime semantic cache handling is wired. |
| `core/analytics` | experimental-gate | Analytics types and engine are feature-gated, with no runtime collector or route. |
| `core/virtual_keys` | experimental-gate | Gateway key routes use `core::keys`; `VirtualKeyManager` is not in `AppState`. |

## Installation

```toml
# Full gateway with SQLite + Redis (default)
[dependencies]
litellm-rs = "0.5"

# API-only - lightweight, no actix-web/argon2/aes-gcm/clap
[dependencies]
litellm-rs = { version = "0.5", default-features = false }

# API-only with metrics
[dependencies]
litellm-rs = { version = "0.5", default-features = false, features = ["lite"] }

# Gateway modules in library context (not standalone gateway binary runtime)
[dependencies]
litellm-rs = { version = "0.5", default-features = false, features = ["gateway"] }
```

## Provider Support

Providers are organised into two tiers (see [CLAUDE.md → Provider Tiers](./CLAUDE.md#provider-tiers) for the engineering definition).

- **Tier 1 — catalog-only**: OpenAI-compatible endpoints declared as data in [`src/core/providers/registry/catalog.rs`](./src/core/providers/registry/catalog.rs). Routed through `OpenAILikeProvider`. Always available (no cargo feature required). The current crate runtime exposes chat completions and chat streaming for these providers; embeddings, images, audio, and other non-chat endpoints are not forwarded yet.
- **Tier 2 — code-based**: providers with custom request/response handling, auth signing, or streaming. Wired into the `Provider` enum and the factory. Some Tier 2 builders are feature-gated.

Router deployments use the closed `Provider` enum. Implementing `LLMProvider`
alone does not make a third-party provider routeable; use the generic
OpenAI-compatible path for compatible endpoints, or wire a code-based provider
into the enum, dispatch, registry metadata, and factory.

> The provider and route-surface matrices below are validated against the provider registry and Tier 1 catalog. The source of truth for Tier 1 entries is [`catalog.rs`](./src/core/providers/registry/catalog.rs); Tier 2 identity and dispatch metadata lives in [`src/core/providers/registry/types.rs`](./src/core/providers/registry/types.rs), with construction branches in [`src/core/providers/factory/registry.rs`](./src/core/providers/factory/registry.rs). Cross-surface support lives in [`src/core/providers/registry/support_matrix.rs`](./src/core/providers/registry/support_matrix.rs). Capability columns describe which endpoints this crate exposes for the provider — `passthrough` means an implemented crate endpoint forwards the call to the upstream OpenAI-compatible endpoint without per-provider transformation.

### Route-surface matrix

| Selector class | HTTP chat / stream | HTTP embeddings / image | SDK chat / stream / embeddings | `completion()` chat / stream | Notes |
|----------------|--------------------|--------------------------|--------------------------------|-------------------------------|-------|
| `openai` | ✅ / ✅ | ✅ / ✅ | ✅ / ✅ / ✅ | ✅ / ✅ | Reference provider across all current surfaces. |
| `anthropic` | ✅ / ✅ | – / – | ✅ / ✅ / – | ✅ / ✅ | Native chat and streaming only. |
| `azure` | passthrough / passthrough | `providers-extra` / `providers-extra` | – / – / ✅ | passthrough / passthrough | SDK exposes Azure embeddings; SDK chat is not implemented. |
| `azure_ai` | passthrough / passthrough | `providers-extra` / `providers-extra` | – / – / – | `providers-extra` / `providers-extra` | `completion()` supports `azure_ai/` and `azure-ai/` routes when the native feature is enabled. |
| `bedrock` | ✅ / ✅ | ✅ / – | – / – / – | – / – | SDK Bedrock and public `completion()` routing are not implemented. |
| `mistral`, `cloudflare`, `cohere`, `vertex_ai`, `gemini`, `fal_ai`, `replicate` | provider-specific | provider-specific | – / – / – | – / – | See `support_matrix.rs` for feature-gated HTTP support. |
| `google` / SDK `Google` | – / – | – / – | – / – / – | – / – | Google/Gemini SDK chat is intentionally unsupported until a real adapter exists. |
| Default catalog dynamic routes: `openrouter`, `deepseek`, `moonshot`, `minimax`, `zhipu`, `zai`, `together_ai`, `fireworks_ai`, `aiml`, `groq`, `xiaomi_mimo`, `xai` | passthrough / passthrough | – / – | – / – / – | ✅ / ✅ | OpenAI-compatible routes wired into default `completion()` routing. |
| Other Tier 1 catalog providers | passthrough / passthrough | – / – | – / – / – | – / – | HTTP gateway chat/stream only unless routed through explicit OpenAI-compatible config. |
| SDK `Custom` | – / – | – / – | – / – / ✅ | – / – | SDK custom providers support embeddings when `base_url` is configured. |
| SDK `Ollama` | – / – | – / – | – / ✅ / – | – / – | SDK streaming uses the OpenAI-compatible stream parser; SDK chat is not implemented. |

### Tier 2 — code-based providers

| Provider | Cargo feature | Chat | Stream | Embed | Image | Audio | Notes |
|----------|---------------|------|--------|-------|-------|-------|-------|
| OpenAI (`openai`) | always | ✅ | ✅ | ✅ | ✅ | ✅ | Reference implementation. |
| Anthropic (`anthropic`) | always | ✅ | ✅ | – | – | – | Native Anthropic messages API. |
| Mistral (`mistral`) | always | ✅ | ✅ | passthrough | – | – | Native client. |
| Cloudflare Workers AI (`cloudflare`) | always | ✅ | – | – | – | – | Native client with account-id auth; streaming and embeddings currently return `NotSupported`. |
| Cohere (`cohere`) | native factory (`providers-extended`) | ✅ | ✅ | ✅ | – | – | Uses native Cohere `/v2/chat` and `/v2/embed`; the concrete provider also exposes a `/v1/rerank` helper. Explicitly unsupported without `providers-extended`. |
| Azure OpenAI (`azure`) | native factory (`providers-extra`); OpenAILike fallback | ✅ | ✅ | ✅ | ✅ | – | Native Azure supports chat, streaming, embeddings, and image generation with `providers-extra`; otherwise the factory path uses OpenAILike chat/stream only. |
| Azure AI Inference (`azure_ai`) | native factory (`providers-extra`); OpenAILike fallback | ✅ | ✅ | ✅ | ✅ | – | Native Azure AI supports chat, streaming, embeddings, and image generation with `providers-extra`; otherwise the factory path uses OpenAILike chat/stream only. |
| AWS Bedrock (`bedrock`) | always | ✅ | ✅ | ✅ | helper API | – | Native AWS Bedrock runtime path with SigV4 signing. Use `openai_compatible` for Bedrock Access Gateway or other OpenAI-compatible proxies. |
| Google Vertex AI (`vertex_ai`) | native factory (`providers-extra`) | ✅ | ✅ | ✅ | ✅ | – | Uses native Vertex auth and Google-specific URLs when `providers-extra` is enabled; otherwise explicitly unsupported. |
| Google Gemini (`gemini`) | native factory (`providers-extended`) | ✅ | ✅ | – | – | – | Uses native Google AI Studio Gemini auth; use `vertex_ai` for Vertex AI project/location credentials. |
| Meta Llama API (`meta_llama`) | catalog-only (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extra`, but runtime construction is catalog metadata. |
| Vercel v0 (`v0`) | catalog-only (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extra`, but runtime construction is catalog metadata. |
| Amazon Nova (`amazon_nova`) | catalog-only (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extended`, but runtime construction is catalog metadata. |
| fal.ai (`fal_ai`) | native factory (`providers-extended`) | – | – | – | ✅ | – | Uses native Fal AI image-generation endpoints; chat and streaming are explicitly unsupported. |
| Replicate (`replicate`) | native factory (`providers-extended`) | ✅ | ✅ | – | ✅ | – | Uses native Replicate prediction lifecycle handling for chat, streaming, and image generation; explicitly unsupported without `providers-extended`. |
| GitHub Models (`github`) | catalog-only (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extended`, but runtime construction is catalog metadata. |
| GitHub Copilot (`github_copilot`) | native factory (`providers-extended`) | ✅ | ✅ | – | – | – | Uses native GitHub Copilot auth and model access when `providers-extended` is enabled; otherwise explicitly unsupported. |
| Generic OpenAI-compatible (`openai_compatible`) | always | ✅ | ✅ | – | – | – | For self-hosted / unlisted chat-completions endpoints. |

### Tier 1 — catalog providers (OpenAI-compatible, always available)

All entries below route through `OpenAILikeProvider`. Chat and streaming work for any endpoint that follows OpenAI's `/chat/completions` SSE protocol. Embeddings, images, audio, and other non-chat endpoints are not exposed through this path today, even when the upstream provider offers them.

**Cloud (`Bearer` auth via env var):**

`groq`, `together`, `together_ai`, `fireworks`, `fireworks_ai`, `perplexity`, `cerebras`, `openrouter`, `deepinfra`, `deepseek`, `novita`, `nvidia_nim`, `nebius`, `nscale`, `hyperbolic`, `featherless`, `galadriel`, `sambanova`, `heroku`, `friendliai`, `xai`, `moonshot`, `dashscope`, `qwen`, `baichuan`, `minimax`, `volcengine`, `xiaomi_mimo`, `zhipu`, `zai`, `lemonade`, `linkup`, `poe`, `wandb`, `nanogpt`, `aiml_api`, `aiml`, `aleph_alpha`, `anyscale`, `bytez`, `comet_api`, `compactifai`, `maritalk`, `siliconflow`, `yi`, `lambda_ai`, `ovhcloud`

**Local (no API key):**

`vllm`, `hosted_vllm`, `lm_studio`, `llamafile`, `docker_model_runner`, `xinference`, `infinity`, `oobabooga`

### Experimental / module-only

The following modules exist under `src/core/providers/` (gated on `providers-extra` or `providers-extended`) but are **not wired into the unified `Provider` enum or the factory** today. They compile but cannot be selected through `create_provider`/`from_config_async`. Treat them as experimental scaffolding subject to change:

`codestral`, `custom_api`, `deepgram`, `elevenlabs`, `jina`, `milvus`, `ollama`, `pg_vector`, `recraft`, `runwayml`, `sagemaker`, `searxng`, `snowflake`, `stability`, `tavily`, `voyage`, `watsonx`

For self-hosted or unlisted OpenAI-compatible endpoints, prefer the generic `openai_compatible` provider type instead.

## Environment Variables

```bash
# Provider API Keys
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GOOGLE_API_KEY=...
AZURE_OPENAI_API_KEY=...
AWS_ACCESS_KEY_ID=...
AWS_SECRET_ACCESS_KEY=...
AWS_REGION=us-east-1
GROQ_API_KEY=...
DEEPSEEK_API_KEY=...
MOONSHOT_API_KEY=...
ZHIPU_API_KEY=...
MINIMAX_API_KEY=...

# Optional
LITELLM_VERBOSE=true  # Enable verbose logging
```

## Examples

### Multi-Provider Routing

```rust
use litellm_rs::{completion, user_message};

// Automatically routes to the right provider based on model name
let openai = completion("gpt-5.5", vec![user_message("Hi")], None).await?;
let anthropic = completion("anthropic/claude-opus-4-8", vec![user_message("Hi")], None).await?;
let groq = completion("groq/llama-3.1-8b-instant", vec![user_message("Hi")], None).await?;
let bedrock = completion(
    "bedrock/us.anthropic.claude-3-5-sonnet-20241022-v2:0",
    vec![user_message("Hi")],
    None,
)
.await?;
```

`bedrock/` uses the native AWS Bedrock provider. It signs requests with AWS
SigV4 and preserves AWS execution model IDs such as `us.*`, `global.*`,
region-prefixed IDs, and Bedrock ARNs. Use `openai_compatible` for Bedrock
Access Gateway or other OpenAI-compatible proxies instead.

### Embeddings

```rust
use litellm_rs::{embedding, embed_text};

// Single text
let embedding = embed_text("text-embedding-3-small", "Hello world").await?;

// Batch
let embeddings = embedding(
    "text-embedding-3-small",
    vec!["Hello", "World"],
    None,
).await?;
```

### Streaming

```rust
use litellm_rs::{completion_stream, user_message};
use futures::StreamExt;

let mut stream = completion_stream(
    "gpt-4",
    vec![user_message("Tell me a story")],
    None,
).await?;

while let Some(chunk) = stream.next().await {
    if let Ok(chunk) = chunk {
        print!("{}", chunk.choices[0].delta.content.unwrap_or_default());
    }
}
```

## Performance

- **Throughput**: 10,000+ requests/second
- **Latency**: <10ms routing overhead
- **Memory**: ~50MB base footprint
- **Concurrency**: Fully async with Tokio

## Troubleshooting

### Build/test uses too much CPU or memory

- Use API-only defaults first: `cargo test --lib --tests --no-default-features --features "lite"`
- Limit local parallelism when needed: `CARGO_BUILD_JOBS=4 cargo test --lib --tests --no-default-features --features "lite" -- --test-threads=4`
- Avoid `--all-features` unless you are doing release/nightly validation

### I only need provider API aggregation, not gateway

- Prefer `default-features = false` with `features = ["lite"]`
- Use gateway runtime commands only when you need HTTP server/auth/storage middleware

## Documentation

- [API Documentation](https://docs.rs/litellm-rs)
- [Documentation Index](./docs/README.md)
- [Development Gateway Config](./config/gateway.dev.yaml.example)
- [Production Gateway Config](./config/gateway.yaml.example)
- [Examples](./examples/README.md)

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development setup and guidelines.

## Security

See [SECURITY.md](./SECURITY.md) for security policy and vulnerability reporting.

## The Agent Infra Stack

This project is one layer of an open-source stack for running coding agents (Claude Code, Codex) as serious infrastructure. Every piece works standalone; together they close the loop:

`litellm-rs` is the **Route** layer — the gateway underneath everything else, speaking OpenAI format to 100+ providers.

| Layer | Project | What it does |
|---|---|---|
| Extend | [claude-skill-registry](https://github.com/majiayu000/claude-skill-registry) | Discover and search community Claude Code skills |
| Extend | [spellbook](https://github.com/majiayu000/spellbook) | Cross-runtime skills for Claude Code, Codex, and multi-agent workflows |
| Trust | [argus](https://github.com/majiayu000/argus) | Static install-time scanner for supply-chain attacks (npm / PyPI / crates.io) |
| Trust | [vibeguard](https://github.com/majiayu000/vibeguard) | Rules, hooks, and guards against hallucinated or unverified agent changes |
| Remember | [remem](https://github.com/majiayu000/remem) | Local-first persistent memory for Claude Code and Codex sessions |
| Orchestrate | [harness](https://github.com/majiayu000/harness) | Rust agent orchestration platform — rules, skills, GC, observability |
| Route | [litellm-rs](https://github.com/majiayu000/litellm-rs) **◀ you are here** | High-performance Rust AI gateway — 100+ LLM APIs via OpenAI format |
| Keep | [keepline](https://github.com/majiayu000/keepline) | Session command center — monitor, recover, never lose agent work |

---

## License

MIT License - see [LICENSE](./LICENSE) for details.

## Acknowledgments

Inspired by [LiteLLM](https://github.com/BerriAI/litellm) (Python).
