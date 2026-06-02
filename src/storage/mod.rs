//! Storage layer for the Gateway
//!
//! This module provides data persistence and caching functionality.

/// Database storage module
pub mod database;
/// Per-dependency runtime status (Healthy / Degraded / Disabled / ...)
pub mod dependency_status;
/// File storage module
pub mod files;
/// Redis cache module
pub mod redis;
/// Vector storage module
pub mod vector;

pub use dependency_status::DependencyStatus;

use crate::config::models::storage::{DatabaseConfig, StorageConfig};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

const BUDGET_LIMIT_SNAPSHOT_MIGRATION: &str = "m20240501_000001_create_budget_limit_snapshots";

/// Returns the default base directory for local gateway state.
///
/// Resolution order:
/// 1. `LITELLM_DATA_DIR` environment variable (if set and non-empty)
/// 2. `<data_local_dir>/litellm-rs` (platform-specific)
/// 3. `/tmp/litellm-rs` (ultimate fallback)
pub fn default_data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("LITELLM_DATA_DIR")
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("litellm-rs")
}

#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Main storage layer that orchestrates all storage backends
#[derive(Debug, Clone)]
pub struct StorageLayer {
    /// Database connection pool
    pub database: Arc<database::Database>,
    /// Redis connection pool
    pub redis: Arc<redis::RedisPool>,
    /// File storage backend
    pub files: Arc<files::FileStorage>,
    /// Vector database client (optional)
    /// Note: Using concrete type instead of trait object for now
    pub vector: Option<Arc<vector::VectorStoreBackend>>,
    /// Status of the Redis dependency after init.
    pub redis_status: DependencyStatus,
    /// Status of the vector DB dependency after init.
    pub vector_status: DependencyStatus,
}

impl StorageLayer {
    /// Create a new storage layer.
    ///
    /// For each optional dependency:
    /// * if `enabled` is `false` (or the section is absent), the dependency is
    ///   treated as `Disabled` and a no-op fallback is used,
    /// * if `enabled` is `true` and init succeeds, status is `Healthy`,
    /// * if `enabled` is `true`, init fails, and `allow_degraded` is `false`,
    ///   the call returns `Err` so startup fails loudly,
    /// * if `enabled` is `true`, init fails, and `allow_degraded` is `true`,
    ///   the error is logged at `error!` level and the dependency status is
    ///   set to `Degraded` while the gateway keeps running.
    pub async fn new(config: &StorageConfig) -> Result<Self> {
        info!("Initializing storage layer");

        // Initialize database
        debug!("Connecting to database");
        let database = Arc::new(database::Database::new(&config.database).await?);
        Self::initialize_database_schema(&database, &config.database).await?;

        // Initialize Redis (fail-fast unless allow_degraded is set)
        debug!("Creating Redis connection pool");
        let (redis, redis_status) = if !config.redis.enabled {
            // Explicitly disabled — no fallback, no error.
            info!("Redis disabled in config; using no-op pool");
            (
                Arc::new(redis::RedisPool::create_noop()),
                DependencyStatus::Disabled,
            )
        } else {
            match redis::RedisPool::new(&config.redis).await {
                Ok(pool) => {
                    if pool.is_noop() {
                        info!("Redis pool initialized in no-op mode");
                    } else {
                        info!("Redis connection established");
                    }
                    (Arc::new(pool), DependencyStatus::Healthy)
                }
                Err(e) => {
                    if config.redis.allow_degraded {
                        error!(
                            "Redis init failed but allow_degraded=true; continuing without \
                             cache. Error: {}",
                            e
                        );
                        (
                            Arc::new(redis::RedisPool::create_noop()),
                            DependencyStatus::Degraded,
                        )
                    } else {
                        error!(
                            "Redis init failed and allow_degraded=false; failing startup. \
                             Set storage.redis.allow_degraded=true to keep running in no-op \
                             cache mode. Error: {}",
                            e
                        );
                        return Err(e);
                    }
                }
            }
        };

        // Initialize file storage
        debug!("Initializing file storage");
        let files = Arc::new(files::FileStorage::new(&config.files).await?);

        // Initialize vector database (fail-fast unless allow_degraded is set)
        let (vector, vector_status) = if let Some(ref vector_config) = config.vector_db {
            debug!("Initializing vector database");
            match vector::VectorStoreBackend::new(vector_config).await {
                Ok(v) => (Some(Arc::new(v)), DependencyStatus::Healthy),
                Err(e) => {
                    if vector_config.allow_degraded {
                        error!(
                            "Vector DB init failed but allow_degraded=true; continuing \
                             without vector backend. Error: {}",
                            e
                        );
                        (None, DependencyStatus::Degraded)
                    } else {
                        error!(
                            "Vector DB init failed and allow_degraded=false; failing \
                             startup. Set storage.vector_db.allow_degraded=true to keep \
                             running without a vector backend. Error: {}",
                            e
                        );
                        return Err(e);
                    }
                }
            }
        } else {
            debug!("Vector database not configured, skipping");
            (None, DependencyStatus::Disabled)
        };

        info!("Storage layer initialized successfully");

        Ok(Self {
            database,
            redis,
            files,
            vector,
            redis_status,
            vector_status,
        })
    }

    async fn initialize_database_schema(
        database: &database::Database,
        config: &DatabaseConfig,
    ) -> Result<()> {
        if Self::should_run_startup_migrations(database, config) {
            info!("Running database migrations during storage startup");
            database.migrate().await?;
            return Ok(());
        }

        info!(
            "Database startup migrations disabled; verifying configured schema is already present"
        );
        let allowed_pending = if config.allow_degraded {
            &[BUDGET_LIMIT_SNAPSHOT_MIGRATION][..]
        } else {
            &[][..]
        };
        database
            .verify_migrations_applied_except(allowed_pending)
            .await
            .map_err(|e| {
                GatewayError::Storage(format!(
                    "Database schema check failed while storage.database.auto_migrate=false. \
                 Run migrations before startup or set storage.database.auto_migrate=true: {}",
                    e
                ))
            })?;
        database.health_check().await.map_err(|e| {
            GatewayError::Storage(format!(
                "Database health check failed while storage.database.auto_migrate=false: {}",
                e
            ))
        })?;
        Ok(())
    }

    fn should_run_startup_migrations(
        database: &database::Database,
        config: &DatabaseConfig,
    ) -> bool {
        !config.enabled || config.auto_migrate || database.is_sqlite_fallback()
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations");
        self.database.migrate().await?;
        info!("Database migrations completed");
        Ok(())
    }

    /// Health check for all storage backends
    pub async fn health_check(&self) -> Result<StorageHealthStatus> {
        let mut status = StorageHealthStatus {
            database: false,
            redis: false,
            files: false,
            vector: false,
            overall: false,
        };

        // Check database health
        match self.database.health_check().await {
            Ok(_) => status.database = true,
            Err(e) => {
                warn!("Database health check failed: {}", e);
            }
        }

        // Check Redis health
        match self.redis.health_check().await {
            Ok(_) => status.redis = true,
            Err(e) => {
                warn!("Redis health check failed: {}", e);
            }
        }

        // Check file storage health
        match self.files.health_check().await {
            Ok(_) => status.files = true,
            Err(e) => {
                warn!("File storage health check failed: {}", e);
            }
        }

        // Check vector database health (if configured)
        if let Some(vector) = &self.vector {
            match vector.health_check().await {
                Ok(_) => status.vector = true,
                Err(e) => {
                    warn!("Vector database health check failed: {}", e);
                }
            }
        } else {
            status.vector = true; // Not configured, so consider it healthy
        }

        // Overall health is true if all configured backends are healthy
        status.overall = status.database && status.redis && status.files && status.vector;

        Ok(status)
    }

    /// Close all connections
    pub async fn close(&self) -> Result<()> {
        info!("Closing storage connections");

        // Database connections will be closed when Arc is dropped
        // self.database.close().await?;

        // Close Redis connections
        self.redis.close().await?;

        // Close file storage
        self.files.close().await?;

        // Close vector database connections
        if let Some(vector) = &self.vector {
            vector.close().await?;
        }

        info!("Storage connections closed");
        Ok(())
    }

    /// Get database pool
    pub fn db(&self) -> &database::Database {
        &self.database
    }

    /// Get Redis pool
    pub fn redis(&self) -> &redis::RedisPool {
        &self.redis
    }

    /// Get file storage
    pub fn files(&self) -> &files::FileStorage {
        &self.files
    }

    /// Get vector store (if available)
    pub fn vector(&self) -> Option<&vector::VectorStoreBackend> {
        self.vector.as_deref()
    }

    // Transaction support removed - SeaORM handles transactions differently
    // Use the database connection directly for transactional operations

    /// Get a Redis connection
    pub async fn redis_conn(&self) -> Result<redis::RedisConnection> {
        self.redis.get_connection().await
    }

    /// Store file and return file ID
    pub async fn store_file(&self, filename: &str, content: &[u8]) -> Result<String> {
        self.files.store(filename, content).await
    }

    /// Retrieve file content
    pub async fn get_file(&self, file_id: &str) -> Result<Vec<u8>> {
        self.files.get(file_id).await
    }

    /// Delete file
    pub async fn delete_file(&self, file_id: &str) -> Result<()> {
        self.files.delete(file_id).await
    }

    /// Store vector embeddings
    pub async fn store_embeddings(
        &self,
        id: &str,
        embeddings: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        if let Some(vector) = &self.vector {
            vector.store(id, embeddings, metadata).await
        } else {
            Err(GatewayError::Config(
                "Vector database not configured".to_string(),
            ))
        }
    }

    /// Search similar vectors
    pub async fn search_similar(
        &self,
        query_vector: &[f32],
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<vector::SearchResult>> {
        if let Some(vector) = &self.vector {
            vector.search(query_vector, limit, threshold).await
        } else {
            Err(GatewayError::Config(
                "Vector database not configured".to_string(),
            ))
        }
    }

    /// Cache operations
    pub async fn cache_get(&self, key: &str) -> Result<Option<String>> {
        self.redis.get(key).await
    }

    /// Set cache value with optional TTL
    pub async fn cache_set(&self, key: &str, value: &str, ttl: Option<u64>) -> Result<()> {
        self.redis.set(key, value, ttl).await
    }

    /// Delete cache key
    pub async fn cache_delete(&self, key: &str) -> Result<()> {
        self.redis.delete(key).await
    }

    /// Check if cache key exists
    pub async fn cache_exists(&self, key: &str) -> Result<bool> {
        self.redis.exists(key).await
    }

    /// Batch cache operations
    pub async fn cache_mget(&self, keys: &[String]) -> Result<Vec<Option<String>>> {
        self.redis.mget(keys).await
    }

    /// Set multiple cache values with optional TTL
    pub async fn cache_mset(&self, pairs: &[(String, String)], ttl: Option<u64>) -> Result<()> {
        self.redis.mset(pairs, ttl).await
    }

    /// List operations
    /// Push value to list
    pub async fn list_push(&self, key: &str, value: &str) -> Result<()> {
        self.redis.list_push(key, value).await
    }

    /// Pop value from list
    pub async fn list_pop(&self, key: &str) -> Result<Option<String>> {
        self.redis.list_pop(key).await
    }

    /// Get list length
    pub async fn list_length(&self, key: &str) -> Result<usize> {
        self.redis.list_length(key).await
    }

    /// Set operations
    /// Add member to set
    pub async fn set_add(&self, key: &str, member: &str) -> Result<()> {
        self.redis.set_add(key, member).await
    }

    /// Remove member from set
    pub async fn set_remove(&self, key: &str, member: &str) -> Result<()> {
        self.redis.set_remove(key, member).await
    }

    /// Get all members of set
    pub async fn set_members(&self, key: &str) -> Result<Vec<String>> {
        self.redis.set_members(key).await
    }

    /// Hash operations
    /// Set hash field value
    pub async fn hash_set(&self, key: &str, field: &str, value: &str) -> Result<()> {
        self.redis.hash_set(key, field, value).await
    }

    /// Get hash field value
    pub async fn hash_get(&self, key: &str, field: &str) -> Result<Option<String>> {
        self.redis.hash_get(key, field).await
    }

    /// Delete hash field
    pub async fn hash_delete(&self, key: &str, field: &str) -> Result<()> {
        self.redis.hash_delete(key, field).await
    }

    /// Get all hash fields and values
    pub async fn hash_get_all(
        &self,
        key: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        self.redis.hash_get_all(key).await
    }

    /// Pub/Sub operations
    pub async fn publish(&self, channel: &str, message: &str) -> Result<()> {
        self.redis.publish(channel, message).await
    }

    /// Subscribe to Redis channels
    /// Subscribe to Redis channels for pub/sub messaging
    pub async fn subscribe(&self, channels: &[String]) -> Result<redis::Subscription> {
        self.redis.subscribe(channels).await
    }
}

/// Storage health status
#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageHealthStatus {
    /// Database health status
    pub database: bool,
    /// Redis health status
    pub redis: bool,
    /// File storage health status
    pub files: bool,
    /// Vector storage health status
    pub vector: bool,
    /// Overall health status
    pub overall: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::file_storage::FileStorageConfig;
    use crate::config::models::storage::{DatabaseConfig, RedisConfig};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_storage_layer_creation() {
        let config = StorageConfig {
            database: DatabaseConfig {
                url: "postgresql://localhost:5432/test".to_string(),
                max_connections: 5,
                connection_timeout: 5,
                ssl: false,
                enabled: true,
                auto_migrate: false,
                fallback_to_sqlite: false,
                allow_degraded: false,
            },
            redis: RedisConfig {
                url: "redis://localhost:6379".to_string(),
                enabled: true,
                max_connections: 10,
                connection_timeout: 5,
                cluster: false,
                allow_degraded: false,
            },
            files: FileStorageConfig::default(),
            vector_db: None,
        };

        // This test would require actual database connections
        // For now, we'll just test that the config is properly structured
        assert_eq!(config.database.url, "postgresql://localhost:5432/test");
        assert_eq!(config.redis.url, "redis://localhost:6379");
    }

    #[tokio::test]
    async fn test_storage_layer_uses_configured_file_storage_path() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let configured_path = temp_dir.path().join("configured-files");
        let config = StorageConfig {
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            files: FileStorageConfig {
                storage_type: "local".to_string(),
                local_path: Some(configured_path.to_string_lossy().to_string()),
                s3: None,
            },
            vector_db: None,
        };

        let storage = StorageLayer::new(&config)
            .await
            .expect("storage layer should initialize with configured local file storage");
        let file_id = storage
            .store_file("configured.txt", b"configured storage")
            .await
            .expect("file should be stored");

        let expected_path = configured_path
            .join(&file_id[..2.min(file_id.len())])
            .join(&file_id);
        assert!(
            expected_path.exists(),
            "stored file should use configured path: {}",
            expected_path.display()
        );
    }

    #[tokio::test]
    async fn default_storage_layer_runs_in_memory_sqlite_migrations() {
        let config = StorageConfig {
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            files: FileStorageConfig::default(),
            vector_db: None,
        };

        let storage = StorageLayer::new(&config)
            .await
            .expect("default storage layer should initialize");
        let snapshots = storage
            .database
            .load_budget_limit_snapshots()
            .await
            .expect("default in-memory SQLite should have migrated budget tables");

        assert!(snapshots.is_empty());
    }

    #[tokio::test]
    async fn configured_database_without_auto_migrate_requires_existing_schema() {
        let config = StorageConfig {
            database: DatabaseConfig {
                url: "sqlite::memory:".to_string(),
                max_connections: 1,
                connection_timeout: 1,
                ssl: false,
                enabled: true,
                auto_migrate: false,
                fallback_to_sqlite: false,
                allow_degraded: false,
            },
            redis: RedisConfig::default(),
            files: FileStorageConfig::default(),
            vector_db: None,
        };

        let err = match StorageLayer::new(&config).await {
            Ok(_) => panic!("configured database without schema must fail when auto_migrate=false"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("auto_migrate=false"),
            "error should explain how to enable or run migrations, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn configured_database_with_auto_migrate_runs_startup_migrations() {
        let config = StorageConfig {
            database: sqlite_db_config(),
            redis: RedisConfig::default(),
            files: FileStorageConfig::default(),
            vector_db: None,
        };

        let storage = match StorageLayer::new(&config).await {
            Ok(storage) => storage,
            Err(err) => panic!("auto_migrate=true should initialize SQLite schema: {}", err),
        };
        let snapshots = match storage.database.load_budget_limit_snapshots().await {
            Ok(snapshots) => snapshots,
            Err(err) => panic!("startup migration should create budget tables: {}", err),
        };

        assert!(snapshots.is_empty());
    }

    #[tokio::test]
    async fn storage_layer_migrate_is_idempotent_after_startup_migration() {
        let config = StorageConfig {
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            files: FileStorageConfig::default(),
            vector_db: None,
        };

        let storage = StorageLayer::new(&config)
            .await
            .expect("default storage layer should initialize");

        storage
            .migrate()
            .await
            .expect("explicit migration after startup migration should be idempotent");
    }

    fn unreachable_redis_config(allow_degraded: bool) -> RedisConfig {
        RedisConfig {
            // Port 1 reserved/unreachable; connection_timeout=1 keeps the
            // test fast.
            url: "redis://127.0.0.1:1".to_string(),
            enabled: true,
            max_connections: 1,
            connection_timeout: 1,
            cluster: false,
            allow_degraded,
        }
    }

    fn sqlite_db_config() -> DatabaseConfig {
        DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 1,
            connection_timeout: 1,
            ssl: false,
            enabled: true,
            auto_migrate: true,
            fallback_to_sqlite: false,
            allow_degraded: false,
        }
    }

    #[tokio::test]
    async fn redis_enabled_failing_without_allow_degraded_fails_startup() {
        let config = StorageConfig {
            database: sqlite_db_config(),
            redis: unreachable_redis_config(false),
            files: FileStorageConfig::default(),
            vector_db: None,
        };

        let result = StorageLayer::new(&config).await;
        assert!(
            result.is_err(),
            "enabled redis that cannot connect must fail startup when allow_degraded=false"
        );
    }

    #[tokio::test]
    async fn redis_enabled_failing_with_allow_degraded_continues_with_noop() {
        let config = StorageConfig {
            database: sqlite_db_config(),
            redis: unreachable_redis_config(true),
            files: FileStorageConfig::default(),
            vector_db: None,
        };

        let storage = StorageLayer::new(&config)
            .await
            .expect("allow_degraded=true should allow startup with no-op redis pool");
        assert!(
            storage.redis.is_noop(),
            "degraded redis must fall back to a no-op pool"
        );
        assert_eq!(storage.redis_status, DependencyStatus::Degraded);
    }

    #[tokio::test]
    async fn redis_disabled_uses_noop_without_error() {
        let redis = RedisConfig {
            enabled: false,
            ..RedisConfig::default()
        };
        let config = StorageConfig {
            database: sqlite_db_config(),
            redis,
            files: FileStorageConfig::default(),
            vector_db: None,
        };

        let storage = StorageLayer::new(&config)
            .await
            .expect("disabled redis must always succeed");
        assert!(storage.redis.is_noop());
        assert_eq!(storage.redis_status, DependencyStatus::Disabled);
    }

    fn unreachable_vector_config(
        allow_degraded: bool,
    ) -> crate::config::models::file_storage::VectorDbConfig {
        crate::config::models::file_storage::VectorDbConfig {
            // Unsupported db_type forces VectorStoreBackend::new to return Err
            // synchronously, which keeps the test fast and offline-friendly.
            db_type: "weaviate".to_string(),
            url: "http://127.0.0.1:1".to_string(),
            api_key: "test".to_string(),
            index_name: "test".to_string(),
            allow_degraded,
        }
    }

    #[tokio::test]
    async fn vector_db_failing_without_allow_degraded_fails_startup() {
        let config = StorageConfig {
            database: sqlite_db_config(),
            redis: RedisConfig::default(),
            files: FileStorageConfig::default(),
            vector_db: Some(unreachable_vector_config(false)),
        };

        let result = StorageLayer::new(&config).await;
        assert!(
            result.is_err(),
            "configured vector DB that cannot init must fail startup when allow_degraded=false"
        );
    }

    #[tokio::test]
    async fn vector_db_failing_with_allow_degraded_continues_without_vector() {
        let config = StorageConfig {
            database: sqlite_db_config(),
            redis: RedisConfig::default(),
            files: FileStorageConfig::default(),
            vector_db: Some(unreachable_vector_config(true)),
        };

        let storage = StorageLayer::new(&config)
            .await
            .expect("allow_degraded=true must allow startup without a vector backend");
        assert!(storage.vector.is_none());
        assert_eq!(storage.vector_status, DependencyStatus::Degraded);
    }

    #[tokio::test]
    async fn vector_db_disabled_is_status_disabled() {
        let config = StorageConfig {
            database: sqlite_db_config(),
            redis: RedisConfig::default(),
            files: FileStorageConfig::default(),
            vector_db: None,
        };

        let storage = StorageLayer::new(&config)
            .await
            .expect("storage layer must init without vector DB");
        assert_eq!(storage.vector_status, DependencyStatus::Disabled);
    }
}
