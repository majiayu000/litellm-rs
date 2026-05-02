//! Tests for file storage implementations

use super::default_data_path;
use super::local::LocalStorage;
use std::path::PathBuf;
use tempfile::TempDir;

struct DataDirEnvRestore(Option<String>);

impl DataDirEnvRestore {
    fn capture() -> Self {
        Self(std::env::var("LITELLM_DATA_DIR").ok())
    }
}

impl Drop for DataDirEnvRestore {
    fn drop(&mut self) {
        unsafe {
            match &self.0 {
                Some(value) => std::env::set_var("LITELLM_DATA_DIR", value),
                None => std::env::remove_var("LITELLM_DATA_DIR"),
            }
        }
    }
}

#[tokio::test]
async fn test_local_storage() {
    let temp_dir = TempDir::new().unwrap();
    let storage = LocalStorage::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    // Test store
    let content = b"Hello, World!";
    let file_id = storage.store("test.txt", content).await.unwrap();
    assert!(!file_id.is_empty());

    // Test exists
    assert!(storage.exists(&file_id).await.unwrap());

    // Test get
    let retrieved = storage.get(&file_id).await.unwrap();
    assert_eq!(retrieved, content);

    // Test metadata
    let metadata = storage.metadata(&file_id).await.unwrap();
    assert_eq!(metadata.filename, "test.txt");
    assert_eq!(metadata.size, content.len() as u64);

    // Test delete
    storage.delete(&file_id).await.unwrap();
    assert!(!storage.exists(&file_id).await.unwrap());
}

#[test]
fn test_default_data_path_is_absolute() {
    let _guard = crate::storage::ENV_LOCK.lock().unwrap();
    let _restore = DataDirEnvRestore::capture();
    unsafe {
        std::env::remove_var("LITELLM_DATA_DIR");
    }

    // Ensure the default path is absolute, not relative like the old "./data"
    let path = default_data_path();
    assert!(
        path.is_absolute(),
        "default_data_path() should return an absolute path, got: {}",
        path.display()
    );
}

#[test]
fn test_default_data_path_ends_with_data() {
    let _guard = crate::storage::ENV_LOCK.lock().unwrap();
    let _restore = DataDirEnvRestore::capture();
    unsafe {
        std::env::remove_var("LITELLM_DATA_DIR");
    }

    let path = default_data_path();
    assert!(
        path.ends_with("litellm-rs/data"),
        "default_data_path() should end with litellm-rs/data, got: {}",
        path.display()
    );
}

#[test]
fn test_default_data_path_env_override() {
    let _guard = crate::storage::ENV_LOCK.lock().unwrap();
    let _restore = DataDirEnvRestore::capture();
    // SAFETY: ENV_LOCK serializes tests in this crate that mutate
    // LITELLM_DATA_DIR. set_var/remove_var are unsafe because environment
    // mutation is process-global.
    unsafe {
        std::env::set_var("LITELLM_DATA_DIR", "/custom/storage/path");
    }
    let path = default_data_path();
    assert_eq!(
        path,
        PathBuf::from("/custom/storage/path/data"),
        "LITELLM_DATA_DIR should define the shared state directory"
    );
}

#[test]
fn test_content_type_detection() {
    assert_eq!(LocalStorage::detect_content_type("test.txt"), "text/plain");
    assert_eq!(
        LocalStorage::detect_content_type("data.json"),
        "application/json"
    );
    assert_eq!(LocalStorage::detect_content_type("image.png"), "image/png");
    assert_eq!(
        LocalStorage::detect_content_type("unknown"),
        "application/octet-stream"
    );
}
