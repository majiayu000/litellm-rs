## Contents

- Factory Methods

## Factory Methods

Constructors live in the `impl ProviderError` block in
`src/core/providers/unified_provider_methods.rs`.

### Basic Factory Methods

```rust
impl ProviderError {
    pub fn authentication(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Authentication {
            provider,
            message: message.into(),
        }
    }

    pub fn rate_limit(provider: &'static str, retry_after: Option<u64>) -> Self {
        Self::RateLimit {
            provider,
            message: match retry_after {
                Some(seconds) => format!("Rate limit exceeded. Retry after {} seconds", seconds),
                None => "Rate limit exceeded".to_string(),
            },
            retry_after,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        }
    }

    pub fn model_not_found(provider: &'static str, model: impl Into<String>) -> Self {
        Self::ModelNotFound {
            provider,
            model: model.into(),
        }
    }

    pub fn invalid_request(provider: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            provider,
            message: message.into(),
        }
    }

    pub fn network(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Network {
            provider,
            message: message.into(),
        }
    }

    pub fn timeout(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Timeout {
            provider,
            message: message.into(),
        }
    }

    pub fn provider_unavailable(provider: &'static str, message: impl Into<String>) -> Self {
        Self::ProviderUnavailable {
            provider,
            message: message.into(),
        }
    }

    pub fn not_supported(provider: &'static str, feature: impl Into<String>) -> Self {
        Self::NotSupported {
            provider,
            feature: feature.into(),
        }
    }

    pub fn configuration(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Configuration {
            provider,
            message: message.into(),
        }
    }

    pub fn serialization(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Serialization {
            provider,
            message: message.into(),
        }
    }

    pub fn api_error(provider: &'static str, status: u16, message: impl Into<String>) -> Self {
        Self::ApiError {
            provider,
            status,
            message: message.into(),
        }
    }

    pub fn response_parsing(provider: &'static str, message: impl Into<String>) -> Self {
        Self::ResponseParsing {
            provider,
            message: message.into(),
        }
    }
}
```

### Enhanced Factory Methods

```rust
impl ProviderError {
    pub fn rate_limit_with_limits(
        provider: &'static str,
        retry_after: Option<u64>,
        rpm_limit: Option<u32>,
        tpm_limit: Option<u32>,
        current_usage: Option<f64>,
    ) -> Self {
        let message = match (rpm_limit, tpm_limit) {
            (Some(rpm), Some(tpm)) => {
                format!("Rate limit exceeded: {}RPM, {}TPM limits reached", rpm, tpm)
            }
            (Some(rpm), None) => format!("Rate limit exceeded: {}RPM limit reached", rpm),
            (None, Some(tpm)) => format!("Rate limit exceeded: {}TPM limit reached", tpm),
            (None, None) => "Rate limit exceeded".to_string(),
        };

        Self::RateLimit {
            provider,
            message,
            retry_after,
            rpm_limit,
            tpm_limit,
            current_usage,
        }
    }

    pub fn context_length_exceeded(
        provider: &'static str,
        max: usize,
        actual: usize,
    ) -> Self {
        Self::ContextLengthExceeded {
            provider,
            max,
            actual,
        }
    }

    pub fn content_filtered(
        provider: &'static str,
        reason: impl Into<String>,
        policy_violations: Option<Vec<String>>,
        potentially_retryable: Option<bool>,
    ) -> Self {
        Self::ContentFiltered {
            provider,
            reason: reason.into(),
            policy_violations,
            potentially_retryable,
        }
    }

    pub fn streaming_error(
        provider: &'static str,
        stream_type: impl Into<String>,
        position: Option<u64>,
        last_chunk: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Streaming {
            provider,
            stream_type: stream_type.into(),
            position,
            last_chunk,
            message: message.into(),
        }
    }

    pub fn routing_error(
        provider: &'static str,
        attempted_providers: Vec<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::RoutingError {
            provider,
            attempted_providers,
            message: message.into(),
        }
    }
}
```

Other constructors in the same impl block: `quota_exceeded`,
`rate_limit_simple`, `rate_limit_with_retry`, `not_implemented`,
`initialization`, `token_limit_exceeded`, `feature_disabled`,
`deployment_error` (hardcodes provider `"azure"`), `transformation_error`,
`cancelled`, and `other`.
