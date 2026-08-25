use super::ProviderError;

/// Optional HTTP headers carried by canonical provider error mapping.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderHttpErrorHeaders {
    pub retry_after: Option<u64>,
    pub rpm_limit: Option<u32>,
    pub tpm_limit: Option<u32>,
}

/// Feature-neutral provider HTTP facts shared by core and gateway adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderHttpErrorFacts {
    pub status: u16,
    pub gateway_code: &'static str,
    pub openai_error_type: &'static str,
    pub openai_code: &'static str,
    pub headers: ProviderHttpErrorHeaders,
}

impl ProviderHttpErrorFacts {
    const fn new(
        status: u16,
        gateway_code: &'static str,
        openai_error_type: &'static str,
        openai_code: &'static str,
    ) -> Self {
        Self {
            status,
            gateway_code,
            openai_error_type,
            openai_code,
            headers: ProviderHttpErrorHeaders {
                retry_after: None,
                rpm_limit: None,
                tpm_limit: None,
            },
        }
    }

    const fn with_headers(mut self, headers: ProviderHttpErrorHeaders) -> Self {
        self.headers = headers;
        self
    }
}

/// Canonical `ProviderError` to HTTP status/code facts.
pub fn provider_http_error_facts(error: &ProviderError) -> ProviderHttpErrorFacts {
    match error {
        ProviderError::Authentication { .. } => facts(
            401,
            "PROVIDER_AUTH_ERROR",
            "authentication_error",
            "authentication_error",
        ),
        ProviderError::RateLimit {
            retry_after,
            rpm_limit,
            tpm_limit,
            ..
        } => facts(
            429,
            "PROVIDER_RATE_LIMIT",
            "rate_limit_error",
            "rate_limit_exceeded",
        )
        .with_headers(ProviderHttpErrorHeaders {
            retry_after: *retry_after,
            rpm_limit: *rpm_limit,
            tpm_limit: *tpm_limit,
        }),
        ProviderError::QuotaExceeded { .. } => facts(
            402,
            "PROVIDER_QUOTA_EXCEEDED",
            "insufficient_quota",
            "insufficient_quota",
        ),
        ProviderError::ModelNotFound { .. } => facts(
            404,
            "MODEL_NOT_FOUND",
            "invalid_request_error",
            "model_not_found",
        ),
        ProviderError::InvalidRequest { message, .. } if is_model_not_priced_message(message) => {
            facts(
                400,
                "INVALID_REQUEST",
                "invalid_request_error",
                "model_not_priced",
            )
        }
        ProviderError::InvalidRequest { .. } => facts(
            400,
            "INVALID_REQUEST",
            "invalid_request_error",
            "invalid_request",
        ),
        ProviderError::Network { .. } => facts(
            502,
            "PROVIDER_NETWORK_ERROR",
            "server_error",
            "provider_network_error",
        ),
        ProviderError::ProviderUnavailable { .. } => facts(
            503,
            "PROVIDER_UNAVAILABLE",
            "server_error",
            "provider_unavailable",
        ),
        ProviderError::NotSupported { .. }
        | ProviderError::NotImplemented { .. }
        | ProviderError::FeatureDisabled { .. } => facts(
            501,
            "PROVIDER_NOT_IMPLEMENTED",
            "invalid_request_error",
            "not_supported",
        ),
        ProviderError::Configuration { .. }
        | ProviderError::Serialization { .. }
        | ProviderError::TransformationError { .. } => facts(
            500,
            "PROVIDER_INTERNAL_ERROR",
            "server_error",
            "internal_error",
        ),
        ProviderError::Timeout { .. } => facts(504, "PROVIDER_TIMEOUT", "server_error", "timeout"),
        ProviderError::ContextLengthExceeded { .. } => facts(
            400,
            "PROVIDER_REQUEST_ERROR",
            "invalid_request_error",
            "context_length_exceeded",
        ),
        ProviderError::ContentFiltered { .. } => facts(
            400,
            "PROVIDER_REQUEST_ERROR",
            "invalid_request_error",
            "content_filter",
        ),
        ProviderError::ApiError { status, .. } => provider_api_error_facts(*status),
        ProviderError::TokenLimitExceeded { .. } => facts(
            400,
            "PROVIDER_REQUEST_ERROR",
            "invalid_request_error",
            "token_limit_exceeded",
        ),
        ProviderError::DeploymentError { .. } => facts(
            404,
            "DEPLOYMENT_NOT_FOUND",
            "invalid_request_error",
            "deployment_not_found",
        ),
        ProviderError::ResponseParsing { .. } | ProviderError::Streaming { .. } => facts(
            502,
            "PROVIDER_RESPONSE_ERROR",
            "server_error",
            "provider_response_error",
        ),
        ProviderError::RoutingError { .. } => facts(
            503,
            "PROVIDER_ROUTING_ERROR",
            "server_error",
            "provider_routing_error",
        ),
        ProviderError::Cancelled { .. } => {
            facts(499, "PROVIDER_CANCELLED", "server_error", "cancelled")
        }
        ProviderError::Other { .. } => {
            facts(502, "PROVIDER_ERROR", "server_error", "provider_error")
        }
    }
}

/// Default HTTP status-code → `ProviderError` mapping shared by most providers.
///
/// Providers with custom handling (e.g. Gemini, Databricks, Anthropic) should
/// implement `map_http_error` manually and call this for the status codes they
/// don't need to override.
pub fn default_http_error_mapper(
    provider: &'static str,
    status_code: u16,
    response_body: &str,
) -> ProviderError {
    match status_code {
        400 => {
            let message = parse_error_message_from_body(response_body)
                .unwrap_or_else(|| response_body.to_string());
            ProviderError::invalid_request(provider, message)
        }
        401 => ProviderError::authentication(provider, "Invalid API key"),
        // 403 means the credential was accepted but lacks permission; keep the
        // upstream status so clients see 403 permission_error, not 401.
        403 => {
            let message = parse_error_message_from_body(response_body)
                .unwrap_or_else(|| response_body.to_string());
            ProviderError::api_error(provider, status_code, message)
        }
        404 => ProviderError::model_not_found(provider, "Model not found"),
        429 => {
            let retry_after =
                crate::core::providers::shared::parse_retry_after_from_body(response_body);
            ProviderError::rate_limit(provider, retry_after)
        }
        500..=599 => ProviderError::api_error(provider, status_code, response_body),
        _ => ProviderError::api_error(provider, status_code, response_body),
    }
}

/// Try to extract an error message from a JSON response body.
///
/// Checks `error.message` and top-level `message` fields.
pub fn parse_error_message_from_body(response_body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(response_body).ok()?;
    json.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .or_else(|| json.get("message").and_then(|m| m.as_str()))
        .map(|s| s.to_string())
}

/// Extended HTTP status-code → `ProviderError` mapping with additional codes.
///
/// Handles 402 (quota), 413 (context length), 408/504 (timeout), 502/503
/// (provider unavailable) in addition to the standard codes.
pub fn extended_http_error_mapper(
    provider: &'static str,
    status_code: u16,
    response_body: &str,
) -> ProviderError {
    match status_code {
        400 => ProviderError::invalid_request(provider, response_body),
        401 => ProviderError::authentication(provider, response_body),
        // Preserve upstream 403 as a permission failure, not an auth failure.
        403 => ProviderError::api_error(provider, status_code, response_body),
        402 => ProviderError::quota_exceeded(provider, response_body),
        404 => ProviderError::model_not_found(provider, response_body),
        408 | 504 => ProviderError::timeout(provider, response_body),
        413 => ProviderError::context_length_exceeded(provider, 0, 0),
        429 => ProviderError::rate_limit(provider, None),
        500 => ProviderError::api_error(provider, status_code, response_body),
        502 | 503 => ProviderError::provider_unavailable(provider, response_body),
        _ => ProviderError::api_error(provider, status_code, response_body),
    }
}

fn provider_api_error_facts(status: u16) -> ProviderHttpErrorFacts {
    let status = valid_status_or_bad_gateway(status);
    match status {
        400 => facts(
            status,
            "PROVIDER_API_ERROR",
            "invalid_request_error",
            "invalid_request",
        ),
        401 => facts(
            status,
            "PROVIDER_API_ERROR",
            "authentication_error",
            "authentication_error",
        ),
        403 => facts(
            status,
            "PROVIDER_API_ERROR",
            "permission_error",
            "permission_denied",
        ),
        404 => facts(
            status,
            "PROVIDER_API_ERROR",
            "invalid_request_error",
            "not_found",
        ),
        408 => facts(status, "PROVIDER_API_ERROR", "server_error", "timeout"),
        409 => facts(
            status,
            "PROVIDER_API_ERROR",
            "invalid_request_error",
            "conflict",
        ),
        429 => facts(
            status,
            "PROVIDER_API_ERROR",
            "rate_limit_error",
            "rate_limit_exceeded",
        ),
        500..=599 => facts(
            status,
            "PROVIDER_API_ERROR",
            "server_error",
            "provider_api_error",
        ),
        _ => facts(
            status,
            "PROVIDER_API_ERROR",
            "server_error",
            "provider_api_error",
        ),
    }
}

const fn facts(
    status: u16,
    gateway_code: &'static str,
    openai_error_type: &'static str,
    openai_code: &'static str,
) -> ProviderHttpErrorFacts {
    ProviderHttpErrorFacts::new(status, gateway_code, openai_error_type, openai_code)
}

const fn valid_status_or_bad_gateway(status: u16) -> u16 {
    match status {
        400..=599 => status,
        _ => 502,
    }
}

fn is_model_not_priced_message(message: &str) -> bool {
    message.starts_with("model_not_priced:")
}

#[cfg(test)]
mod http_403_classification_tests {
    use super::*;

    #[test]
    fn default_mapper_keeps_upstream_403_as_permission_error() {
        let error = default_http_error_mapper("openai", 403, "insufficient permissions");
        assert!(matches!(error, ProviderError::ApiError { status: 403, .. }));

        let facts = provider_http_error_facts(&error);
        assert_eq!(facts.status, 403);
        assert_eq!(facts.openai_error_type, "permission_error");
        assert_eq!(facts.openai_code, "permission_denied");
    }

    #[test]
    fn extended_mapper_splits_401_from_403() {
        let auth = extended_http_error_mapper("openai", 401, "bad key");
        assert!(matches!(auth, ProviderError::Authentication { .. }));

        let permission = extended_http_error_mapper("openai", 403, "forbidden region");
        assert!(matches!(
            permission,
            ProviderError::ApiError { status: 403, .. }
        ));
    }

    #[test]
    fn quota_shaped_403_bodies_keep_authoritative_transport_status() {
        let permission = crate::core::providers::base::HttpErrorMapper::map_status_code(
            "openai",
            403,
            "billing: insufficient_quota",
        );
        assert!(matches!(
            permission,
            ProviderError::ApiError { status: 403, .. }
        ));
    }
}
