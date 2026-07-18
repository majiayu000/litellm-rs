use super::{ContextualError, ProviderError, ProviderHttpErrorFacts, provider_http_error_facts};
use crate::core::providers::failure::ProviderFailureFacts;
use crate::utils::error::{CanonicalError, ErrorCode};

impl ProviderError {
    fn bedrock_modeled_retry_provider() -> &'static str {
        use std::sync::OnceLock;

        static PROVIDER: OnceLock<Box<str>> = OnceLock::new();
        PROVIDER.get_or_init(|| "bedrock".into()).as_ref()
    }

    /// Create authentication error
    pub fn authentication(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Authentication {
            provider,
            message: message.into(),
        }
    }

    /// Create rate limit error
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

    /// Create enhanced rate limit error with usage details
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

    /// Create quota exceeded error
    pub fn quota_exceeded(provider: &'static str, message: impl Into<String>) -> Self {
        Self::QuotaExceeded {
            provider,
            message: message.into(),
        }
    }

    /// Create simple rate limit error (convenience method)
    pub fn rate_limit_simple(provider: &'static str, message: impl Into<String>) -> Self {
        Self::RateLimit {
            provider,
            message: message.into(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        }
    }

    /// Create rate limit error with retry_after only
    pub fn rate_limit_with_retry(
        provider: &'static str,
        message: impl Into<String>,
        retry_after: Option<u64>,
    ) -> Self {
        Self::RateLimit {
            provider,
            message: message.into(),
            retry_after,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        }
    }

    /// Create model not found error
    pub fn model_not_found(provider: &'static str, model: impl Into<String>) -> Self {
        Self::ModelNotFound {
            provider,
            model: model.into(),
        }
    }

    /// Create invalid request error
    pub fn invalid_request(provider: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            provider,
            message: message.into(),
        }
    }

    /// Create network error
    pub fn network(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Network {
            provider,
            message: message.into(),
        }
    }

    /// Create provider unavailable error
    pub fn provider_unavailable(provider: &'static str, message: impl Into<String>) -> Self {
        Self::ProviderUnavailable {
            provider,
            message: message.into(),
        }
    }

    /// Create not supported error
    pub fn not_supported(provider: &'static str, feature: impl Into<String>) -> Self {
        Self::NotSupported {
            provider,
            feature: feature.into(),
        }
    }

    /// Create not implemented error
    pub fn not_implemented(provider: &'static str, feature: impl Into<String>) -> Self {
        Self::NotImplemented {
            provider,
            feature: feature.into(),
        }
    }

    /// Create configuration error
    pub fn configuration(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Configuration {
            provider,
            message: message.into(),
        }
    }

    /// Create serialization error
    pub fn serialization(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Serialization {
            provider,
            message: message.into(),
        }
    }

    /// Create timeout error
    pub fn timeout(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Timeout {
            provider,
            message: message.into(),
        }
    }

    /// Create initialization error (provider failed to start)
    pub fn initialization(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Network {
            provider,
            message: format!("Initialization failed: {}", message.into()),
        }
    }

    // Enhanced factory methods for new error variants

    /// Create context length exceeded error with structured data
    pub fn context_length_exceeded(provider: &'static str, max: usize, actual: usize) -> Self {
        Self::ContextLengthExceeded {
            provider,
            max,
            actual,
        }
    }

    /// Create API error with status code
    pub fn api_error(provider: &'static str, status: u16, message: impl Into<String>) -> Self {
        // Never propagate the internal Bedrock provenance token through a public constructor.
        let provider = if provider == "bedrock" {
            "bedrock"
        } else {
            provider
        };
        Self::ApiError {
            provider,
            status,
            message: message.into(),
        }
    }

    /// Create a Bedrock 424 whose retry provenance came from a structured AWS error code.
    pub(crate) fn bedrock_modeled_retry_error(message: impl Into<String>) -> Self {
        Self::ApiError {
            provider: Self::bedrock_modeled_retry_provider(),
            status: 424,
            message: message.into(),
        }
    }

    /// Whether this API error carries the internal Bedrock modeled-error identity.
    pub(crate) fn is_bedrock_modeled_retry_error(&self) -> bool {
        matches!(
            self,
            Self::ApiError {
                provider,
                status: 424,
                ..
            } if std::ptr::eq(*provider, Self::bedrock_modeled_retry_provider())
        )
    }

    /// Create token limit exceeded error
    pub fn token_limit_exceeded(provider: &'static str, message: impl Into<String>) -> Self {
        Self::TokenLimitExceeded {
            provider,
            message: message.into(),
        }
    }

    /// Create feature disabled error
    pub fn feature_disabled(provider: &'static str, feature: impl Into<String>) -> Self {
        Self::FeatureDisabled {
            provider,
            feature: feature.into(),
        }
    }

    /// Create Azure deployment error
    pub fn deployment_error(deployment: impl Into<String>, message: impl Into<String>) -> Self {
        Self::DeploymentError {
            provider: "azure",
            deployment: deployment.into(),
            message: message.into(),
        }
    }

    /// Create response parsing error
    pub fn response_parsing(provider: &'static str, message: impl Into<String>) -> Self {
        Self::ResponseParsing {
            provider,
            message: message.into(),
        }
    }

    /// Create routing error
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

    /// Create transformation error
    pub fn transformation_error(
        provider: &'static str,
        from_format: impl Into<String>,
        to_format: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::TransformationError {
            provider,
            from_format: from_format.into(),
            to_format: to_format.into(),
            message: message.into(),
        }
    }

    /// Create content filtered error
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

    /// Create cancellation error
    pub fn cancelled(
        provider: &'static str,
        operation_type: impl Into<String>,
        cancellation_reason: Option<String>,
    ) -> Self {
        Self::Cancelled {
            provider,
            operation_type: operation_type.into(),
            cancellation_reason,
        }
    }

    /// Create streaming error
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

    /// Create other/generic error
    pub fn other(provider: &'static str, message: impl Into<String>) -> Self {
        Self::Other {
            provider,
            message: message.into(),
        }
    }

    /// Get the provider name that caused this error
    pub fn provider(&self) -> &'static str {
        match self {
            Self::Authentication { provider, .. }
            | Self::RateLimit { provider, .. }
            | Self::QuotaExceeded { provider, .. }
            | Self::ModelNotFound { provider, .. }
            | Self::InvalidRequest { provider, .. }
            | Self::Network { provider, .. }
            | Self::ProviderUnavailable { provider, .. }
            | Self::NotSupported { provider, .. }
            | Self::NotImplemented { provider, .. }
            | Self::Configuration { provider, .. }
            | Self::Serialization { provider, .. }
            | Self::Timeout { provider, .. }
            | Self::ContextLengthExceeded { provider, .. }
            | Self::ContentFiltered { provider, .. }
            | Self::TokenLimitExceeded { provider, .. }
            | Self::FeatureDisabled { provider, .. }
            | Self::DeploymentError { provider, .. }
            | Self::ResponseParsing { provider, .. }
            | Self::RoutingError { provider, .. }
            | Self::TransformationError { provider, .. }
            | Self::Cancelled { provider, .. }
            | Self::Streaming { provider, .. }
            | Self::Other { provider, .. } => provider,
            Self::ApiError { provider, .. }
                if std::ptr::eq(*provider, Self::bedrock_modeled_retry_provider()) =>
            {
                "bedrock"
            }
            Self::ApiError { provider, .. } => provider,
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        ProviderFailureFacts::from_error(self).legacy_retryable
    }

    /// Get retry delay in seconds
    pub fn retry_delay(&self) -> Option<u64> {
        ProviderFailureFacts::from_error(self)
            .legacy_retry_delay
            .map(|delay| delay.as_secs())
    }

    /// Canonical closed-set classification for protocol adapters.
    pub fn canonical_code(&self) -> ErrorCode {
        CanonicalError::canonical_code(self)
    }

    /// Canonical HTTP facts for protocol adapters.
    pub fn http_facts(&self) -> ProviderHttpErrorFacts {
        provider_http_error_facts(self)
    }

    /// Create an error with request context for better debugging.
    ///
    /// Returns a `ContextualError` that wraps this error with additional request information.
    ///
    /// # Example
    /// ```rust
    /// # use litellm_rs::ProviderError;
    /// let err = ProviderError::network("openai", "Connection refused")
    ///     .with_context("req-123", Some("gpt-4"));
    /// ```
    pub fn with_context(
        self,
        request_id: impl Into<String>,
        model: Option<&str>,
    ) -> ContextualError {
        ContextualError::new(self, request_id, model)
    }

    /// Get HTTP status code for this error
    pub fn http_status(&self) -> u16 {
        super::provider_http_error_facts(self).status
    }
}
