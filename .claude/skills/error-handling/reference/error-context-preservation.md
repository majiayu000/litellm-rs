## Error Context Preservation

### Wrapping Provider Errors with Context

`ContextualError` (`src/core/providers/contextual_error.rs`) is an opt-in
library wrapper that can attach request ID, model, and timestamp to a
`ProviderError` for debugging and logging. Current production request paths do
not automatically wrap provider failures in it; callers must invoke
`ProviderError::with_context` or `ContextualError::new` explicitly:

```rust
use crate::core::providers::ContextualError;

let contextual = ContextualError::new(provider_error, request_id, model.as_deref());
// Display: "[request_id={}] {inner} (model: {model})"
```

`ContextualError` implements `std::error::Error::source()`, returning the
wrapped `ProviderError`, so the chain stays inspectable.

### Preserving Source Chains

`GatewayError::HttpClient(reqwest::Error)` and
`GatewayError::Io(std::io::Error)` are payload variants with thiserror
`#[from]`; those retain the original error as `source()`. In contrast, the
manual `From<serde_json::Error>` and `From<serde_yml::Error>` implementations
convert the error to a `String` inside `GatewayError::Serialization`, so `?`
keeps the message but not the original serde error in the source chain:

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
