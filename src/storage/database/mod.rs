//! Database storage implementation using SeaORM
//!
//! This module provides database connectivity and operations using SeaORM ORM.

// SeaORM implementation
/// Database entities module
pub mod entities;
/// Database migration module
pub mod migration;
/// SeaORM database implementation module
pub mod seaorm_db;

// Re-export the main database interface
pub use seaorm_db::SeaOrmDatabase as Database;
pub use seaorm_db::{DatabaseBackendType, DatabaseStats, SeaOrmTeamRepository};

/// Returns the default absolute path for the SQLite fallback database.
///
/// Resolution order:
/// 1. `LITELLM_SQLITE_PATH` environment variable (if set and non-empty)
/// 2. `<data_local_dir>/litellm-rs/gateway.db` (platform-specific)
/// 3. `/tmp/litellm-rs/gateway.db` (ultimate fallback)
pub fn default_sqlite_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("LITELLM_SQLITE_PATH")
        && !p.is_empty()
    {
        return std::path::PathBuf::from(p);
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("litellm-rs")
        .join("gateway.db")
}
