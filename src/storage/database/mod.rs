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
/// 1. `LITELLM_SQLITE_PATH` explicit file override (if set and non-empty)
/// 2. `LITELLM_DATA_DIR/gateway.db` (if `LITELLM_DATA_DIR` is set and non-empty)
/// 3. `<data_local_dir>/litellm-rs/gateway.db` (platform-specific)
/// 4. `/tmp/litellm-rs/gateway.db` (ultimate fallback)
pub fn default_sqlite_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("LITELLM_SQLITE_PATH")
        && !p.is_empty()
    {
        return std::path::PathBuf::from(p);
    }
    super::default_data_dir().join("gateway.db")
}

#[cfg(test)]
mod tests {
    use super::default_sqlite_path;
    use std::path::PathBuf;

    struct EnvRestore {
        sqlite_path: Option<String>,
        data_dir: Option<String>,
    }

    impl EnvRestore {
        fn capture() -> Self {
            Self {
                sqlite_path: std::env::var("LITELLM_SQLITE_PATH").ok(),
                data_dir: std::env::var("LITELLM_DATA_DIR").ok(),
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
                match &self.data_dir {
                    Some(value) => std::env::set_var("LITELLM_DATA_DIR", value),
                    None => std::env::remove_var("LITELLM_DATA_DIR"),
                }
            }
        }
    }

    #[test]
    fn default_sqlite_path_uses_shared_data_dir() {
        let _guard = crate::storage::ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::capture();
        unsafe {
            std::env::remove_var("LITELLM_SQLITE_PATH");
            std::env::set_var("LITELLM_DATA_DIR", "/custom/litellm-state");
        }

        assert_eq!(
            default_sqlite_path(),
            PathBuf::from("/custom/litellm-state/gateway.db")
        );
    }

    #[test]
    fn default_sqlite_path_explicit_override_wins() {
        let _guard = crate::storage::ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::capture();
        unsafe {
            std::env::set_var("LITELLM_SQLITE_PATH", "/explicit/gateway.db");
            std::env::set_var("LITELLM_DATA_DIR", "/custom/litellm-state");
        }

        assert_eq!(default_sqlite_path(), PathBuf::from("/explicit/gateway.db"));
    }
}
