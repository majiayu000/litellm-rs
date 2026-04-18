//! Sensitive data redaction for logs and traces
//!
//! This module provides utilities to redact sensitive information before logging,
//! preventing accidental exposure of API keys, tokens, and other credentials.

use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::LazyLock;

/// Default sensitive field names that should be redacted
static DEFAULT_SENSITIVE_FIELDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // Authentication
        "api_key",
        "apikey",
        "api-key",
        "authorization",
        "auth",
        "token",
        "access_token",
        "refresh_token",
        "bearer",
        "secret",
        "secret_key",
        "private_key",
        "password",
        "passwd",
        "credential",
        "credentials",
        // Provider-specific
        "openai_api_key",
        "anthropic_api_key",
        "azure_api_key",
        "google_api_key",
        "x-api-key",
        "x-auth-token",
        // Session/Cookie
        "session",
        "session_id",
        "cookie",
        "set-cookie",
        // Other sensitive data
        "ssn",
        "credit_card",
        "card_number",
        "cvv",
        "pin",
    ]
    .into_iter()
    .collect()
});

/// Redacted value placeholder
const REDACTED: &str = "[REDACTED]";

/// Redaction configuration
#[derive(Debug, Clone)]
pub struct RedactionConfig {
    /// Additional field names to redact (case-insensitive)
    pub additional_fields: HashSet<String>,
    /// Fields to exclude from redaction (case-insensitive)
    pub exclude_fields: HashSet<String>,
    /// Whether to redact values that look like API keys (heuristic)
    pub redact_by_pattern: bool,
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            additional_fields: HashSet::new(),
            exclude_fields: HashSet::new(),
            redact_by_pattern: true,
        }
    }
}

/// Redact sensitive data from a string value
///
/// Checks if the key matches known sensitive field names and redacts the value.
pub fn redact_value<'a>(key: &str, value: &'a str, config: &RedactionConfig) -> Cow<'a, str> {
    let key_lower = key.to_lowercase();

    // Check exclusion list first
    if config
        .exclude_fields
        .iter()
        .any(|f| f.to_lowercase() == key_lower)
    {
        return Cow::Borrowed(value);
    }

    // Check if key is in sensitive fields
    if is_sensitive_field(&key_lower, config) {
        return Cow::Borrowed(REDACTED);
    }

    // Optionally check value patterns
    if config.redact_by_pattern && looks_like_api_key(value) {
        return Cow::Borrowed(REDACTED);
    }

    Cow::Borrowed(value)
}

/// Check if a field name is sensitive
fn is_sensitive_field(key_lower: &str, config: &RedactionConfig) -> bool {
    // Check default sensitive fields
    if DEFAULT_SENSITIVE_FIELDS.contains(key_lower) {
        return true;
    }

    // Check additional configured fields
    if config
        .additional_fields
        .iter()
        .any(|f| f.to_lowercase() == key_lower)
    {
        return true;
    }

    // Check for common patterns in field names
    let sensitive_patterns = ["key", "token", "secret", "password", "auth", "credential"];
    for pattern in sensitive_patterns {
        if key_lower.contains(pattern) {
            return true;
        }
    }

    false
}

/// Heuristic check if a value looks like an API key
fn looks_like_api_key(value: &str) -> bool {
    // Skip if too short or too long
    if value.len() < 20 || value.len() > 200 {
        return false;
    }

    // Check for common API key prefixes
    let prefixes = [
        "sk-",     // OpenAI
        "sk_",     // Stripe
        "pk_",     // Stripe public
        "Bearer ", // Bearer tokens
        "Basic ",  // Basic auth
        "ghp_",    // GitHub
        "gho_",    // GitHub OAuth
        "glpat-",  // GitLab
        "xoxb-",   // Slack bot
        "xoxp-",   // Slack user
    ];

    for prefix in prefixes {
        if value.starts_with(prefix) {
            return true;
        }
    }

    // Check for high entropy (looks random)
    // A simple heuristic: mostly alphanumeric with some special chars
    let alphanumeric_ratio =
        value.chars().filter(|c| c.is_alphanumeric()).count() as f64 / value.len() as f64;

    // API keys are typically 80%+ alphanumeric and have mixed case
    if alphanumeric_ratio > 0.8 {
        let has_upper = value.chars().any(|c| c.is_uppercase());
        let has_lower = value.chars().any(|c| c.is_lowercase());
        let has_digit = value.chars().any(|c| c.is_ascii_digit());

        // Likely an API key if it has mixed case and digits
        if has_upper && has_lower && has_digit {
            return true;
        }
    }

    false
}

/// Redact sensitive fields in a JSON Value
pub fn redact_json_value(value: &mut Value, config: &RedactionConfig) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                let key_lower = key.to_lowercase();

                // Check if this key should be redacted
                if is_sensitive_field(&key_lower, config) {
                    *val = Value::String(REDACTED.to_string());
                } else {
                    // Recursively process nested values
                    redact_json_value(val, config);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_json_value(item, config);
            }
        }
        Value::String(s) if config.redact_by_pattern && looks_like_api_key(s) => {
            *value = Value::String(REDACTED.to_string());
        }
        _ => {}
    }
}

/// Redact sensitive headers from a list of header pairs
pub fn redact_headers(
    headers: &[(String, String)],
    config: &RedactionConfig,
) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(key, value)| {
            let redacted_value = redact_value(key, value, config);
            (key.clone(), redacted_value.into_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_sensitive_fields() {
        let config = RedactionConfig::default();

        // Should redact
        assert_eq!(redact_value("api_key", "sk-1234567890", &config), REDACTED);
        assert_eq!(
            redact_value("Authorization", "Bearer token123", &config),
            REDACTED
        );
        assert_eq!(redact_value("password", "secret123", &config), REDACTED);

        // Should not redact
        assert_eq!(redact_value("model", "gpt-4", &config), "gpt-4");
        assert_eq!(
            redact_value("message", "Hello world", &config),
            "Hello world"
        );
    }

    #[test]
    fn test_looks_like_api_key() {
        // Should detect API keys
        assert!(looks_like_api_key("sk-1234567890abcdefghij"));
        assert!(looks_like_api_key("Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6"));

        // Should not flag normal text
        assert!(!looks_like_api_key("Hello world"));
        assert!(!looks_like_api_key("short"));
        assert!(!looks_like_api_key("gpt-4-turbo"));
    }

    #[test]
    fn test_redact_json_value() {
        let config = RedactionConfig::default();
        let mut json = serde_json::json!({
            "model": "gpt-4",
            "api_key": "sk-secret123",
            "nested": {
                "token": "bearer-xyz",
                "data": "normal"
            }
        });

        redact_json_value(&mut json, &config);

        assert_eq!(json["model"], "gpt-4");
        assert_eq!(json["api_key"], REDACTED);
        assert_eq!(json["nested"]["token"], REDACTED);
        assert_eq!(json["nested"]["data"], "normal");
    }

    #[test]
    fn test_custom_fields() {
        let mut config = RedactionConfig::default();
        config.additional_fields.insert("custom_secret".to_string());

        assert_eq!(redact_value("custom_secret", "my-value", &config), REDACTED);
    }

    #[test]
    fn test_exclude_fields() {
        let mut config = RedactionConfig::default();
        config.exclude_fields.insert("api_key".to_string());

        // Normally would be redacted, but excluded
        assert_eq!(
            redact_value("api_key", "sk-1234567890", &config),
            "sk-1234567890"
        );
    }
}
