#![cfg(feature = "storage")]
//! Database integration tests
//!
//! Tests database operations using real in-memory SQLite database.

#[cfg(test)]
mod tests {
    use litellm_rs::config::models::file_storage::FileStorageConfig;
    use litellm_rs::config::models::storage::DatabaseConfig;
    use litellm_rs::config::models::storage::{RedisConfig, StorageConfig};
    use litellm_rs::core::models::user::types::User;
    use litellm_rs::core::models::{ApiKey, Metadata, RateLimits, UsageStats};
    use litellm_rs::storage::StorageLayer;
    use litellm_rs::storage::database::{Database, DatabaseBackendType, migration::Migrator};
    use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, Statement, Value};
    use sea_orm_migration::{
        MigratorTrait, SchemaManager,
        prelude::{Alias, Table},
        seaql_migrations,
    };
    use tempfile::TempDir;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    static ENV_LOCK: Mutex<()> = Mutex::const_new(());

    struct EnvRestore {
        sqlite_path: Option<String>,
    }

    impl EnvRestore {
        fn capture() -> Self {
            Self {
                sqlite_path: std::env::var("LITELLM_SQLITE_PATH").ok(),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            unsafe {
                match &self.sqlite_path {
                    Some(value) => std::env::set_var("LITELLM_SQLITE_PATH", value),
                    None => std::env::remove_var("LITELLM_SQLITE_PATH"),
                }
            }
        }
    }

    fn sqlite_file_db_config(temp_dir: &TempDir, auto_migrate: bool) -> DatabaseConfig {
        DatabaseConfig {
            url: format!(
                "sqlite://{}?mode=rwc",
                temp_dir.path().join("gateway.db").display()
            ),
            max_connections: 1,
            connection_timeout: 1,
            ssl: false,
            enabled: true,
            auto_migrate,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        }
    }

    fn storage_config(database: DatabaseConfig) -> StorageConfig {
        StorageConfig {
            database,
            redis: RedisConfig::default(),
            files: FileStorageConfig::default(),
            vector_db: None,
        }
    }

    /// Test basic database connection and health check
    #[tokio::test]
    async fn test_database_health_check() {
        let config = DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 1,
            connection_timeout: 5,
            ssl: false,
            enabled: true,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };

        let db = Database::new(&config).await;
        assert!(db.is_ok(), "Failed to create database: {:?}", db.err());

        let db = db.unwrap();

        // Run migrations first to create required tables
        let migrate_result = db.migrate().await;
        assert!(
            migrate_result.is_ok(),
            "Migration failed: {:?}",
            migrate_result.err()
        );

        let health = db.health_check().await;
        assert!(health.is_ok(), "Health check failed: {:?}", health.err());
    }

    /// Test database migration
    #[tokio::test]
    async fn test_database_migration() {
        let config = DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 1,
            connection_timeout: 5,
            ssl: false,
            enabled: true,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };

        let db = Database::new(&config)
            .await
            .expect("Failed to create database");
        let result = db.migrate().await;
        assert!(result.is_ok(), "Migration failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_pricing_tables_created_by_migration() {
        let config = DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 1,
            connection_timeout: 5,
            ssl: false,
            enabled: true,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };

        let db = Database::new(&config)
            .await
            .expect("Failed to create database");
        db.migrate().await.expect("Migration failed");

        for table in ["model_pricing", "pricing_history"] {
            let stmt = Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = ?",
                [Value::String(Some(Box::new(table.to_string())))],
            );
            let row = db
                .connection()
                .query_one(stmt)
                .await
                .expect("schema query should succeed")
                .expect("schema query should return a row");
            let count: i64 = row.try_get("", "count").expect("count should decode");
            assert_eq!(count, 1, "{table} table should be created by migration");
        }
    }

    #[tokio::test]
    async fn test_storage_layer_auto_migrate_false_accepts_pre_migrated_schema() {
        let temp_dir = match TempDir::new() {
            Ok(temp_dir) => temp_dir,
            Err(err) => panic!("temp dir should be created: {}", err),
        };
        let migration_config = sqlite_file_db_config(&temp_dir, false);
        let db = match Database::new(&migration_config).await {
            Ok(db) => db,
            Err(err) => panic!("SQLite file database should connect: {}", err),
        };
        if let Err(err) = db.migrate().await {
            panic!("manual migration should prepare schema: {}", err);
        }
        if let Err(err) = db.close().await {
            panic!(
                "database should close cleanly before startup check: {}",
                err
            );
        }

        let storage =
            match StorageLayer::new(&storage_config(sqlite_file_db_config(&temp_dir, false))).await
            {
                Ok(storage) => storage,
                Err(err) => panic!(
                    "pre-migrated configured database should start with auto_migrate=false: {}",
                    err
                ),
            };
        let health = match storage.health_check().await {
            Ok(health) => health,
            Err(err) => panic!("pre-migrated database should pass health check: {}", err),
        };

        assert!(health.database);
    }

    #[tokio::test]
    async fn test_storage_layer_auto_migrate_false_rejects_partial_schema() {
        let temp_dir = match TempDir::new() {
            Ok(temp_dir) => temp_dir,
            Err(err) => panic!("temp dir should be created: {}", err),
        };
        let migration_config = sqlite_file_db_config(&temp_dir, false);
        let db = match Database::new(&migration_config).await {
            Ok(db) => db,
            Err(err) => panic!("SQLite file database should connect: {}", err),
        };
        if let Err(err) = Migrator::up(db.connection(), Some(1)).await {
            panic!("partial migration should apply first migration: {}", err);
        }
        if let Err(err) = db.close().await {
            panic!(
                "database should close cleanly before startup check: {}",
                err
            );
        }

        let err = match StorageLayer::new(&storage_config(sqlite_file_db_config(&temp_dir, false)))
            .await
        {
            Ok(_) => panic!("partial schema must fail when auto_migrate=false"),
            Err(err) => err,
        };

        assert!(
            err.to_string().contains("pending migrations"),
            "error should identify pending migrations, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_storage_layer_allow_degraded_accepts_budget_only_schema_gap() {
        let temp_dir = match TempDir::new() {
            Ok(temp_dir) => temp_dir,
            Err(err) => panic!("temp dir should be created: {}", err),
        };
        let migration_config = sqlite_file_db_config(&temp_dir, false);
        let db = match Database::new(&migration_config).await {
            Ok(db) => db,
            Err(err) => panic!("SQLite file database should connect: {}", err),
        };
        let budget_migration = "m20240501_000001_create_budget_limit_snapshots";
        if let Err(err) = db.migrate().await {
            panic!("complete schema should migrate before test setup: {}", err);
        }
        let schema_manager = SchemaManager::new(db.connection());
        if let Err(err) = schema_manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("budget_limit_snapshots"))
                    .to_owned(),
            )
            .await
        {
            panic!("budget snapshot table should be removed for test setup: {err}");
        }
        let delete_result = match seaql_migrations::Entity::delete_by_id(budget_migration)
            .exec(db.connection())
            .await
        {
            Ok(result) => result,
            Err(err) => panic!("budget migration ledger entry should be removed: {err}"),
        };
        assert_eq!(
            delete_result.rows_affected, 1,
            "test setup should remove exactly one budget migration ledger entry"
        );
        let pending = match Migrator::get_pending_migrations(db.connection()).await {
            Ok(pending) => pending,
            Err(err) => panic!("pending migrations should remain queryable: {err}"),
        };
        let pending_names: Vec<&str> = pending.iter().map(|migration| migration.name()).collect();
        assert_eq!(
            pending_names,
            [budget_migration],
            "test setup should leave only the budget migration pending"
        );
        if let Err(err) = db.close().await {
            panic!(
                "database should close cleanly before startup check: {}",
                err
            );
        }

        let err = match StorageLayer::new(&storage_config(sqlite_file_db_config(&temp_dir, false)))
            .await
        {
            Ok(_) => panic!("budget-only schema gap must fail when allow_degraded=false"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains(budget_migration),
            "error should identify the pending budget migration, got: {}",
            err
        );

        let mut allow_degraded_config = sqlite_file_db_config(&temp_dir, false);
        allow_degraded_config.allow_degraded = true;
        let storage = match StorageLayer::new(&storage_config(allow_degraded_config)).await {
            Ok(storage) => storage,
            Err(err) => panic!(
                "allow_degraded=true should allow startup when only budget migration is pending: {}",
                err
            ),
        };
        let budget_load = storage.database.load_budget_limit_snapshots().await;
        assert!(
            budget_load.is_err(),
            "budget snapshots should remain unavailable for Server::new to degrade"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_storage_layer_sqlite_fallback_runs_startup_migrations() {
        let _guard = ENV_LOCK.lock().await;
        let _restore = EnvRestore::capture();
        let temp_dir = match TempDir::new() {
            Ok(temp_dir) => temp_dir,
            Err(err) => panic!("temp dir should be created: {}", err),
        };
        let sqlite_path = temp_dir.path().join("fallback.db");
        unsafe {
            std::env::set_var("LITELLM_SQLITE_PATH", &sqlite_path);
        }

        let config = storage_config(DatabaseConfig {
            url: "postgresql://127.0.0.1:1/unreachable".to_string(),
            max_connections: 1,
            connection_timeout: 1,
            ssl: false,
            enabled: true,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: true,
            allow_degraded: false,
        });

        let storage = match StorageLayer::new(&config).await {
            Ok(storage) => storage,
            Err(err) => panic!(
                "SQLite fallback should start and migrate even when auto_migrate=false: {}",
                err
            ),
        };
        let snapshots = match storage.database.load_budget_limit_snapshots().await {
            Ok(snapshots) => snapshots,
            Err(err) => panic!(
                "SQLite fallback should have migrated budget tables: {}",
                err
            ),
        };

        assert!(storage.database.is_sqlite_fallback());
        assert_eq!(storage.database.backend_type(), DatabaseBackendType::SQLite);
        assert!(snapshots.is_empty());
        assert!(
            sqlite_path.exists(),
            "SQLite fallback should use configured test path"
        );
    }

    #[tokio::test]
    async fn test_database_disabled_uses_in_memory_sqlite() {
        let config = DatabaseConfig {
            url: "postgresql://unreachable-host:5432/unreachable-db".to_string(),
            max_connections: 10,
            connection_timeout: 1,
            ssl: false,
            enabled: false,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };

        let db = Database::new(&config).await.expect(
            "When database is disabled, runtime should use in-memory SQLite instead of external DB",
        );
        assert_eq!(db.backend_type(), DatabaseBackendType::SQLite);

        db.migrate()
            .await
            .expect("Migration on in-memory DB failed");
        assert!(db.health_check().await.is_ok());
    }

    /// Test database user operations (find_user_by_email, etc.)
    #[tokio::test]
    async fn test_user_operations() {
        let config = DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 1,
            connection_timeout: 5,
            ssl: false,
            enabled: true,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };

        let db = Database::new(&config)
            .await
            .expect("Failed to create database");
        db.migrate().await.expect("Migration failed");

        // Try to find a user that doesn't exist
        let user = db.find_user_by_email("nonexistent@example.com").await;
        assert!(user.is_ok());
        assert!(user.unwrap().is_none());
    }

    /// Test database batch operations
    #[tokio::test]
    async fn test_batch_list() {
        let config = DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 1,
            connection_timeout: 5,
            ssl: false,
            enabled: true,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };

        let db = Database::new(&config)
            .await
            .expect("Failed to create database");
        db.migrate().await.expect("Migration failed");

        // List batches (should be empty)
        let batches = db.list_batches(Some(10), None).await;
        assert!(batches.is_ok());
        assert!(batches.unwrap().is_empty());
    }

    /// Test database statistics
    #[tokio::test]
    async fn test_database_stats() {
        let config = DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 1,
            connection_timeout: 5,
            ssl: false,
            enabled: true,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };

        let db = Database::new(&config)
            .await
            .expect("Failed to create database");
        let stats = db.stats();

        // Just verify we can get stats (size is always >= 0 as usize)
        let _ = stats.size;
    }

    #[tokio::test]
    async fn test_api_key_crud_flow() {
        let config = DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 1,
            connection_timeout: 5,
            ssl: false,
            enabled: true,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };

        let db = Database::new(&config)
            .await
            .expect("Failed to create database");
        db.migrate().await.expect("Migration failed");

        let mut user = User::new(
            "api-key-test-user".to_string(),
            "api-key-test@example.com".to_string(),
            "hashed-password".to_string(),
        );
        user.metadata.id = Uuid::new_v4();
        db.create_user(&user).await.expect("Failed to create user");
        let user_id = user.id();
        let team_id = Uuid::new_v4();
        let key_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let api_key = ApiKey {
            metadata: Metadata {
                id: key_id,
                created_at: now,
                updated_at: now,
                version: 1,
                extra: std::collections::HashMap::new(),
            },
            name: "integration-key".to_string(),
            key_hash: "hash-integration-key".to_string(),
            key_prefix: "gw-int".to_string(),
            user_id: Some(user_id),
            team_id: Some(team_id),
            permissions: vec!["chat:read".to_string()],
            rate_limits: None,
            expires_at: Some(now + chrono::Duration::days(7)),
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats {
                last_reset: now,
                ..UsageStats::default()
            },
        };

        let created = db
            .create_api_key(&api_key)
            .await
            .expect("Failed to create api key");
        assert_eq!(created.metadata.id, key_id);

        let by_hash = db
            .find_api_key_by_hash(&api_key.key_hash)
            .await
            .expect("Failed to find api key by hash")
            .expect("API key not found by hash");
        assert_eq!(by_hash.metadata.id, key_id);

        let by_id = db
            .find_api_key_by_id(key_id)
            .await
            .expect("Failed to find api key by id")
            .expect("API key not found by id");
        assert_eq!(by_id.name, "integration-key");

        db.update_api_key_permissions(key_id, &["chat:write".to_string()])
            .await
            .expect("Failed to update permissions");
        let after_permissions = db
            .find_api_key_by_id(key_id)
            .await
            .expect("Failed to refetch api key")
            .expect("API key missing after permissions update");
        assert_eq!(
            after_permissions.permissions,
            vec!["chat:write".to_string()]
        );

        db.update_api_key_rate_limits(
            key_id,
            &RateLimits {
                rpm: Some(60),
                tpm: Some(10000),
                rpd: None,
                tpd: None,
                concurrent: Some(2),
            },
        )
        .await
        .expect("Failed to update rate limits");
        let after_limits = db
            .find_api_key_by_id(key_id)
            .await
            .expect("Failed to refetch api key")
            .expect("API key missing after rate limit update");
        assert_eq!(after_limits.rate_limits.and_then(|r| r.rpm), Some(60));

        db.update_api_key_usage(key_id, 3, 123, 0.42, false, None)
            .await
            .expect("Failed to update usage");
        let after_usage = db
            .find_api_key_by_id(key_id)
            .await
            .expect("Failed to refetch api key")
            .expect("API key missing after usage update");
        assert_eq!(after_usage.usage_stats.total_requests, 3);
        assert_eq!(after_usage.usage_stats.total_tokens, 123);

        db.update_api_key_usage(key_id, 1, 25, 0.01, true, Some("allow_unpriced"))
            .await
            .expect("Failed to update unpriced usage");
        let after_unpriced_usage = db
            .find_api_key_by_id(key_id)
            .await
            .expect("Failed to refetch api key")
            .expect("API key missing after unpriced usage update");
        assert_eq!(after_unpriced_usage.usage_stats.total_requests, 4);
        assert_eq!(after_unpriced_usage.usage_stats.total_tokens, 148);
        assert_eq!(after_unpriced_usage.usage_stats.unpriced_requests, 1);
        assert_eq!(after_unpriced_usage.usage_stats.unpriced_tokens, 25);
        assert_eq!(after_unpriced_usage.usage_stats.unpriced_cost, 0.01);
        assert!(after_unpriced_usage.usage_stats.last_unpriced_at.is_some());

        db.update_api_key_last_used(key_id)
            .await
            .expect("Failed to update last used");
        let after_last_used = db
            .find_api_key_by_id(key_id)
            .await
            .expect("Failed to refetch api key")
            .expect("API key missing after last_used update");
        assert!(after_last_used.last_used_at.is_some());

        db.deactivate_api_key(key_id)
            .await
            .expect("Failed to deactivate key");
        let deactivated = db
            .find_api_key_by_id(key_id)
            .await
            .expect("Failed to refetch api key")
            .expect("API key missing after deactivate");
        assert!(!deactivated.is_active);

        let user_keys = db
            .list_api_keys_by_user(user_id)
            .await
            .expect("Failed to list user api keys");
        assert_eq!(user_keys.len(), 1);

        let team_keys = db
            .list_api_keys_by_team(team_id)
            .await
            .expect("Failed to list team api keys");
        assert_eq!(team_keys.len(), 1);
    }

    #[tokio::test]
    async fn test_api_key_cleanup_expired() {
        let config = DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 1,
            connection_timeout: 5,
            ssl: false,
            enabled: true,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };

        let db = Database::new(&config)
            .await
            .expect("Failed to create database");
        db.migrate().await.expect("Migration failed");

        let now = chrono::Utc::now();

        for (id, expires_at) in [
            (Uuid::new_v4(), Some(now - chrono::Duration::hours(1))),
            (Uuid::new_v4(), Some(now + chrono::Duration::hours(1))),
        ] {
            let api_key = ApiKey {
                metadata: Metadata {
                    id,
                    created_at: now,
                    updated_at: now,
                    version: 1,
                    extra: std::collections::HashMap::new(),
                },
                name: format!("cleanup-{}", id),
                key_hash: format!("hash-{}", id),
                key_prefix: "gw-clean".to_string(),
                user_id: None,
                team_id: None,
                permissions: vec![],
                rate_limits: None,
                expires_at,
                is_active: true,
                last_used_at: None,
                usage_stats: UsageStats {
                    last_reset: now,
                    ..UsageStats::default()
                },
            };
            db.create_api_key(&api_key)
                .await
                .expect("Failed to create api key");
        }

        let deleted = db
            .delete_expired_api_keys()
            .await
            .expect("Failed to clean expired keys");
        assert_eq!(deleted, 1);
    }
}
