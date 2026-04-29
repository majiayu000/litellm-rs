//! Storage configuration validators
//!
//! This module provides validation implementations for storage-related configuration
//! structures including StorageConfig, DatabaseConfig, RedisConfig, and VectorDbConfig.

use super::trait_def::Validate;
use crate::config::models::file_storage::VectorDbConfig;
use crate::config::models::storage::{DatabaseConfig, RedisConfig, StorageConfig};
use tracing::debug;

impl Validate for StorageConfig {
    fn validate(&self) -> Result<(), String> {
        debug!("Validating storage configuration");

        if self.database.enabled {
            self.database.validate()?;
        }
        if self.redis.enabled {
            self.redis.validate()?;
        }

        if let Some(vector_db) = &self.vector_db {
            vector_db.validate()?;
        }

        Ok(())
    }
}

impl Validate for DatabaseConfig {
    fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        if self.url.is_empty() {
            return Err("Database URL cannot be empty".to_string());
        }

        if !self.url.starts_with("postgresql://") && !self.url.starts_with("postgres://") {
            return Err("Only PostgreSQL databases are supported".to_string());
        }

        if self.max_connections == 0 {
            return Err("Database max connections must be greater than 0".to_string());
        }

        if self.max_connections > 1000 {
            return Err("Database max connections should not exceed 1000".to_string());
        }

        if self.connection_timeout == 0 {
            return Err("Database connection timeout must be greater than 0".to_string());
        }

        Ok(())
    }
}

impl Validate for RedisConfig {
    fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        if self.url.is_empty() {
            return Err("Redis URL cannot be empty".to_string());
        }

        if !self.url.starts_with("redis://") && !self.url.starts_with("rediss://") {
            return Err("Redis URL must start with redis:// or rediss://".to_string());
        }

        if self.max_connections == 0 {
            return Err("Redis max connections must be greater than 0".to_string());
        }

        if self.connection_timeout == 0 {
            return Err("Redis connection timeout must be greater than 0".to_string());
        }

        Ok(())
    }
}

impl Validate for VectorDbConfig {
    fn validate(&self) -> Result<(), String> {
        // Only Qdrant has a complete runtime implementation today. Weaviate
        // and Pinecone modules are placeholders and should be rejected during
        // config validation instead of failing later at runtime.
        let implemented_types = ["qdrant"];
        let planned_types = ["weaviate", "pinecone"];
        if planned_types.contains(&self.db_type.as_str()) {
            return Err(format!(
                "Vector DB type '{}' is declared but not implemented yet. Implemented types: {:?}",
                self.db_type, implemented_types
            ));
        }
        if !implemented_types.contains(&self.db_type.as_str()) {
            return Err(format!(
                "Unsupported vector DB type: {}. Implemented types: {:?}",
                self.db_type, implemented_types
            ));
        }

        if self.url.is_empty() {
            return Err("Vector DB URL cannot be empty".to_string());
        }

        if self.index_name.is_empty() {
            return Err("Vector DB index name cannot be empty".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector_config(db_type: &str) -> VectorDbConfig {
        VectorDbConfig {
            db_type: db_type.to_string(),
            url: "http://localhost:6333".to_string(),
            api_key: "test-key".to_string(),
            index_name: "vectors".to_string(),
        }
    }

    #[test]
    fn vector_validation_accepts_qdrant() {
        assert!(vector_config("qdrant").validate().is_ok());
    }

    #[test]
    fn vector_validation_rejects_placeholder_backends() {
        for db_type in ["pinecone", "weaviate"] {
            let err = vector_config(db_type)
                .validate()
                .expect_err("placeholder vector backend must fail validation");
            assert!(err.contains("not implemented yet"));
        }
    }
}
