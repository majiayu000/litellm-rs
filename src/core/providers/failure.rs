//! Fact-only provider failure view.
//!
//! `ProviderError` remains the compatibility error type used by provider
//! implementations. This module exposes a smaller view for retry policy and
//! gateway adapters so those layers can reason from facts.

use super::ProviderError;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderFailureKind {
    Authentication,
    RateLimit,
    QuotaExceeded,
    ModelNotFound,
    InvalidRequest,
    Network,
    ProviderUnavailable,
    NotSupported,
    NotImplemented,
    Configuration,
    Serialization,
    Timeout,
    ContextLengthExceeded,
    ContentFiltered,
    ApiError,
    TokenLimitExceeded,
    FeatureDisabled,
    DeploymentError,
    ResponseParsing,
    RoutingError,
    TransformationError,
    Cancelled,
    Streaming,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProviderRetryHint {
    pub retry_after: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderFailureFacts {
    pub provider: &'static str,
    pub kind: ProviderFailureKind,
    pub upstream_status: Option<u16>,
    pub explicitly_retryable: bool,
    pub retry_hint: ProviderRetryHint,
}

impl ProviderFailureFacts {
    pub fn from_error(error: &ProviderError) -> Self {
        Self::from(error)
    }
}

impl From<&ProviderError> for ProviderFailureFacts {
    fn from(error: &ProviderError) -> Self {
        Self {
            provider: error.provider(),
            kind: ProviderFailureKind::from(error),
            upstream_status: match error {
                ProviderError::ApiError { status, .. } => Some(*status),
                _ => None,
            },
            explicitly_retryable: error.is_explicitly_retryable_api_error(),
            retry_hint: match error {
                ProviderError::RateLimit { retry_after, .. } => ProviderRetryHint {
                    retry_after: retry_after.map(Duration::from_secs),
                },
                _ => ProviderRetryHint::default(),
            },
        }
    }
}

impl From<&ProviderError> for ProviderFailureKind {
    fn from(error: &ProviderError) -> Self {
        match error {
            ProviderError::Authentication { .. } => Self::Authentication,
            ProviderError::RateLimit { .. } => Self::RateLimit,
            ProviderError::QuotaExceeded { .. } => Self::QuotaExceeded,
            ProviderError::ModelNotFound { .. } => Self::ModelNotFound,
            ProviderError::InvalidRequest { .. } => Self::InvalidRequest,
            ProviderError::Network { .. } => Self::Network,
            ProviderError::ProviderUnavailable { .. } => Self::ProviderUnavailable,
            ProviderError::NotSupported { .. } => Self::NotSupported,
            ProviderError::NotImplemented { .. } => Self::NotImplemented,
            ProviderError::Configuration { .. } => Self::Configuration,
            ProviderError::Serialization { .. } => Self::Serialization,
            ProviderError::Timeout { .. } => Self::Timeout,
            ProviderError::ContextLengthExceeded { .. } => Self::ContextLengthExceeded,
            ProviderError::ContentFiltered { .. } => Self::ContentFiltered,
            ProviderError::ApiError { .. } => Self::ApiError,
            ProviderError::TokenLimitExceeded { .. } => Self::TokenLimitExceeded,
            ProviderError::FeatureDisabled { .. } => Self::FeatureDisabled,
            ProviderError::DeploymentError { .. } => Self::DeploymentError,
            ProviderError::ResponseParsing { .. } => Self::ResponseParsing,
            ProviderError::RoutingError { .. } => Self::RoutingError,
            ProviderError::TransformationError { .. } => Self::TransformationError,
            ProviderError::Cancelled { .. } => Self::Cancelled,
            ProviderError::Streaming { .. } => Self::Streaming,
            ProviderError::Other { .. } => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::bedrock::BedrockErrorMapper;

    #[test]
    fn facts_capture_rate_limit_retry_after_without_policy() {
        let facts = ProviderFailureFacts::from_error(&ProviderError::rate_limit("openai", Some(7)));

        assert_eq!(facts.provider, "openai");
        assert_eq!(facts.kind, ProviderFailureKind::RateLimit);
        assert_eq!(facts.upstream_status, None);
        assert_eq!(facts.retry_hint.retry_after, Some(Duration::from_secs(7)));
    }

    #[test]
    fn facts_capture_upstream_api_status() {
        let facts = ProviderFailureFacts::from_error(&ProviderError::api_error(
            "anthropic",
            503,
            "upstream overloaded",
        ));

        assert_eq!(facts.provider, "anthropic");
        assert_eq!(facts.kind, ProviderFailureKind::ApiError);
        assert_eq!(facts.upstream_status, Some(503));
        assert!(!facts.explicitly_retryable);
        assert_eq!(facts.retry_hint.retry_after, None);
    }

    #[test]
    fn facts_preserve_only_modeled_bedrock_retry_signals() {
        let modeled_error =
            BedrockErrorMapper::map_service_error("ModelNotReadyException", "model not ready")
                .expect("modeled Bedrock service error");
        let modeled = ProviderFailureFacts::from_error(&modeled_error);
        let ordinary = ProviderFailureFacts::from_error(&ProviderError::api_error(
            "bedrock",
            424,
            "ModelNotReadyException: misleading ordinary HTTP message",
        ));

        assert!(modeled.explicitly_retryable);
        assert!(!ordinary.explicitly_retryable);
    }
}
