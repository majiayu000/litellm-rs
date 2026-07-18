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
    use std::process::Command;

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

    #[test]
    fn legacy_provider_error_deprecation_allowlist_does_not_grow() {
        let status = Command::new("python3")
            .args(["-c", r####"
import json, re, subprocess
from pathlib import Path

root = Path.cwd().resolve(strict=True)
def resolved(path): return Path(path).resolve(strict=True)
def relative(path):
    try: return path.relative_to(root).as_posix()
    except ValueError: return str(path)

tracked = []
if (root / ".git").exists():
    tracked = subprocess.run(
        ["git", "ls-files", "*.rs"], cwd=root, check=True, capture_output=True, text=True
    ).stdout.splitlines()
rust_files = {resolved(root / path) for path in tracked}
metadata = json.loads(subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"],
    cwd=root, check=True, capture_output=True, text=True).stdout)
package_roots = {root, resolved(metadata["workspace_root"])}
lint_files = set()
for package in metadata["packages"]:
    manifest = resolved(package["manifest_path"]); lint_files.add(manifest); package_roots.add(manifest.parent)
    rust_files.update(resolved(target["src_path"]) for target in package["targets"])
for package_root in package_roots:
    for directory in ("src", "tests", "examples", "benches"):
        rust_files.update(resolved(path) for path in package_root.glob(f"{directory}/**/*.rs") if path.is_file())
    build = package_root / "build.rs"
    if build.exists(): rust_files.add(resolved(build))
sources = {relative(path): path.read_text(encoding="utf-8") for path in sorted(rust_files)}
legacy = "SDKError::" + "ProviderError"
marker = "SP965-T010 links 0.7 removal follow-up for " + legacy
allow = "#[allow" + "(deprecated)]"
punct = set("{}()[].,:;|=<>?!&+-*/%^#@~$'")
def lex(text):
    out, offset = [], 0
    while offset < len(text):
        if text[offset].isspace(): offset += 1; continue
        start = offset
        if text.startswith("//", offset):
            offset = text.find("\n", offset); offset = len(text) if offset < 0 else offset; continue
        if text.startswith("/*", offset):
            depth = 1; offset += 2
            while offset < len(text) and depth:
                if text.startswith("/*", offset): depth += 1; offset += 2
                elif text.startswith("*/", offset): depth -= 1; offset += 2
                else: offset += 1
            assert depth == 0, "unterminated block comment"; continue
        raw = re.match(r'(?:br|cr|r)(?P<h>#{0,255})"', text[offset:])
        if raw:
            closing = '"' + raw.group("h"); offset += raw.end(); end = text.find(closing, offset)
            assert end >= 0, "unterminated raw string"; offset = end + len(closing); out.append(("LITERAL", start, offset)); continue
        prefix = 1 if text.startswith(('b"', 'c"'), offset) else 0
        if text[offset + prefix:offset + prefix + 1] == '"':
            offset += prefix + 1
            while offset < len(text):
                if text[offset] == "\\": offset += 2
                elif text[offset] == '"': offset += 1; break
                else: offset += 1
            assert offset <= len(text) and text[offset - 1] == '"', "unterminated string"; out.append(("LITERAL", start, offset)); continue
        if text[offset] == "'" and offset + 2 < len(text) and (text[offset + 1] == "\\" or text[offset + 2] == "'"):
            offset += 1; offset += 2 if text[offset] == "\\" else 1
            assert offset < len(text) and text[offset] == "'", "unterminated char"; offset += 1; out.append(("LITERAL", start, offset)); continue
        match = re.match(r"[A-Za-z_][A-Za-z0-9_]*|[0-9][A-Za-z0-9_.]*", text[offset:])
        if match: offset += match.end(); out.append((text[start:offset], start, offset)); continue
        operator = next((item for item in ("::", "=>", "->", "..=", "...", "..") if text.startswith(item, offset)), None)
        if operator: offset += len(operator); out.append((operator, start, offset)); continue
        assert text[offset] in punct, f"unrecognized Rust syntax: {text[offset:offset + 20]!r}"
        offset += 1; out.append((text[start:offset], start, offset))
    return out
def values(source): return [value for value, _, _ in lex(source)]
def occurrences(items, needle): return sum(items[index:index + len(needle)] == needle for index in range(len(items) - len(needle) + 1))
def matching(tokens, start, opening, closing):
    depth = 0
    for index in range(start, len(tokens)):
        if tokens[index][0] == opening: depth += 1
        elif tokens[index][0] == closing:
            depth -= 1
            if not depth: return index
    raise AssertionError(f"unclosed {opening}")
def function_source(source, name):
    tokens = lex(source); items = [value for value, _, _ in tokens]
    starts = [index for index in range(len(items) - 1) if items[index:index + 2] == ["fn", name]]
    assert len(starts) == 1, f"function owner changed: {name}"
    opening = next(index for index in range(starts[0] + 2, len(tokens)) if tokens[index][0] == "{"); return source[tokens[starts[0]][1]:tokens[matching(tokens, opening, "{", "}")][2]]
def owned_arm(source, function, match_header, expected):
    body = function_source(source, function); tokens = lex(body); items = [item[0] for item in tokens]
    header, arm = values(match_header), values(expected)
    matches = [i for i in range(len(items)) if items[i:i + len(header)] == header]
    assert len(matches) == 1 and header[-1] == "{", f"match owner changed: {function}"
    opening = matches[0] + len(header) - 1; closing = matching(tokens, opening, "{", "}")
    depth, starts = 0, []
    for index in range(opening + 1, closing):
        if not depth and items[index:index + len(arm)] == arm: starts.append(index)
        if items[index] in ("{", "(", "["): depth += 1
        elif items[index] in ("}", ")", "]"): depth -= 1
    assert len(starts) == 1, f"arm owner changed: {function}"
    prefix = body[:tokens[starts[0]][1]]
    decoration = rf"(?m)^[ \t]*// {re.escape(marker)}\r?\n[ \t]*{re.escape(allow)}\r?\n[ \t]*\Z"
    assert re.search(decoration, prefix), f"arm decoration changed: {function}"
def deprecated_attrs(source):
    tokens = lex(source); items = [value for value, _, _ in tokens]; found = []
    for index, value in enumerate(items):
        cursor = index + 1
        if value != "#": continue
        if cursor < len(items) and items[cursor] == "!": cursor += 1
        if cursor >= len(items) or items[cursor] != "[": continue
        end = matching(tokens, cursor, "[", "]"); body = items[cursor + 1:end]
        if "deprecated" in body and ("allow" in body or "expect" in body): found.append(source[tokens[index][1]:tokens[end][2]])
    return found
baselines = {"src/server/routes/mod.rs": ("fn test_api_response_to_http_response_remains_compatibility_shim",),
    "src/core/traits/provider/llm_provider/sub_traits.rs": ("impl<T: LLMProvider> LLMChat for T", "impl<T: LLMProvider> LLMEmbed for T", "impl<T: LLMProvider> LLMStream for T",
    "async fn test_llm_chat_blanket_impl", "async fn test_llm_embed_blanket_impl", "async fn test_llm_stream_blanket_impl", "fn _accepts_chat<T: LLMChat>", "fn _accepts_embed<T: LLMEmbed>", "fn _accepts_stream<T: LLMStream>")}
inner_roots = ("src/core/router/tests/concurrency_edge_case_tests.rs",
    "src/core/router/tests/execution_tests.rs", "src/core/router/tests/router_tests.rs", "src/core/router/tests/selection_tests.rs", "src/core/router/tests/strategy_tests.rs")
def verify(candidate):
    refs = {}
    for path, source in candidate.items():
        items = values(source); count = occurrences(items, ["SDKError", "::", "ProviderError"]); refs.update({path: count} if count else {})
        assert not occurrences(items, ["Self", "::", "ProviderError"]), f"Self alias: {path}"
        for index, value in enumerate(items):
            if value not in ("type", "use"): continue
            end = next((cursor for cursor in range(index, len(items)) if items[cursor] == ";"), len(items)); statement = items[index:end]
            direct = [] if "=" not in statement else [item for item in statement[statement.index("=") + 1:] if item not in ("(", ")")]
            if value == "type": assert not (direct and direct[-1] == "SDKError" and all(item == "::" or item.isidentifier() for item in direct)), f"SDKError alias: {path}"
            if value == "use" and "SDKError" in statement: assert not ({"ProviderError", "*", "as"} & set(statement)), f"legacy import: {path}"
    assert refs == {"src/sdk/client/completions.rs": 1, "src/sdk/errors.rs": 8}, refs
    attrs = {path: found for path, source in candidate.items() if (found := deprecated_attrs(source))}
    expected_attrs = {"src/core/traits/provider/llm_provider/sub_traits.rs": 9, "src/sdk/client/completions.rs": 1, "src/sdk/errors.rs": 8, "src/server/routes/mod.rs": 1}
    expected_attrs.update({path: 1 for path in inner_roots + ("tests/integration/router_tests.rs",)})
    assert {path: len(found) for path, found in attrs.items()} == expected_attrs, attrs
    assert all(text in (allow, "#![allow(deprecated)]") for found in attrs.values() for text in found), attrs
    for path, anchors in baselines.items():
        assert all((offset := candidate[path].find(anchor)) >= 0 and candidate[path][:offset].rstrip().endswith(allow) for anchor in anchors), f"broad allow moved: {path}"
    for path in inner_roots: assert re.fullmatch(r"(?s)(?://![^\n]*\n|\s)*#!\[allow\(deprecated\)\]\s*", candidate[path][:candidate[path].find("use ")])
    assert "mod tests {\n    #![allow(deprecated)]\n" in candidate["tests/integration/router_tests.rs"]
    errors = candidate["src/sdk/errors.rs"]; completions = candidate["src/sdk/client/completions.rs"]
    assert sum(source.count(marker) for source in candidate.values()) == 9
    functions = (("is_retryable", False, True), ("sdk_variant", False, False), ("test_sdk_error_provider_error", True, False), ("test_is_retryable_provider_error", True, False),
        ("test_from_gateway_error_provider_unavailable", True, False), ("test_sdk_error_empty_message", True, False))
    for name, test, public in functions:
        prefix = (rf"(?m)^[ \t]*// {re.escape(marker)}\r?\n[ \t]*{re.escape(allow)}\r?\n" + (r"[ \t]*#\[test\]\r?\n" if test else "") + rf"[ \t]*{'pub ' if public else ''}fn {name}\b")
        assert len(re.findall(prefix, errors)) == 1, f"owner decoration: {name}"
        assert occurrences(values(function_source(errors, name)), ["SDKError", "::", "ProviderError"]) == 1, f"owner ref: {name}"
    head = rf"(?m)^[ \t]*// {re.escape(marker)}\r?\n[ \t]*{re.escape(allow)}\r?\n[ \t]*"
    arms = ((errors, r"crate::utils::error::gateway_error::GatewayError::Unavailable\(msg\)\s*=>\s*\{\s*SDKError\s*::\s*ProviderError\s*\(\s*msg\s*\)\s*\}"),
        (errors, r"ErrorCode::Unavailable\s*=>\s*SDKError\s*::\s*ProviderError\s*\(\s*message\s*\)\s*,"))
    for source, arm in arms: assert len(re.findall(head + arm, source)) == 1, f"arm owner: {arm}"
    owned_arm(completions, "execute_chat_request", "match provider.provider_type {", '_ => Err(SDKError::ProviderError(format!("fallback", provider.provider_type))),')
verify(sources)
def replaced(source, old, new): assert source.count(old) == 1; return source.replace(old, new)
def reject(label, mutated, path="src/sdk/errors.rs"):
    try: verify({**sources, path: mutated})
    except AssertionError: return
    raise AssertionError(f"mutation accepted: {label}")
errors = sources["src/sdk/errors.rs"]
prefix = f"    // {marker}\n    {allow}\n    #[test]\n    fn test_sdk_error_provider_error() {{"
relocated = f"    // {marker}\n    {allow}\n    const RELOCATED: fn(String) -> SDKError = {legacy};\n\n    #[test]\n    fn test_sdk_error_provider_error() {{"
reject("relocated", replaced(replaced(errors, prefix, relocated), f'let error = {legacy}("API unavailable".to_string());', 'let error = RELOCATED("API unavailable".to_string());'))
retry_line = f"SDKError::NetworkError(_) | SDKError::RateLimitError(_) | {legacy}(_)"
reject("Self", replaced(errors, retry_line, retry_line + " | Self::ProviderError(_)"))
constructor = f'let error = {legacy}("API unavailable".to_string());'
reject("split", replaced(errors, constructor, constructor + "\n        let _ = SDKError::\n            ProviderError(String::new());"))
reject("alias", replaced(errors, constructor, "type E = SDKError;\n        " + constructor + "\n        let _ = E::ProviderError(String::new());"))
reject("attribute", replaced(errors, "mod tests {\n", "mod tests {\n    #![allow (deprecated, dead_code)]\n"))
completions = sources["src/sdk/client/completions.rs"]
fallback = f'            // {marker}\n            {allow}\n            _ => Err(SDKError::ProviderError(format!(\n                "Provider type {{:?}} is not implemented in SDK client",\n                provider.provider_type\n            ))),'
replacement = '            _ => Err(SDKError::Internal(format!(\n                "Provider type {:?} is not implemented in SDK client",\n                provider.provider_type\n            ))),'
macro = f'#[allow(unused_macros)]\nmacro_rules! relocated_provider_error_arm {{ () => {{\n    // {marker}\n    {allow}\n    _ => Err(SDKError::ProviderError(format!("Provider type {{:?}} is not implemented in SDK client", provider.provider_type))),\n}}; }}\n'
reject("completion arm relocated into macro owner", macro + replaced(completions, fallback, replacement), "src/sdk/client/completions.rs")
for package_root in package_roots:
    for directory in (".cargo", ".github/workflows", "scripts", "checks", "xtask"):
        path = package_root / directory
        if path.exists(): lint_files.update(resolved(item) for item in path.rglob("*") if item.is_file() and "__pycache__" not in item.parts)
    for item in package_root.iterdir():
        if item.is_file() and (item.name.startswith(("Makefile", "justfile", "rust-toolchain")) or item.name == "clippy.toml"): lint_files.add(resolved(item))
def verify_lint(text, label):
    words = re.sub(r'''["'=,\[\]]''', " ", text.lower()).split()
    bad = {("-a", "deprecated"), ("-a", "warnings"), ("--allow", "deprecated"), ("--allow", "warnings"), ("--cap-lints", "allow"), ("--cap-lints", "warn")}
    for index, word in enumerate(words):
        assert word not in ("-adeprecated", "-awarnings"), f"lint downgrade: {label}"; assert tuple(words[index:index + 2]) not in bad, f"lint downgrade: {label}"
for path in sorted(lint_files): verify_lint(path.read_text(encoding="utf-8"), relative(path))
for sample in ("RUSTFLAGS='--cap-lints allow'", 'rustflags=["--cap-lints=warn"]', "-A deprecated"):
    try: verify_lint(sample, "mutation")
    except AssertionError: pass
    else: raise AssertionError(f"lint mutation accepted: {sample}")
"####])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("legacy deprecation guard must run");
        assert!(status.success(), "legacy deprecation guard failed");
    }
}
