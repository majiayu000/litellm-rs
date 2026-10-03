use crate::config::models::storage::DatabaseConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use sea_orm::*;
use sea_orm_migration::{MigratorTrait, seaql_migrations};
use std::collections::HashSet;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::super::entities;
use super::super::migration::Migrator;
use super::types::{DatabaseBackendType, SeaOrmDatabase};

impl SeaOrmDatabase {
    /// Create a new database connection with automatic SQLite fallback
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        if !config.enabled {
            warn!("Database disabled in config; using non-persistent in-memory SQLite backend");
            return Self::in_memory_sqlite(config.connection_timeout).await;
        }

        // Try primary database connection first
        match Self::try_connect(&config.url, config).await {
            Ok(db) => {
                let backend_type = if config.url.starts_with("sqlite") {
                    DatabaseBackendType::SQLite
                } else {
                    DatabaseBackendType::PostgreSQL
                };
                info!("Database connection established ({:?})", backend_type);
                Ok(Self {
                    db,
                    backend_type,
                    sqlite_fallback: false,
                    last_response_prune: std::sync::atomic::AtomicI64::new(0),
                })
            }
            Err(e) => {
                if config.fallback_to_sqlite
                    && (config.url.starts_with("postgresql://")
                        || config.url.starts_with("postgres://"))
                {
                    warn!(
                        "PostgreSQL connection failed: {}. Attempting SQLite fallback because storage.database.fallback_to_sqlite is enabled...",
                        e
                    );
                    Self::fallback_to_sqlite().await
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn in_memory_sqlite(connection_timeout_secs: u64) -> Result<Self> {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(connection_timeout_secs.max(1)))
            .acquire_timeout(Duration::from_secs(30))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(3600))
            .sqlx_logging(false);

        let db = Database::connect(opt).await.map_err(GatewayError::from)?;
        Ok(Self {
            db,
            backend_type: DatabaseBackendType::SQLite,
            sqlite_fallback: false,
            last_response_prune: std::sync::atomic::AtomicI64::new(0),
        })
    }

    /// Try to connect to a database
    async fn try_connect(url: &str, config: &DatabaseConfig) -> Result<DatabaseConnection> {
        let mut opt = ConnectOptions::new(url.to_string());
        opt.max_connections(config.max_connections)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(config.connection_timeout))
            .acquire_timeout(Duration::from_secs(30))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(3600))
            .sqlx_logging(false);

        Database::connect(opt).await.map_err(GatewayError::from)
    }

    /// Fallback to SQLite database
    async fn fallback_to_sqlite() -> Result<Self> {
        let db_path = super::super::default_sqlite_path();
        if let Some(parent) = db_path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                GatewayError::Internal(format!("Failed to create data directory: {}", e))
            })?;
        }

        let sqlite_path = format!("sqlite://{}?mode=rwc", db_path.display());
        info!("Falling back to SQLite database: {}", sqlite_path);

        let mut opt = ConnectOptions::new(sqlite_path.to_string());
        opt.max_connections(5)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(5))
            .acquire_timeout(Duration::from_secs(30))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(3600))
            .sqlx_logging(false);

        let db = Database::connect(opt).await.map_err(GatewayError::from)?;

        info!("SQLite fallback connection established successfully");
        Ok(Self {
            db,
            backend_type: DatabaseBackendType::SQLite,
            sqlite_fallback: true,
            last_response_prune: std::sync::atomic::AtomicI64::new(0),
        })
    }

    /// Get the current backend type
    pub fn backend_type(&self) -> DatabaseBackendType {
        self.backend_type
    }

    /// Check if using SQLite fallback
    pub fn is_sqlite_fallback(&self) -> bool {
        self.sqlite_fallback
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations...");
        Migrator::up(&self.db, None).await.map_err(|e| {
            warn!("Migration failed: {}", e);
            GatewayError::Storage(e.to_string())
        })?;
        info!("Database migrations completed successfully");
        Ok(())
    }

    /// Verify all repository migrations have already been applied without
    /// creating or modifying migration metadata.
    pub async fn verify_migrations_applied(&self) -> Result<()> {
        self.verify_migrations_applied_except(&[]).await
    }

    /// Verify migrations while allowing explicitly degraded feature migrations
    /// to remain pending.
    pub async fn verify_migrations_applied_except(
        &self,
        allowed_pending_migrations: &[&str],
    ) -> Result<()> {
        let applied_migrations = seaql_migrations::Entity::find()
            .all(&self.db)
            .await
            .map_err(|e| {
                GatewayError::Storage(format!(
                    "Database migration metadata is missing or unreadable: {}",
                    e
                ))
            })?;
        let applied_versions: HashSet<String> = applied_migrations
            .into_iter()
            .map(|migration| migration.version)
            .collect();
        let pending_migrations: Vec<String> = Migrator::get_migration_files()
            .into_iter()
            .filter_map(|migration| {
                let name = migration.name();
                (!applied_versions.contains(name) && !allowed_pending_migrations.contains(&name))
                    .then_some(name.to_string())
            })
            .collect();

        if pending_migrations.is_empty() {
            return Ok(());
        }

        Err(GatewayError::Storage(format!(
            "Database schema has pending migrations: {}",
            pending_migrations.join(", ")
        )))
    }

    /// Get the underlying database connection
    pub fn connection(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Close the database connection
    pub async fn close(self) -> Result<()> {
        self.db.close().await.map_err(GatewayError::from)?;
        Ok(())
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        debug!("Performing database health check");

        // Simple query to check database connectivity
        let _result = entities::User::find()
            .limit(1)
            .all(&self.db)
            .await
            .map_err(GatewayError::from)?;

        debug!("Database health check passed");
        Ok(())
    }
}
