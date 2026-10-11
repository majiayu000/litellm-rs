# Xiaomi MiMo Provider

LiteLLM-RS routes Xiaomi MiMo through the OpenAI-compatible provider catalog.
The official pay-as-you-go OpenAI-compatible API base is
`https://api.xiaomimimo.com/v1`.

Token Plan uses region-specific bases. For Singapore, use
`https://token-plan-sgp.xiaomimimo.com/v1` for OpenAI-compatible chat
completions and `https://token-plan-sgp.xiaomimimo.com/anthropic` for the
Anthropic-compatible messages API.

## Models

| Model | Context | Max Output | Pricing per 1M tokens | Notes |
|-------|---------|------------|------------------------|-------|
| `mimo-v2.6-pro` | 1M | 128K | $0.435 cache-miss input, $0.0036 cache-hit input, $0.87 output | Upstream multimodal model; configure each input type explicitly |
| `mimo-v2.6-flash` | 1M | 128K | $0.14 cache-miss input, $0.0028 cache-hit input, $0.28 output | Upstream multimodal model; configure each input type explicitly |

The [official deprecation notice](https://mimo.mi.com/docs/en-US/updates/deprecate)
expires `mimo-v2.5-pro` and `mimo-v2.5` at **10:00 UTC+8 on October 21, 2026**.
Switch to `mimo-v2.6-pro` and `mimo-v2.6-flash` before then. As checked on
October 11, the notice conflicts on the October 14 automatic-routing time:
the table says 10:00 UTC+8, while the prose says 18:00 UTC+8. Do not depend on
that transition window.

Flash's omitted `max_completion_tokens` default increases from 32,768 to
131,072. Set an explicit output cap when migrating. The upstream notice also
says thinking mode fixes `temperature` at 1.0 and `top_p` at 0.95.
The [published overseas token rates](https://mimo.mi.com/docs/en-US/price/pay-as-you-go)
are unchanged between each old/new model pair; regional RMB prices differ.

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

For Singapore Token Plan OpenAI-compatible routing, override the base URL:

```yaml
providers:
  - name: xiaomi_mimo_token_plan_sgp
    provider_type: xiaomi_mimo
    api_key: "${MIMO_API_KEY}"
    base_url: "https://token-plan-sgp.xiaomimimo.com/v1"
    models:
      - "mimo-v2.6-flash"
      - "mimo-v2.6-pro"
```

For the Anthropic-compatible Token Plan endpoint, configure the native
Anthropic provider with an explicit compatible-model opt-in:

```yaml
providers:
  - name: xiaomi_mimo_token_plan_sgp_anthropic
    provider_type: anthropic
    api_key: "${MIMO_API_KEY}"
    base_url: "https://token-plan-sgp.xiaomimimo.com/anthropic"
    settings:
      allow_unknown_models: true
    models:
      - "mimo-v2.6-flash"
      - "mimo-v2.6-pro"
```

`allow_unknown_models` is required because MiMo model IDs are not Claude model
IDs. It keeps first-party Anthropic providers strict by default while allowing
explicit Anthropic-compatible upstreams. This example opts in to text chat only;
upstream multimodal capability does not automatically enable image, audio, or
video input in this configuration.

## Usage

```rust
use litellm_rs::{CompletionOptions, completion, system_message, user_message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let response = completion(
        "xiaomi_mimo/mimo-v2.6-pro",
        vec![
            system_message("You are a helpful assistant."),
            user_message("Summarize Xiaomi MiMo in one paragraph."),
        ],
        Some(CompletionOptions {
            extra_params: std::collections::HashMap::from([
                ("max_completion_tokens".into(), serde_json::json!(1024)),
            ]),
            ..Default::default()
        }),
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
input pricing as `cache_read_input_token_cost`. Anthropic-compatible requests
using bare or `anthropic/`-qualified MiMo IDs resolve the exact
`xiaomi_mimo/mimo-v2.6-pro` or `xiaomi_mimo/mimo-v2.6-flash` catalog row.
Exact custom Xiaomi pricing rows retain priority; unknown models and foreign
provider rows remain rejected. This lookup does not rewrite the upstream model
ID or enable additional modalities.

## Resources

- [Xiaomi MiMo OpenAI API compatibility](https://mimo.mi.com/docs/en-US/api/chat/openai-api)
- [Xiaomi MiMo pricing](https://mimo.mi.com/docs/en-US/price/pay-as-you-go)
- [Xiaomi MiMo deprecation schedule](https://mimo.mi.com/docs/en-US/updates/deprecate)
