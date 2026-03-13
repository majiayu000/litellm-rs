//! Canonical cross-protocol error classification.
//!
//! This provides a single error code taxonomy and retryable semantics that can
//! be reused by HTTP/OpenAI-compatible, A2A, and MCP layers.

use super::gateway_error::GatewayError;
use crate::core::a2a::error::A2AError;
use crate::core::mcp::error::McpError;
use crate::core::providers::unified_provider::ProviderError;

/// Canonical error code shared across protocol boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Authentication,
    Authorization,
    RateLimited,
    QuotaExceeded,
    InvalidRequest,
    NotFound,
    Conflict,
    Timeout,
    Unavailable,
    Network,
    Configuration,
    Parsing,
    NotImplemented,
    Internal,
}

impl ErrorCode {
    /// Stable machine-readable canonical string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authentication => "AUTHENTICATION",
            Self::Authorization => "AUTHORIZATION",
            Self::RateLimited => "RATE_LIMITED",
            Self::QuotaExceeded => "QUOTA_EXCEEDED",
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::Timeout => "TIMEOUT",
            Self::Unavailable => "UNAVAILABLE",
            Self::Network => "NETWORK",
            Self::Configuration => "CONFIGURATION",
            Self::Parsing => "PARSING",
            Self::NotImplemented => "NOT_IMPLEMENTED",
            Self::Internal => "INTERNAL",
        }
    }

    /// Default retryability for canonical classes.
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::RateLimited | Self::Timeout | Self::Unavailable | Self::Network
        )
    }
}

/// Canonical code and retryability mapping.
pub trait CanonicalError {
    fn canonical_code(&self) -> ErrorCode;

    fn canonical_retryable(&self) -> bool {
        self.canonical_code().is_retryable()
    }
}

impl CanonicalError for ProviderError {
    fn canonical_code(&self) -> ErrorCode {
        match self {
            ProviderError::Authentication { .. } => ErrorCode::Authentication,
            ProviderError::RateLimit { .. } => ErrorCode::RateLimited,
            ProviderError::QuotaExceeded { .. } => ErrorCode::QuotaExceeded,
            ProviderError::ModelNotFound { .. } | ProviderError::DeploymentError { .. } => {
                ErrorCode::NotFound
            }
            ProviderError::InvalidRequest { .. }
            | ProviderError::ContextLengthExceeded { .. }
            | ProviderError::ContentFiltered { .. }
            | ProviderError::TokenLimitExceeded { .. }
            | ProviderError::FeatureDisabled { .. }
            | ProviderError::Cancelled { .. } => ErrorCode::InvalidRequest,
            ProviderError::Network { .. } => ErrorCode::Network,
            ProviderError::ProviderUnavailable { .. } | ProviderError::RoutingError { .. } => {
                ErrorCode::Unavailable
            }
            ProviderError::NotSupported { .. } | ProviderError::NotImplemented { .. } => {
                ErrorCode::NotImplemented
            }
            ProviderError::Configuration { .. } => ErrorCode::Configuration,
            ProviderError::Serialization { .. }
            | ProviderError::ResponseParsing { .. }
            | ProviderError::TransformationError { .. } => ErrorCode::Parsing,
            ProviderError::Timeout { .. } => ErrorCode::Timeout,
            ProviderError::ApiError { status, .. } => match *status {
                401 => ErrorCode::Authentication,
                403 => ErrorCode::Authorization,
                404 => ErrorCode::NotFound,
                408 | 504 => ErrorCode::Timeout,
                409 => ErrorCode::Conflict,
                429 => ErrorCode::RateLimited,
                400..=499 => ErrorCode::InvalidRequest,
                500..=599 => ErrorCode::Unavailable,
                _ => ErrorCode::Internal,
            },
            ProviderError::Streaming { .. } | ProviderError::Other { .. } => ErrorCode::Internal,
        }
    }

    fn canonical_retryable(&self) -> bool {
        self.is_retryable()
    }
}

impl CanonicalError for GatewayError {
    fn canonical_code(&self) -> ErrorCode {
        match self {
            GatewayError::Config(_) => ErrorCode::Configuration,
            GatewayError::Auth(_) => ErrorCode::Authentication,
            GatewayError::Forbidden(_) => ErrorCode::Authorization,
            GatewayError::Provider(provider_error) => provider_error.canonical_code(),
            GatewayError::RateLimit { .. } => ErrorCode::RateLimited,
            GatewayError::Validation(_)
            | GatewayError::BadRequest(_)
            | GatewayError::NoProvidersForModel(_) => ErrorCode::InvalidRequest,
            GatewayError::NotFound(_) | GatewayError::ProviderNotFound(_) => ErrorCode::NotFound,
            GatewayError::Conflict(_) => ErrorCode::Conflict,
            GatewayError::Timeout(_) => ErrorCode::Timeout,
            GatewayError::ProviderUnavailable(_)
            | GatewayError::NoProvidersAvailable(_)
            | GatewayError::NoHealthyProviders(_)
            | GatewayError::CircuitBreaker(_) => ErrorCode::Unavailable,
            GatewayError::Network(_) | GatewayError::External(_) => ErrorCode::Network,
            GatewayError::Parsing(_) => ErrorCode::Parsing,
            GatewayError::NotImplemented(_) => ErrorCode::NotImplemented,
            GatewayError::Database(_)
            | GatewayError::Redis(_)
            | GatewayError::HttpClient(_)
            | GatewayError::Serialization(_)
            | GatewayError::Yaml(_)
            | GatewayError::Io(_)
            | GatewayError::Internal(_)
            | GatewayError::Jwt(_)
            | GatewayError::Crypto(_)
            | GatewayError::FileStorage(_)
            | GatewayError::VectorDb(_)
            | GatewayError::Alert(_)
            | GatewayError::Cache(_) => ErrorCode::Internal,
            #[cfg(feature = "s3")]
            GatewayError::S3(_) => ErrorCode::Internal,
            #[cfg(feature = "vector-db")]
            GatewayError::Qdrant(_) => ErrorCode::Internal,
            #[cfg(feature = "websockets")]
            GatewayError::WebSocket(_) => ErrorCode::Network,
        }
    }

    fn canonical_retryable(&self) -> bool {
        match self {
            GatewayError::Provider(provider_error) => provider_error.canonical_retryable(),
            _ => self.canonical_code().is_retryable(),
        }
    }
}

impl CanonicalError for A2AError {
    fn canonical_code(&self) -> ErrorCode {
        match self {
            A2AError::AgentNotFound { .. } | A2AError::TaskNotFound { .. } => ErrorCode::NotFound,
            A2AError::AgentAlreadyExists { .. } => ErrorCode::Conflict,
            A2AError::ConnectionError { .. } => ErrorCode::Network,
            A2AError::AuthenticationError { .. } => ErrorCode::Authentication,
            A2AError::ProtocolError { .. }
            | A2AError::InvalidRequest { .. }
            | A2AError::ContentBlocked { .. } => ErrorCode::InvalidRequest,
            A2AError::Timeout { .. } => ErrorCode::Timeout,
            A2AError::ConfigurationError { .. } => ErrorCode::Configuration,
            A2AError::SerializationError { .. } => ErrorCode::Parsing,
            A2AError::UnsupportedProvider { .. } => ErrorCode::NotImplemented,
            A2AError::RateLimitExceeded { .. } => ErrorCode::RateLimited,
            A2AError::AgentBusy { .. } => ErrorCode::Unavailable,
            A2AError::TaskFailed { .. } => ErrorCode::Internal,
        }
    }

    fn canonical_retryable(&self) -> bool {
        matches!(
            self,
            A2AError::ConnectionError { .. }
                | A2AError::Timeout { .. }
                | A2AError::RateLimitExceeded { .. }
                | A2AError::AgentBusy { .. }
        )
    }
}

impl CanonicalError for McpError {
    fn canonical_code(&self) -> ErrorCode {
        match self {
            McpError::ServerNotFound { .. } | McpError::ToolNotFound { .. } => ErrorCode::NotFound,
            McpError::ConnectionError { .. } | McpError::TransportError { .. } => {
                ErrorCode::Network
            }
            McpError::AuthenticationError { .. } => ErrorCode::Authentication,
            McpError::AuthorizationError { .. } => ErrorCode::Authorization,
            McpError::ProtocolError { .. } | McpError::InvalidUrl { .. } => {
                ErrorCode::InvalidRequest
            }
            McpError::ToolExecutionError { .. } => ErrorCode::Internal,
            McpError::Timeout { .. } => ErrorCode::Timeout,
            McpError::ConfigurationError { .. } => ErrorCode::Configuration,
            McpError::SerializationError { .. } => ErrorCode::Parsing,
            McpError::ServerAlreadyExists { .. } => ErrorCode::Conflict,
            McpError::RateLimitExceeded { .. } => ErrorCode::RateLimited,
        }
    }

    fn canonical_retryable(&self) -> bool {
        matches!(
            self,
            McpError::ConnectionError { .. }
                | McpError::TransportError { .. }
                | McpError::Timeout { .. }
                | McpError::RateLimitExceeded { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_rate_limit_mapping() {
        let err = ProviderError::rate_limit("openai", Some(10));
        assert_eq!(err.canonical_code(), ErrorCode::RateLimited);
        assert!(err.canonical_retryable());
    }

    #[test]
    fn test_provider_auth_mapping() {
        let err = ProviderError::authentication("openai", "bad key");
        assert_eq!(err.canonical_code(), ErrorCode::Authentication);
        assert!(!err.canonical_retryable());
    }

    #[test]
    fn test_gateway_provider_delegates_retryable() {
        let err = GatewayError::Provider(ProviderError::timeout("openai", "timeout"));
        assert_eq!(err.canonical_code(), ErrorCode::Timeout);
        assert!(err.canonical_retryable());
    }

    #[test]
    fn test_gateway_not_found_mapping() {
        let err = GatewayError::NotFound("missing".to_string());
        assert_eq!(err.canonical_code(), ErrorCode::NotFound);
        assert!(!err.canonical_retryable());
    }

    #[cfg(feature = "s3")]
    #[test]
    fn test_gateway_s3_mapping() {
        let err = GatewayError::S3("bucket error".to_string());
        assert_eq!(err.canonical_code(), ErrorCode::Internal);
        assert!(!err.canonical_retryable());
    }

    #[cfg(feature = "vector-db")]
    #[test]
    fn test_gateway_qdrant_mapping() {
        let err = GatewayError::Qdrant("connection failed".to_string());
        assert_eq!(err.canonical_code(), ErrorCode::Internal);
        assert!(!err.canonical_retryable());
    }

    #[cfg(feature = "websockets")]
    #[test]
    fn test_gateway_websocket_mapping() {
        let err = GatewayError::WebSocket("connection closed".to_string());
        assert_eq!(err.canonical_code(), ErrorCode::Network);
        assert!(err.canonical_retryable());
    }

    #[test]
    fn test_a2a_busy_mapping() {
        let err = A2AError::AgentBusy {
            agent_name: "agent-1".to_string(),
            message: "overloaded".to_string(),
        };
        assert_eq!(err.canonical_code(), ErrorCode::Unavailable);
        assert!(err.canonical_retryable());
    }

    #[test]
    fn test_mcp_auth_mapping() {
        let err = McpError::AuthenticationError {
            server_name: "s1".to_string(),
            message: "bad token".to_string(),
        };
        assert_eq!(err.canonical_code(), ErrorCode::Authentication);
        assert!(!err.canonical_retryable());
    }

    #[test]
    fn test_error_code_str_values() {
        assert_eq!(ErrorCode::Authentication.as_str(), "AUTHENTICATION");
        assert_eq!(ErrorCode::RateLimited.as_str(), "RATE_LIMITED");
    }
}
