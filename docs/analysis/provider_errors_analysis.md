# Provider Error Handling Patterns Analysis

**Date**: 2026-01-15
**Analyzed by**: Codex & Claude Code
**Scope**: `src/core/providers/*/error_mapper.rs` and `src/core/providers/*/provider.rs`
**Status**: Historical scan snapshot. Provider lists in this document are not
the current source of truth after GH837 provider-disposition cleanup.

## Executive Summary

This document analyzes error handling patterns across 114 provider implementations in litellm-rs. The analysis identified **7 critical categories** of issues affecting error handling consistency, maintainability, and correctness.

### Key Statistics

- **Total Providers**: 114
- **Providers with error_mapper.rs**: 12 (10.5%)
- **Providers without error_mapper.rs**: 102 (89.5%)
- **Unique Error Patterns**: 3 main patterns identified
- **Issues Found**: 18 specific issues across 5 severity levels

---

## Issue Categories

### 1. Missing Error Mapper Files (CRITICAL)

**Severity**: High
**Impact**: 102 providers

**Description**: 89.5% of providers lack dedicated `error_mapper.rs` files, relying on inline error handling in `provider.rs`. This creates inconsistency and makes error handling patterns hard to maintain.

**Affected Providers** (partial list):
- ai21, amazon_nova, anthropic, azure, azure_ai
- baichuan, baseten, bedrock, cerebras, clarifai
- cloudflare, codestral, cohere, dashscope, databricks
- datarobot, deepgram, deepinfra, deepseek, docker_model_runner
- elevenlabs, empower, exa_ai, fal_ai, featherless
- firecrawl, fireworks, friendliai, galadriel, gigachat
- And 77 more...

**Recommendation**: Create error_mapper.rs files for all providers or document why inline error handling is preferred.

---

### 2. Inconsistent Error Mapping in error_mapper.rs (HIGH)

**Severity**: High
**Impact**: 12 providers with error_mapper.rs

#### Issue 2.1: OpenAI Uses Non-Standard Error Type

**File**: `src/core/providers/openai/error_mapper.rs`
**Lines**: 5-30

**Problem**:
```rust
// OpenAI uses OpenAIError instead of ProviderError
impl ErrorMapper<OpenAIError> for OpenAIErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> OpenAIError {
        match status_code {
            401 => OpenAIError::Authentication {
                provider: "openai",
                message: "Invalid API key".to_string(), // IGNORES response_body
            },
            429 => OpenAIError::rate_limit_simple("openai", "Rate limit exceeded"),
            400 => OpenAIError::InvalidRequest {
                provider: "openai",
                message: response_body.to_string(),
            },
            _ => OpenAIError::Other { // Missing 404, 402, 403, 5xx cases
                provider: "openai",
                message: format!("HTTP {}: {}", status_code, response_body),
            },
        }
    }
}
```

**Issues**:
1. Uses `OpenAIError` type alias instead of `ProviderError` directly (though they're equivalent via `pub use`)
2. 401 error **ignores** `response_body` - hardcodes message as "Invalid API key"
3. Missing explicit cases for 404 (model not found), 402 (quota), 403 (forbidden), 5xx errors
4. Comment says "Simple error mapping - in real implementation would parse OpenAI error format" - suggests incomplete implementation

**Impact**: Error context loss, inconsistent error categorization

---

#### Issue 2.2: Missing HTTP Status Code Coverage

**Severity**: Medium
**Files**: All error_mapper.rs files

**Standard Pattern**:
```rust
match status_code {
    401 => ProviderError::authentication(provider, response_body),
    429 => ProviderError::rate_limit(provider, None),
    404 => ProviderError::model_not_found(provider, response_body),
    400 => ProviderError::invalid_request(provider, response_body),
    _ => ProviderError::api_error(provider, status_code, response_body),
}
```

**Missing Cases Across Providers**:

| HTTP Status | Meaning | Handled By | Missing From |
|-------------|---------|------------|--------------|
| 402 | Payment Required (Quota) | deepl (456 custom), openai | aiml, anyscale, bytez, custom_api, yi, maritalk, siliconflow, comet_api, aleph_alpha |
| 403 | Forbidden | deepl (combined with 401) | All others except deepl |
| 413 | Payload Too Large | None | All providers |
| 500 | Internal Server Error | None (falls to default) | All providers |
| 502 | Bad Gateway | None (falls to default) | All providers |
| 503 | Service Unavailable | None (falls to default) | All providers |

**Recommendation**:
- Add explicit handling for 403 (authentication/authorization)
- Add 502/503 mapping to `ProviderError::provider_unavailable`
- Add 500 mapping to `ProviderError::api_error` with retryable flag
- Add 413 mapping to `ProviderError::context_length_exceeded` or `token_limit_exceeded`

---

#### Issue 2.3: DeepL Non-Standard Status Code (456)

**File**: `src/core/providers/deepl/error_mapper.rs`
**Line**: 15

```rust
456 => ProviderError::quota_exceeded("deepl", "Quota exceeded"),
```

**Problem**: DeepL uses custom HTTP status code 456 for quota exceeded. This is documented behavior but inconsistent with standard HTTP codes (should be 402 or 429).

**Impact**: Works correctly but may confuse developers expecting standard HTTP status codes.

**Recommendation**: Add comment explaining DeepL-specific status code.

---

### 3. Duplicate Error Handling in provider.rs (HIGH)

**Severity**: High
**Impact**: Providers with error_mapper.rs

**File**: `src/core/providers/aiml_api/provider.rs`
**Lines**: 172-179

**Problem**:
```rust
// In chat_completion method
if !response.status().is_success() {
    let status = response.status().as_u16();
    let error_text = response.text().await.unwrap_or_default();

    return Err(match status {
        401 => ProviderError::authentication("aiml", error_text),
        429 => ProviderError::rate_limit("aiml", None),
        404 => ProviderError::model_not_found("aiml", request.model),
        _ => ProviderError::api_error("aiml", status, error_text),
    });
}
```

**Issue**: This code **duplicates** the error mapping logic from `AimlErrorMapper::map_http_error`. The error mapper is defined but **not used** in the actual request handling.

**Expected Pattern**:
```rust
if !response.status().is_success() {
    let status = response.status().as_u16();
    let error_text = response.text().await.unwrap_or_default();
    let error_mapper = self.get_error_mapper();
    return Err(error_mapper.map_http_error(status, &error_text));
}
```

**Impact**:
- Error mapping logic exists in two places
- Changes to error_mapper.rs don't affect actual behavior
- Dead code (error_mapper.rs is unused)

**Recommendation**: Refactor to use `get_error_mapper()` method consistently across all providers.

---

### 4. Inconsistent Provider Name Usage (MEDIUM)

**Severity**: Medium
**Impact**: Multiple providers

#### Issue 4.1: Hardcoded vs Constant Provider Names

**Examples**:

| Provider | error_mapper.rs | PROVIDER_NAME constant |
|----------|----------------|----------------------|
| aiml_api | "aiml" | "aiml" |
| anyscale | "anyscale" | "anyscale" |
| custom_api | "custom_httpx" | "custom_httpx" |
| bytez | "bytez" | "bytez" |
| comet_api | "cometapi" | Unknown |

**Problem**: Provider names are hardcoded strings in error mappers instead of using constants from `mod.rs`. This creates risk of typos and inconsistency.

**File**: `src/core/providers/comet_api/error_mapper.rs`
**Line**: 11

```rust
401 => ProviderError::authentication("cometapi", response_body),
```

**Note**: "cometapi" is one word here, but might be "comet_api" elsewhere.

**Recommendation**:
```rust
// In error_mapper.rs
use super::PROVIDER_NAME;

impl ErrorMapper<ProviderError> for ErrorMapperImpl {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        match status_code {
            401 => ProviderError::authentication(PROVIDER_NAME, response_body),
            // ...
        }
    }
}
```

---

### 5. Missing Error Context Preservation (MEDIUM)

**Severity**: Medium
**Impact**: All error_mapper.rs files

#### Issue 5.1: Response Body Not Parsed for Structured Errors

**Problem**: Most error mappers pass `response_body` as-is without attempting to parse structured error responses from providers.

**Current**:
```rust
401 => ProviderError::authentication("provider", response_body),
```

**Better**:
```rust
401 => {
    // Try to parse structured error response
    if let Ok(error_json) = serde_json::from_str::<Value>(response_body) {
        let message = error_json["error"]["message"]
            .as_str()
            .unwrap_or(response_body);
        ProviderError::authentication("provider", message)
    } else {
        ProviderError::authentication("provider", response_body)
    }
}
```

**Example from OpenAI API Error Response**:
```json
{
  "error": {
    "message": "Invalid API key provided",
    "type": "invalid_request_error",
    "param": null,
    "code": "invalid_api_key"
  }
}
```

**Recommendation**: Add structured error parsing for providers with known error response formats (OpenAI, Anthropic, Azure).

---

#### Issue 5.2: Rate Limit Headers Not Extracted

**Problem**: Rate limit error mapping doesn't extract `retry-after`, `x-ratelimit-*` headers.

**Current**:
```rust
429 => ProviderError::rate_limit("provider", None), // Always None
```

**Better**:
```rust
429 => {
    let retry_after = response.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    ProviderError::rate_limit_with_retry("provider", "Rate limit exceeded", retry_after)
}
```

**Impact**: Retry logic cannot use provider-specified retry delays, leading to suboptimal backoff strategies.

**Recommendation**: Update error mappers to accept HTTP headers and extract rate limit information.

---

### 6. Dead Code and Unused Error Variants (LOW)

**Severity**: Low
**Impact**: Error mappers

#### Issue 6.1: Unreachable Error Variants

**File**: `src/core/providers/unified_provider.rs`
**Lines**: Various

**Unused Variants** (based on error_mapper.rs analysis):

| Variant | Used in error_mapper.rs? | Created via Factory Method? |
|---------|-------------------------|----------------------------|
| `ContextLengthExceeded` | No | Yes (via provider.rs) |
| `ContentFiltered` | No | Yes (via provider.rs) |
| `TokenLimitExceeded` | No | Yes (via provider.rs) |
| `FeatureDisabled` | No | Yes (via provider.rs) |
| `DeploymentError` | No | Yes (Azure provider) |
| `ResponseParsing` | No | Yes (via provider.rs) |
| `RoutingError` | No | Yes (router module) |
| `TransformationError` | No | Yes (via provider.rs) |
| `Cancelled` | No | Yes (async operations) |
| `Streaming` | No | Yes (streaming modules) |

**Analysis**: These variants are **not dead code** - they're created directly in provider.rs files and other modules. However, they're **never created by error_mapper.rs** files.

**Recommendation**: Document which error variants are for HTTP error mapping vs internal provider logic.

---

### 7. Missing Error Mapper Trait Consistency (MEDIUM)

**Severity**: Medium
**Impact**: Error mapper implementations

#### Issue 7.1: Inconsistent Trait Implementations

**Problem**: Error mappers have inconsistent struct names and implementations.

**Patterns Found**:

| Provider | Struct Name | Pattern |
|----------|-------------|---------|
| openai | `OpenAIErrorMapper` | Provider-specific name |
| aiml_api | `AimlErrorMapper` | Provider-specific name |
| anyscale | `AnyscaleErrorMapper` | Provider-specific name |
| bytez | `ErrorMapperImpl` | Generic name |
| custom_api | `ErrorMapperImpl` | Generic name |
| comet_api | `ErrorMapperImpl` | Generic name |

**Issue**: Providers using `ErrorMapperImpl` make code search and debugging harder.

**Recommendation**: Standardize on `{Provider}ErrorMapper` naming pattern.

---

## Comparison with Unified ProviderError Design

### ProviderError Capabilities

The `src/core/providers/unified_provider.rs` defines **20+ error variants** with rich context:

```rust
pub enum ProviderError {
    Authentication { provider, message },
    RateLimit { provider, message, retry_after, rpm_limit, tpm_limit, current_usage },
    QuotaExceeded { provider, message },
    ModelNotFound { provider, model },
    InvalidRequest { provider, message },
    Network { provider, message },
    ProviderUnavailable { provider, message },
    NotSupported { provider, feature },
    NotImplemented { provider, feature },
    Configuration { provider, message },
    Serialization { provider, message },
    Timeout { provider, message },
    ContextLengthExceeded { provider, max, actual },
    ContentFiltered { provider, reason, policy_violations, potentially_retryable },
    ApiError { provider, status, message },
    TokenLimitExceeded { provider, message },
    FeatureDisabled { provider, feature },
    DeploymentError { provider, deployment, message },
    ResponseParsing { provider, message },
    RoutingError { provider, attempted_providers, message },
    TransformationError { provider, from_format, to_format, message },
    Cancelled { provider, operation_type, cancellation_reason },
    Streaming { provider, stream_type, position, last_chunk, message },
    Other { provider, message },
}
```

### Error Mapper Under-Utilization

**Problem**: Error mappers only use **4-5 variants** out of 20+ available:
- `Authentication`
- `RateLimit` (without enhanced fields)
- `ModelNotFound`
- `InvalidRequest`
- `ApiError` (as fallback)

**Missing Opportunities**:
- `QuotaExceeded` (402) - Only deepl maps 456
- `ProviderUnavailable` (503)
- `Timeout` (408, 504)
- `ContextLengthExceeded` (413)

---

## Recommended Fixes

### Priority 1: High Severity (Immediate Action)

1. **Fix OpenAI Error Mapper** (`src/core/providers/openai/error_mapper.rs`)
   - Use response_body in 401 errors
   - Add cases for 404, 402, 403, 5xx
   - Add comment about error format parsing

2. **Remove Duplicate Error Handling** (`src/core/providers/aiml_api/provider.rs` and similar)
   - Replace inline error matching with `get_error_mapper()` calls
   - Verify error_mapper.rs is actually used

3. **Add Missing HTTP Status Codes** (All error_mapper.rs files)
   - 403 → `authentication` or new `forbidden` variant
   - 502/503 → `provider_unavailable`
   - 500 → `api_error` (already handled by default, but explicit is better)
   - 413 → `context_length_exceeded` or `token_limit_exceeded`

### Priority 2: Medium Severity (Next Sprint)

4. **Standardize Provider Names**
   - Use `PROVIDER_NAME` constant in error mappers
   - Audit all providers for name consistency

5. **Add Error Context Parsing**
   - Parse JSON error responses where provider format is known
   - Extract structured error information

6. **Extract Rate Limit Headers**
   - Update `map_http_error` signature to accept headers
   - Extract `retry-after`, `x-ratelimit-*` values

7. **Standardize Error Mapper Names**
   - Rename `ErrorMapperImpl` → `{Provider}ErrorMapper`

### Priority 3: Low Severity (Technical Debt)

8. **Document Error Variant Usage**
   - Add documentation to unified_provider.rs explaining which variants are for HTTP mapping vs internal use

9. **Create Error Mappers for Remaining Providers**
   - Evaluate whether 102 providers without error_mapper.rs need them
   - If inline handling is preferred, document the decision

---

## Testing Recommendations

### Unit Tests for Error Mappers

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_401_authentication_error() {
        let mapper = AimlErrorMapper;
        let error = mapper.map_http_error(401, "Invalid API key");

        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "aiml");
                assert_eq!(message, "Invalid API key");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_429_rate_limit() {
        let mapper = AimlErrorMapper;
        let error = mapper.map_http_error(429, "Rate limit exceeded");

        assert!(crate::utils::error::CanonicalError::canonical_retryable(&error));
        assert_eq!(error.http_status(), 429);
    }

    #[test]
    fn test_404_model_not_found() {
        let mapper = AimlErrorMapper;
        let error = mapper.map_http_error(404, "Model not found: gpt-4");

        match error {
            ProviderError::ModelNotFound { provider, model } => {
                assert_eq!(provider, "aiml");
                assert!(model.contains("not found"));
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_5xx_server_errors() {
        let mapper = AimlErrorMapper;

        for status in [500, 502, 503, 504] {
            let error = mapper.map_http_error(status, "Server error");
            assert!(crate::utils::error::CanonicalError::canonical_retryable(&error), "Status {} should be retryable", status);
        }
    }
}
```

### Integration Tests

Create tests in `tests/provider_errors_test.rs`:

```rust
#[tokio::test]
async fn test_error_mapper_used_in_requests() {
    // Verify that error mappers are actually called during failed requests
    // Use mock HTTP responses with various status codes
}

#[tokio::test]
async fn test_consistent_error_handling_across_providers() {
    // Verify all providers handle standard HTTP errors consistently
}
```

---

## Summary of Issues

| Category | Severity | Count | Providers Affected |
|----------|----------|-------|-------------------|
| Missing error_mapper.rs | High | 102 | 89.5% of providers |
| Duplicate error handling | High | ~12 | Providers with error_mapper.rs |
| Missing HTTP status codes | Medium | 12 | All error_mapper.rs files |
| Inconsistent provider names | Medium | 3 | bytez, custom_api, comet_api |
| No structured error parsing | Medium | 12 | All error_mapper.rs files |
| Missing rate limit headers | Medium | 12 | All error_mapper.rs files |
| Inconsistent mapper names | Medium | 3 | Providers with ErrorMapperImpl |
| Dead code concerns | Low | 0 | None (variants used elsewhere) |

**Total Issues**: 18 across 7 categories
**Lines of Code Affected**: ~200-300 lines across 12 error_mapper.rs files + 102 provider.rs files

---

## Appendix A: Complete Error Mapper File List

```
src/core/providers/aiml_api/error_mapper.rs
src/core/providers/aleph_alpha/error_mapper.rs
src/core/providers/anyscale/error_mapper.rs
src/core/providers/bytez/error_mapper.rs
src/core/providers/comet_api/error_mapper.rs
src/core/providers/compactifai/error_mapper.rs
src/core/providers/custom_api/error_mapper.rs
src/core/providers/deepl/error_mapper.rs
src/core/providers/maritalk/error_mapper.rs
src/core/providers/openai/error_mapper.rs
src/core/providers/siliconflow/error_mapper.rs
src/core/providers/yi/error_mapper.rs
```

---

## Appendix B: Historical Providers Without error_mapper.rs

Historical total on 2026-01-15: 102 providers

```
ai21, amazon_nova, anthropic, azure, azure_ai, baichuan, base, baseten,
bedrock, cerebras, clarifai, cloudflare, codestral, cohere, dashscope,
databricks, datarobot, deepgram, deepinfra, deepseek, docker_model_runner,
elevenlabs, empower, exa_ai, fal_ai, featherless, firecrawl, fireworks,
friendliai, galadriel, gigachat, github, google, google_pse, gooseai,
gradio, groq, huggingface, ibm_watsonx, infinity, lambda_ai, lemonade,
linkup, llamacpp, manus, meta_llama, mistral, monsterapi, morph,
nlp_cloud, nvidia_nim, octoai, ollama, oobabooga, openrouter, ovhcloud,
palm, perplexity, petals, predibase, ragflow, recraft, replicate,
runpod, sagemaker, sambanova, sap_ai, searxng, tavily, text_completion_openai,
together_ai, topaz, triton, vercel_ai, vertex_ai, volcengine, voyage_ai,
vllm, watsonx, xiaomi_mimo, xinference, zhipu
... and more
```

---

## Conclusion

The provider error handling system has a solid foundation with `ProviderError` enum, but error mappers are under-utilized and inconsistently applied. The main issues are:

1. Only 10.5% of providers use error_mapper.rs
2. Error mappers don't use the full capability of ProviderError variants
3. Duplicate error handling logic between error_mapper.rs and provider.rs
4. Missing important HTTP status code mappings (403, 502, 503, 413)

Addressing the **Priority 1** issues will significantly improve error handling consistency and debuggability across the codebase.
