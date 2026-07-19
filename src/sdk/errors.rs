//! Error handling

use thiserror::Error;

use crate::utils::error::ErrorCode;

/// Error
#[derive(Error, Debug)]
pub enum SDKError {
    /// Provider not found
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    /// Default
    #[error("No default provider configured")]
    NoDefaultProvider,

    /// Error
    #[error("Provider error: {0}")]
    #[deprecated(
        since = "0.6.0",
        note = "use the existing typed SDK categories returned by ProviderError conversion"
    )]
    ProviderError(String),

    /// Configuration
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Error
    #[error("Authentication error: {0}")]
    AuthError(String),

    /// Error
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),

    /// Model
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Feature not supported
    #[error("Feature not supported: {0}")]
    NotSupported(String),

    /// Unsupported provider
    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),

    /// Error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Error
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Error
    #[error("API error: {0}")]
    ApiError(String),

    /// Error
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Error
impl From<crate::utils::error::gateway_error::GatewayError> for SDKError {
    fn from(error: crate::utils::error::gateway_error::GatewayError) -> Self {
        match error {
            crate::utils::error::gateway_error::GatewayError::Auth(msg) => SDKError::AuthError(msg),
            crate::utils::error::gateway_error::GatewayError::NotFound(msg) => {
                SDKError::ModelNotFound(msg)
            }
            crate::utils::error::gateway_error::GatewayError::BadRequest(msg) => {
                SDKError::InvalidRequest(msg)
            }
            crate::utils::error::gateway_error::GatewayError::RateLimit { message, .. } => {
                SDKError::RateLimitError(message)
            }
            // SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError
            #[allow(deprecated)]
            crate::utils::error::gateway_error::GatewayError::Unavailable(msg) => {
                SDKError::ProviderError(msg)
            }
            crate::utils::error::gateway_error::GatewayError::Internal(msg) => {
                SDKError::Internal(msg)
            }
            crate::utils::error::gateway_error::GatewayError::Network(msg) => {
                SDKError::NetworkError(msg)
            }
            crate::utils::error::gateway_error::GatewayError::Validation(msg) => {
                SDKError::InvalidRequest(msg)
            }
            // Handle
            _ => SDKError::Internal(error.to_string()),
        }
    }
}

impl From<crate::core::providers::ProviderError> for SDKError {
    fn from(error: crate::core::providers::ProviderError) -> Self {
        let redacted = error.redacted();
        let code = redacted.canonical_code();
        let message = redacted.to_string();

        match code {
            ErrorCode::Authentication | ErrorCode::Authorization => SDKError::AuthError(message),
            ErrorCode::RateLimited | ErrorCode::QuotaExceeded => SDKError::RateLimitError(message),
            ErrorCode::InvalidRequest | ErrorCode::Conflict => SDKError::InvalidRequest(message),
            ErrorCode::NotFound => SDKError::ModelNotFound(message),
            ErrorCode::Timeout | ErrorCode::Network => SDKError::NetworkError(message),
            // SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError
            #[allow(deprecated)]
            ErrorCode::Unavailable => SDKError::ProviderError(message),
            ErrorCode::Configuration => SDKError::ConfigError(message),
            ErrorCode::Parsing => SDKError::ParseError(message),
            ErrorCode::NotImplemented => SDKError::NotSupported(message),
            ErrorCode::Internal => SDKError::Internal(message),
        }
    }
}

/// SDK result type
pub type Result<T> = std::result::Result<T, SDKError>;

impl SDKError {
    /// Error
    // SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError
    #[allow(deprecated)]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            SDKError::NetworkError(_) | SDKError::RateLimitError(_) | SDKError::ProviderError(_)
        )
    }
    /// Error
    pub fn is_auth_error(&self) -> bool {
        matches!(self, SDKError::AuthError(_))
    }
    /// Configuration
    pub fn is_config_error(&self) -> bool {
        matches!(
            self,
            SDKError::ConfigError(_) | SDKError::ProviderNotFound(_) | SDKError::NoDefaultProvider
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::ProviderError;
    use crate::utils::error::gateway_error::GatewayError;
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::{Path, PathBuf},
        process::Command,
    };
    use syn::{
        Expr, GenericArgument, Meta, PathArguments, Type, UseTree,
        ext::IdentExt,
        visit::{self, Visit},
    };

    // SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError
    #[allow(deprecated)]
    fn sdk_variant(error: &SDKError) -> &'static str {
        match error {
            SDKError::ProviderNotFound(_) => "provider_not_found",
            SDKError::NoDefaultProvider => "no_default_provider",
            SDKError::ProviderError(_) => "provider_error",
            SDKError::ConfigError(_) => "config_error",
            SDKError::NetworkError(_) => "network_error",
            SDKError::AuthError(_) => "auth_error",
            SDKError::RateLimitError(_) => "rate_limit_error",
            SDKError::ModelNotFound(_) => "model_not_found",
            SDKError::NotSupported(_) => "not_supported",
            SDKError::UnsupportedProvider(_) => "unsupported_provider",
            SDKError::SerializationError(_) => "serialization_error",
            SDKError::HttpError(_) => "http_error",
            SDKError::InvalidRequest(_) => "invalid_request",
            SDKError::Internal(_) => "internal",
            SDKError::ApiError(_) => "api_error",
            SDKError::ParseError(_) => "parse_error",
        }
    }
    // ==================== SDKError Display Tests ====================

    #[test]
    fn test_sdk_error_provider_not_found() {
        let error = SDKError::ProviderNotFound("openai".to_string());
        assert_eq!(error.to_string(), "Provider not found: openai");
    }
    #[test]
    fn test_sdk_error_no_default_provider() {
        let error = SDKError::NoDefaultProvider;
        assert_eq!(error.to_string(), "No default provider configured");
    }
    // SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError
    #[allow(deprecated)]
    #[test]
    fn test_sdk_error_provider_error() {
        let error = SDKError::ProviderError("API unavailable".to_string());
        assert_eq!(error.to_string(), "Provider error: API unavailable");
    }
    #[test]
    fn test_sdk_error_config_error() {
        let error = SDKError::ConfigError("Missing API key".to_string());
        assert_eq!(error.to_string(), "Configuration error: Missing API key");
    }
    #[test]
    fn test_sdk_error_network_error() {
        let error = SDKError::NetworkError("Connection refused".to_string());
        assert_eq!(error.to_string(), "Network error: Connection refused");
    }
    #[test]
    fn test_sdk_error_auth_error() {
        let error = SDKError::AuthError("Invalid API key".to_string());
        assert_eq!(error.to_string(), "Authentication error: Invalid API key");
    }
    #[test]
    fn test_sdk_error_rate_limit_error() {
        let error = SDKError::RateLimitError("Too many requests".to_string());
        assert_eq!(error.to_string(), "Rate limit exceeded: Too many requests");
    }

    #[test]
    fn test_sdk_error_model_not_found() {
        let error = SDKError::ModelNotFound("gpt-5".to_string());
        assert_eq!(error.to_string(), "Model not found: gpt-5");
    }

    #[test]
    fn test_sdk_error_not_supported() {
        let error = SDKError::NotSupported("streaming".to_string());
        assert_eq!(error.to_string(), "Feature not supported: streaming");
    }

    #[test]
    fn test_sdk_error_unsupported_provider() {
        let error = SDKError::UnsupportedProvider("custom-provider".to_string());
        assert_eq!(error.to_string(), "Unsupported provider: custom-provider");
    }

    #[test]
    fn test_sdk_error_invalid_request() {
        let error = SDKError::InvalidRequest("Missing messages".to_string());
        assert_eq!(error.to_string(), "Invalid request: Missing messages");
    }

    #[test]
    fn test_sdk_error_internal() {
        let error = SDKError::Internal("Unexpected state".to_string());
        assert_eq!(error.to_string(), "Internal error: Unexpected state");
    }

    #[test]
    fn test_sdk_error_api_error() {
        let error = SDKError::ApiError("Server returned 500".to_string());
        assert_eq!(error.to_string(), "API error: Server returned 500");
    }

    #[test]
    fn test_sdk_error_parse_error() {
        let error = SDKError::ParseError("Invalid JSON".to_string());
        assert_eq!(error.to_string(), "Parse error: Invalid JSON");
    }

    #[test]
    fn test_provider_error_auth_maps_to_sdk_auth_error() {
        let error = SDKError::from(ProviderError::authentication("openai", "bad key"));
        assert!(matches!(error, SDKError::AuthError(ref msg) if msg.contains("bad key")));
    }

    #[test]
    fn test_provider_error_rate_limit_maps_to_sdk_rate_limit_error() {
        let error = SDKError::from(ProviderError::rate_limit("openai", Some(30)));
        assert!(matches!(error, SDKError::RateLimitError(ref msg) if !msg.is_empty()));
    }

    #[test]
    fn test_provider_error_model_not_found_maps_to_sdk_model_not_found() {
        let error = SDKError::from(ProviderError::model_not_found("openai", "gpt-missing"));
        assert!(
            matches!(error, SDKError::ModelNotFound(ref message) if message.contains("gpt-missing"))
        );
    }

    #[test]
    fn provider_error_conversion_uses_existing_canonical_categories() {
        let cases = [
            (
                ProviderError::authentication("openai", "bad key"),
                "auth_error",
            ),
            (
                ProviderError::api_error("openai", 403, "forbidden"),
                "auth_error",
            ),
            (
                ProviderError::rate_limit("openai", Some(2)),
                "rate_limit_error",
            ),
            (
                ProviderError::quota_exceeded("openai", "quota"),
                "rate_limit_error",
            ),
            (
                ProviderError::invalid_request("openai", "invalid"),
                "invalid_request",
            ),
            (
                ProviderError::api_error("openai", 409, "conflict"),
                "invalid_request",
            ),
            (
                ProviderError::model_not_found("openai", "missing"),
                "model_not_found",
            ),
            (ProviderError::timeout("openai", "timeout"), "network_error"),
            (ProviderError::network("openai", "network"), "network_error"),
            (
                ProviderError::provider_unavailable("openai", "down"),
                "provider_error",
            ),
            (
                ProviderError::configuration("openai", "bad config"),
                "config_error",
            ),
            (
                ProviderError::response_parsing("openai", "bad json"),
                "parse_error",
            ),
            (
                ProviderError::not_supported("openai", "audio"),
                "not_supported",
            ),
            (ProviderError::other("openai", "other"), "internal"),
        ];

        for (provider_error, expected) in cases {
            assert_eq!(
                sdk_variant(&SDKError::from(provider_error)),
                expected,
                "canonical SDK category mismatch"
            );
        }
    }

    #[test]
    fn provider_error_conversion_redacts_display_and_debug() {
        let raw_key = "sk-sdk-secret-123456789";
        let raw_signature = "sdk-signed-value";
        let sdk_error = SDKError::from(ProviderError::api_error(
            "openai",
            503,
            format!(
                "Authorization: Bearer {raw_key}\nrequest=https://user:password@example.com/v1?X-Amz-Signature={raw_signature}"
            ),
        ));

        assert_eq!(sdk_variant(&sdk_error), "provider_error");
        let display = sdk_error.to_string();
        let debug = format!("{sdk_error:?}");
        for raw in [raw_key, raw_signature, "password"] {
            assert!(!display.contains(raw), "Display leaked {raw}: {display}");
            assert!(!debug.contains(raw), "Debug leaked {raw}: {debug}");
        }
        assert!(display.contains("[REDACTED]") || debug.contains("[REDACTED]"));
    }

    // ==================== SDKError is_retryable Tests ====================

    #[test]
    fn test_is_retryable_network_error() {
        let error = SDKError::NetworkError("timeout".to_string());
        assert!(error.is_retryable());
    }

    #[test]
    fn test_is_retryable_rate_limit_error() {
        let error = SDKError::RateLimitError("limit exceeded".to_string());
        assert!(error.is_retryable());
    }

    // SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError
    #[allow(deprecated)]
    #[test]
    fn test_is_retryable_provider_error() {
        let error = SDKError::ProviderError("unavailable".to_string());
        assert!(error.is_retryable());
    }

    #[test]
    fn test_is_not_retryable_auth_error() {
        let error = SDKError::AuthError("invalid key".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_is_not_retryable_config_error() {
        let error = SDKError::ConfigError("bad config".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_is_not_retryable_invalid_request() {
        let error = SDKError::InvalidRequest("bad request".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_is_not_retryable_internal() {
        let error = SDKError::Internal("bug".to_string());
        assert!(!error.is_retryable());
    }

    // ==================== SDKError is_auth_error Tests ====================

    #[test]
    fn test_is_auth_error_true() {
        let error = SDKError::AuthError("unauthorized".to_string());
        assert!(error.is_auth_error());
    }

    #[test]
    fn test_is_auth_error_false_for_others() {
        let errors = vec![
            SDKError::NetworkError("net".to_string()),
            SDKError::ConfigError("cfg".to_string()),
            SDKError::RateLimitError("rate".to_string()),
            SDKError::Internal("int".to_string()),
        ];

        for error in errors {
            assert!(!error.is_auth_error());
        }
    }

    // ==================== SDKError is_config_error Tests ====================

    #[test]
    fn test_is_config_error_config_error() {
        let error = SDKError::ConfigError("bad config".to_string());
        assert!(error.is_config_error());
    }

    #[test]
    fn test_is_config_error_provider_not_found() {
        let error = SDKError::ProviderNotFound("xyz".to_string());
        assert!(error.is_config_error());
    }

    #[test]
    fn test_is_config_error_no_default_provider() {
        let error = SDKError::NoDefaultProvider;
        assert!(error.is_config_error());
    }

    #[test]
    fn test_is_not_config_error_for_others() {
        let errors = vec![
            SDKError::NetworkError("net".to_string()),
            SDKError::AuthError("auth".to_string()),
            SDKError::RateLimitError("rate".to_string()),
        ];

        for error in errors {
            assert!(!error.is_config_error());
        }
    }

    // ==================== SDKError From GatewayError Tests ====================

    #[test]
    fn test_from_gateway_error_unauthorized() {
        let gateway_error = GatewayError::Auth("Invalid token".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::AuthError(_)));
        assert!(sdk_error.is_auth_error());
    }

    #[test]
    fn test_from_gateway_error_not_found() {
        let gateway_error = GatewayError::NotFound("Resource not found".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::ModelNotFound(_)));
    }

    #[test]
    fn test_from_gateway_error_bad_request() {
        let gateway_error = GatewayError::BadRequest("Invalid params".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_from_gateway_error_rate_limit() {
        let gateway_error = GatewayError::RateLimit {
            message: "Too many requests".to_string(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        };
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::RateLimitError(_)));
        assert!(sdk_error.is_retryable());
    }

    // SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError
    #[allow(deprecated)]
    #[test]
    fn test_from_gateway_error_provider_unavailable() {
        let gateway_error = GatewayError::Unavailable("OpenAI down".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::ProviderError(_)));
    }

    #[test]
    fn test_from_gateway_error_internal() {
        let gateway_error = GatewayError::Internal("Unexpected error".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::Internal(_)));
    }

    #[test]
    fn test_from_gateway_error_network() {
        let gateway_error = GatewayError::Network("Connection refused".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::NetworkError(_)));
        assert!(sdk_error.is_retryable());
    }

    #[test]
    fn test_from_gateway_error_validation() {
        let gateway_error = GatewayError::Validation("Invalid model".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_from_gateway_error_parsing() {
        let gateway_error = GatewayError::Validation("Invalid JSON".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::InvalidRequest(_)));
    }

    // ==================== SDKError Debug Tests ====================

    #[test]
    fn test_sdk_error_debug() {
        let error = SDKError::AuthError("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("AuthError"));
    }

    #[test]
    fn test_sdk_error_is_std_error() {
        let error = SDKError::Internal("test".to_string());
        let _: &dyn std::error::Error = &error;
    }

    // ==================== SDKError Edge Cases ====================

    // SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError
    #[allow(deprecated)]
    #[test]
    fn test_sdk_error_empty_message() {
        let error = SDKError::ProviderError("".to_string());
        assert_eq!(error.to_string(), "Provider error: ");
    }

    #[test]
    fn test_sdk_error_unicode() {
        let error = SDKError::ApiError("错误信息 🚨".to_string());
        assert!(error.to_string().contains("错误信息"));
    }

    #[test]
    fn test_sdk_error_long_message() {
        let long_msg = "a".repeat(1000);
        let error = SDKError::Internal(long_msg.clone());
        assert!(error.to_string().contains(&long_msg));
    }

    #[rustfmt::skip]
    mod legacy_guard {
    use super::*;
    #[derive(Default)]
    struct Guard {
        owner: String,
        impl_owner: String,
        refs: BTreeMap<String, usize>,
        attrs: BTreeMap<String, usize>,
        errors: Vec<String>,
    }
    fn type_name(ty: &Type) -> String {
        match ty { Type::Path(path) => path.path.segments.last().map(|s| s.ident.unraw().to_string()).unwrap_or_default(), _ => String::new() }
    }

    fn legacy_path(qself: Option<&syn::QSelf>, path: &syn::Path) -> (bool, bool) {
        let ids: Vec<_> = path.segments.iter().map(|s| s.ident.unraw().to_string()).collect();
        let raw = path.segments.iter().any(|s| s.ident.to_string().starts_with("r#"));
        let normal = ids.ends_with(&["SDKError".into(), "ProviderError".into()]);
        let qualified = qself.is_some_and(|q| type_name(&q.ty) == "SDKError") && ids.last().is_some_and(|id| id == "ProviderError");
        (normal || qualified, raw || qualified)
    }

    fn token_refs(text: &str) -> (usize, bool) {
        let raw = text.contains("r#ProviderError") || text.contains("r#SDKError");
        let words: Vec<_> = text.split_whitespace().map(|word| word.strip_prefix("r#").unwrap_or(word)).collect();
        let normal = words.windows(3).filter(|w| *w == ["SDKError", "::", "ProviderError"]).count();
        let qualified = words.windows(5).filter(|w| *w == ["<", "SDKError", ">", "::", "ProviderError"]).count();
        let composed = words.contains(&"SDKError") && words.contains(&"ProviderError");
        (normal + qualified, raw || qualified > 0 || (composed && normal + qualified == 0) || words.windows(3).any(|w| w == ["Self", "::", "ProviderError"]))
    }

    fn macro_suppresses(text: &str) -> bool {
        let mut code = String::new(); let mut quoted = false; let mut escaped = false; for c in text.chars() { if quoted { if escaped { escaped = false; } else if c == '\\' { escaped = true; } else if c == '"' { quoted = false; } } else if c == '"' { quoted = true; } else { code.push(c); } }
        let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        [("#[allow(", false), ("#![allow(", false), ("#[expect(", false), ("#![expect(", false), ("#[cfg_attr(", true), ("#![cfg_attr(", true)].iter().any(|(start, nested)| compact.match_indices(start).any(|(at, _)| { let body = &compact[at + start.len()..]; body.find(")]").is_some_and(|end| { let body = &body[..end]; (!nested || body.contains("allow(") || body.contains("expect(")) && body.split(|c: char| !c.is_alphanumeric() && c != '_').any(|lint| matches!(lint, "deprecated" | "warnings")) }) }))
    }
    impl Guard {
        fn record(&mut self, count: usize, alternate: bool) {
            if count > 0 { *self.refs.entry(self.owner.clone()).or_default() += count; }
            if alternate { self.errors.push("alternate legacy path".into()); }
        }
        fn probe(expr: &Expr) -> usize {
            let mut guard = Guard { owner: "probe".into(), ..Guard::default() };
            guard.visit_expr(expr);
            guard.refs.values().sum()
        }
    }

    impl<'ast> Visit<'ast> for Guard {
        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            if node.qself.as_ref().is_some_and(|qself| legacy_path(Some(qself), &node.path).0) { self.record(1, true); }
            visit::visit_expr_path(self, node);
        }
        fn visit_path(&mut self, node: &'ast syn::Path) {
            let (legacy, alternate) = legacy_path(None, node); if legacy { self.record(1, alternate); }
            let ids: Vec<_> = node.segments.iter().map(|s| s.ident.unraw().to_string()).collect();
            if ids.ends_with(&["Self".into(), "ProviderError".into()]) { self.errors.push("Self alias".into()); }
            visit::visit_path(self, node);
        }
        fn visit_macro(&mut self, node: &'ast syn::Macro) {
            let text = node.tokens.to_string();
            let (count, alternate) = token_refs(&text);
            self.record(count, alternate);
            let name = node.path.segments.last().map(|s| s.ident.unraw().to_string()).unwrap_or_default(); let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect(); let pattern = (name == "matches" && compact == "self,SDKError::NetworkError(_)|SDKError::RateLimitError(_)|SDKError::ProviderError(_)") || (name == "assert" && compact == "matches!(sdk_error,SDKError::ProviderError(_))");
            if count > 0 && !pattern { self.errors.push("legacy macro alias".into()); }
            if macro_suppresses(&text) { self.errors.push("macro lint suppression".into()); }
        }
        fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
            let mut metas = vec![node.meta.clone()]; while let Some(meta) = metas.pop() {
                let Meta::List(list) = meta else { continue; };
                if list.path.is_ident("cfg_attr") { let parser = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated; if let Ok(items) = syn::parse::Parser::parse2(parser, list.tokens) { metas.extend(items); } continue; }
                if !matches!(list.path.get_ident().map(|id| id.unraw().to_string()).as_deref(), Some("allow" | "expect")) { continue; } let body = list.tokens.to_string();
                if body.split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == "warnings") { self.errors.push("warnings lint suppression".into()); }
                if body.split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == "deprecated") {
                    *self.attrs.entry(self.owner.clone()).or_default() += 1;
                    if !list.path.is_ident("allow") || body.trim() != "deprecated" { self.errors.push("expanded deprecated suppression".into()); }
                }
            }
            visit::visit_attribute(self, node);
        }
        fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
            let old = std::mem::replace(&mut self.owner, format!("fn:{}", node.sig.ident.unraw()));
            visit::visit_item_fn(self, node); self.owner = old;
        }
        fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
            let trait_ty = node.trait_.as_ref().and_then(|(_, path, _)| path.segments.last()).and_then(|s| match &s.arguments { PathArguments::AngleBracketed(args) => args.args.iter().find_map(|arg| match arg { GenericArgument::Type(ty) => Some(type_name(ty)), _ => None }), _ => None });
            let old = std::mem::replace(&mut self.impl_owner, trait_ty.unwrap_or_else(|| type_name(&node.self_ty)));
            visit::visit_item_impl(self, node); self.impl_owner = old;
        }
        fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
            let old = std::mem::replace(&mut self.owner, format!("impl:{}:{}", self.impl_owner, node.sig.ident.unraw()));
            visit::visit_impl_item_fn(self, node); self.owner = old;
        }
        fn visit_local(&mut self, node: &'ast syn::Local) {
            if let Some(init) = &node.init {
                let direct = matches!(init.expr.as_ref(), Expr::Call(call) if matches!(call.func.as_ref(), Expr::Path(path) if legacy_path(path.qself.as_ref(), &path.path) == (true, false)));
                if Guard::probe(&init.expr) > 0 && !direct { self.errors.push("legacy value alias".into()); }
            }
            visit::visit_local(self, node);
        }
        fn visit_item_const(&mut self, node: &'ast syn::ItemConst) { if Guard::probe(&node.expr) > 0 { self.errors.push("legacy const alias".into()); } visit::visit_item_const(self, node); }
        fn visit_item_static(&mut self, node: &'ast syn::ItemStatic) { if Guard::probe(&node.expr) > 0 { self.errors.push("legacy static alias".into()); } visit::visit_item_static(self, node); }
        fn visit_item_type(&mut self, node: &'ast syn::ItemType) { if type_name(&node.ty) == "SDKError" { self.errors.push("SDKError type alias".into()); } visit::visit_item_type(self, node); }
        fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) { let text = node.mac.tokens.to_string(); if text.split_whitespace().any(|word| word.trim_start_matches("r#") == "SDKError") || token_refs(&text).0 > 0 { self.errors.push("legacy macro alias".into()); } visit::visit_item_macro(self, node); }
        fn visit_item_use(&mut self, node: &'ast syn::ItemUse) { if risky_use(&node.tree, false) { self.errors.push("legacy import alias".into()); } visit::visit_item_use(self, node); }
    }

    fn risky_use(tree: &UseTree, sdk: bool) -> bool {
        match tree {
            UseTree::Path(path) => risky_use(&path.tree, sdk || path.ident.unraw() == "SDKError"),
            UseTree::Name(name) => sdk && name.ident.unraw() == "ProviderError",
            UseTree::Rename(rename) => sdk || rename.ident.unraw() == "SDKError",
            UseTree::Glob(_) => sdk,
            UseTree::Group(group) => group.items.iter().any(|item| risky_use(item, sdk)),
        }
    }

    fn walk(directory: &Path, rust_only: bool, files: &mut BTreeSet<PathBuf>) -> std::io::Result<()> {
        if !directory.exists() { return Ok(()); }
        for entry in fs::read_dir(directory)? {
            let entry = entry?; let path = entry.path(); let kind = entry.file_type()?;
            if kind.is_dir() && entry.file_name() != "__pycache__" { walk(&path, rust_only, files)?; }
            else if kind.is_file() && (!rust_only || path.extension().is_some_and(|ext| ext == "rs")) { files.insert(path.canonicalize()?); }
        }
        Ok(())
    }

    fn verify_sources(sources: &BTreeMap<String, String>) -> std::result::Result<(), String> {
        let expected = BTreeMap::from([
            (("src/sdk/errors.rs", "impl:GatewayError:from"), 1), (("src/sdk/errors.rs", "impl:ProviderError:from"), 1),
            (("src/sdk/errors.rs", "impl:SDKError:is_retryable"), 1), (("src/sdk/errors.rs", "fn:sdk_variant"), 1),
            (("src/sdk/errors.rs", "fn:test_sdk_error_provider_error"), 1), (("src/sdk/errors.rs", "fn:test_is_retryable_provider_error"), 1),
            (("src/sdk/errors.rs", "fn:test_from_gateway_error_provider_unavailable"), 1), (("src/sdk/errors.rs", "fn:test_sdk_error_empty_message"), 1),
            (("src/sdk/client/completions.rs", "impl:LLMClient:execute_chat_request"), 1),
        ]);
        let mut refs = BTreeMap::new(); let mut owner_attrs = BTreeMap::new(); let mut attr_counts = BTreeMap::new();
        for (path, source) in sources {
            let file = syn::parse_file(source).map_err(|error| format!("{path}: {error}"))?;
            let mut guard = Guard { owner: "outside".into(), ..Guard::default() }; guard.visit_file(&file);
            if !guard.errors.is_empty() { return Err(format!("{path}: {:?}", guard.errors)); }
            for (owner, count) in guard.refs { refs.insert((path.as_str(), owner.leak() as &str), count); }
            for (owner, count) in guard.attrs { owner_attrs.insert((path.as_str(), owner.leak() as &str), count); *attr_counts.entry(path.as_str()).or_default() += count; }
        }
        if refs != expected { return Err(format!("legacy owner references changed: {refs:?}")); }
        if owner_attrs.iter().filter(|((path, _), _)| path.starts_with("src/sdk/")).collect::<BTreeMap<_, _>>() != expected.iter().collect::<BTreeMap<_, _>>() { return Err(format!("legacy allow owners changed: {owner_attrs:?}")); }
        let mut expected_attrs = BTreeMap::from([("src/core/traits/provider/llm_provider/sub_traits.rs", 9), ("src/sdk/client/completions.rs", 1), ("src/sdk/errors.rs", 8), ("src/server/routes/mod.rs", 1)]);
        for path in ["src/core/router/tests/concurrency_edge_case_tests.rs", "src/core/router/tests/execution_tests.rs", "src/core/router/tests/router_tests.rs", "src/core/router/tests/selection_tests.rs", "src/core/router/tests/strategy_tests.rs", "tests/integration/router_tests.rs"] { if sources.contains_key(path) { expected_attrs.insert(path, 1); } }
        if attr_counts != expected_attrs { return Err(format!("deprecated attribute baseline changed: {attr_counts:?}")); }
        let marker = format!("SP965-T010 links 0.7 removal follow-up for {}{}", "SDKError::", "ProviderError"); let allow = format!("#[allow{}]", "(deprecated)"); let errors = &sources["src/sdk/errors.rs"]; let completions = &sources["src/sdk/client/completions.rs"];
        for (source, site) in [(errors, format!("// {marker}\n            {allow}\n            crate::utils::error::gateway_error::GatewayError::Unavailable(msg) =>")), (errors, format!("// {marker}\n            {allow}\n            ErrorCode::Unavailable =>")), (completions, format!("// {marker}\n            {allow}\n            _ => Err(SDKError::ProviderError"))] { if source.matches(&site).count() != 1 { return Err(format!("arm marker moved: {site}")); } } for (name, test, public) in [("is_retryable", false, true), ("sdk_variant", false, false), ("test_sdk_error_provider_error", true, false), ("test_is_retryable_provider_error", true, false), ("test_from_gateway_error_provider_unavailable", true, false), ("test_sdk_error_empty_message", true, false)] { let site = format!("// {marker}\n    {allow}\n{}    {}fn {name}", if test { "    #[test]\n" } else { "" }, if public { "pub " } else { "" }); if errors.matches(&site).count() != 1 { return Err(format!("function marker moved: {name}")); } } if sources.values().map(|source| source.matches(&marker).count()).sum::<usize>() != 9 { return Err("T010 marker count changed".into()); }
        let baselines: [(&str, &[&str]); 2] = [("src/server/routes/mod.rs", &["fn test_api_response_to_http_response_remains_compatibility_shim"]), ("src/core/traits/provider/llm_provider/sub_traits.rs", &["impl<T: LLMProvider> LLMChat for T", "impl<T: LLMProvider> LLMEmbed for T", "impl<T: LLMProvider> LLMStream for T", "async fn test_llm_chat_blanket_impl", "async fn test_llm_embed_blanket_impl", "async fn test_llm_stream_blanket_impl", "fn _accepts_chat<T: LLMChat>", "fn _accepts_embed<T: LLMEmbed>", "fn _accepts_stream<T: LLMStream>"])]; for (path, anchors) in baselines { if let Some(source) = sources.get(path) { for anchor in anchors { let at = source.find(anchor).ok_or_else(|| format!("baseline anchor moved: {path}"))?; if !source[..at].trim_end().ends_with(&allow) { return Err(format!("broad allow moved: {path}: {anchor}")); } } } }
        for path in ["src/core/router/tests/concurrency_edge_case_tests.rs", "src/core/router/tests/execution_tests.rs", "src/core/router/tests/router_tests.rs", "src/core/router/tests/selection_tests.rs", "src/core/router/tests/strategy_tests.rs"] { if let Some(source) = sources.get(path) { let at = source.find("use ").ok_or_else(|| format!("missing use anchor: {path}"))?; if !source[..at].trim_end().ends_with("#![allow(deprecated)]") { return Err(format!("inner allow moved: {path}")); } } } if let Some(source) = sources.get("tests/integration/router_tests.rs") && !source.contains("mod tests {\n    #![allow(deprecated)]\n") { return Err("integration allow moved".into()); }
        Ok(())
    }

    fn verify_lint(text: &str) -> bool {
        let normalized: String = text.to_ascii_lowercase().chars().map(|c| if "\"'=,[]".contains(c) { ' ' } else { c }).collect();
        let words: Vec<_> = normalized.split_whitespace().collect();
        !words.iter().enumerate().any(|(i, word)| matches!(*word, "-adeprecated" | "-awarnings") || matches!(words.get(i..i + 2), Some(["-a", "deprecated"] | ["-a", "warnings"] | ["--allow", "deprecated"] | ["--allow", "warnings"] | ["--cap-lints", "allow"] | ["--cap-lints", "warn"])))
    }

    pub(super) fn run_legacy_provider_error_guard() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize().expect("manifest root");
        let mut rust_files = BTreeSet::new();
        if root.join(".git").exists() {
            let output = Command::new("git").args(["ls-files", "*.rs"]).current_dir(&root).output().expect("git discovery");
            assert!(output.status.success()); for path in String::from_utf8(output.stdout).unwrap().lines() { rust_files.insert(root.join(path).canonicalize().unwrap()); }
        }
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo).args(["metadata", "--no-deps", "--format-version", "1"]).current_dir(&root).output().expect("cargo metadata");
        assert!(output.status.success()); let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let mut roots = BTreeSet::from([root.clone(), PathBuf::from(metadata["workspace_root"].as_str().unwrap()).canonicalize().unwrap()]); let mut lint_files = BTreeSet::new();
        for package in metadata["packages"].as_array().unwrap() {
            let manifest = PathBuf::from(package["manifest_path"].as_str().unwrap()).canonicalize().unwrap(); lint_files.insert(manifest.clone()); roots.insert(manifest.parent().unwrap().into());
            for target in package["targets"].as_array().unwrap() { rust_files.insert(PathBuf::from(target["src_path"].as_str().unwrap()).canonicalize().unwrap()); }
        }
        for base in &roots { for dir in ["src", "tests", "examples", "benches"] { walk(&base.join(dir), true, &mut rust_files).unwrap(); } if base.join("build.rs").exists() { rust_files.insert(base.join("build.rs").canonicalize().unwrap()); } }
        let mut sources = BTreeMap::new(); for path in rust_files { let label = path.strip_prefix(&root).map_or_else(|_| path.display().to_string(), |rel| rel.to_string_lossy().replace('\\', "/")); sources.insert(label, fs::read_to_string(path).unwrap()); }
        verify_sources(&sources).expect("legacy deprecation source guard");
        let errors = &sources["src/sdk/errors.rs"]; let legacy = format!("{}{}", "SDKError::", "ProviderError"); let constructor = format!("let error = {legacy}(\"API unavailable\".to_string());"); let provider_assert = format!("assert!(matches!(sdk_error, {legacy}(_)));" ); let tests_mod = format!("{}{}", "#[cfg(test)]\nmod tests ", "{"); let marker = format!("SP965-T010 links 0.7 removal follow-up for {legacy}"); let allow = format!("#[allow{}]", "(deprecated)"); let marker_site = format!("// {marker}\n    {allow}\n    #[test]\n    fn test_sdk_error_provider_error() {{");
        for (label, old, new) in [
            ("value alias", constructor.as_str(), format!("let make = {legacy}; let error = make(\"API unavailable\".to_string());")),
            ("allow warnings", tests_mod.as_str(), format!("{tests_mod} #![allow(warnings)]")), ("cfg_attr warnings", tests_mod.as_str(), format!("{tests_mod} #![cfg_attr(all(), allow(warnings))]")),
            ("qualified path", constructor.as_str(), constructor.replace(&legacy, "<SDKError>::ProviderError")),
            ("raw path", constructor.as_str(), constructor.replace(&legacy, "SDKError::r#ProviderError")),
            ("macro alias", constructor.as_str(), format!("macro_rules! make {{ ($v:expr) => {{ {legacy}($v) }} }} let error = make!(\"API unavailable\".to_string());")), ("generic macro composition", constructor.as_str(), "macro_rules! make { ($ty:path, $variant:ident, $value:expr) => { $ty::$variant($value) }; } let error = make!(SDKError, ProviderError, \"API unavailable\".to_string());".into()), ("split constructor macro", constructor.as_str(), format!("macro_rules! bind {{ ($name:ident = $ctor:path) => {{ let $name = $ctor; }}; }} bind!(make = {legacy}); let error = make(\"one\".into()); let extra = make(\"two\".into());")), ("split variant macro", constructor.as_str(), "macro_rules! make_sdk { ($variant:ident, $value:expr) => { SDKError::$variant($value) }; } let error = make_sdk!(ProviderError, \"extra\".into());".into()), ("macro lint suppression", tests_mod.as_str(), format!("{tests_mod} macro_rules! allow_legacy {{ ($expr:expr) => {{ #[allow(deprecated)] $expr }} }} let _ = allow_legacy!({legacy}(\"x\".into()));")), ("macro cfg_attr suppression", tests_mod.as_str(), format!("{tests_mod} macro_rules! allow_legacy {{ ($expr:expr) => {{ #[cfg_attr(all(), allow(deprecated))] $expr }} }} let _ = allow_legacy!({legacy}(\"x\".into()));")), ("assert macro smuggle", provider_assert.as_str(), format!("assert!({{ let make = {legacy}; let _extra = make(\"extra\".into()); matches!(sdk_error, _) }});")), ("marker relocation", marker_site.as_str(), format!("{allow}\n    #[test]\n    fn test_sdk_error_provider_error() {{\n        // {marker}")),
        ] { assert_eq!(errors.matches(old).count(), 1); let mut changed = sources.clone(); changed.insert("src/sdk/errors.rs".into(), errors.replacen(old, &new, 1)); assert!(verify_sources(&changed).is_err(), "mutation accepted: {label}"); }
        for (label, path, old, new) in [("server anchor", "src/server/routes/mod.rs", "#[allow(deprecated)]\n    fn test_api_response_to_http_response_remains_compatibility_shim() {", "fn test_api_response_to_http_response_remains_compatibility_shim() {\n        #[allow(deprecated)]"), ("subtrait anchor", "src/core/traits/provider/llm_provider/sub_traits.rs", "#[allow(deprecated)]\nimpl<T: LLMProvider> LLMChat for T {", "impl<T: LLMProvider> LLMChat for T {\n    #[allow(deprecated)]"), ("inner router anchor", "src/core/router/tests/execution_tests.rs", "#![allow(deprecated)]\n\nuse ", "#[allow(deprecated)]\nuse ")] { let source = &sources[path]; assert_eq!(source.matches(old).count(), 1); let mut changed = sources.clone(); changed.insert(path.into(), source.replacen(old, new, 1)); assert!(syn::parse_file(&source.replacen(old, new, 1)).is_ok(), "invalid mutation: {label}"); assert!(verify_sources(&changed).is_err(), "mutation accepted: {label}"); } let mut packaged = sources.clone(); packaged.remove("tests/integration/router_tests.rs"); verify_sources(&packaged).expect("packaged source baseline");
        for base in &roots { for dir in [".cargo", ".github/workflows", "scripts", "checks", "xtask"] { walk(&base.join(dir), false, &mut lint_files).unwrap(); } for entry in fs::read_dir(base).unwrap().flatten() { let path = entry.path(); let name = path.file_name().and_then(|n| n.to_str()).unwrap_or(""); if path.is_file() && (name.starts_with("Makefile") || name.starts_with("justfile") || name.starts_with("rust-toolchain") || name == "clippy.toml") { lint_files.insert(path.canonicalize().unwrap()); } } }
        for path in lint_files { assert!(verify_lint(&fs::read_to_string(path).unwrap()), "lint downgrade"); }
        for sample in ["RUSTFLAGS='--cap-lints allow'", "rustflags=[\"--cap-lints=warn\"]", "-A deprecated", "-A warnings"] { assert!(!verify_lint(sample)); }
    }

    }

    #[test]
    fn legacy_provider_error_deprecation_allowlist_does_not_grow() {
        legacy_guard::run_legacy_provider_error_guard();
    }
}
