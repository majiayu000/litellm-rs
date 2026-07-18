//! Fact-only provider failure view.
//!
//! `ProviderError` remains the compatibility error type used by provider
//! implementations. This module exposes a smaller view for retry policy and
//! gateway adapters so those layers can reason from facts.

use super::ProviderError;
use super::unified_provider::{ProviderHttpErrorFacts, provider_http_error_facts};
use crate::utils::error::{CanonicalError, ErrorCode};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProviderRetryHint {
    pub retry_after: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderFailureFacts {
    pub provider: &'static str,
    pub canonical_code: ErrorCode,
    pub http: ProviderHttpErrorFacts,
    pub upstream_status: Option<u16>,
    pub retry_hint: ProviderRetryHint,
    pub content_filter_retryable: bool,
    pub(crate) retry_candidate: bool,
    pub pre_output_only: bool,
    pub streaming_failure: bool,
    pub cancelled: bool,
    pub modeled_bedrock_retry: bool,
    pub(super) legacy_retryable: bool,
    pub(super) legacy_retry_delay: Option<Duration>,
}

impl ProviderFailureFacts {
    pub fn from_error(error: &ProviderError) -> Self {
        Self::from(error)
    }
}

impl From<&ProviderError> for ProviderFailureFacts {
    fn from(error: &ProviderError) -> Self {
        let mut facts = Self {
            provider: error.provider(),
            canonical_code: CanonicalError::canonical_code(error),
            http: provider_http_error_facts(error),
            upstream_status: None,
            retry_hint: ProviderRetryHint::default(),
            content_filter_retryable: false,
            retry_candidate: false,
            pre_output_only: false,
            streaming_failure: false,
            cancelled: false,
            modeled_bedrock_retry: false,
            legacy_retryable: false,
            legacy_retry_delay: None,
        };

        match error {
            ProviderError::RateLimit { retry_after, .. } => {
                facts.retry_candidate = true;
                facts.legacy_retryable = true;
                facts.retry_hint.retry_after = retry_after.map(Duration::from_secs);
                facts.legacy_retry_delay = facts.retry_hint.retry_after;
            }
            ProviderError::Network { .. } | ProviderError::Timeout { .. } => {
                facts.retry_candidate = true;
                facts.legacy_retryable = true;
                facts.legacy_retry_delay = Some(Duration::from_secs(1));
            }
            ProviderError::ProviderUnavailable { .. } => {
                facts.retry_candidate = true;
                facts.legacy_retryable = true;
                facts.legacy_retry_delay = Some(Duration::from_secs(5));
            }
            ProviderError::ContentFiltered {
                potentially_retryable,
                ..
            } => {
                facts.content_filter_retryable = potentially_retryable.unwrap_or(false);
                facts.retry_candidate = facts.content_filter_retryable;
                facts.pre_output_only = true;
                facts.legacy_retryable = facts.content_filter_retryable;
                facts.legacy_retry_delay = facts
                    .content_filter_retryable
                    .then_some(Duration::from_secs(10));
            }
            ProviderError::ApiError { status, .. } => {
                facts.upstream_status = Some(*status);
                facts.modeled_bedrock_retry = error.is_bedrock_modeled_retry_error();
                facts.retry_candidate =
                    facts.modeled_bedrock_retry || matches!(*status, 408 | 429 | 500..=599);
                facts.legacy_retryable =
                    facts.modeled_bedrock_retry || *status == 429 || (500..=599).contains(status);
                facts.legacy_retry_delay = match *status {
                    424 if facts.modeled_bedrock_retry => Some(Duration::from_secs(3)),
                    429 => Some(Duration::from_secs(60)),
                    500..=599 => Some(Duration::from_secs(3)),
                    _ => None,
                };
            }
            ProviderError::DeploymentError { .. } => {
                facts.retry_candidate = true;
                facts.pre_output_only = true;
                facts.legacy_retryable = true;
                facts.legacy_retry_delay = Some(Duration::from_secs(5));
            }
            ProviderError::Cancelled { .. } => facts.cancelled = true,
            ProviderError::Streaming { .. } => {
                facts.retry_candidate = true;
                facts.pre_output_only = true;
                facts.streaming_failure = true;
                facts.legacy_retryable = true;
                facts.legacy_retry_delay = Some(Duration::from_secs(2));
            }
            ProviderError::Authentication { .. }
            | ProviderError::QuotaExceeded { .. }
            | ProviderError::ModelNotFound { .. }
            | ProviderError::InvalidRequest { .. }
            | ProviderError::NotSupported { .. }
            | ProviderError::NotImplemented { .. }
            | ProviderError::Configuration { .. }
            | ProviderError::Serialization { .. }
            | ProviderError::ContextLengthExceeded { .. }
            | ProviderError::TokenLimitExceeded { .. }
            | ProviderError::FeatureDisabled { .. }
            | ProviderError::ResponseParsing { .. }
            | ProviderError::RoutingError { .. }
            | ProviderError::TransformationError { .. }
            | ProviderError::Other { .. } => {}
        }

        facts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::bedrock::BedrockErrorMapper;
    use crate::core::router::RouterConfig;
    use crate::core::router::retry_policy::{RetryContext, RetryPolicy};

    #[test]
    fn facts_capture_rate_limit_retry_after_without_policy() {
        let facts = ProviderFailureFacts::from_error(&ProviderError::rate_limit("openai", Some(7)));

        assert_eq!(facts.provider, "openai");
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
        assert_eq!(facts.upstream_status, Some(503));
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

        assert_eq!(modeled.upstream_status, Some(424));
        assert!(modeled.modeled_bedrock_retry);
        assert_eq!(ordinary.upstream_status, Some(424));
        assert!(!ordinary.modeled_bedrock_retry);
    }

    #[test]
    fn variant_table_locks_canonical_http_and_stream_retry_facts() {
        macro_rules! case {
            ($error:expr, $code:ident, $status:expr, $retry:expr) => {{
                let error = $error;
                let facts = ProviderFailureFacts::from_error(&error);
                let before = RetryPolicy.decide(
                    &RouterConfig::default(),
                    &error,
                    RetryContext::stream_pre_output(1, 2),
                );
                let after = RetryPolicy.decide(
                    &RouterConfig::default(),
                    &error,
                    RetryContext::stream_after_chunks(1, 2),
                );
                assert_eq!(
                    (facts.canonical_code, facts.http.status, before.should_retry),
                    (ErrorCode::$code, $status, $retry)
                );
                assert!(!after.should_retry);
            }};
        }
        let (p, m) = ("p", "m");
        case!(
            ProviderError::authentication(p, m),
            Authentication,
            401,
            false
        );
        case!(ProviderError::rate_limit(p, None), RateLimited, 429, true);
        case!(
            ProviderError::quota_exceeded(p, m),
            QuotaExceeded,
            402,
            false
        );
        case!(ProviderError::model_not_found(p, m), NotFound, 404, false);
        case!(
            ProviderError::invalid_request(p, m),
            InvalidRequest,
            400,
            false
        );
        case!(ProviderError::network(p, m), Network, 502, true);
        case!(
            ProviderError::provider_unavailable(p, m),
            Unavailable,
            503,
            true
        );
        case!(
            ProviderError::not_supported(p, m),
            NotImplemented,
            501,
            false
        );
        case!(
            ProviderError::not_implemented(p, m),
            NotImplemented,
            501,
            false
        );
        case!(
            ProviderError::configuration(p, m),
            Configuration,
            500,
            false
        );
        case!(ProviderError::serialization(p, m), Parsing, 500, false);
        case!(ProviderError::timeout(p, m), Timeout, 504, true);
        case!(
            ProviderError::context_length_exceeded(p, 1, 2),
            InvalidRequest,
            400,
            false
        );
        case!(
            ProviderError::content_filtered(p, m, None, Some(true)),
            InvalidRequest,
            400,
            true
        );
        let api_408 = ProviderError::api_error(p, 408, m);
        assert!(!api_408.is_retryable());
        assert_eq!(api_408.retry_delay(), None);
        case!(api_408, Timeout, 408, true);
        case!(ProviderError::api_error(p, 429, m), RateLimited, 429, true);
        case!(ProviderError::api_error(p, 200, m), Internal, 502, false);
        case!(ProviderError::api_error(p, 302, m), Internal, 502, false);
        case!(ProviderError::api_error(p, 700, m), Internal, 502, false);
        case!(
            ProviderError::token_limit_exceeded(p, m),
            InvalidRequest,
            400,
            false
        );
        case!(
            ProviderError::feature_disabled(p, m),
            NotImplemented,
            501,
            false
        );
        case!(ProviderError::deployment_error(m, m), NotFound, 404, true);
        case!(ProviderError::response_parsing(p, m), Parsing, 502, false);
        case!(
            ProviderError::routing_error(p, Vec::new(), m),
            Unavailable,
            503,
            false
        );
        case!(
            ProviderError::transformation_error(p, m, m, m),
            Parsing,
            500,
            false
        );
        case!(
            ProviderError::cancelled(p, m, None),
            InvalidRequest,
            499,
            false
        );
        case!(
            ProviderError::streaming_error(p, m, None, None, m),
            Internal,
            502,
            true
        );
        case!(ProviderError::other(p, m), Internal, 502, false);
    }
}
