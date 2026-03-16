//! Error Conversions for ProviderError
//!
//! This module contains all `From` implementations for converting various error types
//! into the unified `ProviderError` type.

use super::unified_provider::ProviderError;
use crate::{impl_from_reqwest_error, impl_from_serde_error};

// Convert from common error types
impl_from_reqwest_error!(ProviderError,
    timeout => |e| Self::timeout("unknown", e.to_string()),
    connect => |e| Self::network("unknown", e.to_string()),
    other   => |e| Self::network("unknown", e.to_string())
);

impl_from_serde_error!(ProviderError, |e| Self::serialization(
    "unknown",
    e.to_string()
));

// Note: OpenAIError is now a type alias for ProviderError (see src/core/types/errors/openai.rs),
// so no From conversion is needed — they are the same type.

// Azure provider uses ProviderError directly, no conversion needed

// Add more error type conversions for better interoperability
impl From<Box<dyn std::error::Error + Send + Sync>> for ProviderError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::network("unknown", format!("{}", err))
    }
}

impl From<String> for ProviderError {
    fn from(err: String) -> Self {
        Self::network("unknown", err)
    }
}

// Provider-specific error conversions for unified error handling
// Note: MoonshotError, MistralError, and LlamaError are now type aliases for ProviderError, so no From impl needed

// DeepInfraError is now a type alias for ProviderError, no conversion needed

impl From<crate::core::cost::types::CostError> for ProviderError {
    fn from(err: crate::core::cost::types::CostError) -> Self {
        use crate::core::cost::types::CostError;
        match err {
            CostError::ModelNotSupported { model, provider } => Self::model_not_found(
                "cost",
                format!("Model {} not supported for provider {}", model, provider),
            ),
            CostError::ProviderNotSupported { provider } => Self::not_implemented(
                "cost",
                format!("Provider {} does not support cost calculation", provider),
            ),
            CostError::MissingPricing { model } => {
                Self::invalid_request("cost", format!("Missing pricing for model: {}", model))
            }
            CostError::InvalidUsage { message } => Self::invalid_request("cost", message),
            CostError::CalculationError { message } => Self::api_error("cost", 500, message),
            CostError::ConfigError { message } => Self::invalid_request("cost", message),
        }
    }
}

// VertexAIError is now a type alias for ProviderError, no conversion needed

// V0 provider now uses ProviderError directly - no conversion needed

// DeepSeek now uses ProviderError directly - no conversion needed

// Azure AI provider uses ProviderError directly - no conversion needed

// Anthropic provider now uses ProviderError directly - no conversion needed

// ==================== ProviderErrorTrait Implementation ====================

use crate::core::types::errors::ProviderErrorTrait;

impl ProviderErrorTrait for ProviderError {
    fn error_type(&self) -> &'static str {
        match self {
            Self::Authentication { .. } => "authentication",
            Self::RateLimit { .. } => "rate_limit",
            Self::QuotaExceeded { .. } => "quota_exceeded",
            Self::ModelNotFound { .. } => "model_not_found",
            Self::InvalidRequest { .. } => "invalid_request",
            Self::Network { .. } => "network",
            Self::ProviderUnavailable { .. } => "provider_unavailable",
            Self::NotSupported { .. } => "not_supported",
            Self::NotImplemented { .. } => "not_implemented",
            Self::Configuration { .. } => "configuration",
            Self::Serialization { .. } => "serialization",
            Self::Timeout { .. } => "timeout",

            // Enhanced error variants
            Self::ContextLengthExceeded { .. } => "context_length_exceeded",
            Self::ContentFiltered { .. } => "content_filtered",
            Self::ApiError { .. } => "api_error",
            Self::TokenLimitExceeded { .. } => "token_limit_exceeded",
            Self::FeatureDisabled { .. } => "feature_disabled",
            Self::DeploymentError { .. } => "deployment_error",
            Self::ResponseParsing { .. } => "response_parsing",
            Self::RoutingError { .. } => "routing_error",
            Self::TransformationError { .. } => "transformation_error",
            Self::Cancelled { .. } => "cancelled",
            Self::Streaming { .. } => "streaming",

            Self::Other { .. } => "other",
        }
    }

    fn is_retryable(&self) -> bool {
        // Delegate to the main implementation
        ProviderError::is_retryable(self)
    }

    fn retry_delay(&self) -> Option<u64> {
        // Delegate to the main implementation
        ProviderError::retry_delay(self)
    }

    fn http_status(&self) -> u16 {
        // Delegate to the main implementation
        ProviderError::http_status(self)
    }

    fn not_supported(feature: &str) -> Self {
        Self::NotSupported {
            provider: "unknown",
            feature: feature.to_string(),
        }
    }

    fn authentication_failed(reason: &str) -> Self {
        Self::Authentication {
            provider: "unknown",
            message: reason.to_string(),
        }
    }

    fn rate_limited(retry_after: Option<u64>) -> Self {
        Self::RateLimit {
            provider: "unknown",
            message: "Rate limit exceeded".to_string(),
            retry_after,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        }
    }

    fn network_error(details: &str) -> Self {
        Self::Network {
            provider: "unknown",
            message: details.to_string(),
        }
    }

    fn parsing_error(details: &str) -> Self {
        Self::Serialization {
            provider: "unknown",
            message: details.to_string(),
        }
    }

    fn not_implemented(feature: &str) -> Self {
        Self::NotImplemented {
            provider: "unknown",
            feature: feature.to_string(),
        }
    }
}
