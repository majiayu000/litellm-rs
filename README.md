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
- **Enterprise Ready** - Auth, rate limiting, caching, observability

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

> The matrix below is **hand-maintained** and reflects the runtime surface today. The source of truth for Tier 1 entries is [`catalog.rs`](./src/core/providers/registry/catalog.rs); Tier 2 wiring lives in [`src/core/providers/factory/registry.rs`](./src/core/providers/factory/registry.rs). Capability columns describe which endpoints this crate exposes for the provider — `passthrough` means an implemented crate endpoint forwards the call to the upstream OpenAI-compatible endpoint without per-provider transformation. A dynamically-generated matrix is tracked as a follow-up.

### Tier 2 — code-based providers

| Provider | Cargo feature | Chat | Stream | Embed | Image | Audio | Notes |
|----------|---------------|------|--------|-------|-------|-------|-------|
| OpenAI (`openai`) | always | ✅ | ✅ | ✅ | ✅ | ✅ | Reference implementation. |
| Anthropic (`anthropic`) | always | ✅ | ✅ | – | – | – | Native Anthropic messages API. |
| Mistral (`mistral`) | always | ✅ | ✅ | passthrough | – | – | Native client. |
| Cloudflare Workers AI (`cloudflare`) | always | ✅ | – | – | – | – | Native client with account-id auth; streaming and embeddings currently return `NotSupported`. |
| Azure OpenAI (`azure`) | wired via factory (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extra`, but the factory path uses OpenAILike chat/stream only. |
| Azure AI Inference (`azure_ai`) | wired via factory (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extra`, but the factory path uses OpenAILike chat/stream only. |
| AWS Bedrock (`bedrock`) | always | ✅ | ✅ | ✅ | helper API | – | Native AWS Bedrock runtime path with SigV4 signing. Use `openai_compatible` for Bedrock Access Gateway or other OpenAI-compatible proxies. |
| Google Vertex AI (`vertex_ai`) | wired via factory (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extra`, but the factory path uses OpenAILike chat/stream only. |
| Meta Llama API (`meta_llama`) | wired via factory (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extra`, but the factory path uses OpenAILike chat/stream only. |
| Vercel v0 (`v0`) | wired via factory (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extra`, but the factory path uses OpenAILike chat/stream only. |
| Amazon Nova (`amazon_nova`) | wired via factory (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extended`, but the factory path uses OpenAILike chat/stream only. |
| fal.ai (`fal_ai`) | wired via factory (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extended`, but the factory path uses OpenAILike chat/stream only. |
| Replicate (`replicate`) | wired via factory (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extended`, but the factory path uses OpenAILike chat/stream only. |
| GitHub Models (`github`) | wired via factory (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extended`, but the factory path uses OpenAILike chat/stream only. |
| GitHub Copilot (`github_copilot`) | wired via factory (`OpenAILike`) | ✅ | ✅ | – | – | – | Native module retained behind `providers-extended`, but the factory path uses OpenAILike chat/stream only. |
| Generic OpenAI-compatible (`openai_compatible`) | always | ✅ | ✅ | – | – | – | For self-hosted / unlisted chat-completions endpoints. |

### Tier 1 — catalog providers (OpenAI-compatible, always available)

All entries below route through `OpenAILikeProvider`. Chat and streaming work for any endpoint that follows OpenAI's `/chat/completions` SSE protocol. Embeddings, images, audio, and other non-chat endpoints are not exposed through this path today, even when the upstream provider offers them.

**Cloud (`Bearer` auth via env var):**

`groq`, `together`, `fireworks`, `perplexity`, `cerebras`, `openrouter`, `deepinfra`, `deepseek`, `novita`, `nvidia_nim`, `nebius`, `nscale`, `hyperbolic`, `featherless`, `galadriel`, `sambanova`, `heroku`, `friendliai`, `xai`, `moonshot`, `dashscope`, `qwen`, `baichuan`, `minimax`, `volcengine`, `xiaomi_mimo`, `zhipu`, `lemonade`, `linkup`, `poe`, `wandb`, `nanogpt`, `aiml_api`, `aleph_alpha`, `anyscale`, `bytez`, `comet_api`, `compactifai`, `maritalk`, `siliconflow`, `yi`, `lambda_ai`, `ovhcloud`

**Local (no API key):**

`vllm`, `hosted_vllm`, `lm_studio`, `llamafile`, `docker_model_runner`, `xinference`, `infinity`, `oobabooga`

### Experimental / module-only

The following modules exist under `src/core/providers/` (gated on `providers-extra` or `providers-extended`) but are **not wired into the unified `Provider` enum or the factory** today. They compile but cannot be selected through `create_provider`/`from_config_async`. Treat them as experimental scaffolding subject to change:

`ai21`, `baseten`, `clarifai`, `codestral`, `cohere`, `custom_api`, `databricks`, `datarobot`, `deepgram`, `deepl`, `elevenlabs`, `empower`, `exa_ai`, `firecrawl`, `gemini`, `gigachat`, `google_pse`, `gradient_ai`, `huggingface`, `jina`, `langgraph`, `manus`, `milvus`, `morph`, `nlp_cloud`, `oci`, `ollama`, `petals`, `pg_vector`, `predibase`, `ragflow`, `recraft`, `runwayml`, `sagemaker`, `sap_ai`, `searxng`, `snowflake`, `spark`, `stability`, `tavily`, `topaz`, `triton`, `vercel_ai`, `voyage`, `watsonx`

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
let openai = completion("gpt-4", vec![user_message("Hi")], None).await?;
let anthropic = completion("anthropic/claude-3-opus", vec![user_message("Hi")], None).await?;
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

## License

MIT License - see [LICENSE](./LICENSE) for details.

## Acknowledgments

Inspired by [LiteLLM](https://github.com/BerriAI/litellm) (Python).
