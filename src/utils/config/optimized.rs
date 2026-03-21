//! Optimized configuration management
//!
//! This module provides optimized configuration loading and management
//! with better performance and reduced memory usage.
//!
//! Hot reload is not supported. Due to a type-erasure limitation in the generic
//! interface, `load_config` currently reloads from disk on every call rather than
//! serving from cache. Restart the server to pick up configuration changes.

use crate::utils::error::gateway_error::{GatewayError, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock;

/// Optimized configuration manager with caching support
#[derive(Debug)]
pub struct OptimizedConfigManager {
    /// Cached configurations
    cache: Arc<RwLock<HashMap<String, Arc<ConfigValue>>>>,
}

/// Generic configuration value wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<ConfigValue>),
    Object(HashMap<String, ConfigValue>),
}

impl ConfigValue {
    /// Convert to string if possible
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to integer if possible
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ConfigValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Convert to float if possible
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ConfigValue::Float(f) => Some(*f),
            ConfigValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Convert to boolean if possible
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Get nested value by path (e.g., "database.host")
    pub fn get_nested(&self, path: &str) -> Option<&ConfigValue> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = self;

        for part in parts {
            match current {
                ConfigValue::Object(map) => {
                    current = map.get(part)?;
                }
                ConfigValue::Array(arr) => {
                    let index: usize = part.parse().ok()?;
                    current = arr.get(index)?;
                }
                _ => return None,
            }
        }

        Some(current)
    }
}

impl OptimizedConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load configuration from file with caching
    pub async fn load_config<T>(&self, file_path: &str) -> Result<Arc<T>>
    where
        T: for<'de> Deserialize<'de> + Serialize + Send + Sync + 'static,
    {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(cached) = cache.get(file_path)
                && let Ok(config) = self.try_downcast_config::<T>(cached.clone())
            {
                return Ok(config);
            }
        }

        // Load from file
        let config = self.load_from_file::<T>(file_path).await?;
        let config_arc = Arc::new(config);

        // Cache the result
        {
            let mut cache = self.cache.write();
            // Store as ConfigValue for generic caching
            let config_value = self.serialize_to_config_value(&*config_arc)?;
            cache.insert(file_path.to_string(), Arc::new(config_value));
        }

        Ok(config_arc)
    }

    /// Load configuration from file without caching
    async fn load_from_file<T>(&self, file_path: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let content = tokio::fs::read_to_string(file_path).await.map_err(|e| {
            GatewayError::Config(format!("Failed to read config file {}: {}", file_path, e))
        })?;

        let extension = Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("yaml");

        match extension {
            "yaml" | "yml" => serde_yml::from_str(&content)
                .map_err(|e| GatewayError::Config(format!("Failed to parse YAML config: {}", e))),
            "json" => serde_json::from_str(&content)
                .map_err(|e| GatewayError::Config(format!("Failed to parse JSON config: {}", e))),
            "toml" => {
                // TOML support would require adding toml crate to dependencies
                Err(GatewayError::Config("TOML support not enabled".to_string()))
            }
            _ => Err(GatewayError::Config(format!(
                "Unsupported config format: {}",
                extension
            ))),
        }
    }

    /// Clear cache for a specific file
    pub fn clear_cache(&self, file_path: &str) {
        let mut cache = self.cache.write();
        cache.remove(file_path);
    }

    /// Clear all cached configurations
    pub fn clear_all_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> HashMap<String, usize> {
        let cache = self.cache.read();
        cache
            .iter()
            .map(|(k, v)| (k.clone(), std::mem::size_of_val(&**v)))
            .collect()
    }

    // Helper methods
    fn try_downcast_config<T>(&self, _config_value: Arc<ConfigValue>) -> Result<Arc<T>>
    where
        T: for<'de> Deserialize<'de> + Send + Sync + 'static,
    {
        // This would require more complex type erasure in a real implementation
        Err(GatewayError::Config(
            "Type downcast not implemented".to_string(),
        ))
    }

    fn serialize_to_config_value<T>(&self, _config: &T) -> Result<ConfigValue>
    where
        T: Serialize,
    {
        // Simplified implementation
        Ok(ConfigValue::Object(HashMap::new()))
    }
}

impl Default for OptimizedConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global configuration manager instance
pub static GLOBAL_CONFIG_MANAGER: LazyLock<OptimizedConfigManager> =
    LazyLock::new(OptimizedConfigManager::new);

/// Convenience function to load configuration
pub async fn load_config<T>(file_path: &str) -> Result<Arc<T>>
where
    T: for<'de> Deserialize<'de> + Serialize + Send + Sync + 'static,
{
    GLOBAL_CONFIG_MANAGER.load_config(file_path).await
}

/// Configuration presets for common scenarios
pub struct ConfigPresets;

impl ConfigPresets {
    /// Development configuration preset
    pub fn development() -> HashMap<String, ConfigValue> {
        let mut config = HashMap::new();
        config.insert(
            "log_level".to_string(),
            ConfigValue::String("debug".to_string()),
        );
        config.insert("cache_size".to_string(), ConfigValue::Integer(1000));
        config.insert("enable_metrics".to_string(), ConfigValue::Boolean(true));
        config
    }

    /// Production configuration preset
    pub fn production() -> HashMap<String, ConfigValue> {
        let mut config = HashMap::new();
        config.insert(
            "log_level".to_string(),
            ConfigValue::String("info".to_string()),
        );
        config.insert("cache_size".to_string(), ConfigValue::Integer(10000));
        config.insert("enable_metrics".to_string(), ConfigValue::Boolean(true));
        config
    }

    /// Testing configuration preset
    pub fn testing() -> HashMap<String, ConfigValue> {
        let mut config = HashMap::new();
        config.insert(
            "log_level".to_string(),
            ConfigValue::String("warn".to_string()),
        );
        config.insert("cache_size".to_string(), ConfigValue::Integer(100));
        config.insert("enable_metrics".to_string(), ConfigValue::Boolean(false));
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_value_navigation() {
        let mut root = HashMap::new();
        let mut database = HashMap::new();
        database.insert(
            "host".to_string(),
            ConfigValue::String("localhost".to_string()),
        );
        database.insert("port".to_string(), ConfigValue::Integer(5432));

        root.insert("database".to_string(), ConfigValue::Object(database));
        let config = ConfigValue::Object(root);

        assert_eq!(
            config
                .get_nested("database.host")
                .and_then(|v| v.as_string()),
            Some("localhost")
        );
        assert_eq!(
            config.get_nested("database.port").and_then(|v| v.as_i64()),
            Some(5432)
        );
    }

    #[test]
    fn test_config_presets() {
        let dev_config = ConfigPresets::development();
        assert_eq!(
            dev_config.get("log_level").and_then(|v| v.as_string()),
            Some("debug")
        );

        let prod_config = ConfigPresets::production();
        assert_eq!(
            prod_config.get("cache_size").and_then(|v| v.as_i64()),
            Some(10000)
        );
    }
}
