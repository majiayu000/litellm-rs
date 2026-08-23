## Error Context Preservation

### Wrapping Provider Errors with Context

The gateway wraps `ProviderError` in `ContextualError`
(`src/core/providers/contextual_error.rs`) to attach request ID, model, and
timestamp for debugging and logging:

```rust
use crate::core::providers::ContextualError;

let contextual = ContextualError::new(provider_error, request_id, model.as_deref());
// Display: "[request_id={}] {inner} (model: {model})"
```

`ContextualError` implements `std::error::Error::source()`, returning the
wrapped `ProviderError`, so the chain stays inspectable.

### Preserving Source Chains

`GatewayError` uses thiserror `#[from]` conversions for `reqwest::Error`,
`std::io::Error`, `serde_json::Error`, and others, so `?` keeps the original
error as the `source()`:

```rust
async fn process_request(request: ChatRequest) -> LiteLLMResult<ChatResponse> {
    let raw = std::fs::read_to_string("state.json")?; // Io -> GatewayError
    // ...
    Ok(response)
}
```

At call sites without a `From` conversion, attach context with `map_err`
while keeping the original message:

```rust
provider.chat_completion(request, context)
    .await
    .map_err(|e| GatewayError::Provider(e))?
```

### Inspecting an Error Chain

```rust
let mut current: Option<&dyn std::error::Error> = Some(&err);
while let Some(e) = current {
    eprintln!("caused by: {}", e);
    current = e.source();
}
```
