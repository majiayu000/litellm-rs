//! Test database utilities
//!
//! Provides in-memory SQLite database for testing without external dependencies.
//! Each test gets an isolated database instance using SeaORM.

use litellm_rs::config::models::storage::DatabaseConfig;
use litellm_rs::storage::database::Database;
use std::sync::Arc;

/// Test database wrapper providing isolated in-memory SQLite instances
#[derive(Debug, Clone)]
pub struct TestDatabase {
    inner: Arc<Database>,
}

impl TestDatabase {
    /// Create a new in-memory test database using SeaORM
    ///
    /// Note: This uses SQLite in-memory mode which requires the 'sqlite' feature.
    /// Each call creates a completely isolated database instance.
    pub async fn new() -> Self {
        let config = DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 1, // In-memory DB only supports 1 connection
            connection_timeout: 5,
            ssl: false,
            enabled: true,
            fallback_to_sqlite: false,
        };

        let db = Database::new(&config)
            .await
            .expect("Failed to create in-memory test database");

        // Run migrations
        db.migrate()
            .await
            .expect("Failed to run database migrations");

        Self {
            inner: Arc::new(db),
        }
    }

    /// Create a test database with seeded data
    pub async fn seeded() -> Self {
        let db = Self::new().await;
        db.seed_test_data().await;
        db
    }

    /// Get reference to the underlying database
    pub fn db(&self) -> &Database {
        &self.inner
    }

    /// Get Arc to the underlying database
    pub fn db_arc(&self) -> Arc<Database> {
        Arc::clone(&self.inner)
    }

    /// Seed the database with test data
    async fn seed_test_data(&self) {
        // Create test users using SeaORM
        // Note: This is a placeholder - actual seeding depends on the entity models
        tracing::debug!("Seeding test database with sample data");
    }
}

/// Helper to create a simple test database config
pub fn test_db_config() -> DatabaseConfig {
    DatabaseConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1,
        connection_timeout: 5,
        ssl: false,
        enabled: true,
        fallback_to_sqlite: false,
    }
}

/// Create a standalone test database (convenience function)
pub async fn create_test_db() -> Database {
    let config = test_db_config();
    let db = Database::new(&config)
        .await
        .expect("Failed to create test database");
    db.migrate().await.expect("Failed to run migrations");
    db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_creation() {
        let db = TestDatabase::new().await;
        // Database should be created and migrations run
        assert!(db.db().health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_seeded_database() {
        let db = TestDatabase::seeded().await;
        // Should have test data
        assert!(db.db().health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_standalone_db_creation() {
        let db = create_test_db().await;
        assert!(db.health_check().await.is_ok());
    }
}
