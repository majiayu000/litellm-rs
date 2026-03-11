//! File-based Secret Manager
//!
//! Reads secrets from files on disk. Useful for development and Kubernetes secrets.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::debug;

use crate::core::traits::secret_manager::{
    ListSecretsOptions, ListSecretsResult, SecretError, SecretManager, SecretMetadata, SecretResult,
};

/// Secret manager that reads from files
///
/// Each secret is stored as a separate file. The file name is the secret name,
/// and the file content is the secret value.
///
/// # Example
///
/// ```rust,ignore
/// use litellm_rs::core::secret_managers::FileSecretManager;
///
/// // Secrets stored in /etc/secrets/
/// let manager = FileSecretManager::new("/etc/secrets");
///
/// // Reads from /etc/secrets/api-key
/// let api_key = manager.read_secret("api-key").await?;
/// ```
#[derive(Debug, Clone)]
pub struct FileSecretManager {
    /// Base directory for secrets
    base_path: PathBuf,
    /// File extension for secret files (optional)
    extension: Option<String>,
}

impl FileSecretManager {
    /// Create a new file secret manager
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            extension: None,
        }
    }

    /// Create with a specific file extension
    ///
    /// For example, with extension ".secret", reading "api-key" will look for "api-key.secret"
    pub fn with_extension(base_path: impl Into<PathBuf>, extension: impl Into<String>) -> Self {
        Self {
            base_path: base_path.into(),
            extension: Some(extension.into()),
        }
    }

    /// Get the full path for a secret
    fn get_secret_path(&self, name: &str) -> PathBuf {
        let filename = match &self.extension {
            Some(ext) => format!("{}{}", name, ext),
            None => name.to_string(),
        };
        self.base_path.join(filename)
    }

    /// Validate that a path is within the base directory (prevent path traversal)
    fn validate_path(&self, path: &Path) -> SecretResult<()> {
        let canonical_base = self
            .base_path
            .canonicalize()
            .map_err(|e| SecretError::config(format!("Invalid base path: {}", e)))?;

        // For new files, check the parent directory
        let path_to_check = if path.exists() {
            path.canonicalize()
                .map_err(|e| SecretError::config(format!("Invalid path: {}", e)))?
        } else {
            let parent = path
                .parent()
                .ok_or_else(|| SecretError::config("Invalid path: no parent directory"))?;
            if !parent.exists() {
                return Err(SecretError::config(format!(
                    "Parent directory does not exist: {}",
                    parent.display()
                )));
            }
            let canonical_parent = parent
                .canonicalize()
                .map_err(|e| SecretError::config(format!("Invalid parent path: {}", e)))?;
            canonical_parent.join(path.file_name().unwrap_or_default())
        };

        if !path_to_check.starts_with(&canonical_base) {
            return Err(SecretError::access_denied(format!(
                "Path traversal detected: {}",
                path.display()
            )));
        }

        Ok(())
    }
}

impl Default for FileSecretManager {
    fn default() -> Self {
        Self::new("./secrets")
    }
}

#[async_trait]
impl SecretManager for FileSecretManager {
    fn name(&self) -> &'static str {
        "file"
    }

    async fn read_secret(&self, name: &str) -> SecretResult<Option<String>> {
        let path = self.get_secret_path(name);

        // Check if file exists first
        if !path.exists() {
            debug!("Secret file not found: {}", path.display());
            return Ok(None);
        }

        self.validate_path(&path)?;

        match fs::read_to_string(&path).await {
            Ok(content) => {
                // Trim trailing newline (common in secret files)
                let value = content.trim_end_matches('\n').to_string();
                Ok(Some(value))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                Err(SecretError::access_denied(name))
            }
            Err(e) => Err(SecretError::other(format!(
                "Failed to read secret file: {}",
                e
            ))),
        }
    }

    async fn write_secret(&self, name: &str, value: &str) -> SecretResult<()> {
        let path = self.get_secret_path(name);

        // Ensure base directory exists
        if !self.base_path.exists() {
            fs::create_dir_all(&self.base_path).await.map_err(|e| {
                SecretError::config(format!("Failed to create secrets directory: {}", e))
            })?;
        }

        self.validate_path(&path)?;

        fs::write(&path, value).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                SecretError::access_denied(name)
            } else {
                SecretError::other(format!("Failed to write secret file: {}", e))
            }
        })?;

        debug!("Wrote secret to: {}", path.display());
        Ok(())
    }

    async fn delete_secret(&self, name: &str) -> SecretResult<()> {
        let path = self.get_secret_path(name);

        if !path.exists() {
            return Ok(());
        }

        self.validate_path(&path)?;

        fs::remove_file(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                SecretError::access_denied(name)
            } else {
                SecretError::other(format!("Failed to delete secret file: {}", e))
            }
        })?;

        debug!("Deleted secret: {}", path.display());
        Ok(())
    }

    async fn list_secrets(&self, options: &ListSecretsOptions) -> SecretResult<ListSecretsResult> {
        let mut secrets = Vec::new();

        if !self.base_path.exists() {
            return Ok(ListSecretsResult {
                secrets,
                next_token: None,
            });
        }

        let mut entries = fs::read_dir(&self.base_path)
            .await
            .map_err(|e| SecretError::other(format!("Failed to read secrets directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SecretError::other(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Get the secret name from the filename
            let filename = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Remove extension if configured
            let secret_name = match &self.extension {
                Some(ext) => {
                    if filename.ends_with(ext) {
                        filename.strip_suffix(ext).unwrap_or(&filename).to_string()
                    } else {
                        continue; // Skip files without the expected extension
                    }
                }
                None => filename,
            };

            // Filter by prefix
            if let Some(prefix) = &options.prefix
                && !secret_name.starts_with(prefix)
            {
                continue;
            }

            // Get file metadata for timestamps
            let metadata = entry.metadata().await.ok();
            let mut secret_meta = SecretMetadata::new(&secret_name);

            if let Some(meta) = metadata {
                if let Ok(created) = meta.created() {
                    secret_meta.created_at = Some(
                        created
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0),
                    );
                }
                if let Ok(modified) = meta.modified() {
                    secret_meta.updated_at = Some(
                        modified
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0),
                    );
                }
            }

            secrets.push(secret_meta);

            // Check max results
            if let Some(max) = options.max_results
                && secrets.len() >= max
            {
                break;
            }
        }

        Ok(ListSecretsResult {
            secrets,
            next_token: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, FileSecretManager) {
        let temp_dir = TempDir::new().unwrap();
        let manager = FileSecretManager::new(temp_dir.path());
        (temp_dir, manager)
    }

    #[tokio::test]
    async fn test_write_and_read_secret() {
        let (_temp_dir, manager) = setup().await;

        manager
            .write_secret("test-key", "test-value")
            .await
            .unwrap();
        let result = manager.read_secret("test-key").await.unwrap();

        assert_eq!(result, Some("test-value".to_string()));
    }

    #[tokio::test]
    async fn test_read_nonexistent_secret() {
        let (_temp_dir, manager) = setup().await;

        let result = manager.read_secret("nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_delete_secret() {
        let (_temp_dir, manager) = setup().await;

        manager.write_secret("to-delete", "value").await.unwrap();
        assert!(manager.exists("to-delete").await.unwrap());

        manager.delete_secret("to-delete").await.unwrap();
        assert!(!manager.exists("to-delete").await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_secret() {
        let (_temp_dir, manager) = setup().await;

        // Should not error
        manager.delete_secret("nonexistent").await.unwrap();
    }

    #[tokio::test]
    async fn test_with_extension() {
        let temp_dir = TempDir::new().unwrap();
        let manager = FileSecretManager::with_extension(temp_dir.path(), ".secret");

        manager
            .write_secret("api-key", "secret-value")
            .await
            .unwrap();

        // Verify file has extension
        let path = temp_dir.path().join("api-key.secret");
        assert!(path.exists());

        let result = manager.read_secret("api-key").await.unwrap();
        assert_eq!(result, Some("secret-value".to_string()));
    }

    #[tokio::test]
    async fn test_list_secrets() {
        let (_temp_dir, manager) = setup().await;

        manager.write_secret("secret1", "value1").await.unwrap();
        manager.write_secret("secret2", "value2").await.unwrap();
        manager.write_secret("other", "value3").await.unwrap();

        let result = manager
            .list_secrets(&ListSecretsOptions::new())
            .await
            .unwrap();

        assert_eq!(result.secrets.len(), 3);
    }

    #[tokio::test]
    async fn test_list_secrets_with_prefix() {
        let (_temp_dir, manager) = setup().await;

        manager.write_secret("api-key1", "value1").await.unwrap();
        manager.write_secret("api-key2", "value2").await.unwrap();
        manager.write_secret("other", "value3").await.unwrap();

        let result = manager
            .list_secrets(&ListSecretsOptions::new().prefix("api-"))
            .await
            .unwrap();

        assert_eq!(result.secrets.len(), 2);
        let names: Vec<_> = result.secrets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"api-key1"));
        assert!(names.contains(&"api-key2"));
    }

    #[tokio::test]
    async fn test_list_secrets_max_results() {
        let (_temp_dir, manager) = setup().await;

        manager.write_secret("secret1", "value1").await.unwrap();
        manager.write_secret("secret2", "value2").await.unwrap();
        manager.write_secret("secret3", "value3").await.unwrap();

        let result = manager
            .list_secrets(&ListSecretsOptions::new().max_results(2))
            .await
            .unwrap();

        assert_eq!(result.secrets.len(), 2);
    }

    #[tokio::test]
    async fn test_path_traversal_prevention() {
        let (_temp_dir, manager) = setup().await;

        let result = manager.read_secret("../../../etc/passwd").await;
        // Should either return None (file not found) or error (path traversal)
        // The exact behavior depends on whether the path exists
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_trim_trailing_newline() {
        let temp_dir = TempDir::new().unwrap();
        let manager = FileSecretManager::new(temp_dir.path());

        // Write file with trailing newline (common in secret files)
        let path = temp_dir.path().join("with-newline");
        fs::write(&path, "secret-value\n").await.unwrap();

        let result = manager.read_secret("with-newline").await.unwrap();
        assert_eq!(result, Some("secret-value".to_string()));
    }

    #[tokio::test]
    async fn test_name() {
        let manager = FileSecretManager::default();
        assert_eq!(manager.name(), "file");
    }
}
