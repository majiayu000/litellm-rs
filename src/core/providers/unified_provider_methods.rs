use super::{ContextualError, ProviderError, ProviderHttpErrorFacts, provider_http_error_facts};
use crate::core::providers::failure::ProviderFailureFacts;
use crate::utils::error::{CanonicalError, ErrorCode};
use regex::Regex;
use std::sync::OnceLock;
use url::Url;

const REDACTED: &str = "[REDACTED]";

fn compiled_regex<'a>(cell: &'a OnceLock<Option<Regex>>, pattern: &str) -> Option<&'a Regex> {
    cell.get_or_init(|| Regex::new(pattern).ok()).as_ref()
}

fn redact_sensitive_text(value: &str) -> String {
    static URL_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
    static HEADER_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
    static AUTH_SCHEME_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
    static SECRET_PAIR_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
    static KNOWN_CREDENTIAL_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
    static JWT_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();

    let Some(url_pattern) = compiled_regex(&URL_PATTERN, r#"(?i)https?://[^\s<>"']+"#) else {
        return REDACTED.to_string();
    };
    let Some(header_pattern) = compiled_regex(
        &HEADER_PATTERN,
        r#"(?im)\b(authorization|proxy-authorization|cookie|set-cookie)["']?\s*[:=]\s*[^\r\n]+"#,
    ) else {
        return REDACTED.to_string();
    };
    let Some(auth_scheme_pattern) = compiled_regex(
        &AUTH_SCHEME_PATTERN,
        r"(?i)\b(bearer|basic)\s+[A-Za-z0-9._~+/=-]+",
    ) else {
        return REDACTED.to_string();
    };
    let Some(secret_pair_pattern) = compiled_regex(
        &SECRET_PAIR_PATTERN,
        r#"(?i)(["']?(?:api[_-]?key|access[_-]?token|refresh[_-]?token|auth[_-]?token|client[_-]?secret|secret[_-]?access[_-]?key|private[_-]?key|credential|password|passwd|token|secret|signature|sig)["']?\s*[:=]\s*)(?:"[^"]*"|'[^']*'|[^\s,;&]+)"#,
    ) else {
        return REDACTED.to_string();
    };
    let Some(known_credential_pattern) = compiled_regex(
        &KNOWN_CREDENTIAL_PATTERN,
        r"\b(?:sk-[A-Za-z0-9_-]{8,}|AIza[A-Za-z0-9_-]{16,}|AKIA[A-Z0-9]{12,}|gh[pousr]_[A-Za-z0-9]{12,}|xox[baprs]-[A-Za-z0-9-]{8,})\b",
    ) else {
        return REDACTED.to_string();
    };
    let Some(jwt_pattern) = compiled_regex(
        &JWT_PATTERN,
        r"\beyJ[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{4,}\b",
    ) else {
        return REDACTED.to_string();
    };

    let value = url_pattern
        .replace_all(value, |captures: &regex::Captures<'_>| {
            redact_url(&captures[0])
        })
        .into_owned();
    let value = header_pattern
        .replace_all(&value, |captures: &regex::Captures<'_>| {
            format!("{}: {REDACTED}", &captures[1])
        })
        .into_owned();
    let value = auth_scheme_pattern
        .replace_all(&value, |captures: &regex::Captures<'_>| {
            format!("{} {REDACTED}", &captures[1])
        })
        .into_owned();
    let value = secret_pair_pattern
        .replace_all(&value, |captures: &regex::Captures<'_>| {
            format!("{}{REDACTED}", &captures[1])
        })
        .into_owned();
    let value = known_credential_pattern
        .replace_all(&value, REDACTED)
        .into_owned();
    jwt_pattern.replace_all(&value, REDACTED).into_owned()
}

fn redact_url(candidate: &str) -> String {
    let core = candidate.trim_end_matches(['.', ',', ';', ')', ']', '}']);
    let suffix = &candidate[core.len()..];
    let Ok(mut url) = Url::parse(core) else {
        return REDACTED.to_string();
    };

    if !url.username().is_empty() && url.set_username(REDACTED).is_err() {
        return REDACTED.to_string();
    }
    if url.password().is_some() && url.set_password(Some(REDACTED)).is_err() {
        return REDACTED.to_string();
    }
    if url.query().is_some() {
        let keys = url
            .query_pairs()
            .map(|(key, _)| key.into_owned())
            .collect::<Vec<_>>();
        url.set_query(None);
        let mut query = url.query_pairs_mut();
        for key in keys {
            query.append_pair(&key, REDACTED);
        }
    }
    if url.fragment().is_some() {
        url.set_fragment(Some(REDACTED));
    }

    format!("{url}{suffix}")
}

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

    /// Return a typed copy safe for logs, protocol adapters, and public errors.
    ///
    /// The original variant and non-secret identity/limit fields are preserved.
    pub fn redacted(&self) -> Self {
        let mut redacted = self.clone();
        match &mut redacted {
            Self::Authentication { message, .. }
            | Self::QuotaExceeded { message, .. }
            | Self::InvalidRequest { message, .. }
            | Self::Network { message, .. }
            | Self::ProviderUnavailable { message, .. }
            | Self::Configuration { message, .. }
            | Self::Serialization { message, .. }
            | Self::Timeout { message, .. }
            | Self::ApiError { message, .. }
            | Self::TokenLimitExceeded { message, .. }
            | Self::ResponseParsing { message, .. }
            | Self::Other { message, .. } => {
                *message = redact_sensitive_text(message);
            }
            Self::RateLimit { message, .. } => {
                *message = redact_sensitive_text(message);
            }
            Self::ContentFiltered {
                reason,
                policy_violations,
                ..
            } => {
                *reason = redact_sensitive_text(reason);
                if let Some(violations) = policy_violations {
                    for violation in violations {
                        *violation = redact_sensitive_text(violation);
                    }
                }
            }
            Self::DeploymentError { message, .. }
            | Self::RoutingError { message, .. }
            | Self::TransformationError { message, .. } => {
                *message = redact_sensitive_text(message);
            }
            Self::Cancelled {
                cancellation_reason,
                ..
            } => {
                if let Some(reason) = cancellation_reason {
                    *reason = redact_sensitive_text(reason);
                }
            }
            Self::Streaming {
                last_chunk,
                message,
                ..
            } => {
                if let Some(chunk) = last_chunk {
                    *chunk = redact_sensitive_text(chunk);
                }
                *message = redact_sensitive_text(message);
            }
            Self::ModelNotFound { model, .. } => *model = redact_sensitive_text(model),
            Self::NotSupported { feature, .. }
            | Self::NotImplemented { feature, .. }
            | Self::FeatureDisabled { feature, .. } => {
                *feature = redact_sensitive_text(feature);
            }
            Self::ContextLengthExceeded { .. } => {}
        }
        redacted
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

#[cfg(test)]
mod redaction_tests {
    use super::*;

    const RAW_KEY: &str = "sk-raw-secret-123456789";
    const RAW_SIGNATURE: &str = "signed-value-123";

    fn assert_secret_absent(error: &ProviderError) {
        let display = error.to_string();
        let debug = format!("{error:?}");
        for raw in [RAW_KEY, RAW_SIGNATURE, "cookie-value", "password-value"] {
            assert!(!display.contains(raw), "Display leaked {raw}: {display}");
            assert!(!debug.contains(raw), "Debug leaked {raw}: {debug}");
        }
        assert!(display.contains(REDACTED) || debug.contains(REDACTED));
    }

    #[test]
    fn redacted_preserves_variant_provider_and_limits() {
        let error = ProviderError::RateLimit {
            provider: "openai",
            message: format!("Authorization: Bearer {RAW_KEY}"),
            retry_after: Some(7),
            rpm_limit: Some(60),
            tpm_limit: Some(1_000),
            current_usage: Some(0.75),
        };

        let redacted = error.redacted();

        assert!(matches!(
            redacted,
            ProviderError::RateLimit {
                provider: "openai",
                retry_after: Some(7),
                rpm_limit: Some(60),
                tpm_limit: Some(1_000),
                current_usage: Some(0.75),
                ..
            }
        ));
        assert_eq!(redacted.canonical_code(), error.canonical_code());
        assert_secret_absent(&redacted);
    }

    #[test]
    fn redacted_covers_url_userinfo_signed_query_and_known_patterns() {
        let error = ProviderError::authentication(
            "openai",
            format!(
                r#"request HTTPS://user:password-value@example.com/v1?X-Amz-Signature={RAW_SIGNATURE} debug={{"Cookie":"session=cookie-value","Authorization":"ApiKey password-value"}}"#
            ),
        );
        let malformed = ProviderError::authentication(
            "openai",
            format!("request https://user:password-value@[::1?X-Amz-Signature={RAW_SIGNATURE}"),
        );

        for redacted in [error.redacted(), malformed.redacted()] {
            assert!(matches!(
                redacted,
                ProviderError::Authentication {
                    provider: "openai",
                    ..
                }
            ));
            assert_secret_absent(&redacted);
        }
    }

    #[test]
    fn redacted_covers_reason_and_optional_body_fields() {
        let content = ProviderError::content_filtered(
            "vertex_ai",
            format!("client_secret={RAW_KEY}"),
            Some(vec![format!("password=password-value; token={RAW_KEY}")]),
            Some(true),
        )
        .redacted();
        let streaming = ProviderError::streaming_error(
            "openai",
            "chat",
            Some(9),
            Some(format!(r#"{{"access_token":"{RAW_KEY}"}}"#)),
            format!("Bearer {RAW_KEY}"),
        )
        .redacted();
        let cancelled =
            ProviderError::cancelled("openai", "chat", Some(format!("signature={RAW_SIGNATURE}")))
                .redacted();

        for error in [&content, &streaming, &cancelled] {
            assert_secret_absent(error);
        }
        assert!(matches!(
            content,
            ProviderError::ContentFiltered {
                provider: "vertex_ai",
                potentially_retryable: Some(true),
                ..
            }
        ));
        assert!(matches!(
            streaming,
            ProviderError::Streaming {
                provider: "openai",
                stream_type,
                position: Some(9),
                ..
            } if stream_type == "chat"
        ));
    }

    #[test]
    fn redacted_preserves_model_and_deployment_identity() {
        let identities = [
            ProviderError::model_not_found(
                "openai",
                format!("gpt-safe https://example.com?signature={RAW_SIGNATURE}"),
            ),
            ProviderError::not_supported("openai", format!("audio-safe Authorization: {RAW_KEY}")),
            ProviderError::not_implemented("openai", "batch-safe Cookie: session=cookie-value"),
            ProviderError::feature_disabled("vertex_ai", "vision-safe password=password-value"),
        ];
        let redacted = identities.map(|error| error.redacted());
        let combined = ProviderError::other("identity", format!("{redacted:?}"));
        let deployment =
            ProviderError::deployment_error("deployment-safe", format!("api_key={RAW_KEY}"))
                .redacted();

        for safe in ["gpt-safe", "audio-safe", "batch-safe", "vision-safe"] {
            assert!(combined.to_string().contains(safe));
        }
        assert!(matches!(
            deployment,
            ProviderError::DeploymentError {
                provider: "azure",
                ref deployment,
                ..
            } if deployment == "deployment-safe"
        ));
        assert_secret_absent(&combined);
        assert_secret_absent(&deployment);
    }
}
