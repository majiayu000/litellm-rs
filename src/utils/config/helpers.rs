//! Configuration utilities for the Gateway
//!
//! This module provides utilities for configuration management and environment handling.

use crate::utils::error::gateway_error::{GatewayError, Result};
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::LazyLock;

/// Environment variable utilities
pub struct EnvUtils;

impl EnvUtils {
    /// Get environment variable with default value
    pub fn get_env_or_default(key: &str, default: &str) -> String {
        env::var(key).unwrap_or_else(|_| default.to_string())
    }

    /// Get required environment variable
    pub fn get_required_env(key: &str) -> Result<String> {
        env::var(key).map_err(|_| {
            GatewayError::Config(format!("Required environment variable {} not found", key))
        })
    }

    /// Get environment variable as integer
    pub fn get_env_as_int(key: &str, default: i32) -> Result<i32> {
        match env::var(key) {
            Ok(value) => value.parse().map_err(|e| {
                GatewayError::Config(format!("Invalid integer value for {}: {}", key, e))
            }),
            Err(_) => Ok(default),
        }
    }

    /// Get environment variable as boolean
    pub fn get_env_as_bool(key: &str, default: bool) -> bool {
        match env::var(key) {
            Ok(value) => {
                matches!(value.to_lowercase().as_str(), "true" | "1" | "yes" | "on")
            }
            Err(_) => default,
        }
    }

    /// Get environment variable as float
    pub fn get_env_as_float(key: &str, default: f64) -> Result<f64> {
        match env::var(key) {
            Ok(value) => value.parse().map_err(|e| {
                GatewayError::Config(format!("Invalid float value for {}: {}", key, e))
            }),
            Err(_) => Ok(default),
        }
    }

    /// Get environment variable as list (comma-separated)
    pub fn get_env_as_list(key: &str, default: Vec<String>) -> Vec<String> {
        match env::var(key) {
            Ok(value) => value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            Err(_) => default,
        }
    }

    /// Check if running in development mode
    pub fn is_development() -> bool {
        Self::get_env_or_default("ENVIRONMENT", "development") == "development"
    }

    /// Check if running in production mode
    pub fn is_production() -> bool {
        Self::get_env_or_default("ENVIRONMENT", "development") == "production"
    }

    /// Get all environment variables with a prefix
    pub fn get_env_with_prefix(prefix: &str) -> HashMap<String, String> {
        env::vars()
            .filter(|(key, _)| key.starts_with(prefix))
            .map(|(key, value)| (key[prefix.len()..].to_string(), value))
            .collect()
    }

    /// Set environment variable (for testing)
    #[cfg(test)]
    pub fn set_env(key: &str, value: &str) {
        unsafe {
            env::set_var(key, value);
        }
    }

    /// Remove environment variable (for testing)
    #[cfg(test)]
    pub fn remove_env(key: &str) {
        unsafe {
            env::remove_var(key);
        }
    }
}

/// Configuration file utilities
pub struct ConfigFileUtils;

impl ConfigFileUtils {
    /// Check if file exists
    pub fn file_exists<P: AsRef<Path>>(path: P) -> bool {
        path.as_ref().exists()
    }

    /// Get file extension
    pub fn get_file_extension<P: AsRef<Path>>(path: P) -> Option<String> {
        path.as_ref()
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
    }

    /// Read file content
    pub async fn read_file<P: AsRef<Path>>(path: P) -> Result<String> {
        tokio::fs::read_to_string(path)
            .await
            .map_err(|e| GatewayError::Config(format!("Failed to read file: {}", e)))
    }

    /// Write file content
    pub async fn write_file<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
        // Create parent directories if they don't exist
        if let Some(parent) = path.as_ref().parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                GatewayError::Config(format!("Failed to create directories: {}", e))
            })?;
        }

        tokio::fs::write(path, content)
            .await
            .map_err(|e| GatewayError::Config(format!("Failed to write file: {}", e)))
    }

    /// Parse YAML file
    pub async fn parse_yaml_file<T, P>(path: P) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
        P: AsRef<Path>,
    {
        let content = Self::read_file(path).await?;
        serde_yml::from_str(&content)
            .map_err(|e| GatewayError::Config(format!("Failed to parse YAML: {}", e)))
    }

    /// Parse JSON file
    pub async fn parse_json_file<T, P>(path: P) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
        P: AsRef<Path>,
    {
        let content = Self::read_file(path).await?;
        serde_json::from_str(&content)
            .map_err(|e| GatewayError::Config(format!("Failed to parse JSON: {}", e)))
    }

    /// Write YAML file
    pub async fn write_yaml_file<T, P>(path: P, data: &T) -> Result<()>
    where
        T: serde::Serialize,
        P: AsRef<Path>,
    {
        let content = serde_yml::to_string(data)
            .map_err(|e| GatewayError::Config(format!("Failed to serialize YAML: {}", e)))?;
        Self::write_file(path, &content).await
    }

    /// Write JSON file
    pub async fn write_json_file<T, P>(path: P, data: &T) -> Result<()>
    where
        T: serde::Serialize,
        P: AsRef<Path>,
    {
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| GatewayError::Config(format!("Failed to serialize JSON: {}", e)))?;
        Self::write_file(path, &content).await
    }

    /// Find configuration file in multiple locations
    pub fn find_config_file(filename: &str) -> Option<std::path::PathBuf> {
        let search_paths = [
            std::path::PathBuf::from(filename),
            std::path::PathBuf::from(format!("config/{}", filename)),
            std::path::PathBuf::from(format!("./config/{}", filename)),
            std::path::PathBuf::from(format!("/etc/gateway/{}", filename)),
            std::path::PathBuf::from(format!("~/.config/gateway/{}", filename)),
        ];

        for path in &search_paths {
            if path.exists() {
                return Some(path.clone());
            }
        }

        None
    }
}

/// Configuration validation utilities
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate URL format
    pub fn validate_url(url: &str) -> Result<()> {
        url::Url::parse(url)
            .map_err(|e| GatewayError::Validation(format!("Invalid URL format: {}", e)))?;
        Ok(())
    }

    /// Validate email format
    pub fn validate_email(email: &str) -> Result<()> {
        static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
                .expect("email regex is a valid constant pattern")
        });

        if !EMAIL_REGEX.is_match(email) {
            return Err(GatewayError::Validation("Invalid email format".to_string()));
        }
        Ok(())
    }

    /// Validate port number
    pub fn validate_port(port: u16) -> Result<()> {
        if port == 0 {
            return Err(GatewayError::Validation("Port cannot be 0".to_string()));
        }
        Ok(())
    }

    /// Validate positive integer
    pub fn validate_positive_int(value: i32, field_name: &str) -> Result<()> {
        if value <= 0 {
            return Err(GatewayError::Validation(format!(
                "{} must be positive",
                field_name
            )));
        }
        Ok(())
    }

    /// Validate range
    pub fn validate_range<T>(value: T, min: T, max: T, field_name: &str) -> Result<()>
    where
        T: PartialOrd + std::fmt::Display,
    {
        if value < min || value > max {
            return Err(GatewayError::Validation(format!(
                "{} must be between {} and {}",
                field_name, min, max
            )));
        }
        Ok(())
    }

    /// Validate string length
    pub fn validate_string_length(
        value: &str,
        min: usize,
        max: usize,
        field_name: &str,
    ) -> Result<()> {
        let len = value.len();
        if len < min || len > max {
            return Err(GatewayError::Validation(format!(
                "{} length must be between {} and {} characters",
                field_name, min, max
            )));
        }
        Ok(())
    }

    /// Validate required field
    pub fn validate_required<T>(value: &Option<T>, field_name: &str) -> Result<()> {
        if value.is_none() {
            return Err(GatewayError::Validation(format!(
                "{} is required",
                field_name
            )));
        }
        Ok(())
    }

    /// Validate non-empty string
    pub fn validate_non_empty(value: &str, field_name: &str) -> Result<()> {
        if value.trim().is_empty() {
            return Err(GatewayError::Validation(format!(
                "{} cannot be empty",
                field_name
            )));
        }
        Ok(())
    }

    /// Validate alphanumeric string
    pub fn validate_alphanumeric(value: &str, field_name: &str) -> Result<()> {
        if !value
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(GatewayError::Validation(format!(
                "{} can only contain alphanumeric characters, underscores, and hyphens",
                field_name
            )));
        }
        Ok(())
    }

    /// Validate JSON format
    pub fn validate_json(value: &str) -> Result<()> {
        serde_json::from_str::<serde_json::Value>(value)
            .map_err(|e| GatewayError::Validation(format!("Invalid JSON format: {}", e)))?;
        Ok(())
    }

    /// Validate duration string (e.g., "30s", "5m", "1h")
    pub fn validate_duration_string(value: &str) -> Result<std::time::Duration> {
        static DURATION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"^(\d+)(s|m|h|d)$").expect("duration regex is a valid constant pattern")
        });

        if let Some(captures) = DURATION_REGEX.captures(value) {
            let number: u64 = captures[1]
                .parse()
                .map_err(|e| GatewayError::Validation(format!("Invalid duration number: {}", e)))?;

            let unit = &captures[2];
            let duration = match unit {
                "s" => std::time::Duration::from_secs(number),
                "m" => std::time::Duration::from_secs(number * 60),
                "h" => std::time::Duration::from_secs(number * 3600),
                "d" => std::time::Duration::from_secs(number * 86400),
                _ => {
                    return Err(GatewayError::Validation(
                        "Invalid duration unit".to_string(),
                    ));
                }
            };

            Ok(duration)
        } else {
            Err(GatewayError::Validation(
                "Invalid duration format. Use format like '30s', '5m', '1h', '1d'".to_string(),
            ))
        }
    }
}

#[cfg(test)]
#[path = "helpers_tests.rs"]
mod tests;
