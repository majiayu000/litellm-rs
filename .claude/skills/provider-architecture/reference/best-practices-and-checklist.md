## Contents

- Best Practices
- Checklist for New Providers
- Configuration
- Provider Implementation
- Model Information
- Quality
- Registration

## Best Practices

### 1. Error Factory Methods

Always use factory methods for consistent error creation:

```rust
// Good
ProviderError::authentication(PROVIDER_NAME, "Invalid API key")
ProviderError::rate_limit(PROVIDER_NAME, Some(60))
ProviderError::model_not_found(PROVIDER_NAME, &model)

// Bad
ProviderError::Authentication {
    provider: PROVIDER_NAME,
    message: "Invalid API key".to_string()
}
```

### 2. No Unwrap

Never use `.unwrap()` in provider code:

```rust
// Good
let api_key = self.config.get_api_key()
    .ok_or_else(|| ProviderError::authentication(PROVIDER_NAME, "API key required"))?;

// Bad
let api_key = self.config.get_api_key().unwrap();
```

### 3. Provider Name Constant

Use a constant for provider name:

```rust
const PROVIDER_NAME: &str = "my_provider";

// Used in all error creation
ProviderError::network(PROVIDER_NAME, msg)
```

### 4. Request Transformation

Transform OpenAI-compatible requests to provider-specific format:

```rust
async fn transform_request(
    &self,
    mut request: ChatRequest,
    _context: RequestContext,
) -> Result<Value, Self::Error> {
    // Provider-specific transformations
    if let Some(ref mut tool_choice) = request.tool_choice {
        // Map "required" to "any" for this provider
        if let ToolChoice::String(s) = tool_choice {
            if s == "required" {
                *s = "any".to_string();
            }
        }
    }

    serde_json::to_value(&request)
        .map_err(|e| ProviderError::serialization(PROVIDER_NAME, e.to_string()))
}
```

---

## Checklist for New Providers

```markdown
## Configuration
- [ ] Implement ProviderConfig trait
- [ ] Include validate() method
- [ ] Support environment variables
- [ ] Set reasonable defaults

## Provider Implementation
- [ ] Use ProviderError (unified error type)
- [ ] Implement all required LLMProvider methods
- [ ] Map HTTP errors correctly
- [ ] Support streaming (if applicable)

## Model Information
- [ ] Define supported models
- [ ] Include pricing information
- [ ] Specify capabilities correctly

## Quality
- [ ] No unwrap() calls
- [ ] Clear error messages
- [ ] All errors include provider name
- [ ] Unit tests for error mapping

## Registration
- [ ] Add to providers/mod.rs
- [ ] Add to provider registry
```

---

