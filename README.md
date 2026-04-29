# litellm-rs

A high-performance Rust library and gateway for calling 100+ LLM APIs in an OpenAI-compatible format.

[![Crates.io](https://img.shields.io/crates/v/litellm-rs.svg)](https://crates.io/crates/litellm-rs)
[![Documentation](https://docs.rs/litellm-rs/badge.svg)](https://docs.rs/litellm-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- **100+ AI Providers** - OpenAI, Anthropic, Google, Azure, AWS Bedrock, and more
- **OpenAI-Compatible API** - Drop-in replacement for OpenAI SDK
- **High Performance** - 10,000+ requests/second, <10ms routing overhead
- **Intelligent Routing** - Load balancing, failover, cost optimization
- **Enterprise Ready** - Auth, rate limiting, caching, observability

## Quick Start (5 Minutes, API-Only Recommended)

Most users use this project as a unified API library, not as a gateway server. Start with API-only mode first.

```toml
[dependencies]
litellm-rs = { version = "0.4", default-features = false, features = ["lite"] }
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
cp config/gateway.yaml.example config/gateway.yaml
cargo run --bin gateway
```

#### Install binary and run

```bash
cargo install litellm-rs --bin gateway
mkdir -p config
curl -L https://raw.githubusercontent.com/majiayu000/litellm-rs/main/config/gateway.yaml.example -o config/gateway.yaml
gateway
```

Notes:

- `gateway` requires the `storage` feature at build time.
- Default features include `sqlite`, so default `cargo run`/`cargo install` satisfy this requirement.

## Installation

```toml
# Full gateway with SQLite + Redis (default)
[dependencies]
litellm-rs = "0.4"

# API-only - lightweight, no actix-web/argon2/aes-gcm/clap
[dependencies]
litellm-rs = { version = "0.4", default-features = false }

# API-only with metrics
[dependencies]
litellm-rs = { version = "0.4", default-features = false, features = ["lite"] }

# Gateway modules in library context (not standalone gateway binary runtime)
[dependencies]
litellm-rs = { version = "0.4", default-features = false, features = ["gateway"] }
```

## Supported Providers

| Provider | Chat | Embeddings | Images | Audio |
|----------|------|------------|--------|-------|
| OpenAI | ✅ | ✅ | ✅ | ✅ |
| Anthropic | ✅ | - | - | - |
| Google (Gemini) | ✅ | ✅ | ✅ | - |
| Azure OpenAI | ✅ | ✅ | ✅ | ✅ |
| AWS Bedrock | ✅ | ✅ | - | - |
| Google Vertex AI | ✅ | ✅ | ✅ | - |
| Groq | ✅ | - | - | ✅ |
| DeepSeek | ✅ | - | - | - |
| Kimi (Moonshot AI) | ✅ | - | - | - |
| GLM (Zhipu AI) | ✅ | - | - | - |
| MiniMax | ✅ | - | - | - |
| Mistral | ✅ | ✅ | - | - |
| Cohere | ✅ | ✅ | - | - |
| OpenRouter | ✅ | - | - | - |
| Together AI | ✅ | ✅ | - | - |
| Fireworks AI | ✅ | ✅ | - | - |
| Perplexity | ✅ | - | - | - |
| Replicate | ✅ | - | ✅ | - |
| Hugging Face | ✅ | ✅ | - | - |
| Ollama | ✅ | ✅ | - | - |
| And 80+ more... | | | | |

## Environment Variables

```bash
# Provider API Keys
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GOOGLE_API_KEY=...
AZURE_OPENAI_API_KEY=...
AWS_ACCESS_KEY_ID=...
AWS_SECRET_ACCESS_KEY=...
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
let google = completion("gemini/gemini-pro", vec![user_message("Hi")], None).await?;
let bedrock = completion(
    "bedrock/us.anthropic.claude-3-5-sonnet-20241022-v2:0",
    vec![user_message("Hi")],
    None,
)
.await?;
```

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
- [Configuration Guide](./config/gateway.yaml.example)
- [Examples](./examples/README.md)

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development setup and guidelines.

## Security

See [SECURITY.md](./SECURITY.md) for security policy and vulnerability reporting.

## License

MIT License - see [LICENSE](./LICENSE) for details.

## Acknowledgments

Inspired by [LiteLLM](https://github.com/BerriAI/litellm) (Python).
