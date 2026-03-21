use sea_orm::DatabaseConnection;

/// SeaORM-based database implementation
#[derive(Debug)]
pub struct SeaOrmDatabase {
    pub(super) db: DatabaseConnection,
    /// Backend type indicator
    pub(super) backend_type: DatabaseBackendType,
}

/// Database backend type indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseBackendType {
    PostgreSQL,
    SQLite,
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    /// Total number of users
    pub total_users: u64,
    /// Database size
    pub size: u32,
    /// Number of idle connections
    pub idle: usize,
}
