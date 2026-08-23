## Contents

- Best Practices
- Checklist for New Providers

## Best Practices

### 1. Error Factory Methods

Always use factory methods for consistent error creation:

```rust
// Good
ProviderError::authentication(PROVIDER_NAME, "Invalid API key")
ProviderError::rate_limit(PROVIDER_NAME, Some(60))
ProviderError::model_not_found(PROVIDER_NAME, &model)

// Bad - direct struct literals skip normalization; some variants carry extra
// optional fields (e.g. RateLimit has retry_after/rpm_limit/tpm_limit/current_usage)
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

Use a constant for the static provider name (it also feeds `ProviderError`
constructors, which take `&'static str`, and `error_provider_name()`):

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
) -> Result<Value, ProviderError> {
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

### Tier 1 (Catalog-Only)

- [ ] Endpoint serves OpenAI-compatible `/chat/completions` with standard SSE `data: [DONE]` streaming
- [ ] One `def_chat(...)` entry in `src/core/providers/registry/catalog.rs` (or `def_local_chat(...)` for keyless local servers)
- [ ] Annotation comment in `src/core/providers/mod.rs`: `// <name>: Tier 1 -> registry/catalog.rs`
- [ ] `auth_env_var` matches the provider's documented env var; add `alternate_auth_env_vars` for known aliases
- [ ] `model_prefix: Some(...)` only when selector-prefix stripping is actually needed
- [ ] Unit test asserting `get_definition("<name>")` resolves with the right base URL and key (see tests in `catalog.rs`)
- [ ] No new module, enum variant, or factory branch

### Tier 2 (Code-Based)

**Configuration**
- [ ] Implement `ProviderConfig` trait (or use `define_provider_config!`)
- [ ] Include `validate()` method
- [ ] Support environment variables
- [ ] Set reasonable defaults

**Provider Implementation**
- [ ] Use `ProviderError` (unified error type) for every trait method
- [ ] Implement all required `LLMProvider` methods (no associated types exist)
- [ ] Map HTTP errors through an `ErrorMapper` (`DefaultErrorMapper` unless the API has bespoke bodies)
- [ ] Support streaming via `chat_completion_stream` gated on `ChatCompletionStream` capability (if applicable)

**Model Information**
- [ ] Define supported models returning the real `ModelInfo` shape (`src/core/types/model.rs`)
- [ ] Include pricing information (per 1K tokens fields)
- [ ] Specify capabilities correctly (static slice of `ProviderCapability`)

**Quality**
- [ ] No unwrap() calls
- [ ] Clear error messages
- [ ] All errors include provider name
- [ ] Unit tests for error mapping

**Registration**
- [ ] `pub mod <name>;` in `src/core/providers/mod.rs` (feature-gated as needed)
- [ ] Variant in the closed `Provider` enum plus dispatch arms (`dispatch_provider!` expansions)
- [ ] Factory builder branch under `src/core/providers/factory/`
