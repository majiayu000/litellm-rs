//! File storage implementation
//!
//! This module provides file storage functionality with support for local and cloud storage.

mod local;
mod s3;
mod storage;
#[cfg(test)]
mod tests;
mod types;

// Re-export public types
pub use local::LocalStorage;
pub use s3::S3Storage;
pub use types::{FileMetadata, FileStorage};

/// Returns the default absolute path for local file storage.
///
/// Resolution order:
/// 1. `LITELLM_DATA_DIR/data` (if `LITELLM_DATA_DIR` is set and non-empty)
/// 2. `<data_local_dir>/litellm-rs/data` (platform-specific)
/// 3. `/tmp/litellm-rs/data` (ultimate fallback)
pub fn default_data_path() -> std::path::PathBuf {
    super::default_data_dir().join("data")
}
