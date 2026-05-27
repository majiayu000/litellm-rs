# LiteLLM Rust Examples

This directory contains examples demonstrating how to use the LiteLLM Rust library with various AI providers.

## Available Examples

### OpenRouter
- `openrouter_completion.rs` - Basic completion with OpenRouter API

**Setup:**
```bash
export OPENROUTER_API_KEY="your-api-key"
```

**Run:**
```bash
cargo run --example openrouter_completion
```

### Azure AI
- `azure_ai_completion.rs` - Basic completion with Azure AI
- `azure_ai_streaming.rs` - Streaming responses from Azure AI
- `azure_ai_chat.rs` - Chat completion with Azure AI

**Setup:**
```bash
export AZURE_AI_API_KEY="your-api-key"
export AZURE_AI_API_BASE="https://your-resource.cognitiveservices.azure.com"
```

**Run:**
```bash
cargo run --example azure_ai_completion    # Basic completion
cargo run --example azure_ai_streaming     # Streaming responses
cargo run --example azure_ai_chat         # Chat completion
```

### DeepSeek
- `deepseek_completion.rs` - Completion with DeepSeek models

**Setup:**
```bash
export DEEPSEEK_API_KEY="your-api-key"
```

**Run:**
```bash
cargo run --example deepseek_completion
```

### AWS Bedrock

Bedrock has two setup paths — pick one before writing code:

- Native AWS Bedrock runtime (SigV4 Converse) — see [`docs/providers/bedrock.md`](../docs/providers/bedrock.md).
- OpenAI-compatible Bedrock proxy (Bedrock Access Gateway and similar) — see [`docs/providers/openai-compatible-bedrock-proxy.md`](../docs/providers/openai-compatible-bedrock-proxy.md).

## Quick Test All Providers

```bash
# Test OpenRouter
OPENROUTER_API_KEY=xxx cargo run --example openrouter_completion

# Test Azure AI
AZURE_AI_API_KEY=xxx AZURE_AI_API_BASE=xxx cargo run --example azure_ai_completion

# Test DeepSeek  
DEEPSEEK_API_KEY=xxx cargo run --example deepseek_completion
```

## Example Code Structure

All examples follow a similar pattern:

1. **Environment Setup** - Check for required API keys
2. **Message Creation** - Use helper functions like `system_message()`, `user_message()`
3. **API Call** - Use the unified `completion()` function with provider prefix
4. **Response Handling** - Process and display the response

### Basic Usage Pattern

```rust
use litellm_rs::{completion, CompletionOptions};
use litellm_rs::{system_message, user_message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let messages = vec![
        system_message("You are a helpful assistant"),
        user_message("Hello!"),
    ];
    
    // Provider format: "provider_name/model_name"
    let response = completion("azure_ai/gpt-4o", messages, None).await?;
    println!("Response: {:?}", response.choices[0].message.content);
    
    Ok(())
}
```

### With Parameters

```rust
let params = CompletionOptions {
    temperature: Some(0.7),
    max_tokens: Some(100),
    top_p: Some(0.9),
    stream: false,
    ..Default::default()
};

let response = completion("provider/model", messages, Some(params)).await?;
```

## Supported Providers and Models

### OpenRouter
- **Models**: claude-3-sonnet, gpt-4o, llama-3.1-70b-instruct, mixtral-8x7b-instruct
- **Features**: Streaming, Function Calling, Vision, Embeddings
- **Docs**: https://openrouter.ai/docs

### Azure AI
- **Models**: ALL Azure AI deployed models (gpt-4o, gpt-4, gpt-35-turbo, etc.)
- **Features**: Streaming, Function Calling, Vision, Embeddings
- **Docs**: https://azure.microsoft.com/en-us/products/ai-services/openai-service

### DeepSeek
- **Models**: deepseek-chat, deepseek-coder
- **Features**: Streaming, Function Calling
- **Docs**: https://platform.deepseek.com/docs

### OpenAI (built-in)
- **Models**: gpt-4o, gpt-4-turbo, gpt-3.5-turbo, text-embedding-3-large
- **Features**: Streaming, Function Calling, Vision, Embeddings

### Anthropic (built-in)
- **Models**: claude-3-opus, claude-3-sonnet, claude-3-haiku
- **Features**: Streaming, Function Calling, Vision

## Environment Variables

Each provider requires specific environment variables:

| Provider | Required Variables |
|----------|-------------------|
| OpenRouter | `OPENROUTER_API_KEY` |
| Azure AI | `AZURE_AI_API_KEY`, `AZURE_AI_API_BASE` |
| DeepSeek | `DEEPSEEK_API_KEY` |
| OpenAI | `OPENAI_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |

## Streaming Examples

For providers that support streaming:

```rust
use litellm_rs::{completion_stream, CompletionOptions};
use futures::StreamExt;

let params = CompletionOptions {
    stream: true,
    ..Default::default()
};

let mut stream = completion_stream("provider/model", messages, Some(params)).await?;

while let Some(chunk) = stream.next().await {
    if let Ok(chunk) = chunk {
        if let Some(content) = &chunk.choices[0].delta.content {
            print!("{}", content);
        }
    }
}
```

## Error Handling

All examples include proper error handling:

```rust
match completion("provider/model", messages, None).await {
    Ok(response) => {
        // Handle successful response
    }
    Err(e) => {
        println!("Error: {}", e);
        // The library provides detailed error messages
    }
}
```

## Tips

1. **Model Names**: Always use the format `provider/model-name`
2. **API Keys**: Set environment variables before running examples
3. **Streaming**: Use `completion_stream()` for real-time responses
4. **Error Messages**: The library provides detailed error messages for debugging
5. **Rate Limits**: Be aware of provider-specific rate limits
6. **Costs**: Monitor usage as API calls incur costs

## Contributing

To add a new example:

1. Create a new file in the `examples/` directory
2. Follow the naming convention: `provider_feature.rs`
3. Include environment variable checks
4. Add clear comments and output formatting
5. Update this README with the new example

## License

These examples are part of the LiteLLM Rust project and are licensed under the MIT License.