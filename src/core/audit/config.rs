//! Configuration for the Audit Logging system

use crate::config::models::defaults::default_true;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::types::LogLevel;

/// Main configuration for the audit logging system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Whether audit logging is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Minimum log level
    #[serde(default)]
    pub min_level: LogLevel,

    /// Whether to log requests
    #[serde(default = "default_true")]
    pub log_requests: bool,

    /// Whether to log responses
    #[serde(default = "default_true")]
    pub log_responses: bool,

    /// Whether to log user actions
    #[serde(default = "default_true")]
    pub log_user_actions: bool,

    /// Whether to log request bodies
    #[serde(default)]
    pub log_request_body: bool,

    /// Whether to log response bodies
    #[serde(default)]
    pub log_response_body: bool,

    /// Maximum body size to log (bytes)
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,

    /// Headers to include in logs (empty = none)
    #[serde(default)]
    pub include_headers: Vec<String>,

    /// Headers to exclude from logs
    #[serde(default = "default_excluded_headers")]
    pub exclude_headers: Vec<String>,

    /// Paths to exclude from logging
    #[serde(default = "default_excluded_paths")]
    pub exclude_paths: Vec<String>,

    /// File output configuration
    #[serde(default)]
    pub file_output: Option<FileOutputConfig>,

    /// Buffer size for async logging
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,

    /// Flush interval in milliseconds
    #[serde(default = "default_flush_interval")]
    pub flush_interval_ms: u64,

    /// Retention period in days (0 = no retention)
    #[serde(default)]
    pub retention_days: u32,

    /// Whether to redact sensitive data
    #[serde(default = "default_true")]
    pub redact_sensitive: bool,

    /// Patterns to redact
    #[serde(default = "default_redact_patterns")]
    pub redact_patterns: Vec<String>,
}

fn default_max_body_size() -> usize {
    10 * 1024 // 10KB
}

fn default_excluded_headers() -> Vec<String> {
    vec![
        "authorization".to_string(),
        "x-api-key".to_string(),
        "cookie".to_string(),
        "set-cookie".to_string(),
    ]
}

fn default_excluded_paths() -> Vec<String> {
    vec![
        "/health".to_string(),
        "/metrics".to_string(),
        "/ready".to_string(),
        "/live".to_string(),
    ]
}

fn default_buffer_size() -> usize {
    1000
}

fn default_flush_interval() -> u64 {
    1000 // 1 second
}

fn default_redact_patterns() -> Vec<String> {
    vec![
        r#"sk-[a-zA-Z0-9_-]{20,}"#.to_string(),
        r#"sk-ant-[a-zA-Z0-9_-]{20,}"#.to_string(),
        r#"gw-[a-zA-Z0-9_-]{20,}"#.to_string(),
        r#"AKIA[0-9A-Z]{16}"#.to_string(),
        r#"(?i)bearer\s+[a-z0-9._~+/=-]{10,}"#.to_string(),
        r#"eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+"#.to_string(),
        r#"(?i)authorization["']?\s*[:=]\s*["']?[^"',\s}]+"#.to_string(),
        r#"api[_-]?key["']?\s*[:=]\s*["']?[a-zA-Z0-9-_]+"#.to_string(),
    ]
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_level: LogLevel::Info,
            log_requests: true,
            log_responses: true,
            log_user_actions: true,
            log_request_body: false,
            log_response_body: false,
            max_body_size: default_max_body_size(),
            include_headers: Vec::new(),
            exclude_headers: default_excluded_headers(),
            exclude_paths: default_excluded_paths(),
            file_output: None,
            buffer_size: default_buffer_size(),
            flush_interval_ms: default_flush_interval(),
            retention_days: 0,
            redact_sensitive: true,
            redact_patterns: default_redact_patterns(),
        }
    }
}

impl AuditConfig {
    /// Create a new audit config
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable audit logging
    pub fn enable(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Set minimum log level
    pub fn with_min_level(mut self, level: LogLevel) -> Self {
        self.min_level = level;
        self
    }

    /// Enable file output
    pub fn with_file_output(mut self, path: impl Into<PathBuf>) -> Self {
        self.file_output = Some(FileOutputConfig {
            path: path.into(),
            ..Default::default()
        });
        self
    }

    /// Enable request body logging
    pub fn with_request_body(mut self, enabled: bool) -> Self {
        self.log_request_body = enabled;
        self
    }

    /// Enable response body logging
    pub fn with_response_body(mut self, enabled: bool) -> Self {
        self.log_response_body = enabled;
        self
    }

    /// Set max body size
    pub fn with_max_body_size(mut self, size: usize) -> Self {
        self.max_body_size = size;
        self
    }

    /// Add excluded path
    pub fn exclude_path(mut self, path: impl Into<String>) -> Self {
        self.exclude_paths.push(path.into());
        self
    }

    /// Set retention days
    pub fn with_retention_days(mut self, days: u32) -> Self {
        self.retention_days = days;
        self
    }

    /// Check if a path should be excluded
    pub fn is_path_excluded(&self, path: &str) -> bool {
        self.exclude_paths.iter().any(|p| path.starts_with(p))
    }

    /// Check if a header should be excluded
    pub fn is_header_excluded(&self, header: &str) -> bool {
        let header_lower = header.to_lowercase();
        self.exclude_headers
            .iter()
            .any(|h| h.to_lowercase() == header_lower)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.buffer_size == 0 {
            return Err("audit buffer_size must be greater than zero".to_string());
        }
        if self.flush_interval_ms == 0 {
            return Err("audit flush_interval_ms must be greater than zero".to_string());
        }
        Ok(())
    }
}

/// File output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOutputConfig {
    /// Path to the log file
    pub path: PathBuf,

    /// Whether to rotate logs
    #[serde(default = "default_true")]
    pub rotate: bool,

    /// Maximum file size before rotation (bytes)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Maximum number of backup files
    #[serde(default = "default_max_backups")]
    pub max_backups: u32,

    /// Whether to compress rotated files
    #[serde(default)]
    pub compress: bool,
}

fn default_max_file_size() -> u64 {
    100 * 1024 * 1024 // 100MB
}

fn default_max_backups() -> u32 {
    10
}

impl Default for FileOutputConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from(r"./logs/audit.log"),
            rotate: true,
            max_file_size: default_max_file_size(),
            max_backups: default_max_backups(),
            compress: false,
        }
    }
}

impl FileOutputConfig {
    /// Create a new file output config
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            ..Default::default()
        }
    }

    /// Set max file size
    pub fn with_max_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    /// Set max backups
    pub fn with_max_backups(mut self, count: u32) -> Self {
        self.max_backups = count;
        self
    }

    /// Enable compression
    pub fn with_compression(mut self, enabled: bool) -> Self {
        self.compress = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_config_default() {
        let config = AuditConfig::default();
        assert!(!config.enabled);
        assert!(config.log_requests);
        assert!(config.log_responses);
        assert!(!config.log_request_body);
        assert!(config.redact_sensitive);
    }

    #[test]
    fn test_audit_config_builder() {
        let config = AuditConfig::new()
            .enable()
            .with_min_level(LogLevel::Debug)
            .with_file_output(r"./logs/test.log")
            .with_request_body(true)
            .with_response_body(true)
            .with_max_body_size(1024)
            .with_retention_days(30)
            .exclude_path(r"/internal");

        assert!(config.enabled);
        assert_eq!(config.min_level, LogLevel::Debug);
        assert!(config.file_output.is_some());
        assert!(config.log_request_body);
        assert!(config.log_response_body);
        assert_eq!(config.max_body_size, 1024);
        assert_eq!(config.retention_days, 30);
    }

    #[test]
    fn test_path_exclusion() {
        let config = AuditConfig::default();
        assert!(config.is_path_excluded(r"/health"));
        assert!(config.is_path_excluded(r"/health/live"));
        assert!(config.is_path_excluded(r"/metrics"));
        assert!(!config.is_path_excluded(r"/v1/chat/completions"));
    }

    #[test]
    fn test_header_exclusion() {
        let config = AuditConfig::default();
        assert!(config.is_header_excluded("Authorization"));
        assert!(config.is_header_excluded("authorization"));
        assert!(config.is_header_excluded("X-API-Key"));
        assert!(!config.is_header_excluded("Content-Type"));
    }

    #[test]
    fn test_default_redact_patterns_compile() {
        for pattern in default_redact_patterns() {
            regex::Regex::new(&pattern).unwrap_or_else(|error| {
                panic!("default redact pattern should compile: {pattern}: {error}")
            });
        }
    }

    #[test]
    fn test_file_output_config() {
        let config = FileOutputConfig::new(r"./logs/audit.log")
            .with_max_size(50 * 1024 * 1024)
            .with_max_backups(5)
            .with_compression(true);

        assert_eq!(config.path, PathBuf::from(r"./logs/audit.log"));
        assert_eq!(config.max_file_size, 50 * 1024 * 1024);
        assert_eq!(config.max_backups, 5);
        assert!(config.compress);
    }

    #[test]
    fn test_config_validation() {
        let config = AuditConfig::default();
        assert!(config.validate().is_ok());

        let enabled_config = AuditConfig::new().enable();
        assert!(enabled_config.validate().is_ok());

        let mut invalid = AuditConfig::new().enable();
        invalid.buffer_size = 1;
        assert!(invalid.validate().is_ok());
        invalid.buffer_size = 0;
        assert!(invalid.validate().is_err());
        invalid.buffer_size = 1;
        invalid.flush_interval_ms = 0;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = AuditConfig::new()
            .enable()
            .with_file_output(r"./logs/test.log");

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AuditConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.enabled, deserialized.enabled);
        assert!(deserialized.file_output.is_some());
    }
}
