## Error Context Preservation

### Adding Context to Errors

```rust
// Using the anyhow pattern for context
use anyhow::Context;

async fn process_request(request: ChatRequest) -> Result<ChatResponse, LiteLLMError> {
    let provider = select_provider(&request.model)
        .context("Failed to select provider")?;

    let response = provider
        .chat_completion(request, context)
        .await
        .context("Chat completion failed")?;

    Ok(response)
}
```

### Error Chain Display

```rust
impl ProviderError {
    pub fn display_chain(&self) -> String {
        let mut chain = vec![self.to_string()];

        // Add any source errors
        if let Some(source) = std::error::Error::source(self) {
            chain.push(format!("Caused by: {}", source));
        }

        chain.join("\n")
    }
}
```
