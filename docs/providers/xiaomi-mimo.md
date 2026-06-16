# Xiaomi MiMo Provider

LiteLLM-RS routes Xiaomi MiMo through the OpenAI-compatible provider catalog.
The official pay-as-you-go OpenAI-compatible API base is
`https://api.xiaomimimo.com/v1`.

## Models

| Model | Context | Max Output | Pricing per 1M tokens | Notes |
|-------|---------|------------|------------------------|-------|
| `mimo-v2.5-pro` | 1M | 128K | $0.435 cache-miss input, $0.0036 cache-hit input, $0.87 output | Text model |
| `mimo-v2.5` | 1M | 128K | $0.14 cache-miss input, $0.0028 cache-hit input, $0.28 output | Text and image input |

Legacy MiMo V2 model names are being replaced by the V2.5 series. Xiaomi's
deprecation schedule lists `mimo-v2-pro`, `mimo-v2-omni`, `mimo-v2-flash`, and
`mimo-v2-tts` for deprecation on 2026-06-30 Beijing time.

## Setup

```bash
export MIMO_API_KEY=your_mimo_api_key_here
```

`XIAOMI_API_KEY` remains accepted as a compatibility fallback, but new
configuration should use `MIMO_API_KEY` to match Xiaomi's docs.

```yaml
providers:
  - name: xiaomi_mimo
    provider_type: xiaomi_mimo
    api_key: "${MIMO_API_KEY}"
    base_url: "https://api.xiaomimimo.com/v1"
    timeout: 30
    max_retries: 3
```

## Usage

```rust
use litellm_rs::{completion, system_message, user_message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let response = completion(
        "xiaomi_mimo/mimo-v2.5-pro",
        vec![
            system_message("You are a helpful assistant."),
            user_message("Summarize Xiaomi MiMo in one paragraph."),
        ],
        None,
    )
    .await?;

    println!(
        "{}",
        response.choices[0]
            .message
            .content
            .as_ref()
            .map(|content| content.to_string())
            .unwrap_or_default()
    );
    Ok(())
}
```

## Cost Notes

LiteLLM-RS stores Xiaomi MiMo cache-miss input pricing as
`input_cost_per_token`, output pricing as `output_cost_per_token`, and cache-hit
input pricing as `cache_read_input_token_cost`.

## Resources

- [Xiaomi MiMo OpenAI API compatibility](https://mimo.mi.com/docs/en-US/api/chat/openai-api)
- [Xiaomi MiMo pricing](https://mimo.mi.com/docs/en-US/price/pay-as-you-go)
- [Xiaomi MiMo deprecation schedule](https://mimo.mi.com/docs/en-US/updates/deprecate)
