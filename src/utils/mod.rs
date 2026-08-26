//! Utility modules for the LiteLLM Gateway
//!
//! This module contains various utility functions and types organized into logical modules.
//! The utilities are organized by functionality to provide better separation of concerns
//! and easier maintenance.
//!
//! ## Module Organization
//!
//! - **auth**: Authentication and security utilities
//! - **config**: Configuration management and loading
//! - **net**: Network, HTTP client, and rate limiting utilities  
//! - **ai**: AI/ML model and token management utilities
//! - **data**: Data processing, validation, and transformation utilities
//! - **logging**: Structured logging and monitoring utilities
//! - **error**: Error handling, recovery, and context management
//! - **sys**: System utilities, dependency injection, and shared state

// Core utility modules organized by functionality
pub mod ai; // AI/ML & model utilities
pub mod auth; // Authentication & security
pub mod config; // Configuration management
pub mod data; // Data processing utilities
pub mod error; // Error handling
pub mod event; // Event publish-subscribe system
pub mod logging; // Logging & monitoring
pub mod net; // Network & client utilities
pub mod sync; // Concurrent-safe value helpers
pub mod sys; // System utilities // Business logic
#[cfg(feature = "gateway")]
pub(crate) mod tls;

// Re-export commonly used types from each module for convenience
pub use ai::models::capabilities::ModelCapabilities;
pub use ai::models::utils::ModelUtils;
pub use ai::{TokenUsage, TokenUtils, TokenizerType};
pub use auth::AuthUtils;
pub use config::{ConfigDefaults, ConfigManager, ConfigUtils};
pub use data::DataUtils;
pub use error::{ErrorCategory, ErrorContext, ErrorUtils};
pub use event::{Event, EventBroker, EventType, Subscriber, SubscriptionHandle};
pub use logging::{LogEntry, LogLevel, Logger, LoggingUtils};
pub use net::client::types::{HttpClientConfig, ProviderRequestMetrics, RetryConfig};
pub use net::client::utils::ClientUtils;
pub use sync::AtomicValue;

use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Generate a unique request ID
pub fn generate_request_id() -> String {
    Uuid::new_v4().to_string()
}

/// Get current timestamp in seconds
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Get current timestamp in milliseconds
pub fn current_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Format bytes as human readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: u64 = 1024;

    if bytes < THRESHOLD {
        return format!("{} B", bytes);
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD as f64;
        unit_index += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_index])
}

/// Format duration as human readable string
pub fn format_duration(duration_ms: u64) -> String {
    if duration_ms < 1000 {
        format!("{}ms", duration_ms)
    } else if duration_ms < 60_000 {
        format!("{:.1}s", duration_ms as f64 / 1000.0)
    } else if duration_ms < 3_600_000 {
        format!("{:.1}m", duration_ms as f64 / 60_000.0)
    } else {
        format!("{:.1}h", duration_ms as f64 / 3_600_000.0)
    }
}

/// Sanitize string for logging (remove sensitive information)
pub fn sanitize_for_logging(input: &str) -> String {
    use regex::Regex;
    use std::sync::LazyLock;

    static SANITIZE_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
        vec![
            (
                Regex::new(r#"(?i)api[_-]?key["']?\s*[:=]\s*["']?([a-zA-Z0-9\-_]{20,})"#).unwrap(),
                "api_key: [REDACTED]",
            ),
            (
                Regex::new(r#"(?i)token["']?\s*[:=]\s*["']?([a-zA-Z0-9\-_\.]{20,})"#).unwrap(),
                "token: [REDACTED]",
            ),
            (
                Regex::new(r#"(?i)password["']?\s*[:=]\s*["']?([^\s"']{8,})"#).unwrap(),
                "password: [REDACTED]",
            ),
            (
                Regex::new(r#"(?i)secret["']?\s*[:=]\s*["']?([a-zA-Z0-9\-_]{16,})"#).unwrap(),
                "secret: [REDACTED]",
            ),
        ]
    });

    let mut result = input.to_string();
    for (re, replacement) in SANITIZE_PATTERNS.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }

    result
}

/// Truncate string to specified length with ellipsis
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Check if a string is a valid URL
pub fn is_valid_url(url: &str) -> bool {
    url::Url::parse(url).is_ok()
}

/// Check if a string is a valid email
pub fn is_valid_email(email: &str) -> bool {
    // Simple email validation regex
    let email_regex =
        regex::Regex::new(r#"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"#).unwrap();
    email_regex.is_match(email)
}

/// Normalize model name (remove provider prefix if present)
pub fn normalize_model_name(model: &str) -> String {
    // Remove common provider prefixes
    let prefixes = ["openai/", "anthropic/", "azure/", "google/", "bedrock/"];

    for prefix in &prefixes {
        if let Some(stripped) = model.strip_prefix(prefix) {
            return stripped.to_string();
        }
    }

    model.to_string()
}

/// Extract provider from model name
pub fn extract_provider_from_model(model: &str) -> Option<String> {
    model
        .find('/')
        .map(|slash_pos| model[..slash_pos].to_string())
}

/// Merge two JSON values
pub fn merge_json_values(base: &mut serde_json::Value, overlay: &serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                match base_map.get_mut(key) {
                    Some(base_value) => merge_json_values(base_value, value),
                    None => {
                        base_map.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base_val, overlay_val) => *base_val = overlay_val.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1500), "1.5s");
        assert_eq!(format_duration(90000), "1.5m");
        assert_eq!(format_duration(7200000), "2.0h");
    }

    #[test]
    fn test_normalize_model_name() {
        assert_eq!(normalize_model_name("openai/gpt-4"), "gpt-4");
        assert_eq!(normalize_model_name("anthropic/claude-3"), "claude-3");
        assert_eq!(normalize_model_name("gpt-4"), "gpt-4");
    }

    #[test]
    fn test_extract_provider_from_model() {
        assert_eq!(
            extract_provider_from_model("openai/gpt-4"),
            Some("openai".to_string())
        );
        assert_eq!(extract_provider_from_model("gpt-4"), None);
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
    }

    #[test]
    fn test_is_valid_url() {
        assert!(is_valid_url("https://api.openai.com/v1"));
        assert!(is_valid_url("http://localhost:8080"));
        assert!(!is_valid_url("not-a-url"));
    }

    #[test]
    fn test_is_valid_email() {
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("test.email+tag@domain.co.uk"));
        assert!(!is_valid_email("invalid-email"));
        assert!(!is_valid_email("@domain.com"));
    }
}
