# DeepSeek Provider

LiteLLM-RS routes DeepSeek through the OpenAI-compatible provider catalog. The
official DeepSeek API base is `https://api.deepseek.com`.

## Models

### DeepSeek V4

| Model | Context | Max Output | Pricing per 1M tokens | Notes |
|-------|---------|------------|------------------------|-------|
| `deepseek-v4-flash` | 1M | 384K | Off-peak: $0.22 cache-miss input, $0.007 cache-hit input, $0.66 output | Fast V4 model; thinking is enabled by default and non-thinking mode is supported |
| `deepseek-v4-flash-vision-exp` | 1M | 384K | Off-peak: $0.22 cache-miss input, $0.007 cache-hit input, $0.66 output | Experimental vision model; images are converted to input tokens and thinking is enabled by default |
| `deepseek-v4-pro` | 1M | 384K | Off-peak: $0.66 cache-miss input, $0.022 cache-hit input, $1.98 output | Higher quality V4 model; thinking is enabled by default and non-thinking mode is supported |

### Legacy Aliases

DeepSeek's current docs keep these aliases for compatibility. They were
deprecated on 2026-07-24 at 15:59 UTC.

| Alias | Current mapping | Thinking behavior |
|-------|-----------------|-------------------|
| `deepseek-chat` | `deepseek-v4-flash` | Non-thinking mode (thinking disabled) |
| `deepseek-reasoner` | `deepseek-v4-flash` | Always-on thinking (cannot be disabled) |

Note on thinking semantics: `deepseek-reasoner` is the always-on reasoning
alias — every request is dispatched in thinking mode. The canonical
`deepseek-v4-flash` and `deepseek-v4-pro` IDs default to thinking enabled but
support an optional non-thinking mode. The `deepseek-chat` alias pins the
non-thinking path on top of `deepseek-v4-flash`.

Use the canonical `deepseek-v4-flash` or `deepseek-v4-pro` IDs for new code.

## Capabilities

| Feature | `deepseek-v4-flash` | `deepseek-v4-pro` | `deepseek-v4-flash-vision-exp` |
|---------|---------------------|-------------------|----------------------------------|
| Chat completion | Yes | Yes | Yes |
| Responses API | Yes | Yes | Yes |
| Anthropic API | Yes | Yes | Yes |
| Streaming | Yes | Yes | Yes |
| Tool calls | Yes | Yes | Yes |
| JSON output | Yes | Yes | Yes |
| Chat prefix completion | Beta | Beta | Beta |
| FIM completion | Non-thinking mode only | Non-thinking mode only | No |
| Vision | No | No | Yes |
| Embeddings | No | No | No |

## Setup

```bash
export DEEPSEEK_API_KEY=your_deepseek_api_key_here
```

```yaml
providers:
  deepseek:
    api_key: "${DEEPSEEK_API_KEY}"
    api_base: "https://api.deepseek.com"
    timeout_seconds: 30
    max_retries: 3
```

## Usage

```rust
use litellm_rs::{completion, system_message, user_message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let response = completion(
        "deepseek-v4-flash",
        vec![
            system_message("You are a helpful programming assistant."),
            user_message("Explain stack and heap memory in Rust."),
        ],
        None,
    )
    .await?;

    println!("{}", response.choices[0].message.content);
    Ok(())
}
```

Explicit provider prefixes are also supported:

```rust
let response = completion("deepseek/deepseek-v4-pro", messages, None).await?;
```

Legacy aliases continue to work:

```rust
let chat = completion("deepseek-chat", messages.clone(), None).await?;
let reasoning = completion("deepseek-reasoner", messages, None).await?;
```

## Cost Notes

DeepSeek publishes cache-hit and cache-miss input prices with peak and off-peak
rates. LiteLLM-RS currently stores scalar prices, so it uses the official
off-peak card: the rate that applies outside 01:00-04:00 and 06:00-10:00 UTC on
weekdays. During those peak windows, DeepSeek charges twice the stored rates.

The cache-miss price is stored as `input_cost_per_token`, the output price as
`output_cost_per_token`, and the cache-hit price as
`cache_read_input_token_cost`. Rates below were checked against DeepSeek's
official pricing page on 2026-08-24.

For current V4 pricing:

- `deepseek-v4-flash`: off-peak $0.22 cache-miss input, $0.007 cache-hit input, $0.66 output per 1M tokens; peak $0.44, $0.014, and $1.32 respectively.
- `deepseek-v4-flash-vision-exp`: uses the same rates as Flash; image tokens are billed as input tokens after dimension-based conversion.
- `deepseek-v4-pro`: off-peak $0.66 cache-miss input, $0.022 cache-hit input, $1.98 output per 1M tokens; peak $1.32, $0.044, and $3.96 respectively.
- `deepseek-chat` and `deepseek-reasoner` continue to use `deepseek-v4-flash` pricing as deprecated compatibility aliases.

## Integration Testing

```bash
export DEEPSEEK_API_KEY=your_key_here
cargo test --all-features deepseek_integration -- --ignored
```

## Resources

- [DeepSeek API docs](https://api-docs.deepseek.com/)
- [DeepSeek pricing](https://api-docs.deepseek.com/quick_start/pricing)
